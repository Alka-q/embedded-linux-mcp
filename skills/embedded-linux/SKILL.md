---
name: embedded-linux
description: Use when controlling, testing, inspecting, or debugging network-connected embedded Linux boards (ARM, ARM64, RISC-V, Buildroot, Yocto, OpenWrt, BusyBox/Dropbear) via the embedded-linux-mcp server or SSH.
---

# Embedded Linux Control, Testing & Debugging

## Overview

Use this skill whenever you interact with a network-attached embedded Linux target. Embedded Linux boards differ fundamentally from cloud servers: they often run **BusyBox** userlands, **Dropbear** SSH (frequently without an SFTP subsystem), **uclibc/musl** C libraries, small RAM budgets (64 MB – 512 MB), and raw **SPI NAND/NOR or eMMC flash** that wears out under heavy writes.

## Mandatory Onboarding Workflow

Whenever starting work on a board or IP address:

1. **Check Connection Pool (`device_list` / `device_connect`):**
   - Call `device_list` first. If the target device is not registered or connected, call `device_connect` with `host` (e.g., `"192.168.1.100"`), `username` (default `"root"`), and the board's credentials.
2. **Take a System Snapshot (`system_info` & `read_dmesg`):**
   - Call `system_info` before running custom commands. Note:
     - **Architecture & ABI:** `armv7l`, `aarch64`, `riscv64`, `mips`
     - **Board Model:** from `/proc/device-tree/model`
     - **Available RAM & `/tmp` capacity:** Check `memory` and `storage` (`tmpfs` size)
   - Call `read_dmesg(lines=50)` to spot boot errors, driver probe failures, or voltage/MMC warnings.

## Flash-Safe Execution Rules

1. **Always Use `/tmp` for Ephemeral Files:**
   - Upload test scripts, cross-compiled binaries, and diagnostic captures to `/tmp/<name>` (`tmpfs` in RAM).
   - Never write temporary artifacts to `/` or `/root` unless persistence across reboots is explicitly requested by the user.
2. **Check Binary Size & Linked Libc Before Uploading:**
   - Before calling `file_transfer(direction="upload")`, verify `free` RAM in `/tmp` via `system_info`.
   - When cross-compiling on the host, match the target's C library (`uclibc`, `musl`, or `glibc`) or link statically (`-static` / `*-unknown-linux-musl*`) so the binary does not fail with deceptive `sh: ./app: not found` (which indicates a missing dynamic linker in `/lib/ld-*`).
3. **Respect BusyBox Command Dialect:**
   - Avoid GNU-only flags (`ps aux --sort`, `df --output`, `grep -P`, `stat -c`).
   - Prefer POSIX/BusyBox flags or reading `/proc` and `/sys` directly via `file_read`.

## Hardware & Debugging Playbooks

Load the relevant reference document when performing peripheral control or low-level debugging:

- **SoC Families, Cross-Compilation & GPIO Pin Math:** Read `references/soc-gpio-notes.md` for GPIO bank/group pin number calculation, static cross-compilation targets, and media/camera service contention.
- **Generic Sysfs, Peripherals & Crash Debugging:** Read `references/sysfs-debug.md` for `/sys/class/gpio`, `/sys/class/pwm`, `i2c-tools`, `spidev`, `gdbserver`, and `strace` workflows.
