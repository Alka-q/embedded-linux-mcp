# embedded-linux-mcp

An open-source **Rust Model Context Protocol (MCP) Server** and **AI Agent Skill** designed for controlling, testing, and debugging network-connected embedded Linux devices (ARM, ARM64, RISC-V, MIPS SBCs, Buildroot, Yocto, OpenWrt, and BusyBox/Dropbear targets).

## Key Features

- **Pure-Rust Single Binary (`embedded-linux-mcp`):** Uses `russh` for multiplexed, persistent SSH2 sessions with zero C/OpenSSL build dependencies across Windows, Linux, and macOS.
- **BusyBox & Dropbear Native:** Works out-of-the-box on minimal 64MB–512MB RAM embedded targets even when the SFTP subsystem is absent (automatic POSIX shell stream fallback with SHA-256 verification).
- **3-Tier Hardware Safety Guardrails:**
  - **Blocked:** Prevents catastrophic flash/storage overwrites (`rm -rf /`, `rm -rf /*`, `mkfs.*`, `flash_erase`, `dd of=/dev/mtdblock*`).
  - **Confirmation Required:** Protects against accidental network disconnection or reboots (`reboot`, `ip link set ... down`, `killall dropbear`) unless explicitly confirmed (`force: true`).
- **Companion Agent Skill (`skills/embedded-linux/`):** Teaches AI agents embedded Linux discovery discipline, RAM-backed `/tmp` execution, sysfs peripheral debugging, and SoC GPIO/pinmux inspection.

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
git clone https://github.com/Alka-q/embedded-linux-mcp.git
cd embedded-linux-mcp
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
        "EMBEDDED_HOST": "192.168.1.100",
        "EMBEDDED_USER": "root",
        "EMBEDDED_PASS": "password",
        "EMBEDDED_NAME": "device_name"
      }
    }
  }
}
```

Or pass `--config /path/to/devices.toml` using `devices.example.toml` as a template. You can also connect dynamically at runtime without any config file by asking your agent:
> *"Connect to the embedded device at 192.168.1.100 (user: root, pass: password) and inspect system_info and dmesg."*

### 3. Install the Agent Skill

Copy or symlink `skills/embedded-linux` into your agent's skills directory (e.g. `~/.gemini/config/skills/embedded-linux` or `~/.claude/skills/embedded-linux`).

## License

MIT
