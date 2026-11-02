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
