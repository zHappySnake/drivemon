# drivemon

A `btop`-inspired TUI disk monitor written in Rust with [Ratatui](https://ratatui.rs).

```
┌ 💽 drivemon   Overview   SMART Attrs   I/O History   [↑↓/jk] disk  [Tab] tab  [r] refresh SMART  [q] quit ┐
├──────────────────┬──────────────────────────────────────────────────────────────────────────────────────────┤
│ Drives           │ Drive Info                                                                               │
│ ● /dev/sda 42°C │ /dev/sda  ─  Samsung SSD 870 EVO 1TB                                                   │
│ ○ /dev/sdb       │ ✔ PASSED  🌡 42°C  ⏱ 8432 h (0.96 yr)  💾 1.0 TB                                    │
│                  │ S/N: S3EVNX0K123456                                                                     │
│                  ├── Read ──────────────────────────────────────────────── 125.4 MB/s ──────────────────── │
│                  │ ▁▁▁▁▂▃▄▅▆▇█▇▆▅▄▃▂▁▁▁▁▁▁▁                                                              │
│                  ├── Write ─────────────────────────────────────────────── 45.2 MB/s ───────────────────── │
│                  │ ▁▁▁▂▁▁▁▁▂▃▂▁▁▁▁▁▁▁▁▁▁                                                                  │
│                  ├── Partitions ────────────────────────────────────────────────────────────────────────── │
│                  │ Device  Mount  FS    Usage              %    Capacity                                   │
│                  │ sda1    /boot  vfat  ████████░░░░░░░░░  45%  200.0 GB / 110.0 GB free                  │
│                  │ sda2    /      ext4  ███████████████░░  78%  800.0 GB / 176.0 GB free                  │
└──────────────────┴──────────────────────────────────────────────────────────────────────────────────────────┘
```

## Features

- **Overview tab** – drive info, live read/write speed sparklines, partition usage bars
- **SMART Attrs tab** – full ATA/NVMe SMART attribute table with threshold highlighting
- **I/O History tab** – 60-second read and write sparklines with peak tracking
- Automatic NVMe support (temperature in Kelvin converted automatically)
- Graceful degradation when `smartctl` is unavailable

## Dependencies

### Runtime

| Tool | Purpose |
|------|---------|
| `smartmontools` | SMART health data (`smartctl`) |

Install on Debian/Ubuntu: `sudo apt install smartmontools`  
Install on Arch: `sudo pacman -S smartmontools`  
Install on Fedora: `sudo dnf install smartmontools`  

### SMART permissions

`smartctl` requires elevated privileges to read drive health data. Either:

```bash
# Option A – run drivemon as root
sudo drivemon

# Option B – grant your user access (Linux)
sudo usermod -aG disk $USER   # then log out and back in
sudo setcap cap_sys_rawio+ep $(which smartctl)

# Option C – use a sudoers rule
# Add to /etc/sudoers.d/smartctl:
# %disk ALL=(root) NOPASSWD: /usr/bin/smartctl
```

Without SMART access the app still shows I/O speeds and partition usage — you'll just see a note in the SMART tab.

## Build

```bash
cargo build --release
./target/release/drivemon

# Or run directly
cargo run --release
```

## Releases

Prebuilt binaries are available for Linux on the [GitHub Releases](https://github.com/zHappySnake/drivemon/releases) page.

Supported platforms:
- Linux (x86_64)
- Linux (ARM64)
