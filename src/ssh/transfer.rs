use crate::ssh::session::{shell_quote, SshDeviceSession, MAX_OUTPUT_BYTES};
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileReadOutput {
    pub remote_path: String,
    pub content: String,
    pub encoding: String,
    pub bytes_read: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileTransferOutput {
    pub direction: String,
    pub local_path: String,
    pub remote_path: String,
    pub bytes_transferred: usize,
    pub method: String,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemInfoOutput {
    pub uname: String,
    pub cpu_model: String,
    pub board_model: String,
    pub uptime: String,
    pub memory: String,
    pub storage: String,
    pub network_interfaces: String,
}

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

pub async fn read_remote_file(
    session: &mut SshDeviceSession,
    remote_path: &str,
    max_bytes: usize,
    offset: usize,
    encoding: &str,
) -> Result<FileReadOutput> {
    let clamped_max = max_bytes.clamp(1, MAX_OUTPUT_BYTES * 4);
    let read_limit = clamped_max + 1;
    let quoted = shell_quote(remote_path);

    let cmd = if offset > 0 {
        format!(
            "dd if={} bs=1 skip={} count={} 2>/dev/null",
            quoted, offset, read_limit
        )
    } else {
        format!("head -c {} {}", read_limit, quoted)
    };

    let (exit_code, out_bytes, err_bytes, _, timed_out, _) = session
        .exec_raw_bytes(&cmd, None, 15_000, read_limit)
        .await?;

    if timed_out {
        return Err(anyhow!("Timed out reading remote file '{}'", remote_path));
    }
    if exit_code != 0 && out_bytes.is_empty() {
        return Err(anyhow!(
            "Failed to read remote file '{}' (exit {}): {}",
            remote_path,
            exit_code,
            String::from_utf8_lossy(&err_bytes).trim()
        ));
    }

    let truncated = out_bytes.len() > clamped_max;
    let slice = if truncated {
        &out_bytes[..clamped_max]
    } else {
        &out_bytes[..]
    };

    let (content, final_encoding) = if encoding.eq_ignore_ascii_case("base64") {
        (
            base64::engine::general_purpose::STANDARD.encode(slice),
            "base64".to_string(),
        )
    } else {
        (
            String::from_utf8_lossy(slice).to_string(),
            "utf8".to_string(),
        )
    };

    Ok(FileReadOutput {
        remote_path: remote_path.to_string(),
        content,
        encoding: final_encoding,
        bytes_read: slice.len(),
        truncated,
    })
}

async fn try_sftp_upload(
    session: &mut SshDeviceSession,
    remote_path: &str,
    data: &[u8],
) -> Result<()> {
    let sftp = session.open_sftp().await?;
    let mut file = sftp
        .create(remote_path)
        .await
        .with_context(|| format!("SFTP create failed for '{}'", remote_path))?;
    file.write_all(data)
        .await
        .with_context(|| format!("SFTP write failed for '{}'", remote_path))?;
    file.shutdown()
        .await
        .with_context(|| format!("SFTP close failed for '{}'", remote_path))?;
    let _ = sftp.close().await;
    Ok(())
}

async fn try_sftp_download(
    session: &mut SshDeviceSession,
    remote_path: &str,
    max_download: usize,
) -> Result<Vec<u8>> {
    let sftp = session.open_sftp().await?;
    let mut file = sftp
        .open(remote_path)
        .await
        .with_context(|| format!("SFTP open failed for '{}'", remote_path))?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 32_768];
    loop {
        let n = file
            .read(&mut chunk)
            .await
            .with_context(|| format!("SFTP read failed for '{}'", remote_path))?;
        if n == 0 {
            break;
        }
        if buf.len() + n > max_download {
            return Err(anyhow!(
                "Remote file '{}' exceeds 16 MB transfer cap",
                remote_path
            ));
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let _ = sftp.close().await;
    Ok(buf)
}

pub async fn transfer_file(
    session: &mut SshDeviceSession,
    direction: &str,
    local_path: &Path,
    remote_path: &str,
    mode: Option<&str>,
) -> Result<FileTransferOutput> {
    let quoted_remote = shell_quote(remote_path);

    match direction.to_lowercase().as_str() {
        "upload" => {
            let data = std::fs::read(local_path)
                .with_context(|| format!("Failed to read local file '{}'", local_path.display()))?;
            let checksum = sha256_hex(&data);

            // Attempt SFTP first; fall back cleanly to shell_stream (e.g., on minimal Dropbear setups)
            let method = if try_sftp_upload(session, remote_path, &data).await.is_ok() {
                if let Some(m) = mode {
                    if m.chars().all(|c| c.is_ascii_digit()) && !m.is_empty() {
                        let chmod_cmd = format!("chmod {} {}", m, quoted_remote);
                        let _ = session.exec_raw_bytes(&chmod_cmd, None, 10_000, 1024).await;
                    }
                }
                "sftp".to_string()
            } else {
                let mut cmd = format!("cat > {}", quoted_remote);
                if let Some(m) = mode {
                    if m.chars().all(|c| c.is_ascii_digit()) && !m.is_empty() {
                        cmd.push_str(&format!(" && chmod {} {}", m, quoted_remote));
                    }
                }
                cmd.push_str(&format!(
                    " && (sha256sum {} 2>/dev/null || true)",
                    quoted_remote
                ));

                let (exit_code, out_bytes, err_bytes, _, timed_out, _) = session
                    .exec_raw_bytes(&cmd, Some(&data), 60_000, 4096)
                    .await?;

                if timed_out || exit_code != 0 {
                    return Err(anyhow!(
                        "Upload to '{}' failed (exit {}, timed_out={}): {}",
                        remote_path,
                        exit_code,
                        timed_out,
                        String::from_utf8_lossy(&err_bytes).trim()
                    ));
                }

                let remote_out = String::from_utf8_lossy(&out_bytes);
                if let Some(remote_hash) = remote_out.split_whitespace().next() {
                    if remote_hash.len() == 64
                        && remote_hash.chars().all(|c| c.is_ascii_hexdigit())
                        && !remote_hash.eq_ignore_ascii_case(&checksum)
                    {
                        return Err(anyhow!(
                            "SHA-256 checksum mismatch after upload to '{}': expected {}, got {}",
                            remote_path,
                            checksum,
                            remote_hash
                        ));
                    }
                }
                "shell_stream".to_string()
            };

            Ok(FileTransferOutput {
                direction: "upload".to_string(),
                local_path: local_path.display().to_string(),
                remote_path: remote_path.to_string(),
                bytes_transferred: data.len(),
                method,
                checksum,
            })
        }
        "download" => {
            let max_download = 16 * 1024 * 1024; // 16 MB safety limit
            let (out_bytes, method) =
                match try_sftp_download(session, remote_path, max_download).await {
                    Ok(bytes) => (bytes, "sftp".to_string()),
                    Err(_) => {
                        let cmd = format!("cat {}", quoted_remote);
                        let (exit_code, out_bytes, err_bytes, _, timed_out, truncated) = session
                            .exec_raw_bytes(&cmd, None, 60_000, max_download)
                            .await?;

                        if timed_out || exit_code != 0 {
                            return Err(anyhow!(
                                "Download from '{}' failed (exit {}, timed_out={}): {}",
                                remote_path,
                                exit_code,
                                timed_out,
                                String::from_utf8_lossy(&err_bytes).trim()
                            ));
                        }
                        if truncated {
                            return Err(anyhow!(
                                "Remote file '{}' exceeds 16 MB transfer cap",
                                remote_path
                            ));
                        }
                        (out_bytes, "shell_stream".to_string())
                    }
                };

            if let Some(parent) = local_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(local_path, &out_bytes).with_context(|| {
                format!("Failed to write local file '{}'", local_path.display())
            })?;

            let checksum = sha256_hex(&out_bytes);
            Ok(FileTransferOutput {
                direction: "download".to_string(),
                local_path: local_path.display().to_string(),
                remote_path: remote_path.to_string(),
                bytes_transferred: out_bytes.len(),
                method,
                checksum,
            })
        }
        other => Err(anyhow!(
            "Unsupported direction '{}': must be 'upload' or 'download'",
            other
        )),
    }
}

pub async fn gather_system_info(session: &mut SshDeviceSession) -> Result<SystemInfoOutput> {
    let script = r#"
echo "===UNAME==="
uname -a 2>/dev/null || true
echo "===CPU==="
grep -E 'model name|Hardware|Processor|cpu model' /proc/cpuinfo 2>/dev/null | head -n 8 || head -n 10 /proc/cpuinfo 2>/dev/null || true
echo "===BOARD==="
(tr -d '\0' < /proc/device-tree/model 2>/dev/null; echo "") || true
echo "===UPTIME==="
uptime 2>/dev/null || cat /proc/uptime 2>/dev/null || true
echo "===MEM==="
free -h 2>/dev/null || free 2>/dev/null || head -n 5 /proc/meminfo 2>/dev/null || true
echo "===STORAGE==="
df -h 2>/dev/null || df 2>/dev/null || true
echo "===NET==="
ip -br addr 2>/dev/null || ip addr 2>/dev/null || ifconfig 2>/dev/null || true
"#;
    let out = session.exec(script, None, None, 10_000).await?;
    Ok(parse_system_info_sections(&out.stdout))
}

pub fn parse_system_info_sections(raw: &str) -> SystemInfoOutput {
    let mut uname = String::new();
    let mut cpu_model = String::new();
    let mut board_model = String::new();
    let mut uptime = String::new();
    let mut memory = String::new();
    let mut storage = String::new();
    let mut network_interfaces = String::new();

    let mut current = "";
    for line in raw.lines() {
        match line.trim() {
            "===UNAME===" => current = "uname",
            "===CPU===" => current = "cpu",
            "===BOARD===" => current = "board",
            "===UPTIME===" => current = "uptime",
            "===MEM===" => current = "mem",
            "===STORAGE===" => current = "storage",
            "===NET===" => current = "net",
            _ => {
                let target = match current {
                    "uname" => &mut uname,
                    "cpu" => &mut cpu_model,
                    "board" => &mut board_model,
                    "uptime" => &mut uptime,
                    "mem" => &mut memory,
                    "storage" => &mut storage,
                    "net" => &mut network_interfaces,
                    _ => continue,
                };
                if !target.is_empty() {
                    target.push('\n');
                }
                target.push_str(line);
            }
        }
    }

    SystemInfoOutput {
        uname: uname.trim().to_string(),
        cpu_model: cpu_model.trim().to_string(),
        board_model: board_model.trim().to_string(),
        uptime: uptime.trim().to_string(),
        memory: memory.trim().to_string(),
        storage: storage.trim().to_string(),
        network_interfaces: network_interfaces.trim().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_system_info_markers_correctly() {
        let sample = "===UNAME===\nLinux luckfox 5.10.160 armv7l\n===CPU===\nARMv7 Processor rev 5 (v7l)\n===BOARD===\nLuckfox Pico Plus\n===UPTIME===\n 10:00:00 up 1:23\n===MEM===\nMemTotal: 64 MB\n===STORAGE===\n/dev/root 100M\n===NET===\neth0 192.168.111.7\n";
        let parsed = parse_system_info_sections(sample);
        assert_eq!(parsed.uname, "Linux luckfox 5.10.160 armv7l");
        assert_eq!(parsed.board_model, "Luckfox Pico Plus");
        assert_eq!(parsed.network_interfaces, "eth0 192.168.111.7");
    }
}
