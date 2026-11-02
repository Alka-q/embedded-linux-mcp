# Luckfox Pico / Pico Plus / Pico Pro / Max (Rockchip RV1106 / RV1103) Reference

## Default Access & Credentials
- **Default SSH Username:** `root`
- **Default SSH Password:** `luckfox`
- **USB RNDIS Default IP:** `172.32.0.93`
- **Ethernet (Pico Plus / Pro / Max):** DHCP or static IP on local LAN (e.g., `192.168.111.7`)

## SoC & System Characteristics
- **SoC:** Rockchip RV1106G2 / RV1106G3 (Single-core ARM Cortex-A7 @ 1.2GHz + RISC-V MCU + 0.5/1.0 TOPS NPU)
- **RAM:** 64 MB (RV1103), 128 MB (Pico Plus RV1106G2), or 256 MB (Pico Max RV1106G3) — note that a portion is reserved by CMA/MPP/NPU media buffers.
- **Userland:** Buildroot / BusyBox with `uclibc` (`arm-rockchip830-linux-uclibcgnueabihf`) or `glibc` depending on SDK image.
- **Rust Cross-Compile Target:** Use `armv7-unknown-linux-musleabihf` (statically linked) so binaries run regardless of `uclibc` vs `glibc` rootfs.

## GPIO Pin Number Calculation (Sysfs)

Rockchip RV1106 organizes GPIOs into **5 banks** (`GPIO0`..`GPIO4`), **4 groups** per bank (`A`=0, `B`=1, `C`=2, `D`=3), and **8 pins** per group (`0`..`7`).

Formula:
```
pin_num = (bank * 32) + (group * 8) + index
```
Examples:
- `GPIO1_C7` -> `1 * 32 + 2 * 8 + 7 = 55`
- `GPIO1_D0` -> `1 * 32 + 3 * 8 + 0 = 56`
- `GPIO3_A4` -> `3 * 32 + 0 * 8 + 4 = 100`

Inspect currently claimed GPIOs and mux states:
- Read `/sys/kernel/debug/gpio` via `file_read`
- Read `/sys/kernel/debug/pinctrl/pinctrl-rockchip-pinctrl/pinmux-pins` via `file_read`

## Camera, RTSP & NPU Services
- Default SDK images start `rkipc` automatically at boot, which claims the CSI camera and NPU and streams RTSP on `rtsp://<board-ip>/live/0`.
- Before running custom RKNN NPU or V4L2 camera tests, check if `rkipc` is running (`ps | grep rkipc`) and stop it (`killall rkipc`) so it releases `/dev/video*` and `/dev/rknpu`.
