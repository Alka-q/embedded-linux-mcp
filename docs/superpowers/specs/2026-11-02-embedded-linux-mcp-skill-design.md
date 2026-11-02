# Embedded Linux MCP Server & Agent Skill — Design Specification

- **Date:** 2026-11-02
- **Status:** Draft (Pending User Review)
- **Target Repository:** `embedded-linux-skill` (Open-Source Hybrid Rust MCP Server + Agent Skill)

---

## 1. Overview & Goals

This project provides an open-source, hybrid **Model Context Protocol (MCP) Server** (written in Rust) and an **AI Agent Skill** (`skills/embedded-linux/SKILL.md`) for controlling, testing, and debugging network-connected embedded Linux devices (such as Rockchip RV1106-based Luckfox Pico Plus boards, Raspberry Pi, BeagleBone, Zynq, i.MX, and generic BusyBox/Buildroot/Yocto targets).

### Primary Goals (v1)
1. **Zero-Dependency Single Binary MCP Server:** Built in pure Rust using asynchronous SSH (`russh` / `russh-sftp`) so users on Windows, Linux, and macOS can compile or install a single binary without C toolchain or OpenSSL headaches.
2. **Persistent Multiplexed SSH Pool:** Maintain persistent connections to one or more embedded targets, opening lightweight channels per command with automatic reconnection if a board reboots or drops link temporarily.
3. **BusyBox & Dropbear Compatibility:** First-class support for constrained embedded environments (64MB–256MB RAM) running BusyBox and Dropbear SSH, including automatic fallback from SFTP to raw shell streaming (`cat`/`dd` + checksum verification) when the target lacks an SFTP subsystem.
4. **Safety Guardrails:** Prevent AI agents from accidentally bricking flash partitions (`mtdblock`, `mmcblk`) or severing network/SSH connectivity without explicit human confirmation.
5. **Embedded Engineering Skill (`SKILL.md`):** Guide AI agents through systematic discovery, RAM-backed `/tmp` workspace discipline, `/sys` & `/proc` hardware inspection, and board-specific quirks (including a dedicated reference for **Luckfox Pico Plus / Rockchip RV1106**).

---

## 2. Repository Structure

```text
embedded-linux-skill/
├── Cargo.toml                        # Rust package manifest (binary: embedded-linux-mcp)
├── LICENSE                           # MIT License
├── README.md                         # Open-source documentation, quickstart, MCP config examples
├── devices.example.toml              # Example multi-device profile configuration
├── docs/
│   └── superpowers/
│       └── specs/
│           └── 2026-11-02-embedded-linux-mcp-skill-design.md
├── src/
│   ├── main.rs                       # CLI args (--config, --allow-destructive) & stdio JSON-RPC server loop
│   ├── config.rs                     # TOML file & environment variable configuration loader
│   ├── safety.rs                     # 3-tier command and path safety classifier
│   ├── mcp/
│   │   ├── mod.rs                    # JSON-RPC 2.0 request/response types & stdio transport
│   │   └── tools.rs                  # MCP tool schemas (inputSchema) and dispatch handlers
│   └── ssh/
│       ├── mod.rs
│       ├── session.rs                # Single-device russh connection, channel exec, timeout & output cap
│       ├── manager.rs                # Thread-safe multi-device session pool & auto-reconnect logic
│       └── transfer.rs               # Hybrid file reader/writer (SFTP primary + POSIX shell stream fallback)
└── skills/
    └── embedded-linux/
        ├── SKILL.md                  # Core workflow instructions for AI agents
        └── references/
            ├── luckfox-rv1106.md     # Luckfox Pico Plus (RV1106) pinout/GPIO math, rkipc, BusyBox notes
            └── sysfs-debug.md        # Linux sysfs GPIO/PWM/I2C/SPI, dmesg, gdbserver, strace patterns
```

---

## 3. Architecture & Data Flow

```mermaid
flowchart LR
    Agent["AI Agent\n(Antigravity / Claude / Cursor)"] <-->|"JSON-RPC 2.0 (stdio)"| MCP["MCP Handler\n(src/mcp/tools.rs)"]
    MCP --> Safety["Safety Classifier\n(src/safety.rs)"]
    Safety -->|"Allowed / Confirmed"| Pool["SessionManager\n(src/ssh/manager.rs)"]
    Safety -->|"Blocked / Needs Confirm"| MCP
    Pool <-->|"Persistent SSH2\n(russh channels)"| Board["Embedded Linux Target\n(e.g. Luckfox Pico Plus\n192.168.111.7)"]
```

### Component Responsibilities

1. **Configuration Loader (`src/config.rs`):**
   - Loads optional device profiles from `--config <path>` (or `./devices.toml` if present).
   - Reads environment variables (`EMBEDDED_HOST`, `EMBEDDED_PORT`, `EMBEDDED_USER`, `EMBEDDED_PASS`, `EMBEDDED_KEY`) and registers a `"default"` profile automatically if `EMBEDDED_HOST` is set.
   - Allows password values in `devices.toml` to reference environment variables via `password_env = "LUCKFOX_SSH_PASSWORD"`.

2. **Safety Classifier (`src/safety.rs`):**
   - Evaluates every command string and target file path before execution:
     - **`SafetyLevel::Blocked`:** Commands matching catastrophic patterns (`rm -rf /`, `mkfs`, `flash_erase`, `dd` writing to `/dev/mtd*`, `/dev/mmcblk*`, `/dev/sda*`, `/dev/nvme*`). Rejected unconditionally unless the MCP server was started with `--allow-destructive`.
     - **`SafetyLevel::RequiresConfirmation`:** Commands that disrupt connectivity or reboot the target (`reboot`, `poweroff`, `halt`, `shutdown`, `ip link set .* down`, `ifconfig .* down`, `killall dropbear`, `killall sshd`, `systemctl stop ssh*`). Rejected when `force == false` with an actionable warning instructing the agent to ask the user and retry with `force: true`.
     - **`SafetyLevel::Allowed`:** Standard diagnostic, compilation, `/tmp` execution, and `/sys` or `/proc` interactions.

3. **Session Manager & SSH Client (`src/ssh/manager.rs`, `src/ssh/session.rs`):**
   - Stores device profiles and active `russh::client::Handle` instances inside an `Arc<tokio::sync::RwLock<...>>`.
   - Supports authentication via:
     1. Password (`auth_password`)
     2. Keyboard-interactive fallback (if required by PAM/Dropbear)
     3. Private key file (`auth_publickey` supporting Ed25519/RSA)
   - **Auto-Reconnect:** Before opening a channel on an existing session, checks connectivity (or catches channel creation failure), transparently re-establishes the TCP/SSH handshake once, and runs the requested operation.
   - **Timeout & Output Cap:** Every command runs under `tokio::time::timeout(Duration::from_millis(timeout_ms))` (default `15000` ms, max `300000` ms). Standard output and error streams are capped at `65536` bytes (64 KB) per stream to prevent runaway processes from exhausting host memory or LLM context windows.

4. **Hybrid File Transfer (`src/ssh/transfer.rs`):**
   - **`file_read`:** Reads up to `max_bytes` (default `65536`) starting at `offset` from `remote_path`. Uses `dd if=<path> bs=1 skip=<offset> count=<max_bytes> 2>/dev/null | base64` (or SFTP when available) so both virtual kernel files (`/proc/*`, `/sys/*` where file size is reported as 0 or 4096) and binary files work reliably.
   - **`file_transfer` (`upload` / `download`):**
     - First attempts `russh-sftp` subsystem if available.
     - Falls back automatically to streaming over an SSH exec channel (`cat > 'remote_path'` for upload, `cat 'remote_path'` for download), sets permissions (`chmod <mode>`) if requested, and verifies transfer integrity using `sha256sum` or `md5sum` (both standard in BusyBox).

---

## 4. MCP Tools Specification

All tools accept an optional `device` string parameter. When omitted, the server targets the active `default_device` (or the sole connected device).

| Tool Name | Purpose | Parameters (`*` = required) | Output Fields |
| :--- | :--- | :--- | :--- |
| `device_list` | List configured and runtime-added devices and their connection states | *(none)* | `default_device`, `devices: [{ name, host, port, username, connected, description }]` |
| `device_connect` | Register/update a device profile, establish SSH connection, and optionally set as default | `*host`, `name` (defaults to `host`), `port` (`22`), `username` (`"root"`), `password`, `key_path`, `set_default` (`true`) | `name`, `host`, `connected`, `server_banner`, `uname` |
| `device_disconnect` | Close active SSH session for a device | `device` | `device`, `disconnected` |
| `exec_command` | Execute a shell command on the target device | `*command`, `device`, `cwd`, `env` (map), `timeout_ms` (`15000`), `force` (`false`) | `exit_code`, `stdout`, `stderr`, `duration_ms`, `timed_out`, `truncated` |
| `system_info` | Gather BusyBox-compatible hardware, OS, memory, storage, and network summary | `device` | `uname`, `cpu_model`, `board_model`, `uptime`, `memory`, `storage`, `network_interfaces` |
| `read_dmesg` | Read and filter kernel ring buffer (`dmesg`) | `device`, `lines` (`100`), `level` (`err`\|`warn`\|`info`), `grep` | `entries`, `line_count`, `truncated` |
| `file_read` | Safely read text or binary/sysfs files from target | `*remote_path`, `device`, `max_bytes` (`65536`), `offset` (`0`), `encoding` (`"utf8"`\|`"base64"`) | `remote_path`, `content`, `encoding`, `bytes_read`, `truncated` |
| `file_transfer` | Upload or download files between host and target with checksum verification | `*direction` (`"upload"`\|`"download"`), `*local_path`, `*remote_path`, `device`, `mode` (e.g. `"0755"`) | `direction`, `local_path`, `remote_path`, `bytes_transferred`, `method` (`"sftp"`\|`"shell_stream"`), `checksum` |

---

## 5. Agent Skill Specification (`skills/embedded-linux/SKILL.md`)

The companion skill provides structured operational knowledge for AI agents:

1. **Target Onboarding Sequence:**
   - Check `device_list`; if target isn't connected, call `device_connect`.
   - Run `system_info` and `read_dmesg(lines=50)` to identify SoC architecture (`armv7l`, `aarch64`, `riscv64`), libc (`uclibc`, `musl`, `glibc`), init system (`BusyBox init`, `OpenRC`, `systemd`), and mounted filesystems.
2. **Flash-Safe Execution Rules:**
   - Always upload test binaries, scripts, and diagnostic captures to `/tmp` (`tmpfs` in RAM) rather than root flash (`/root` or `/`) to avoid wearing out SPI NAND/NOR or filling small rootfs partitions.
   - Verify free RAM/tmpfs via `system_info` before uploading binaries larger than 5 MB.
3. **Reference Guides:**
   - `references/luckfox-rv1106.md`: Default credentials (`root` / `luckfox`), Rockchip RV1106G2/G3 architecture (ARM Cortex-A7 + RISC-V MCU + NPU), GPIO number calculation (`32 * bank + group * 8 + pin`), `luckfox-config` usage, camera (`rkipc` / `v4l2-ctl`) inspection, and SPI NAND partition layout.
   - `references/sysfs-debug.md`: Inspecting `/sys/kernel/debug/gpio`, `/sys/class/pwm`, `i2cdetect`/`i2cdump`, remote `gdbserver` attachment workflow, and `strace` / `/proc/<pid>/maps` crash triage.

---

## 6. Testing & Verification Strategy

1. **Unit Tests (`cargo test`):**
   - `safety::tests`: Verify `Blocked`, `RequiresConfirmation`, and `Allowed` classifications across destructive, network-dropping, and normal embedded commands.
   - `config::tests`: Verify TOML parsing, environment variable overrides, and password env indirection.
   - `mcp::tests`: Verify JSON-RPC 2.0 `initialize`, `tools/list`, and `tools/call` serialization and error formatting.
2. **Live Hardware Integration Test (`192.168.111.7` — Luckfox Pico Plus):**
   - Connect to `192.168.111.7` using `device_connect`.
   - Retrieve `system_info` and `read_dmesg`.
   - Test `file_read` on `/proc/cpuinfo` and `/proc/device-tree/model`.
   - Test `file_transfer` (`upload` a test script/file to `/tmp`, execute via `exec_command`, `download` result, verify checksum, and clean up).
   - Verify `safety` guard blocks `reboot` when `force: false`.
