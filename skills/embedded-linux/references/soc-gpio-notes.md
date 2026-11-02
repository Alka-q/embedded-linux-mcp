# Embedded SoC Families, Cross-Compilation & GPIO Bank Calculation

## Cross-Compilation & Static Linking
Constrained Buildroot / OpenWrt / Yocto boards often use `uclibc` or `musl` instead of `glibc`.
- **Rust static targets:**
  - 32-bit ARMv7 hard-float: `armv7-unknown-linux-musleabihf`
  - 64-bit ARM64: `aarch64-unknown-linux-musl`
  - 64-bit RISC-V: `riscv64gc-unknown-linux-musl`
- **C/C++ static flag:** Pass `-static` when the target rootfs lacks matching shared libraries.

## GPIO Bank & Group Pin Number Calculation (Sysfs)

Many embedded SoCs organize GPIOs into **banks** (`GPIO0`..`GPION`), **groups/ports** (`A`=0, `B`=1, `C`=2, `D`=3), and **pin indices** (`0`..`7` or `0`..`31`).

### 32-Pin Bank with 4x8 Groups (e.g., Rockchip)
```
pin_num = (bank * 32) + (group * 8) + index
```
Examples:
- `GPIO1_C7` -> `1 * 32 + 2 * 8 + 7 = 55`
- `GPIO1_D0` -> `1 * 32 + 3 * 8 + 0 = 56`
- `GPIO3_A4` -> `3 * 32 + 0 * 8 + 4 = 100`

### 32-Pin Port Bank (e.g., Allwinner / NXP i.MX)
```
pin_num = (bank * 32) + pin
```

### Inspecting Claimed GPIOs & Pinmux States
- Read `/sys/kernel/debug/gpio` via `file_read`
- Read `/sys/kernel/debug/pinctrl/*/pinmux-pins` via `file_read`

## Hardware Media / Camera / NPU Daemon Contention
Vendor SDK images frequently start background media or RTSP daemons at boot that claim `/dev/video*` (CSI/V4L2) or hardware accelerators (NPU/VPU).
- Before running custom V4L2 camera captures or NPU inference tests, inspect running processes (`ps`) and stop any vendor camera/streaming daemon holding the device node.
