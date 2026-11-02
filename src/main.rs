use anyhow::Result;
use clap::Parser;
use embedded_linux_mcp::config::AppConfig;
use embedded_linux_mcp::mcp::McpServer;
use embedded_linux_mcp::safety::SafetyChecker;
use embedded_linux_mcp::ssh::SessionManager;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Parser, Debug)]
#[command(
    name = "embedded-linux-mcp",
    version,
    about = "MCP server for controlling, testing, and debugging network-connected embedded Linux devices"
)]
struct Cli {
    /// Optional path to devices.toml configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Disable safety blocks against destructive flash/filesystem commands
    #[arg(long, default_value_t = false)]
    allow_destructive: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let app_config = AppConfig::load(cli.config.as_deref())?;
    let manager = SessionManager::new(app_config);
    let safety = SafetyChecker::new(cli.allow_destructive);
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
