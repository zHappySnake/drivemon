use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::process::Command;

// ---- Public types ----

#[derive(Debug, Clone, Default)]
pub struct SmartData {
    pub model: Option<String>,
    pub serial: Option<String>,
    pub capacity_bytes: Option<u64>,
    pub passed: Option<bool>,
    /// Celsius
    pub temperature: Option<i64>,
    pub power_on_hours: Option<u64>,
    pub attrs: Vec<SmartAttrRaw>,
}

#[derive(Debug, Clone)]
pub struct SmartAttrRaw {
    pub id: u8,
    pub name: String,
    pub value: u64,
    pub worst: u64,
    pub thresh: u64,
    pub raw_string: String,
}

// ---- serde structs for smartctl -j output ─────----────────────────────────────────

#[derive(Deserialize, Default)]
struct SmartctlJson {
    model_name: Option<String>,
    model_family: Option<String>,
    serial_number: Option<String>,
    user_capacity: Option<UserCapacity>,
    smart_status: Option<SmartStatusJson>,
    temperature: Option<TemperatureJson>,
    power_on_time: Option<PowerOnTime>,
    ata_smart_attributes: Option<AtaAttributes>,
    nvme_smart_health_information_log: Option<NvmeLog>,
}

#[derive(Deserialize)]
struct UserCapacity {
    bytes: Option<u64>,
}

#[derive(Deserialize)]
struct SmartStatusJson {
    passed: bool,
}

#[derive(Deserialize)]
struct TemperatureJson {
    current: Option<i64>,
}

#[derive(Deserialize)]
struct PowerOnTime {
    hours: Option<u64>,
}

#[derive(Deserialize)]
struct AtaAttributes {
    table: Option<Vec<AtaAttribute>>,
}

#[derive(Deserialize)]
struct AtaAttribute {
    id: Option<u8>,
    name: Option<String>,
    value: Option<u64>,
    worst: Option<u64>,
    thresh: Option<u64>,
    raw: Option<AtaRaw>,
}

#[derive(Deserialize)]
struct AtaRaw {
    value: Option<u64>,
    string: Option<String>,
}

#[derive(Deserialize)]
struct NvmeLog {
    temperature: Option<i64>,
    power_on_hours: Option<u64>,
    media_errors: Option<u64>,
    available_spare: Option<u64>,
    percentage_used: Option<u64>,
}

// ---- Query ----

/// Run `smartctl -j -a <device>` and parse the JSON output.
/// smartctl often exits with non-zero on partial errors but still emits JSON.
pub fn query_smart(device: &str) -> Result<SmartData> {
    let output = Command::new("smartctl")
        .args(["-j", "-a", device])
        .output()
        .map_err(|e| anyhow!("Cannot run smartctl: {e}. Install smartmontools."))?;

    let text = String::from_utf8_lossy(&output.stdout);
    if text.trim().is_empty() {
        return Err(anyhow!(
            "smartctl returned no output for {device}. Try running as root."
        ));
    }

    let json: SmartctlJson =
        serde_json::from_str(&text).map_err(|e| anyhow!("JSON parse error: {e}"))?;

    let mut data = SmartData {
        model: json.model_name.or(json.model_family),
        serial: json.serial_number,
        capacity_bytes: json.user_capacity.and_then(|c| c.bytes),
        passed: json.smart_status.map(|s| s.passed),
        temperature: json.temperature.and_then(|t| t.current),
        power_on_hours: json.power_on_time.and_then(|p| p.hours),
        ..SmartData::default()
    };

    // NVMe health log (supplements or replaces ATA fields)
    if let Some(nvme) = json.nvme_smart_health_information_log {
        if data.temperature.is_none() {
            // NVMe reports temperature in Kelvin
            data.temperature = nvme.temperature.map(|k| k - 273);
        }
        if data.power_on_hours.is_none() {
            data.power_on_hours = nvme.power_on_hours;
        }
        if let Some(v) = nvme.media_errors {
            data.attrs.push(SmartAttrRaw {
                id: 0,
                name: "Media Errors".into(),
                value: v,
                worst: 0,
                thresh: 0,
                raw_string: v.to_string(),
            });
        }
        if let Some(v) = nvme.available_spare {
            data.attrs.push(SmartAttrRaw {
                id: 0,
                name: "Available Spare".into(),
                value: v,
                worst: 0,
                thresh: 0,
                raw_string: format!("{v}%"),
            });
        }
        if let Some(v) = nvme.percentage_used {
            data.attrs.push(SmartAttrRaw {
                id: 0,
                name: "Percentage Used".into(),
                value: v,
                worst: 0,
                thresh: 0,
                raw_string: format!("{v}%"),
            });
        }
    }

    // ATA SMART attributes
    if let Some(ata) = json.ata_smart_attributes
        && let Some(table) = ata.table
    {
        for attr in table {
            let raw_str = attr
                .raw
                .as_ref()
                .and_then(|r| r.string.clone())
                .or_else(|| {
                    attr.raw
                        .as_ref()
                        .and_then(|r| r.value)
                        .map(|v| v.to_string())
                })
                .unwrap_or_default();

            data.attrs.push(SmartAttrRaw {
                id: attr.id.unwrap_or(0),
                name: attr.name.unwrap_or_default().replace('_', " "),
                value: attr.value.unwrap_or(0),
                worst: attr.worst.unwrap_or(0),
                thresh: attr.thresh.unwrap_or(0),
                raw_string: raw_str,
            });
        }
    }

    Ok(data)
}
