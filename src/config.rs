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
