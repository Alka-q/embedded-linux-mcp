use crate::config::DeviceProfile;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use russh::client::{self, Handle};
use russh::{ChannelMsg, Disconnect};
use russh_keys::key::PublicKey;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MAX_OUTPUT_BYTES: usize = 65_536;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub truncated: bool,
}

struct ClientHandler;

#[async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // Embedded boards on local development networks frequently regenerate Dropbear keys on re-flash.
        Ok(true)
    }
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

pub fn build_remote_command(
    command: &str,
    cwd: Option<&str>,
    env: Option<&BTreeMap<String, String>>,
) -> String {
    let mut parts = Vec::new();
    if let Some(dir) = cwd {
        if !dir.trim().is_empty() {
            parts.push(format!("cd {} &&", shell_quote(dir)));
        }
    }
    if let Some(vars) = env {
        for (k, v) in vars {
            if k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !k.is_empty() {
                parts.push(format!("export {}={};", k, shell_quote(v)));
            }
        }
    }
    parts.push(command.to_string());
    parts.join(" ")
}

pub fn truncate_bytes_to_string(buf: &[u8], max_bytes: usize) -> (String, bool) {
    if buf.len() > max_bytes {
        let s = String::from_utf8_lossy(&buf[..max_bytes]).to_string();
        (s, true)
    } else {
        (String::from_utf8_lossy(buf).to_string(), false)
    }
}

pub struct SshDeviceSession {
    pub profile: DeviceProfile,
    handle: Handle<ClientHandler>,
}

impl SshDeviceSession {
    pub async fn connect(profile: DeviceProfile) -> Result<Self> {
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(Duration::from_secs(300)),
            keepalive_interval: Some(Duration::from_secs(30)),
            ..Default::default()
        });

        let addr = format!("{}:{}", profile.host, profile.port);
        let mut handle = tokio::time::timeout(
            Duration::from_secs(10),
            client::connect(config, &addr, ClientHandler),
        )
        .await
        .with_context(|| format!("Connection timed out to {}", addr))?
        .with_context(|| format!("Failed to establish SSH connection to {}", addr))?;

        let mut authenticated = false;

        if let Some(key_path) = &profile.key_path {
            if !key_path.trim().is_empty() {
                let key_pair = russh_keys::load_secret_key(key_path, None)
                    .with_context(|| format!("Failed to load SSH key from {}", key_path))?;
                authenticated = handle
                    .authenticate_publickey(&profile.username, Arc::new(key_pair))
                    .await
                    .context("Public key authentication failed")?;
            }
        }

        if !authenticated {
            if let Some(password) = profile.resolved_password() {
                authenticated = handle
                    .authenticate_password(&profile.username, &password)
                    .await
                    .context("Password authentication failed")?;
            }
        }

        if !authenticated {
            return Err(anyhow!(
                "SSH authentication failed for {}@{}:{} (check password or key_path)",
                profile.username,
                profile.host,
                profile.port
            ));
        }

        Ok(Self { profile, handle })
    }

    pub async fn open_sftp(&mut self) -> Result<SftpSession> {
        let channel = self
            .handle
            .channel_open_session()
            .await
            .context("Failed to open SSH channel for SFTP")?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .context("Failed to request sftp subsystem")?;
        let sftp = tokio::time::timeout(
            Duration::from_secs(5),
            SftpSession::new(channel.into_stream()),
        )
        .await
        .context("Timed out initializing SFTP subsystem")?
        .context("SFTP subsystem unavailable")?;
        Ok(sftp)
    }

    pub async fn exec_raw_bytes(
        &mut self,
        full_command: &str,
        stdin_data: Option<&[u8]>,
        timeout_ms: u64,
        max_bytes: usize,
    ) -> Result<(i32, Vec<u8>, Vec<u8>, u64, bool, bool)> {
        let start = Instant::now();
        let mut channel = self
            .handle
            .channel_open_session()
            .await
            .context("Failed to open SSH session channel")?;

        channel
            .exec(true, full_command)
            .await
            .context("Failed to execute command on SSH channel")?;

        if let Some(input) = stdin_data {
            channel
                .data(input)
                .await
                .context("Failed to write stdin data to SSH channel")?;
            channel
                .eof()
                .await
                .context("Failed to send EOF to SSH channel")?;
        }

        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();
        let mut exit_code: Option<i32> = None;
        let mut truncated = false;
        let mut timed_out = false;

        let clamped_timeout = timeout_ms.clamp(500, 300_000);
        let loop_res = tokio::time::timeout(Duration::from_millis(clamped_timeout), async {
            while let Some(msg) = channel.wait().await {
                match msg {
                    ChannelMsg::Data { ref data } => {
                        if stdout_buf.len() < max_bytes {
                            let remaining = max_bytes - stdout_buf.len();
                            if data.len() > remaining {
                                stdout_buf.extend_from_slice(&data[..remaining]);
                                truncated = true;
                            } else {
                                stdout_buf.extend_from_slice(data);
                            }
                        } else {
                            truncated = true;
                        }
                    }
                    ChannelMsg::ExtendedData { ref data, ext } => {
                        if ext == 1 {
                            if stderr_buf.len() < max_bytes {
                                let remaining = max_bytes - stderr_buf.len();
                                if data.len() > remaining {
                                    stderr_buf.extend_from_slice(&data[..remaining]);
                                    truncated = true;
                                } else {
                                    stderr_buf.extend_from_slice(data);
                                }
                            } else {
                                truncated = true;
                            }
                        }
                    }
                    ChannelMsg::ExitStatus { exit_status } => {
                        exit_code = Some(exit_status as i32);
                    }
                    ChannelMsg::Eof | ChannelMsg::Close => {}
                    _ => {}
                }
            }
        })
        .await;

        if loop_res.is_err() {
            timed_out = true;
            let _ = channel.close().await;
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        Ok((
            exit_code.unwrap_or(if timed_out { -1 } else { 0 }),
            stdout_buf,
            stderr_buf,
            duration_ms,
            timed_out,
            truncated,
        ))
    }

    pub async fn exec(
        &mut self,
        command: &str,
        cwd: Option<&str>,
        env: Option<&BTreeMap<String, String>>,
        timeout_ms: u64,
    ) -> Result<ExecOutput> {
        let full_cmd = build_remote_command(command, cwd, env);
        let (exit_code, out_bytes, err_bytes, duration_ms, timed_out, truncated) = self
            .exec_raw_bytes(&full_cmd, None, timeout_ms, MAX_OUTPUT_BYTES)
            .await?;

        Ok(ExecOutput {
            exit_code,
            stdout: String::from_utf8_lossy(&out_bytes).to_string(),
            stderr: String::from_utf8_lossy(&err_bytes).to_string(),
            duration_ms,
            timed_out,
            truncated,
        })
    }

    pub async fn close(&mut self) -> Result<()> {
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, "MCP session closed", "en")
            .await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_remote_command_with_cwd_and_env() {
        let mut env = BTreeMap::new();
        env.insert("RUST_LOG".to_string(), "debug".to_string());
        let cmd = build_remote_command("./app --test", Some("/tmp/work dir"), Some(&env));
        assert_eq!(
            cmd,
            "cd '/tmp/work dir' && export RUST_LOG='debug'; ./app --test"
        );
    }

    #[test]
    fn truncates_buffer_past_limit() {
        let data = b"helloembeddedworld";
        let (text, truncated) = truncate_bytes_to_string(data, 5);
        assert_eq!(text, "hello");
        assert!(truncated);
    }
}
