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
