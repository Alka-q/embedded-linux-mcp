# Embedded Linux MCP Server & Agent Skill Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an open-source, pure-Rust MCP server (`embedded-linux-mcp`) and companion AI Agent Skill (`skills/embedded-linux/SKILL.md`) for persistent SSH command execution, BusyBox-compatible diagnostics, hybrid file transfer, and hardware debugging on embedded Linux targets (validated against a live Luckfox Pico Plus at `192.168.111.7`).

**Architecture:** A single-binary asynchronous Rust application communicating over `stdio` via JSON-RPC 2.0 (Model Context Protocol). It manages a thread-safe pool of persistent `russh` SSH2 connections with per-command channel multiplexing, timeout/output capping, 3-tier safety filtering (`Blocked`, `RequiresConfirmation`, `Allowed`), and SFTP/POSIX-shell-stream hybrid file transfer, paired with a structured `SKILL.md` knowledge package.

**Tech Stack:** Rust (2021 edition), `tokio`, `russh` (`0.45`), `russh-keys` (`0.45`), `russh-sftp` (`2.0`), `async-trait`, `serde`, `serde_json`, `toml`, `clap`, `sha2`, `base64`, `anyhow`, `thiserror`, `regex`.

**Spec:** `docs/superpowers/specs/2026-11-02-embedded-linux-mcp-skill-design.md`

## Global Constraints

- Zero C toolchain/OpenSSL build dependencies: use `russh` with `ring` or pure-Rust crypto backend so compilation works natively on Windows, Linux, and macOS.
- Default command timeout is `15000` ms (`15` seconds), maximum `300000` ms (`5` minutes).
- Output per stream (`stdout`, `stderr`) and `file_read` default payload is capped at `65536` bytes (`64` KB) to prevent memory/context overflow.
- All built-in diagnostic commands (`system_info`, `read_dmesg`, `file_transfer`, `file_read`) must be POSIX / BusyBox / Dropbear compatible (no GNU-only flags).
- Destructive flash/partition commands are `Blocked` unless `--allow-destructive` is set at startup; connectivity/reboot commands return `RequiresConfirmation` unless `force: true` is passed to `exec_command`.

---

### Task 1: Project Scaffolding, Configuration Loader (`src/config.rs`), and Safety Classifier (`src/safety.rs`)

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `devices.example.toml`
- Create: `src/lib.rs`
- Create: `src/config.rs`
- Create: `src/safety.rs`

**Interfaces:**
- Consumes: *(none — foundational module)*
- Produces:
  - `config::DeviceProfile { name: String, host: String, port: u16, username: String, password: Option<String>, password_env: Option<String>, key_path: Option<String>, description: Option<String> }`
  - `config::AppConfig { default_device: Option<String>, devices: BTreeMap<String, DeviceProfile> }`
  - `config::AppConfig::load(path: Option<&Path>) -> anyhow::Result<AppConfig>`
  - `config::DeviceProfile::resolved_password(&self) -> Option<String>`
  - `safety::SafetyLevel { Allowed, RequiresConfirmation(String), Blocked(String) }`
  - `safety::SafetyChecker::new(allow_destructive: bool) -> Self`
  - `safety::SafetyChecker::check_command(&self, cmd: &str, force: bool) -> SafetyLevel`
  - `safety::SafetyChecker::check_remote_write(&self, remote_path: &str) -> SafetyLevel`

- [ ] **Step 1: Create `Cargo.toml`, `.gitignore`, `devices.example.toml`, and initial module stubs with failing tests**

Create `.gitignore`:
```gitignore
/target
devices.toml
*.log
.DS_Store
```

Create `devices.example.toml`:
```toml
default_device = "luckfox-pico-plus"

[devices.luckfox-pico-plus]
host = "192.168.111.7"
port = 22
username = "root"
password = "luckfox"
description = "Luckfox Pico Plus (Rockchip RV1106G2, ARM Cortex-A7, BusyBox)"
```

Create `Cargo.toml`:
```toml
[package]
name = "embedded-linux-mcp"
version = "0.1.0"
edition = "2021"
description = "MCP server and AI agent toolkit for controlling, testing, and debugging network-connected embedded Linux devices"
license = "MIT"

[dependencies]
anyhow = "1.0"
async-trait = "0.1"
base64 = "0.22"
clap = { version = "4.5", features = ["derive", "env"] }
regex = "1.10"
russh = "0.45"
russh-keys = "0.45"
russh-sftp = "2.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = "0.10"
thiserror = "1.0"
tokio = { version = "1.40", features = ["full"] }
toml = "0.8"
```

Create `src/lib.rs`:
```rust
pub mod config;
pub mod safety;
```

Create `src/config.rs` with unit tests first:
```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceProfile {
    #[serde(default)]
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_username")]
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub password_env: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_port() -> u16 {
    22
}

fn default_username() -> String {
    "root".to_string()
}

impl DeviceProfile {
    pub fn resolved_password(&self) -> Option<String> {
        todo!("implement resolved_password")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub default_device: Option<String>,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceProfile>,
}

impl AppConfig {
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        todo!("implement from_toml_str")
    }

    pub fn load(path: Option<&Path>) -> Result<Self> {
        todo!("implement load")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_devices_toml_and_normalizes_names() {
        let raw = r#"
            default_device = "luckfox"

            [devices.luckfox]
            host = "192.168.111.7"
            username = "root"
            password = "luckfox"
            description = "Luckfox Pico Plus"
        "#;
        let cfg = AppConfig::from_toml_str(raw).expect("valid toml");
        assert_eq!(cfg.default_device.as_deref(), Some("luckfox"));
        let dev = cfg.devices.get("luckfox").expect("luckfox profile");
        assert_eq!(dev.name, "luckfox");
        assert_eq!(dev.host, "192.168.111.7");
        assert_eq!(dev.port, 22);
        assert_eq!(dev.resolved_password().as_deref(), Some("luckfox"));
    }

    #[test]
    fn resolves_password_from_env_when_configured() {
        std::env::set_var("TEST_LUCKFOX_PASS", "secret123");
        let dev = DeviceProfile {
            name: "luckfox".into(),
            host: "192.168.111.7".into(),
            port: 22,
            username: "root".into(),
            password: None,
            password_env: Some("TEST_LUCKFOX_PASS".into()),
            key_path: None,
            description: None,
        };
        assert_eq!(dev.resolved_password().as_deref(), Some("secret123"));
        std::env::remove_var("TEST_LUCKFOX_PASS");
    }
}
```

Create `src/safety.rs` with unit tests first:
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyLevel {
    Allowed,
    RequiresConfirmation(String),
    Blocked(String),
}

#[derive(Debug, Clone)]
pub struct SafetyChecker {
    allow_destructive: bool,
}

impl SafetyChecker {
    pub fn new(allow_destructive: bool) -> Self {
        Self { allow_destructive }
    }

    pub fn check_command(&self, _cmd: &str, _force: bool) -> SafetyLevel {
        todo!("implement check_command")
    }

    pub fn check_remote_write(&self, _remote_path: &str) -> SafetyLevel {
        todo!("implement check_remote_write")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_catastrophic_commands_by_default() {
        let checker = SafetyChecker::new(false);
        assert!(matches!(
            checker.check_command("rm -rf /", true),
            SafetyLevel::Blocked(_)
        ));
        assert!(matches!(
            checker.check_command("dd if=boot.img of=/dev/mtdblock0", true),
            SafetyLevel::Blocked(_)
        ));
        assert!(matches!(
            checker.check_command("mkfs.ext4 /dev/mmcblk0p1", true),
            SafetyLevel::Blocked(_)
        ));
    }

    #[test]
    fn requires_confirmation_for_reboot_and_network_down_unless_forced() {
        let checker = SafetyChecker::new(false);
        assert!(matches!(
            checker.check_command("reboot", false),
            SafetyLevel::RequiresConfirmation(_)
        ));
        assert_eq!(checker.check_command("reboot", true), SafetyLevel::Allowed);

        assert!(matches!(
            checker.check_command("ip link set eth0 down", false),
            SafetyLevel::RequiresConfirmation(_)
        ));
        assert_eq!(
            checker.check_command("ip link set eth0 down", true),
            SafetyLevel::Allowed
        );
    }

    #[test]
    fn allows_normal_embedded_diagnostics() {
        let checker = SafetyChecker::new(false);
        assert_eq!(
            checker.check_command("cat /proc/cpuinfo && dmesg | tail -n 20", false),
            SafetyLevel::Allowed
        );
        assert_eq!(
            checker.check_command("echo 55 > /sys/class/gpio/export", false),
            SafetyLevel::Allowed
        );
    }

    #[test]
    fn blocks_direct_raw_block_device_file_writes() {
        let checker = SafetyChecker::new(false);
        assert!(matches!(
            checker.check_remote_write("/dev/mtdblock0"),
            SafetyLevel::Blocked(_)
        ));
        assert_eq!(
            checker.check_remote_write("/tmp/test_binary"),
            SafetyLevel::Allowed
        );
    }
}
```

- [ ] **Step 2: Run `cargo test` to verify the tests fail as expected**

Run: `cargo test`
Expected: FAIL with `not yet implemented: implement ...`

- [ ] **Step 3: Implement `src/config.rs` and `src/safety.rs`**

Update `src/config.rs`:
```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceProfile {
    #[serde(default)]
    pub name: String,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_username")]
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub password_env: Option<String>,
    #[serde(default)]
    pub key_path: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_port() -> u16 {
    22
}

fn default_username() -> String {
    "root".to_string()
}

impl DeviceProfile {
    pub fn resolved_password(&self) -> Option<String> {
        if let Some(env_key) = &self.password_env {
            if let Ok(val) = std::env::var(env_key) {
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
        self.password.clone()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    #[serde(default)]
    pub default_device: Option<String>,
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceProfile>,
}

impl AppConfig {
    pub fn from_toml_str(raw: &str) -> Result<Self> {
        let mut cfg: AppConfig = toml::from_str(raw).context("Failed to parse TOML config")?;
        for (key, profile) in cfg.devices.iter_mut() {
            if profile.name.is_empty() {
                profile.name = key.clone();
            }
        }
        if cfg.default_device.is_none() && cfg.devices.len() == 1 {
            cfg.default_device = cfg.devices.keys().next().cloned();
        }
        Ok(cfg)
    }

    pub fn load(path: Option<&Path>) -> Result<Self> {
        let mut cfg = if let Some(p) = path {
            let content = std::fs::read_to_string(p)
                .with_context(|| format!("Failed to read config file: {}", p.display()))?;
            Self::from_toml_str(&content)?
        } else if Path::new("devices.toml").exists() {
            let content = std::fs::read_to_string("devices.toml")
                .context("Failed to read ./devices.toml")?;
            Self::from_toml_str(&content)?
        } else {
            AppConfig::default()
        };

        if let Ok(env_host) = std::env::var("EMBEDDED_HOST") {
            if !env_host.trim().is_empty() {
                let port = std::env::var("EMBEDDED_PORT")
                    .ok()
                    .and_then(|p| p.parse::<u16>().ok())
                    .unwrap_or(22);
                let username = std::env::var("EMBEDDED_USER").unwrap_or_else(|_| "root".into());
                let password = std::env::var("EMBEDDED_PASS").ok();
                let key_path = std::env::var("EMBEDDED_KEY").ok();
                let name = std::env::var("EMBEDDED_NAME").unwrap_or_else(|_| "default".into());

                cfg.devices.insert(
                    name.clone(),
                    DeviceProfile {
                        name: name.clone(),
                        host: env_host,
                        port,
                        username,
                        password,
                        password_env: None,
                        key_path,
                        description: Some("Loaded from EMBEDDED_* environment variables".into()),
                    },
                );
                if cfg.default_device.is_none() {
                    cfg.default_device = Some(name);
                }
            }
        }

        Ok(cfg)
    }
}
```

Update `src/safety.rs`:
```rust
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SafetyLevel {
    Allowed,
    RequiresConfirmation(String),
    Blocked(String),
}

#[derive(Debug, Clone)]
pub struct SafetyChecker {
    allow_destructive: bool,
}

struct CompiledRules {
    blocked: Vec<(Regex, &'static str)>,
    confirm: Vec<(Regex, &'static str)>,
}

fn rules() -> &'static CompiledRules {
    static RULES: OnceLock<CompiledRules> = OnceLock::new();
    RULES.get_or_init(|| CompiledRules {
        blocked: vec![
            (
                Regex::new(r"(?i)(?:^|[;&|]\s*)rm\s+(-[a-zA-Z]*r[a-zA-Z]*\s+)*(-[a-zA-Z]*f[a-zA-Z]*\s+)*/(\s|$|[;&|])").unwrap(),
                "Root filesystem deletion ('rm -rf /') is blocked.",
            ),
            (
                Regex::new(r"(?i)\bmkfs(\.[a-z0-9]+)?\b").unwrap(),
                "Filesystem formatting ('mkfs') is blocked to protect flash/storage partitions.",
            ),
            (
                Regex::new(r"(?i)\bflash_erase(all)?\b").unwrap(),
                "Direct MTD flash erasing ('flash_erase') is blocked.",
            ),
            (
                Regex::new(r"(?i)\bdd\b[^\n;|&]*\bof=/dev/(mtd|mmcblk|sd|nvme)").unwrap(),
                "Raw block/flash device overwrite via 'dd' is blocked.",
            ),
        ],
        confirm: vec![
            (
                Regex::new(r"(?i)(?:^|[;&|]\s*)(reboot|poweroff|halt|shutdown)\b").unwrap(),
                "This command will reboot or halt the embedded device. Confirm with the user and retry with force=true.",
            ),
            (
                Regex::new(r"(?i)\bip\s+link\s+set\s+(dev\s+)?\S+\s+down\b").unwrap(),
                "Bringing a network interface down may sever the SSH session. Confirm with the user and retry with force=true.",
            ),
            (
                Regex::new(r"(?i)\bifconfig\s+\S+\s+down\b").unwrap(),
                "Bringing a network interface down may sever the SSH session. Confirm with the user and retry with force=true.",
            ),
            (
                Regex::new(r"(?i)\b(killall|pkill)\s+(-[0-9a-zA-Z]+\s+)?(dropbear|sshd)\b").unwrap(),
                "Terminating the SSH daemon will sever remote access. Confirm with the user and retry with force=true.",
            ),
        ],
    })
}

impl SafetyChecker {
    pub fn new(allow_destructive: bool) -> Self {
        Self { allow_destructive }
    }

    pub fn check_command(&self, cmd: &str, force: bool) -> SafetyLevel {
        let compiled = rules();
        if !self.allow_destructive {
            for (re, reason) in &compiled.blocked {
                if re.is_match(cmd) {
                    return SafetyLevel::Blocked((*reason).to_string());
                }
            }
        }

        if !force {
            for (re, reason) in &compiled.confirm {
                if re.is_match(cmd) {
                    return SafetyLevel::RequiresConfirmation((*reason).to_string());
                }
            }
        }

        SafetyLevel::Allowed
    }

    pub fn check_remote_write(&self, remote_path: &str) -> SafetyLevel {
        if !self.allow_destructive {
            let trimmed = remote_path.trim();
            if trimmed.starts_with("/dev/mtd")
                || trimmed.starts_with("/dev/mmcblk")
                || trimmed.starts_with("/dev/sd")
                || trimmed.starts_with("/dev/nvme")
            {
                return SafetyLevel::Blocked(format!(
                    "Direct file write to raw storage device '{}' is blocked.",
                    trimmed
                ));
            }
        }
        SafetyLevel::Allowed
    }
}
```

- [ ] **Step 4: Run `cargo test` to verify all config and safety tests pass**

Run: `cargo test`
Expected: PASS (6 tests passing)

- [ ] **Step 5: Commit Task 1**

```bash
git add Cargo.toml Cargo.lock .gitignore devices.example.toml src/lib.rs src/config.rs src/safety.rs
git commit -m "feat(core): add configuration loader and 3-tier safety classifier"
```

---

### Task 2: Persistent SSH Session, Command Execution, Hybrid File Transfer, and Session Pool (`src/ssh/`)

**Files:**
- Create: `src/ssh/mod.rs`
- Create: `src/ssh/session.rs`
- Create: `src/ssh/transfer.rs`
- Create: `src/ssh/manager.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `crate::config::{AppConfig, DeviceProfile}`
- Produces:
  - `ssh::ExecOutput { exit_code: i32, stdout: String, stderr: String, duration_ms: u64, timed_out: bool, truncated: bool }`
  - `ssh::FileReadOutput { remote_path: String, content: String, encoding: String, bytes_read: usize, truncated: bool }`
  - `ssh::FileTransferOutput { direction: String, local_path: String, remote_path: String, bytes_transferred: usize, method: String, checksum: String }`
  - `ssh::SystemInfoOutput { uname: String, cpu_model: String, board_model: String, uptime: String, memory: String, storage: String, network_interfaces: String }`
  - `ssh::SessionManager::new(config: AppConfig) -> Self`
  - `ssh::SessionManager::list_devices(&self) -> Vec<DeviceStatus>`
  - `ssh::SessionManager::connect_device(&self, profile: DeviceProfile, set_default: bool) -> Result<ConnectSummary>`
  - `ssh::SessionManager::disconnect_device(&self, device: Option<&str>) -> Result<String>`
  - `ssh::SessionManager::exec(&self, device: Option<&str>, command: &str, cwd: Option<&str>, env: Option<&BTreeMap<String, String>>, timeout_ms: u64) -> Result<ExecOutput>`
  - `ssh::SessionManager::system_info(&self, device: Option<&str>) -> Result<SystemInfoOutput>`
  - `ssh::SessionManager::read_dmesg(&self, device: Option<&str>, lines: usize, level: Option<&str>, grep: Option<&str>) -> Result<String>`
  - `ssh::SessionManager::file_read(&self, device: Option<&str>, remote_path: &str, max_bytes: usize, offset: usize, encoding: &str) -> Result<FileReadOutput>`
  - `ssh::SessionManager::file_transfer(&self, device: Option<&str>, direction: &str, local_path: &Path, remote_path: &str, mode: Option<&str>) -> Result<FileTransferOutput>`

- [ ] **Step 1: Create `src/ssh/` modules and unit tests for command wrapping, output truncation, and shell escaping**

Update `src/lib.rs`:
```rust
pub mod config;
pub mod safety;
pub mod ssh;
```

Create `src/ssh/mod.rs`:
```rust
pub mod manager;
pub mod session;
pub mod transfer;

pub use manager::{ConnectSummary, DeviceStatus, SessionManager};
pub use session::{ExecOutput, SshDeviceSession, MAX_OUTPUT_BYTES};
pub use transfer::{FileReadOutput, FileTransferOutput, SystemInfoOutput};
```

Create `src/ssh/session.rs`:
```rust
use crate::config::DeviceProfile;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use russh::client::{self, Handle, Msg};
use russh::{ChannelMsg, Disconnect};
use russh_keys::key::PublicKey;
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
        let mut config = client::Config::default();
        config.inactivity_timeout = Some(Duration::from_secs(300));
        config.keepalive_interval = Some(Duration::from_secs(30));
        let config = Arc::new(config);

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
```

Create `src/ssh/transfer.rs`:
```rust
use crate::ssh::session::{shell_quote, SshDeviceSession, MAX_OUTPUT_BYTES};
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

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
        (String::from_utf8_lossy(slice).to_string(), "utf8".to_string())
    };

    Ok(FileReadOutput {
        remote_path: remote_path.to_string(),
        content,
        encoding: final_encoding,
        bytes_read: slice.len(),
        truncated,
    })
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
            let data = std::fs::read(local_path).with_context(|| {
                format!("Failed to read local file '{}'", local_path.display())
            })?;
            let checksum = sha256_hex(&data);

            let mut cmd = format!("cat > {}", quoted_remote);
            if let Some(m) = mode {
                if m.chars().all(|c| c.is_ascii_digit()) && !m.is_empty() {
                    cmd.push_str(&format!(" && chmod {} {}", m, quoted_remote));
                }
            }

            let (exit_code, _, err_bytes, _, timed_out, _) = session
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

            Ok(FileTransferOutput {
                direction: "upload".to_string(),
                local_path: local_path.display().to_string(),
                remote_path: remote_path.to_string(),
                bytes_transferred: data.len(),
                method: "shell_stream".to_string(),
                checksum,
            })
        }
        "download" => {
            let cmd = format!("cat {}", quoted_remote);
            let max_download = 16 * 1024 * 1024; // 16 MB safety limit
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
                method: "shell_stream".to_string(),
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
```

Create `src/ssh/manager.rs`:
```rust
use crate::config::{AppConfig, DeviceProfile};
use crate::ssh::session::{shell_quote, ExecOutput, SshDeviceSession};
use crate::ssh::transfer::{
    gather_system_info, read_remote_file, transfer_file, FileReadOutput, FileTransferOutput,
    SystemInfoOutput,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceStatus {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub connected: bool,
    pub is_default: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectSummary {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub connected: bool,
    pub is_default: bool,
    pub uname: String,
}

struct ManagerState {
    default_device: Option<String>,
    profiles: BTreeMap<String, DeviceProfile>,
    sessions: BTreeMap<String, SshDeviceSession>,
}

#[derive(Clone)]
pub struct SessionManager {
    state: Arc<Mutex<ManagerState>>,
}

impl SessionManager {
    pub fn new(config: AppConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(ManagerState {
                default_device: config.default_device,
                profiles: config.devices,
                sessions: BTreeMap::new(),
            })),
        }
    }

    pub async fn list_devices(&self) -> Vec<DeviceStatus> {
        let st = self.state.lock().await;
        st.profiles
            .values()
            .map(|p| DeviceStatus {
                name: p.name.clone(),
                host: p.host.clone(),
                port: p.port,
                username: p.username.clone(),
                connected: st.sessions.contains_key(&p.name),
                is_default: st.default_device.as_deref() == Some(p.name.as_str()),
                description: p.description.clone(),
            })
            .collect()
    }

    pub async fn connect_device(
        &self,
        mut profile: DeviceProfile,
        set_default: bool,
    ) -> Result<ConnectSummary> {
        if profile.name.trim().is_empty() {
            profile.name = profile.host.clone();
        }
        let name = profile.name.clone();
        let mut session = SshDeviceSession::connect(profile.clone()).await?;
        let uname_out = session
            .exec("uname -a", None, None, 5_000)
            .await
            .map(|o| o.stdout.trim().to_string())
            .unwrap_or_default();

        let mut st = self.state.lock().await;
        st.profiles.insert(name.clone(), profile.clone());
        st.sessions.insert(name.clone(), session);
        if set_default || st.default_device.is_none() {
            st.default_device = Some(name.clone());
        }
        let is_default = st.default_device.as_deref() == Some(name.as_str());

        Ok(ConnectSummary {
            name,
            host: profile.host,
            port: profile.port,
            username: profile.username,
            connected: true,
            is_default,
            uname: uname_out,
        })
    }

    pub async fn disconnect_device(&self, device: Option<&str>) -> Result<String> {
        let mut st = self.state.lock().await;
        let target = Self::resolve_device_name(&st, device)?;
        if let Some(mut sess) = st.sessions.remove(&target) {
            let _ = sess.close().await;
        }
        Ok(target)
    }

    fn resolve_device_name(st: &ManagerState, device: Option<&str>) -> Result<String> {
        if let Some(d) = device {
            if !d.trim().is_empty() {
                if st.profiles.contains_key(d) {
                    return Ok(d.to_string());
                }
                // Also allow matching by IP/host directly
                if let Some(found) = st.profiles.values().find(|p| p.host == d) {
                    return Ok(found.name.clone());
                }
                return Err(anyhow!(
                    "Unknown device '{}'. Use device_connect first or check device_list.",
                    d
                ));
            }
        }
        if let Some(def) = &st.default_device {
            return Ok(def.clone());
        }
        if st.profiles.len() == 1 {
            return Ok(st.profiles.keys().next().unwrap().clone());
        }
        Err(anyhow!(
            "No active default device configured. Call device_connect with host (e.g. '192.168.111.7') first."
        ))
    }

    async fn ensure_connected<'a>(
        st: &'a mut ManagerState,
        target: &str,
    ) -> Result<&'a mut SshDeviceSession> {
        if !st.sessions.contains_key(target) {
            let profile = st
                .profiles
                .get(target)
                .cloned()
                .ok_or_else(|| anyhow!("Profile '{}' not found", target))?;
            let session = SshDeviceSession::connect(profile).await?;
            st.sessions.insert(target.to_string(), session);
        }
        Ok(st.sessions.get_mut(target).unwrap())
    }

    pub async fn exec(
        &self,
        device: Option<&str>,
        command: &str,
        cwd: Option<&str>,
        env: Option<&BTreeMap<String, String>>,
        timeout_ms: u64,
    ) -> Result<ExecOutput> {
        let mut st = self.state.lock().await;
        let target = Self::resolve_device_name(&st, device)?;

        let first_attempt = {
            let sess = Self::ensure_connected(&mut st, &target).await?;
            sess.exec(command, cwd, env, timeout_ms).await
        };

        match first_attempt {
            Ok(out) => Ok(out),
            Err(_) => {
                // Auto-reconnect once if the persistent SSH channel broke
                st.sessions.remove(&target);
                let sess = Self::ensure_connected(&mut st, &target).await?;
                sess.exec(command, cwd, env, timeout_ms).await
            }
        }
    }

    pub async fn system_info(&self, device: Option<&str>) -> Result<SystemInfoOutput> {
        let mut st = self.state.lock().await;
        let target = Self::resolve_device_name(&st, device)?;
        let first_attempt = {
            let sess = Self::ensure_connected(&mut st, &target).await?;
            gather_system_info(sess).await
        };
        match first_attempt {
            Ok(info) => Ok(info),
            Err(_) => {
                st.sessions.remove(&target);
                let sess = Self::ensure_connected(&mut st, &target).await?;
                gather_system_info(sess).await
            }
        }
    }

    pub async fn read_dmesg(
        &self,
        device: Option<&str>,
        lines: usize,
        level: Option<&str>,
        grep: Option<&str>,
    ) -> Result<String> {
        let clamped_lines = lines.clamp(1, 2000);
        let mut pipeline = String::from("dmesg");
        if let Some(lvl) = level {
            let l = lvl.trim().to_lowercase();
            if ["emerg", "alert", "crit", "err", "warn", "notice", "info", "debug"]
                .contains(&l.as_str())
            {
                pipeline = format!("(dmesg -l {} 2>/dev/null || dmesg)", l);
            }
        }
        if let Some(pattern) = grep {
            if !pattern.trim().is_empty() {
                pipeline.push_str(&format!(" | grep -i -- {}", shell_quote(pattern)));
            }
        }
        pipeline.push_str(&format!(" | tail -n {}", clamped_lines));

        let out = self.exec(device, &pipeline, None, None, 10_000).await?;
        Ok(out.stdout)
    }

    pub async fn file_read(
        &self,
        device: Option<&str>,
        remote_path: &str,
        max_bytes: usize,
        offset: usize,
        encoding: &str,
    ) -> Result<FileReadOutput> {
        let mut st = self.state.lock().await;
        let target = Self::resolve_device_name(&st, device)?;
        let sess = Self::ensure_connected(&mut st, &target).await?;
        read_remote_file(sess, remote_path, max_bytes, offset, encoding).await
    }

    pub async fn file_transfer(
        &self,
        device: Option<&str>,
        direction: &str,
        local_path: &Path,
        remote_path: &str,
        mode: Option<&str>,
    ) -> Result<FileTransferOutput> {
        let mut st = self.state.lock().await;
        let target = Self::resolve_device_name(&st, device)?;
        let sess = Self::ensure_connected(&mut st, &target).await?;
        transfer_file(sess, direction, local_path, remote_path, mode).await
    }
}
```

- [ ] **Step 2: Run `cargo test` to verify all SSH helper and parser unit tests compile and pass**

Run: `cargo test`
Expected: PASS (9 tests passing)

- [ ] **Step 3: Commit Task 2**

```bash
git add src/lib.rs src/ssh/mod.rs src/ssh/session.rs src/ssh/transfer.rs src/ssh/manager.rs
git commit -m "feat(ssh): add persistent russh session pool, diagnostics, and file transfer"
```

---

### Task 3: MCP JSON-RPC 2.0 Server, Tool Schemas, and Tool Dispatchers (`src/mcp/`, `src/main.rs`)

**Files:**
- Create: `src/mcp/mod.rs`
- Create: `src/mcp/tools.rs`
- Create: `src/main.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes:
  - `crate::config::{AppConfig, DeviceProfile}`
  - `crate::safety::{SafetyChecker, SafetyLevel}`
  - `crate::ssh::SessionManager`
- Produces:
  - `mcp::McpServer::new(manager: SessionManager, safety: SafetyChecker) -> Self`
  - `mcp::McpServer::handle_message(&self, raw_json: &str) -> Option<String>`
  - `mcp::tools::tool_definitions() -> serde_json::Value`
  - Binary entrypoint `embedded-linux-mcp` (`--config <path>`, `--allow-destructive`)

- [ ] **Step 1: Create `src/mcp/mod.rs`, `src/mcp/tools.rs`, and `src/main.rs` with JSON-RPC & tool dispatch unit tests**

Update `src/lib.rs`:
```rust
pub mod config;
pub mod mcp;
pub mod safety;
pub mod ssh;
```

Create `src/mcp/mod.rs`:
```rust
pub mod tools;

use crate::safety::SafetyChecker;
use crate::ssh::SessionManager;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

pub struct McpServer {
    manager: SessionManager,
    safety: SafetyChecker,
}

impl McpServer {
    pub fn new(manager: SessionManager, safety: SafetyChecker) -> Self {
        Self { manager, safety }
    }

    pub async fn handle_message(&self, line: &str) -> Option<String> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: Value::Null,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                    }),
                };
                return serde_json::to_string(&resp).ok();
            }
        };

        // Notifications (no id) do not receive a response in JSON-RPC 2.0
        let id = req.id.clone()?;

        let response = match req.method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "embedded-linux-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                })),
                error: None,
            },
            "ping" => JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({})),
                error: None,
            },
            "tools/list" => JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: Some(json!({
                    "tools": tools::tool_definitions()
                })),
                error: None,
            },
            "tools/call" => {
                let tool_name = req
                    .params
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let arguments = req
                    .params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));

                let call_result =
                    tools::dispatch_tool(tool_name, arguments, &self.manager, &self.safety).await;

                match call_result {
                    Ok(payload) => JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: Some(json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&payload).unwrap_or_default()
                            }],
                            "isError": false
                        })),
                        error: None,
                    },
                    Err(err_msg) => JsonRpcResponse {
                        jsonrpc: "2.0",
                        id,
                        result: Some(json!({
                            "content": [{
                                "type": "text",
                                "text": err_msg
                            }],
                            "isError": true
                        })),
                        error: None,
                    },
                }
            }
            other => JsonRpcResponse {
                jsonrpc: "2.0",
                id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: format!("Method not found: {}", other),
                }),
            },
        };

        serde_json::to_string(&response).ok()
    }
}
```

Create `src/mcp/tools.rs`:
```rust
use crate::config::DeviceProfile;
use crate::safety::{SafetyChecker, SafetyLevel};
use crate::ssh::SessionManager;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "device_list",
            "description": "List all configured and runtime-connected embedded Linux devices and their connection status.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "device_connect",
            "description": "Connect to a network-attached embedded Linux device over SSH (e.g. Luckfox Pico Plus at 192.168.111.7) and register it in the session pool.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "host": { "type": "string", "description": "IP address or hostname of the target device (e.g. '192.168.111.7')" },
                    "name": { "type": "string", "description": "Optional alias for the device (defaults to host)" },
                    "port": { "type": "integer", "description": "SSH port (default: 22)", "default": 22 },
                    "username": { "type": "string", "description": "SSH username (default: 'root')", "default": "root" },
                    "password": { "type": "string", "description": "SSH password (e.g. 'luckfox')" },
                    "key_path": { "type": "string", "description": "Optional path to SSH private key file" },
                    "description": { "type": "string", "description": "Optional description of the board" },
                    "set_default": { "type": "boolean", "description": "Set this device as the default target (default: true)", "default": true }
                },
                "required": ["host"]
            }
        },
        {
            "name": "device_disconnect",
            "description": "Close the persistent SSH session for a device.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "device": { "type": "string", "description": "Device alias or IP (optional; defaults to active default device)" }
                }
            }
        },
        {
            "name": "exec_command",
            "description": "Execute a shell command on the target embedded Linux device with timeout and safety guardrails.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command to run on the target device" },
                    "device": { "type": "string", "description": "Target device alias or IP (optional)" },
                    "cwd": { "type": "string", "description": "Working directory (e.g. '/tmp')" },
                    "env": { "type": "object", "additionalProperties": { "type": "string" }, "description": "Optional environment variables" },
                    "timeout_ms": { "type": "integer", "description": "Execution timeout in milliseconds (default: 15000)", "default": 15000 },
                    "force": { "type": "boolean", "description": "Set true ONLY after explicit user approval for commands that reboot or drop network interfaces", "default": false }
                },
                "required": ["command"]
            }
        },
        {
            "name": "system_info",
            "description": "Gather BusyBox-compatible hardware, SoC, kernel, device-tree board model, RAM, storage, and network interface details.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "device": { "type": "string", "description": "Target device alias or IP (optional)" }
                }
            }
        },
        {
            "name": "read_dmesg",
            "description": "Read and filter the kernel ring buffer (dmesg) on the embedded target.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "device": { "type": "string", "description": "Target device alias or IP (optional)" },
                    "lines": { "type": "integer", "description": "Number of tail lines to return (default: 100)", "default": 100 },
                    "level": { "type": "string", "description": "Optional log level filter (emerg, alert, crit, err, warn, notice, info, debug)" },
                    "grep": { "type": "string", "description": "Optional case-insensitive substring/regex filter (e.g. 'gpio', 'i2c', 'mmc', 'usb')" }
                }
            }
        },
        {
            "name": "file_read",
            "description": "Read a text, binary, /proc, or /sys file from the embedded target with byte cap.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "remote_path": { "type": "string", "description": "Absolute path on the target device (e.g. '/proc/device-tree/model' or '/sys/kernel/debug/gpio')" },
                    "device": { "type": "string", "description": "Target device alias or IP (optional)" },
                    "max_bytes": { "type": "integer", "description": "Maximum bytes to read (default: 65536)", "default": 65536 },
                    "offset": { "type": "integer", "description": "Byte offset to start reading from (default: 0)", "default": 0 },
                    "encoding": { "type": "string", "enum": ["utf8", "base64"], "default": "utf8" }
                },
                "required": ["remote_path"]
            }
        },
        {
            "name": "file_transfer",
            "description": "Upload or download files/binaries between the host machine and the embedded target with SHA-256 verification.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "direction": { "type": "string", "enum": ["upload", "download"], "description": "'upload' (host -> device) or 'download' (device -> host)" },
                    "local_path": { "type": "string", "description": "Path on the host computer" },
                    "remote_path": { "type": "string", "description": "Path on the embedded target (prefer '/tmp/...' for test binaries)" },
                    "device": { "type": "string", "description": "Target device alias or IP (optional)" },
                    "mode": { "type": "string", "description": "Optional octal permissions to apply after upload (e.g. '0755')" }
                },
                "required": ["direction", "local_path", "remote_path"]
            }
        }
    ])
}

pub async fn dispatch_tool(
    name: &str,
    args: Value,
    manager: &SessionManager,
    safety: &SafetyChecker,
) -> std::result::Result<Value, String> {
    match name {
        "device_list" => {
            let devices = manager.list_devices().await;
            Ok(json!({ "devices": devices }))
        }
        "device_connect" => {
            let host = args
                .get("host")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'host'".to_string())?
                .to_string();
            let dev_name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(&host)
                .to_string();
            let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(22) as u16;
            let username = args
                .get("username")
                .and_then(|v| v.as_str())
                .unwrap_or("root")
                .to_string();
            let password = args
                .get("password")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let key_path = args
                .get("key_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let description = args
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let set_default = args
                .get("set_default")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let profile = DeviceProfile {
                name: dev_name,
                host,
                port,
                username,
                password,
                password_env: None,
                key_path,
                description,
            };

            let summary = manager
                .connect_device(profile, set_default)
                .await
                .map_err(|e| format!("{:#}", e))?;
            Ok(serde_json::to_value(summary).unwrap())
        }
        "device_disconnect" => {
            let device = args.get("device").and_then(|v| v.as_str());
            let closed = manager
                .disconnect_device(device)
                .await
                .map_err(|e| format!("{:#}", e))?;
            Ok(json!({ "device": closed, "disconnected": true }))
        }
        "exec_command" => {
            let command = args
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'command'".to_string())?;
            let device = args.get("device").and_then(|v| v.as_str());
            let cwd = args.get("cwd").and_then(|v| v.as_str());
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(15_000);
            let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
            let env_map: Option<BTreeMap<String, String>> = args
                .get("env")
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            match safety.check_command(command, force) {
                SafetyLevel::Blocked(reason) => {
                    return Err(format!("SAFETY BLOCKED: {}", reason));
                }
                SafetyLevel::RequiresConfirmation(reason) => {
                    return Err(format!("CONFIRMATION REQUIRED: {}", reason));
                }
                SafetyLevel::Allowed => {}
            }

            let out = manager
                .exec(device, command, cwd, env_map.as_ref(), timeout_ms)
                .await
                .map_err(|e| format!("{:#}", e))?;
            Ok(serde_json::to_value(out).unwrap())
        }
        "system_info" => {
            let device = args.get("device").and_then(|v| v.as_str());
            let info = manager
                .system_info(device)
                .await
                .map_err(|e| format!("{:#}", e))?;
            Ok(serde_json::to_value(info).unwrap())
        }
        "read_dmesg" => {
            let device = args.get("device").and_then(|v| v.as_str());
            let lines = args.get("lines").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
            let level = args.get("level").and_then(|v| v.as_str());
            let grep = args.get("grep").and_then(|v| v.as_str());
            let entries = manager
                .read_dmesg(device, lines, level, grep)
                .await
                .map_err(|e| format!("{:#}", e))?;
            let line_count = entries.lines().count();
            Ok(json!({
                "line_count": line_count,
                "entries": entries
            }))
        }
        "file_read" => {
            let remote_path = args
                .get("remote_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'remote_path'".to_string())?;
            let device = args.get("device").and_then(|v| v.as_str());
            let max_bytes = args
                .get("max_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(65_536) as usize;
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let encoding = args
                .get("encoding")
                .and_then(|v| v.as_str())
                .unwrap_or("utf8");

            let out = manager
                .file_read(device, remote_path, max_bytes, offset, encoding)
                .await
                .map_err(|e| format!("{:#}", e))?;
            Ok(serde_json::to_value(out).unwrap())
        }
        "file_transfer" => {
            let direction = args
                .get("direction")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'direction'".to_string())?;
            let local_path = args
                .get("local_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'local_path'".to_string())?;
            let remote_path = args
                .get("remote_path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Missing required parameter 'remote_path'".to_string())?;
            let device = args.get("device").and_then(|v| v.as_str());
            let mode = args.get("mode").and_then(|v| v.as_str());

            if direction.eq_ignore_ascii_case("upload") {
                if let SafetyLevel::Blocked(reason) = safety.check_remote_write(remote_path) {
                    return Err(format!("SAFETY BLOCKED: {}", reason));
                }
            }

            let out = manager
                .file_transfer(device, direction, Path::new(local_path), remote_path, mode)
                .await
                .map_err(|e| format!("{:#}", e))?;
            Ok(serde_json::to_value(out).unwrap())
        }
        other => Err(format!("Unknown tool '{}'", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::mcp::McpServer;

    #[tokio::test]
    async fn handles_initialize_and_tools_list() {
        let server = McpServer::new(
            SessionManager::new(AppConfig::default()),
            SafetyChecker::new(false),
        );

        let init_resp = server
            .handle_message(r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#)
            .await
            .expect("response");
        assert!(init_resp.contains("embedded-linux-mcp"));

        let list_resp = server
            .handle_message(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#)
            .await
            .expect("response");
        assert!(list_resp.contains("device_connect"));
        assert!(list_resp.contains("exec_command"));
        assert!(list_resp.contains("file_transfer"));
    }

    #[tokio::test]
    async fn blocks_dangerous_commands_via_mcp_call() {
        let server = McpServer::new(
            SessionManager::new(AppConfig::default()),
            SafetyChecker::new(false),
        );

        let call_resp = server
            .handle_message(
                r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"exec_command","arguments":{"command":"rm -rf /"}}}"#,
            )
            .await
            .expect("response");
        assert!(call_resp.contains("SAFETY BLOCKED"));
        assert!(call_resp.contains(r#""isError":true"#));
    }
}
```

Create `src/main.rs`:
```rust
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
```

- [ ] **Step 2: Run `cargo test` and `cargo build` to verify MCP server compiles and tests pass**

Run: `cargo test`
Expected: PASS (11 tests passing)

- [ ] **Step 3: Commit Task 3**

```bash
git add src/lib.rs src/mcp/mod.rs src/mcp/tools.rs src/main.rs
git commit -m "feat(mcp): implement stdio JSON-RPC 2.0 MCP server and 8 embedded Linux tools"
```

---

### Task 4: Open-Source Agent Skill (`skills/embedded-linux/`), Documentation (`README.md`, `LICENSE`), and Live Hardware Verification (`192.168.111.7`)

**Files:**
- Create: `skills/embedded-linux/SKILL.md`
- Create: `skills/embedded-linux/references/luckfox-rv1106.md`
- Create: `skills/embedded-linux/references/sysfs-debug.md`
- Create: `LICENSE`
- Create: `README.md`
- Create: `tests/live_luckfox.rs` (opt-in integration test gated by `LUCKFOX_LIVE_TEST=1`)

**Interfaces:**
- Consumes: `embedded-linux-mcp` binary and library crates
- Produces:
  - Complete agent skill package (`skills/embedded-linux/SKILL.md` + `references/`)
  - Open-source documentation (`README.md`, `LICENSE`)
  - Opt-in hardware-in-the-loop integration test (`tests/live_luckfox.rs`) verified against `192.168.111.7`

- [ ] **Step 1: Create `skills/embedded-linux/SKILL.md` and reference guides**

Create `skills/embedded-linux/SKILL.md`:
```markdown
---
name: embedded-linux
description: Use when controlling, testing, inspecting, or debugging network-connected embedded Linux boards (Luckfox Pico Plus / Rockchip RV1106, Raspberry Pi, BeagleBone, Zynq, Buildroot, Yocto, BusyBox) via the embedded-linux-mcp server or SSH.
---

# Embedded Linux Control, Testing & Debugging

## Overview

Use this skill whenever you interact with a network-attached embedded Linux target. Embedded Linux boards differ fundamentally from cloud servers: they often run **BusyBox** userlands, **Dropbear** SSH (frequently without an SFTP subsystem), **uclibc/musl** C libraries, small RAM budgets (64 MB – 512 MB), and raw **SPI NAND/NOR or eMMC flash** that wears out under heavy writes.

## Mandatory Onboarding Workflow

Whenever starting work on a board or IP address:

1. **Check Connection Pool (`device_list` / `device_connect`):**
   - Call `device_list` first. If the target device is not registered or connected, call `device_connect` with `host` (e.g., `"192.168.111.7"`), `username` (default `"root"`), and credentials (e.g., `"luckfox"` on Luckfox Pico boards).
2. **Take a System Snapshot (`system_info` & `read_dmesg`):**
   - Call `system_info` before running custom commands. Note:
     - **Architecture & ABI:** `armv7l`, `aarch64`, `riscv64`
     - **Board Model:** from `/proc/device-tree/model`
     - **Available RAM & `/tmp` capacity:** Check `memory` and `storage` (`tmpfs` size)
   - Call `read_dmesg(lines=50)` to spot boot errors, driver probe failures, or voltage/MMC warnings.

## Flash-Safe Execution Rules

1. **Always Use `/tmp` for Ephemeral Files:**
   - Upload test scripts, cross-compiled binaries, and diagnostic captures to `/tmp/<name>` (`tmpfs` in RAM).
   - Never write temporary artifacts to `/` or `/root` unless persistence across reboots is explicitly requested by the user.
2. **Check Binary Size & Linked Libc Before Uploading:**
   - Before calling `file_transfer(direction="upload")`, verify `free` RAM in `/tmp` via `system_info`.
   - When cross-compiling on the host, match the target's C library (`uclibc`, `musl`, or `glibc`) or link statically (`-static` / `armv7-unknown-linux-musleabihf`) so the binary does not fail with deceptive `sh: ./app: not found` (which indicates a missing dynamic linker in `/lib/ld-*`).
3. **Respect BusyBox Command Dialect:**
   - Avoid GNU-only flags (`ps aux --sort`, `df --output`, `grep -P`, `stat -c`).
   - Prefer POSIX/BusyBox flags or reading `/proc` and `/sys` directly via `file_read`.

## Hardware & Debugging Playbooks

Load the relevant reference document when performing peripheral control or low-level debugging:

- **Luckfox Pico / Rockchip RV1106 Boards:** Read `references/luckfox-rv1106.md` for GPIO pin number calculation (`bank * 32 + group * 8 + pin`), `luckfox-config`, CSI camera (`rkipc`), and RV1106 memory layout.
- **Generic Sysfs, Peripherals & Crash Debugging:** Read `references/sysfs-debug.md` for `/sys/class/gpio`, `/sys/class/pwm`, `i2c-tools`, `spidev`, `gdbserver`, and `strace` workflows.
```

Create `skills/embedded-linux/references/luckfox-rv1106.md`:
```markdown
# Luckfox Pico / Pico Plus / Pico Pro / Max (Rockchip RV1106 / RV1103) Reference

## Default Access & Credentials
- **Default SSH Username:** `root`
- **Default SSH Password:** `luckfox`
- **USB RNDIS Default IP:** `172.32.0.93`
- **Ethernet (Pico Plus / Pro / Max):** DHCP or static IP on local LAN (e.g., `192.168.111.7`)

## SoC & System Characteristics
- **SoC:** Rockchip RV1106G2 / RV1106G3 (Single-core ARM Cortex-A7 @ 1.2GHz + RISC-V MCU + 0.5/1.0 TOPS NPU)
- **RAM:** 64 MB (RV1103), 128 MB (Pico Plus RV1106G2), or 256 MB (Pico Max RV1106G3) — note that a portion is reserved by CMA/MPP/NPU media buffers.
- **Userland:** Buildroot / BusyBox with `uclibc` (`arm-rockchip830-linux-uclibcgnueabihf`) or `glibc` depending on SDK image.
- **Rust Cross-Compile Target:** Use `armv7-unknown-linux-musleabihf` (statically linked) so binaries run regardless of `uclibc` vs `glibc` rootfs.

## GPIO Pin Number Calculation (Sysfs)

Rockchip RV1106 organizes GPIOs into **5 banks** (`GPIO0`..`GPIO4`), **4 groups** per bank (`A`=0, `B`=1, `C`=2, `D`=3), and **8 pins** per group (`0`..`7`).

Formula:
```
pin_num = (bank * 32) + (group * 8) + index
```
Examples:
- `GPIO1_C7` -> `1 * 32 + 2 * 8 + 7 = 55`
- `GPIO1_D0` -> `1 * 32 + 3 * 8 + 0 = 56`
- `GPIO3_A4` -> `3 * 32 + 0 * 8 + 4 = 100`

Inspect currently claimed GPIOs and mux states:
- Read `/sys/kernel/debug/gpio` via `file_read`
- Read `/sys/kernel/debug/pinctrl/pinctrl-rockchip-pinctrl/pinmux-pins` via `file_read`

## Camera, RTSP & NPU Services
- Default SDK images start `rkipc` automatically at boot, which claims the CSI camera and NPU and streams RTSP on `rtsp://<board-ip>/live/0`.
- Before running custom RKNN NPU or V4L2 camera tests, check if `rkipc` is running (`ps | grep rkipc`) and stop it (`killall rkipc`) so it releases `/dev/video*` and `/dev/rknpu`.
```

Create `skills/embedded-linux/references/sysfs-debug.md`:
```markdown
# Embedded Linux Sysfs, Peripherals & Remote Debugging Reference

## 1. GPIO via Sysfs & libgpiod

### Check Kernel GPIO Allocations First
Always read `/sys/kernel/debug/gpio` using `file_read` before exporting a GPIO to ensure it is not already claimed by a kernel driver (e.g. LEDs, I2C, SPI, UART).

### Legacy Sysfs (`/sys/class/gpio`)
```sh
# Export GPIO 55, set as output high
echo 55 > /sys/class/gpio/export
echo out > /sys/class/gpio/gpio55/direction
echo 1 > /sys/class/gpio/gpio55/value
# Release when done
echo 55 > /sys/class/gpio/unexport
```

### Character Device (`libgpiod` / `/dev/gpiochip*`)
If `gpiodetect`, `gpioinfo`, `gpioset`, `gpioget` are installed:
```sh
gpiodetect
gpioinfo gpiochip1
```

## 2. I2C, SPI & PWM Inspection

- **I2C Buses:** List buses via `ls -l /dev/i2c-*` or `/sys/bus/i2c/devices/`. Scan bus `N` safely with `i2cdetect -y N`.
- **PWM Channels:** Inspect `/sys/kernel/debug/pwm` or `/sys/class/pwm/pwmchip*/`. Note that `duty_cycle` must be `<= period` (in nanoseconds) and `period` must be set before enabling.
- **Device Tree Live Inspection:** Walk `/proc/device-tree/` to verify `status` (`"okay"` vs `"disabled"`) of peripheral nodes:
  ```sh
  find /proc/device-tree -name "status" -exec head -v {} +
  ```

## 3. Process Triage & Remote Debugging

- **Missing Shared Library vs Missing Binary:**
  If executing `/tmp/my_app` prints `sh: /tmp/my_app: not found` even though the file exists and is `+x`, the ELF interpreter (`PT_INTERP`) is missing on the board. Run `readelf -l my_app | grep interpreter` on the host or compile statically.
- **Remote `gdbserver`:**
  Start `gdbserver :2345 /tmp/my_app` inside a backgrounded shell or attach to a running PID (`gdbserver --attach :2345 <PID>`), then connect `gdb-multiarch` from the host to `<board-ip>:2345`.
```

- [ ] **Step 2: Create `LICENSE` and `README.md` for open-source distribution**

Create `LICENSE`:
```text
MIT License

Copyright (c) 2026 Embedded Linux Skill Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Create `README.md`:
```markdown
# embedded-linux-skill (`embedded-linux-mcp` + Agent Skill)

An open-source **Rust Model Context Protocol (MCP) Server** and **AI Agent Skill** designed for controlling, testing, and debugging network-connected embedded Linux devices (such as **Luckfox Pico Plus / Rockchip RV1106**, Raspberry Pi, BeagleBone, Zynq, and generic BusyBox/Buildroot/Yocto targets).

## Key Features

- **Pure-Rust Single Binary (`embedded-linux-mcp`):** Uses `russh` for multiplexed, persistent SSH2 sessions with zero C/OpenSSL build dependencies across Windows, Linux, and macOS.
- **BusyBox & Dropbear Native:** Works out-of-the-box on minimal 64MB–256MB RAM embedded targets even when the SFTP subsystem is absent (automatic POSIX shell stream fallback with SHA-256 verification).
- **3-Tier Hardware Safety Guardrails:**
  - **Blocked:** Prevents catastrophic flash/storage overwrites (`rm -rf /`, `mkfs.*`, `flash_erase`, `dd of=/dev/mtdblock*`).
  - **Confirmation Required:** Protects against accidental network disconnection or reboots (`reboot`, `ip link set ... down`, `killall dropbear`) unless explicitly confirmed (`force: true`).
- **Companion Agent Skill (`skills/embedded-linux/`):** Teaches AI agents embedded Linux discovery discipline, RAM-backed `/tmp` execution, sysfs peripheral debugging, and Rockchip RV1106 / Luckfox Pico specifics.

## MCP Tools Provided

| Tool | Description |
| :--- | :--- |
| `device_list` | List configured and runtime-connected embedded targets |
| `device_connect` | Connect to an embedded target over SSH and optionally set it as default |
| `device_disconnect` | Close an active SSH session |
| `exec_command` | Execute a command with timeout, output cap, and safety checks |
| `system_info` | Gather BusyBox-compatible SoC, board model, RAM, storage, and IP summary |
| `read_dmesg` | Read and filter kernel ring buffer (`dmesg`) by level or keyword |
| `file_read` | Safely read `/proc`, `/sys`, text, or binary files from the target |
| `file_transfer` | Upload or download files/binaries with SHA-256 verification and `chmod` support |

## Quickstart

### 1. Build from Source

```bash
cargo build --release
```

The compiled binary will be located at `target/release/embedded-linux-mcp` (or `target\release\embedded-linux-mcp.exe` on Windows).

### 2. Configure Your MCP Client (Antigravity / Claude Code / Cursor)

```json
{
  "mcpServers": {
    "embedded-linux": {
      "command": "/path/to/embedded-linux-mcp",
      "args": [],
      "env": {
        "EMBEDDED_HOST": "192.168.111.7",
        "EMBEDDED_USER": "root",
        "EMBEDDED_PASS": "luckfox",
        "EMBEDDED_NAME": "luckfox-pico-plus"
      }
    }
  }
}
```

Or pass `--config /path/to/devices.toml` using `devices.example.toml` as a template. You can also connect dynamically at runtime without any config file by asking your agent:
> *"Connect to the Luckfox Pico Plus at 192.168.111.7 (user: root, pass: luckfox) and inspect system_info and dmesg."*

### 3. Install the Agent Skill

Copy or symlink `skills/embedded-linux` into your agent's skills directory (e.g. `~/.gemini/config/skills/embedded-linux` or `~/.claude/skills/embedded-linux`).

## License

MIT
```

- [ ] **Step 3: Create live hardware verification test (`tests/live_luckfox.rs`) and run it against `192.168.111.7`**

Create `tests/live_luckfox.rs`:
```rust
use embedded_linux_mcp::config::{AppConfig, DeviceProfile};
use embedded_linux_mcp::safety::{SafetyChecker, SafetyLevel};
use embedded_linux_mcp::ssh::SessionManager;

#[tokio::test]
async fn live_luckfox_pico_plus_end_to_end() {
    if std::env::var("LUCKFOX_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("Skipping live hardware test (set LUCKFOX_LIVE_TEST=1 to run)");
        return;
    }

    let host = std::env::var("LUCKFOX_HOST").unwrap_or_else(|_| "192.168.111.7".to_string());
    let user = std::env::var("LUCKFOX_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("LUCKFOX_PASS").unwrap_or_else(|_| "luckfox".to_string());

    let manager = SessionManager::new(AppConfig::default());
    let summary = manager
        .connect_device(
            DeviceProfile {
                name: "luckfox-pico-plus".into(),
                host: host.clone(),
                port: 22,
                username: user,
                password: Some(pass),
                password_env: None,
                key_path: None,
                description: Some("Live Luckfox Pico Plus".into()),
            },
            true,
        )
        .await
        .expect("Failed to connect to live Luckfox Pico Plus");

    assert!(summary.connected);
    println!("Connected uname: {}", summary.uname);

    let sys = manager
        .system_info(None)
        .await
        .expect("system_info should succeed");
    println!("Board model: {}", sys.board_model);
    println!("Memory:\n{}", sys.memory);
    assert!(!sys.uname.is_empty());

    let dmesg = manager
        .read_dmesg(None, 20, None, None)
        .await
        .expect("read_dmesg should succeed");
    assert!(!dmesg.is_empty());

    let cpuinfo = manager
        .file_read(None, "/proc/cpuinfo", 4096, 0, "utf8")
        .await
        .expect("file_read /proc/cpuinfo should succeed");
    assert!(cpuinfo.bytes_read > 0);

    // Test file upload, execution in /tmp, and cleanup
    let tmp_local = std::env::temp_dir().join("luckfox_mcp_probe.sh");
    std::fs::write(&tmp_local, "#!/bin/sh\necho \"MCP_PROBE_OK:$(uname -m)\"\n").unwrap();

    let up = manager
        .file_transfer(
            None,
            "upload",
            &tmp_local,
            "/tmp/luckfox_mcp_probe.sh",
            Some("0755"),
        )
        .await
        .expect("file_transfer upload should succeed");
    assert!(up.bytes_transferred > 0);

    let exec_out = manager
        .exec(None, "/tmp/luckfox_mcp_probe.sh && rm -f /tmp/luckfox_mcp_probe.sh", None, None, 5000)
        .await
        .expect("exec probe script should succeed");
    assert_eq!(exec_out.exit_code, 0);
    assert!(exec_out.stdout.contains("MCP_PROBE_OK:"));

    let _ = std::fs::remove_file(&tmp_local);

    // Verify safety check blocks unforced reboot
    let safety = SafetyChecker::new(false);
    assert!(matches!(
        safety.check_command("reboot", false),
        SafetyLevel::RequiresConfirmation(_)
    ));

    manager.disconnect_device(None).await.unwrap();
}
```

- [ ] **Step 4: Run unit tests and live hardware verification against `192.168.111.7`**

Run: `cargo test`
Run: `$env:LUCKFOX_LIVE_TEST="1"; cargo test --test live_luckfox -- --nocapture`
Expected: PASS with live connection to `192.168.111.7`, printing `uname`, `board_model`, `memory`, and `MCP_PROBE_OK`.

- [ ] **Step 5: Commit Task 4**

```bash
git add skills/ LICENSE README.md tests/live_luckfox.rs
git commit -m "feat(skill): add embedded-linux skill, docs, and live Luckfox verification test"
```
