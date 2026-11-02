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
            "No active default device configured. Call device_connect with host (e.g. '192.168.1.100') first."
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
            if [
                "emerg", "alert", "crit", "err", "warn", "notice", "info", "debug",
            ]
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
