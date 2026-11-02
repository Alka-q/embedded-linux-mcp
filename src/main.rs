use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use embedded_linux_mcp::config::{AppConfig, DeviceProfile};
use embedded_linux_mcp::mcp::McpServer;
use embedded_linux_mcp::safety::{SafetyChecker, SafetyLevel};
use embedded_linux_mcp::ssh::SessionManager;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Parser, Debug)]
#[command(
    name = "embedded-linux-mcp",
    version,
    about = "MCP server and CLI for controlling, testing, and debugging network-connected embedded Linux devices"
)]
pub struct Cli {
    /// Optional path to devices.toml configuration file
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Target device alias or IP from configuration
    #[arg(short, long, global = true)]
    pub device: Option<String>,

    /// Target SSH host/IP override (e.g. 192.168.1.100)
    #[arg(long, global = true)]
    pub host: Option<String>,

    /// Target SSH port override (default: 22)
    #[arg(long, global = true, default_value_t = 22)]
    pub port: u16,

    /// Target SSH username override (default: root)
    #[arg(long, global = true, default_value = "root")]
    pub user: String,

    /// Target SSH password override
    #[arg(long, global = true)]
    pub pass: Option<String>,

    /// Target SSH private key path override
    #[arg(long, global = true)]
    pub key: Option<String>,

    /// Output results as raw JSON in CLI mode
    #[arg(long, global = true, default_value_t = false)]
    pub json: bool,

    /// Disable safety blocks against destructive flash/filesystem commands
    #[arg(long, global = true, default_value_t = false)]
    pub allow_destructive: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Commands {
    /// Start the stdio JSON-RPC 2.0 MCP server (default when no subcommand is given)
    Serve,
    /// List configured devices
    Devices,
    /// Gather hardware, SoC, memory, storage, and network summary from the target
    Info,
    /// Execute a shell command on the target device
    Exec {
        /// Shell command to execute
        command: String,
        /// Optional working directory on the target
        #[arg(long)]
        cwd: Option<String>,
        /// Execution timeout in milliseconds
        #[arg(long, default_value_t = 15_000)]
        timeout_ms: u64,
        /// Confirm execution of reboot or network-disrupting commands
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Read and filter the kernel ring buffer (dmesg)
    Dmesg {
        /// Number of tail lines to fetch
        #[arg(short = 'n', long, default_value_t = 100)]
        lines: usize,
        /// Optional log level filter (err, warn, info, etc.)
        #[arg(short, long)]
        level: Option<String>,
        /// Optional case-insensitive pattern filter
        #[arg(short, long)]
        grep: Option<String>,
    },
    /// Read a text, binary, /proc, or /sys file from the target
    Read {
        /// Remote file path on the target device
        remote_path: String,
        /// Maximum bytes to read
        #[arg(long, default_value_t = 65_536)]
        max_bytes: usize,
        /// Byte offset to start reading from
        #[arg(long, default_value_t = 0)]
        offset: usize,
        /// Encode output as base64
        #[arg(long, default_value_t = false)]
        base64: bool,
    },
    /// Upload a file from the host to the target device with SHA-256 verification
    Upload {
        /// Local file path on the host
        local_path: PathBuf,
        /// Destination file path on the target
        remote_path: String,
        /// Optional octal file permissions (e.g. 0755)
        #[arg(long)]
        mode: Option<String>,
    },
    /// Download a file from the target device to the host with SHA-256 verification
    Download {
        /// Remote file path on the target
        remote_path: String,
        /// Destination file path on the host
        local_path: PathBuf,
    },
}

pub fn build_app_config(cli: &Cli) -> Result<AppConfig> {
    let mut app_config = AppConfig::load(cli.config.as_deref())?;

    if let Some(host) = &cli.host {
        if !host.trim().is_empty() {
            let profile_name = cli.device.clone().unwrap_or_else(|| "cli".to_string());
            app_config.devices.insert(
                profile_name.clone(),
                DeviceProfile {
                    name: profile_name.clone(),
                    host: host.clone(),
                    port: cli.port,
                    username: cli.user.clone(),
                    password: cli.pass.clone(),
                    password_env: None,
                    key_path: cli.key.clone(),
                    description: Some("CLI --host target".to_string()),
                },
            );
            app_config.default_device = Some(profile_name);
        }
    }

    Ok(app_config)
}

async fn run_mcp_stdio(manager: SessionManager, safety: SafetyChecker) -> Result<()> {
    let server = McpServer::new(manager, safety);
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        if let Some(resp) = server.handle_message(&line).await {
            stdout.write_all(resp.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let app_config = build_app_config(&cli)?;
    let manager = SessionManager::new(app_config);
    let safety = SafetyChecker::new(cli.allow_destructive);
    let target_device = cli.device.as_deref();

    match &cli.command {
        None | Some(Commands::Serve) => {
            run_mcp_stdio(manager, safety).await?;
        }
        Some(Commands::Devices) => {
            let devices = manager.list_devices().await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&devices)?);
            } else if devices.is_empty() {
                println!("No devices configured. Pass --host <IP> or configure devices.toml / EMBEDDED_HOST.");
            } else {
                for d in devices {
                    let def = if d.is_default { " (default)" } else { "" };
                    println!(
                        "- {}{}: {}@{}:{} [{}]",
                        d.name,
                        def,
                        d.username,
                        d.host,
                        d.port,
                        d.description.unwrap_or_default()
                    );
                }
            }
        }
        Some(Commands::Info) => {
            let info = manager.system_info(target_device).await?;
            let _ = manager.disconnect_device(target_device).await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&info)?);
            } else {
                println!("=== Board & Kernel ===");
                if !info.board_model.is_empty() {
                    println!("Board : {}", info.board_model);
                }
                println!("Uname : {}", info.uname);
                if !info.uptime.is_empty() {
                    println!("Uptime: {}", info.uptime);
                }
                if !info.cpu_model.is_empty() {
                    println!("\n=== CPU ===\n{}", info.cpu_model);
                }
                println!("\n=== Memory ===\n{}", info.memory);
                println!("\n=== Storage ===\n{}", info.storage);
                println!("\n=== Network ===\n{}", info.network_interfaces);
            }
        }
        Some(Commands::Exec {
            command,
            cwd,
            timeout_ms,
            force,
        }) => {
            match safety.check_command(command, *force) {
                SafetyLevel::Blocked(reason) => bail!("SAFETY BLOCKED: {}", reason),
                SafetyLevel::RequiresConfirmation(reason) => {
                    bail!("CONFIRMATION REQUIRED: {}", reason)
                }
                SafetyLevel::Allowed => {}
            }

            let out = manager
                .exec(target_device, command, cwd.as_deref(), None, *timeout_ms)
                .await?;
            let _ = manager.disconnect_device(target_device).await;

            if cli.json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                if !out.stdout.is_empty() {
                    print!("{}", out.stdout);
                    if !out.stdout.ends_with('\n') {
                        println!();
                    }
                }
                if !out.stderr.is_empty() {
                    eprint!("{}", out.stderr);
                    if !out.stderr.ends_with('\n') {
                        eprintln!();
                    }
                }
                if out.timed_out {
                    eprintln!("[timed out after {} ms]", out.duration_ms);
                }
            }
            if out.exit_code != 0 {
                std::process::exit(out.exit_code);
            }
        }
        Some(Commands::Dmesg { lines, level, grep }) => {
            let entries = manager
                .read_dmesg(target_device, *lines, level.as_deref(), grep.as_deref())
                .await?;
            let _ = manager.disconnect_device(target_device).await;
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "line_count": entries.lines().count(),
                        "entries": entries
                    }))?
                );
            } else {
                print!("{}", entries);
                if !entries.is_empty() && !entries.ends_with('\n') {
                    println!();
                }
            }
        }
        Some(Commands::Read {
            remote_path,
            max_bytes,
            offset,
            base64,
        }) => {
            let encoding = if *base64 { "base64" } else { "utf8" };
            let out = manager
                .file_read(target_device, remote_path, *max_bytes, *offset, encoding)
                .await?;
            let _ = manager.disconnect_device(target_device).await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                print!("{}", out.content);
                if !out.content.is_empty() && !out.content.ends_with('\n') {
                    println!();
                }
            }
        }
        Some(Commands::Upload {
            local_path,
            remote_path,
            mode,
        }) => {
            if let SafetyLevel::Blocked(reason) = safety.check_remote_write(remote_path) {
                bail!("SAFETY BLOCKED: {}", reason);
            }
            let out = manager
                .file_transfer(
                    target_device,
                    "upload",
                    local_path,
                    remote_path,
                    mode.as_deref(),
                )
                .await?;
            let _ = manager.disconnect_device(target_device).await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!(
                    "Uploaded {} -> {} ({} bytes via {}, sha256: {})",
                    out.local_path,
                    out.remote_path,
                    out.bytes_transferred,
                    out.method,
                    out.checksum
                );
            }
        }
        Some(Commands::Download {
            remote_path,
            local_path,
        }) => {
            let out = manager
                .file_transfer(target_device, "download", local_path, remote_path, None)
                .await?;
            let _ = manager.disconnect_device(target_device).await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&out)?);
            } else {
                println!(
                    "Downloaded {} -> {} ({} bytes via {}, sha256: {})",
                    out.remote_path,
                    out.local_path,
                    out.bytes_transferred,
                    out.method,
                    out.checksum
                );
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_mcp_serve_when_no_subcommand_given() {
        let cli = Cli::try_parse_from(["embedded-linux-mcp"]).expect("parse");
        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_exec_subcommand_and_injects_cli_host_profile() {
        let cli = Cli::try_parse_from([
            "embedded-linux-mcp",
            "--host",
            "192.168.1.100",
            "--user",
            "root",
            "--pass",
            "password",
            "exec",
            "ls -la /root",
            "--cwd",
            "/tmp",
        ])
        .expect("parse exec");

        assert_eq!(
            cli.command,
            Some(Commands::Exec {
                command: "ls -la /root".to_string(),
                cwd: Some("/tmp".to_string()),
                timeout_ms: 15_000,
                force: false,
            })
        );

        let cfg = build_app_config(&cli).expect("build_app_config");
        assert_eq!(cfg.default_device.as_deref(), Some("cli"));
        let dev = cfg.devices.get("cli").expect("cli profile");
        assert_eq!(dev.host, "192.168.1.100");
        assert_eq!(dev.username, "root");
        assert_eq!(dev.resolved_password().as_deref(), Some("password"));
    }

    #[test]
    fn parses_upload_and_dmesg_subcommands() {
        let cli = Cli::try_parse_from([
            "embedded-linux-mcp",
            "upload",
            "./app.bin",
            "/tmp/app.bin",
            "--mode",
            "0755",
        ])
        .expect("parse upload");

        assert_eq!(
            cli.command,
            Some(Commands::Upload {
                local_path: PathBuf::from("./app.bin"),
                remote_path: "/tmp/app.bin".to_string(),
                mode: Some("0755".to_string()),
            })
        );
    }
}
