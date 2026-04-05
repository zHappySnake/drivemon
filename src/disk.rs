use std::{
    collections::{HashMap, VecDeque},
    fs,
    time::Instant,
};

use crate::smart::{SmartAttrRaw, SmartData, query_smart};

pub const HISTORY_LEN: usize = 60;

// ---- Public types ----

#[derive(Debug, Clone)]
pub enum SmartStatus {
    Passed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct SmartAttr {
    pub id: u8,
    pub name: String,
    pub value: u64,
    pub worst: u64,
    pub thresh: u64,
    pub raw_string: String,
}

#[derive(Debug, Clone)]
pub struct PartitionInfo {
    pub device: String,
    pub mount_point: String,
    pub fs_type: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub free_bytes: u64,
}

impl PartitionInfo {
    pub fn usage_ratio(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes as f64 / self.total_bytes as f64
        }
    }
}

#[derive(Debug, Clone)]
pub struct DiskInfo {
    pub name: String,
    pub model: String,
    pub serial: String,
    pub capacity_bytes: u64,
    pub smart_status: SmartStatus,
    /// Celsius
    pub temperature: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub smart_attrs: Vec<SmartAttr>,
    pub partitions: Vec<PartitionInfo>,
    /// Current read speed in bytes/s
    pub read_speed_bps: f64,
    /// Current write speed in bytes/s
    pub write_speed_bps: f64,
    /// Rolling 60-second history (bytes/s per sample)
    pub read_history: VecDeque<f64>,
    pub write_history: VecDeque<f64>,
    /// Message when SMART data is unavailable
    pub smart_note: Option<String>,
}

// ---- Internal I/O accounting ----

#[derive(Default, Clone)]
struct IoSnapshot {
    sectors_read: u64,
    sectors_written: u64,
}

// ---- Collector ----

pub struct DiskCollector {
    prev_io: HashMap<String, (IoSnapshot, Instant)>,
    smart_cache: HashMap<String, Result<SmartData, String>>,
    read_histories: HashMap<String, VecDeque<f64>>,
    write_histories: HashMap<String, VecDeque<f64>>,
}

impl DiskCollector {
    pub fn new() -> Self {
        let mut collector = DiskCollector {
            prev_io: HashMap::new(),
            smart_cache: HashMap::new(),
            read_histories: HashMap::new(),
            write_histories: HashMap::new(),
        };
        collector.refresh_smart();
        collector
    }

    /// Re-query smartctl for every physical drive (can be slow, do sparingly).
    pub fn refresh_smart(&mut self) {
        for name in list_block_devices() {
            let device = format!("/dev/{name}");
            let result = query_smart(&device).map_err(|e| e.to_string());
            self.smart_cache.insert(name, result);
        }
    }

    /// Collect a fresh snapshot: updates I/O speeds, partitions, merges SMART cache.
    pub fn collect(&mut self) -> Vec<DiskInfo> {
        let devices = list_block_devices();
        let io_now = read_diskstats();
        let partitions = read_partitions();
        let now = Instant::now();

        let mut result = Vec::new();

        for name in devices {
            // ---- I/O speed ----
            let (read_bps, write_bps) = match io_now.get(&name) {
                Some(cur) => match self.prev_io.get(&name) {
                    Some((prev, prev_time)) => {
                        let dt = now.duration_since(*prev_time).as_secs_f64();
                        if dt > 0.0 {
                            let dr = cur.sectors_read.saturating_sub(prev.sectors_read) as f64
                                * 512.0
                                / dt;
                            let dw = cur.sectors_written.saturating_sub(prev.sectors_written)
                                as f64
                                * 512.0
                                / dt;
                            (dr, dw)
                        } else {
                            (0.0, 0.0)
                        }
                    }
                    None => (0.0, 0.0),
                },
                None => (0.0, 0.0),
            };

            // Update prev snapshot
            if let Some(snap) = io_now.get(&name) {
                self.prev_io.insert(name.clone(), (snap.clone(), now));
            }

            // ---- Rolling history ----
            let rh = self.read_histories.entry(name.clone()).or_default();
            rh.push_back(read_bps);
            if rh.len() > HISTORY_LEN {
                rh.pop_front();
            }

            let wh = self.write_histories.entry(name.clone()).or_default();
            wh.push_back(write_bps);
            if wh.len() > HISTORY_LEN {
                wh.pop_front();
            }

            // ---- SMART ----
            let (model, serial, capacity, status, temperature, power_on_hours, attrs, note) =
                match self.smart_cache.get(&name) {
                    Some(Ok(s)) => (
                        s.model.clone().unwrap_or_default(),
                        s.serial.clone().unwrap_or_default(),
                        s.capacity_bytes.unwrap_or_else(|| disk_size_bytes(&name)),
                        match s.passed {
                            Some(true) => SmartStatus::Passed,
                            Some(false) => SmartStatus::Failed,
                            None => SmartStatus::Unknown,
                        },
                        s.temperature.map(|t| t as f64),
                        s.power_on_hours,
                        s.attrs.iter().map(raw_to_attr).collect(),
                        None,
                    ),
                    Some(Err(e)) => (
                        String::new(),
                        String::new(),
                        disk_size_bytes(&name),
                        SmartStatus::Unknown,
                        None,
                        None,
                        vec![],
                        Some(e.clone()),
                    ),
                    None => (
                        String::new(),
                        String::new(),
                        disk_size_bytes(&name),
                        SmartStatus::Unknown,
                        None,
                        None,
                        vec![],
                        Some("SMART data not loaded yet".into()),
                    ),
                };

            // ---- Partitions for this disk ----
            let disk_partitions: Vec<PartitionInfo> = partitions
                .iter()
                .filter(|p| is_partition_of(&p.device, &name))
                .cloned()
                .collect();

            result.push(DiskInfo {
                name: name.clone(),
                model,
                serial,
                capacity_bytes: capacity,
                smart_status: status,
                temperature,
                power_on_hours,
                smart_attrs: attrs,
                partitions: disk_partitions,
                read_speed_bps: read_bps,
                write_speed_bps: write_bps,
                read_history: self.read_histories[&name].clone(),
                write_history: self.write_histories[&name].clone(),
                smart_note: note,
            });
        }

        result
    }
}

// ---- Helpers ----

fn raw_to_attr(r: &SmartAttrRaw) -> SmartAttr {
    SmartAttr {
        id: r.id,
        name: r.name.clone(),
        value: r.value,
        worst: r.worst,
        thresh: r.thresh,
        raw_string: r.raw_string.clone(),
    }
}

/// List physical block devices from /sys/block, excluding virtual ones.
pub fn list_block_devices() -> Vec<String> {
    let mut devices: Vec<String> = fs::read_dir("/sys/block")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| is_physical_disk(n))
        .collect();
    devices.sort();
    devices
}

fn is_physical_disk(name: &str) -> bool {
    // Skip loop, ram, zram, device-mapper, optical, floppy, md RAID
    for prefix in &["loop", "ram", "zram", "dm-", "sr", "fd", "md"] {
        if name.starts_with(prefix) {
            return false;
        }
    }
    if name.starts_with("nvme") {
        // nvme0n1  → physical  (no 'p' suffix)
        // nvme0n1p1 → partition (has 'p' before trailing digits)
        if let Some(pos) = name.rfind('p') {
            let after = &name[pos + 1..];
            if !after.is_empty() && after.chars().all(|c| c.is_ascii_digit()) {
                return false;
            }
        }
        return true;
    }
    // sd*, hd*, vd*, xvd* etc: partition if last char is a digit
    !name
        .chars()
        .last()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(true)
}

/// Returns true if `part_device` (e.g. "/dev/sda1") belongs to disk `disk_name` (e.g. "sda").
fn is_partition_of(part_device: &str, disk_name: &str) -> bool {
    let prefix = format!("/dev/{disk_name}");
    if !part_device.starts_with(&prefix) {
        return false;
    }
    let rest = &part_device[prefix.len()..];
    rest.is_empty() || rest.starts_with(|c: char| c.is_ascii_digit() || c == 'p')
}

/// Parse /proc/diskstats into a map of device name → I/O snapshot.
fn read_diskstats() -> HashMap<String, IoSnapshot> {
    let mut map = HashMap::new();
    let Ok(content) = fs::read_to_string("/proc/diskstats") else {
        return map;
    };
    for line in content.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 14 {
            continue;
        }
        let name = parts[2].to_string();
        let sectors_read: u64 = parts[5].parse().unwrap_or(0);
        let sectors_written: u64 = parts[9].parse().unwrap_or(0);
        map.insert(
            name,
            IoSnapshot {
                sectors_read,
                sectors_written,
            },
        );
    }
    map
}

/// Use sysinfo to get mounted partition info.
fn read_partitions() -> Vec<PartitionInfo> {
    use sysinfo::Disks;
    // Bind to a local so the temporary lives long enough for the iterator.
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .map(|d| {
            let total = d.total_space();
            let avail = d.available_space();
            PartitionInfo {
                device: d.name().to_string_lossy().to_string(),
                mount_point: d.mount_point().to_string_lossy().to_string(),
                fs_type: d.file_system().to_string_lossy().to_string(),
                total_bytes: total,
                used_bytes: total.saturating_sub(avail),
                free_bytes: avail,
            }
        })
        .collect()
}

/// Read disk capacity from /sys/block/<name>/size (in 512-byte sectors).
fn disk_size_bytes(name: &str) -> u64 {
    fs::read_to_string(format!("/sys/block/{name}/size"))
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0)
        * 512
}

// ---- Formatting ----

pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes == 0 {
        return "0 B".into();
    }
    let mut val = bytes as f64;
    let mut idx = 0usize;
    while val >= 1024.0 && idx + 1 < UNITS.len() {
        val /= 1024.0;
        idx += 1;
    }
    format!("{val:.1} {}", UNITS[idx])
}

pub fn format_speed(bps: f64) -> String {
    format!("{}/s", format_bytes(bps as u64))
}
