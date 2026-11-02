use embedded_linux_mcp::config::{AppConfig, DeviceProfile};
use embedded_linux_mcp::safety::{SafetyChecker, SafetyLevel};
use embedded_linux_mcp::ssh::SessionManager;

#[tokio::test]
async fn live_embedded_device_end_to_end() {
    if std::env::var("EMBEDDED_LIVE_TEST").ok().as_deref() != Some("1") {
        eprintln!("Skipping live hardware test (set EMBEDDED_LIVE_TEST=1 to run)");
        return;
    }

    let host = std::env::var("EMBEDDED_HOST").unwrap_or_else(|_| "192.168.1.100".to_string());
    let user = std::env::var("EMBEDDED_USER").unwrap_or_else(|_| "root".to_string());
    let pass = std::env::var("EMBEDDED_PASS").unwrap_or_else(|_| "password".to_string());
    let name = std::env::var("EMBEDDED_NAME").unwrap_or_else(|_| "device_name".to_string());

    let manager = SessionManager::new(AppConfig::default());
    let summary = manager
        .connect_device(
            DeviceProfile {
                name,
                host: host.clone(),
                port: 22,
                username: user,
                password: Some(pass),
                password_env: None,
                key_path: None,
                description: Some("Live Embedded Linux Target".into()),
            },
            true,
        )
        .await
        .expect("Failed to connect to live embedded target");

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

    let tmp_local = std::env::temp_dir().join("embedded_mcp_probe.sh");
    std::fs::write(&tmp_local, "#!/bin/sh\necho \"MCP_PROBE_OK:$(uname -m)\"\n").unwrap();

    let up = manager
        .file_transfer(
            None,
            "upload",
            &tmp_local,
            "/tmp/embedded_mcp_probe.sh",
            Some("0755"),
        )
        .await
        .expect("file_transfer upload should succeed");
    assert!(up.bytes_transferred > 0);

    let exec_out = manager
        .exec(
            None,
            "/tmp/embedded_mcp_probe.sh && rm -f /tmp/embedded_mcp_probe.sh",
            None,
            None,
            5000,
        )
        .await
        .expect("exec probe script should succeed");
    assert_eq!(exec_out.exit_code, 0);
    assert!(exec_out.stdout.contains("MCP_PROBE_OK:"));

    let _ = std::fs::remove_file(&tmp_local);

    let safety = SafetyChecker::new(false);
    assert!(matches!(
        safety.check_command("reboot", false),
        SafetyLevel::RequiresConfirmation(_)
    ));

    manager.disconnect_device(None).await.unwrap();
}
