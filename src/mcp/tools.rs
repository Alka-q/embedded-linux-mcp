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
            "description": "Connect to a network-attached embedded Linux device over SSH (e.g. 192.168.1.100) and register it in the session pool.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "host": { "type": "string", "description": "IP address or hostname of the target device (e.g. '192.168.1.100')" },
                    "name": { "type": "string", "description": "Optional alias for the device (defaults to host)" },
                    "port": { "type": "integer", "description": "SSH port (default: 22)", "default": 22 },
                    "username": { "type": "string", "description": "SSH username (default: 'root')", "default": "root" },
                    "password": { "type": "string", "description": "SSH password" },
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
            let default_device = devices
                .iter()
                .find(|d| d.is_default)
                .map(|d| d.name.clone());
            Ok(json!({
                "default_device": default_device,
                "devices": devices
            }))
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
            let clamped_lines = lines.clamp(1, 2000);
            let truncated = line_count >= clamped_lines;
            Ok(json!({
                "line_count": line_count,
                "truncated": truncated,
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
