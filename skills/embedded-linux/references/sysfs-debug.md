# Embedded Linux Sysfs, Peripherals & Remote Debugging Reference

## 1. GPIO via Sysfs & libgpiod

### Check Kernel GPIO Allocations First
Always read `/sys/kernel/debug/gpio` using `file_read` before exporting a GPIO to ensure it is not already claimed by a kernel driver (e.g. LEDs, I2C, SPI, UART).

### Legacy Sysfs (`/sys/class/gpio`)
```sh
# Export GPIO 55, set as output high
echo 55 > /sys/class/gpio/export
echo out > /sys/class/gpio/gpio55/direction
echo 1 > /sys/class/gpio/gpio55/value
# Release when done
echo 55 > /sys/class/gpio/unexport
```

### Character Device (`libgpiod` / `/dev/gpiochip*`)
If `gpiodetect`, `gpioinfo`, `gpioset`, `gpioget` are installed:
```sh
gpiodetect
gpioinfo gpiochip1
```

## 2. I2C, SPI & PWM Inspection

- **I2C Buses:** List buses via `ls -l /dev/i2c-*` or `/sys/bus/i2c/devices/`. Scan bus `N` safely with `i2cdetect -y N`.
- **PWM Channels:** Inspect `/sys/kernel/debug/pwm` or `/sys/class/pwm/pwmchip*/`. Note that `duty_cycle` must be `<= period` (in nanoseconds) and `period` must be set before enabling.
- **Device Tree Live Inspection:** Walk `/proc/device-tree/` to verify `status` (`"okay"` vs `"disabled"`) of peripheral nodes:
  ```sh
  find /proc/device-tree -name "status" -exec head -v {} +
  ```

## 3. Process Triage & Remote Debugging

- **Missing Shared Library vs Missing Binary:**
  If executing `/tmp/my_app` prints `sh: /tmp/my_app: not found` even though the file exists and is `+x`, the ELF interpreter (`PT_INTERP`) is missing on the board. Run `readelf -l my_app | grep interpreter` on the host or compile statically.
- **Remote `gdbserver`:**
  Start `gdbserver :2345 /tmp/my_app` inside a backgrounded shell or attach to a running PID (`gdbserver --attach :2345 <PID>`), then connect `gdb-multiarch` from the host to `<board-ip>:2345`.
