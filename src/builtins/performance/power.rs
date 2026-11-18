//! Power sampling stays independent of GPU/driver queries and process scans.
use crate::core::latest::{self, LatestReceiver};
use std::{
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct PowerSnapshot {
    pub percent: Option<u8>,
    pub external: Option<bool>,
    pub charging: bool,
}

pub(super) fn start() -> LatestReceiver<PowerSnapshot> {
    let (tx, rx) = latest::channel();
    thread::spawn(move || loop {
        let started = Instant::now();
        if !tx.send(sample()) {
            break;
        }
        thread::sleep(Duration::from_millis(250).saturating_sub(started.elapsed()));
    });
    rx
}

fn sample() -> PowerSnapshot {
    #[cfg(feature = "demo-power")]
    if let Some(demo) = demo_input() {
        return demo;
    }
    #[cfg(target_os = "windows")]
    if let Some(snapshot) = super::sampler::power_status() {
        return snapshot;
    }

    #[cfg(target_os = "linux")]
    let external = linux_external_power(std::path::Path::new("/sys/class/power_supply"));
    #[cfg(target_os = "macos")]
    let external = mac_external_power();
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let external = None;

    let mut result = PowerSnapshot {
        external,
        ..Default::default()
    };
    let Ok(manager) = battery::Manager::new() else {
        return result;
    };
    let Ok(batteries) = manager.batteries() else {
        return result;
    };
    let mut energy = 0.0;
    let mut full = 0.0;
    let mut fallback = None;
    for battery in batteries.flatten() {
        let ratio = battery.state_of_charge().value;
        fallback = ratio_to_percent(ratio).or(fallback);
        let capacity = battery.energy_full().value;
        if capacity.is_finite() && capacity > 0.0 && ratio_to_percent(ratio).is_some() {
            full += capacity;
            energy += capacity * ratio.clamp(0.0, 1.0);
        }
        result.charging |= battery.state() == battery::State::Charging;
    }
    result.percent = if full > 0.0 {
        Some((energy / full * 100.0).round() as u8)
    } else {
        fallback
    };
    // Explicit disconnection wins over a stale per-battery charging flag.
    result.charging &= result.external != Some(false);
    if result.charging && result.external.is_none() {
        result.external = Some(true);
    }
    result
}

fn ratio_to_percent(ratio: f32) -> Option<u8> {
    (ratio.is_finite() && (0.0..=1.0).contains(&ratio)).then(|| (ratio * 100.0).round() as u8)
}

#[cfg(any(target_os = "linux", test))]
fn linux_external_power(root: &std::path::Path) -> Option<bool> {
    let mut known = false;
    let mut online = false;
    for entry in std::fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        let Ok(kind) = std::fs::read_to_string(path.join("type")) else {
            continue;
        };
        if !matches!(
            kind.trim(),
            "Mains" | "USB" | "USB_C" | "USB_PD" | "USB_DCP" | "USB_CDP" | "Wireless"
        ) {
            continue;
        }
        match std::fs::read_to_string(path.join("online"))
            .ok()
            .as_deref()
            .map(str::trim)
        {
            Some("0") => known = true,
            Some("1") => {
                known = true;
                online = true;
            }
            _ => {}
        }
    }
    known.then_some(online)
}

#[cfg(target_os = "macos")]
fn mac_external_power() -> Option<bool> {
    let output = crate::core::external::process::command("pmset")
        .args(["-g", "batt"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    decode_mac_power(&String::from_utf8_lossy(&output.stdout))
}
#[cfg(any(target_os = "macos", test))]
fn decode_mac_power(text: &str) -> Option<bool> {
    // Only the source line is authoritative. Battery detail may say "not charging".
    let source = text
        .lines()
        .find(|line| line.contains("Now drawing from"))?;
    if source.contains("AC Power") {
        Some(true)
    } else if source.contains("Battery Power") {
        Some(false)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adapter_detection_distinguishes_unknown_offline_and_multiple_supplies() {
        let root = tempfile::tempdir().unwrap();
        let make = |name: &str, kind: &str, online: &str| {
            let path = root.path().join(name);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("type"), kind).unwrap();
            std::fs::write(path.join("online"), online).unwrap();
        };
        assert_eq!(linux_external_power(root.path()), None);
        make("BAT0", "Battery", "1");
        assert_eq!(linux_external_power(root.path()), None);
        make("AC", "Mains", "0");
        assert_eq!(linux_external_power(root.path()), Some(false));
        make("USB", "USB_PD", "1");
        assert_eq!(linux_external_power(root.path()), Some(true));
        make("USB", "USB_PD", "0");
        assert_eq!(linux_external_power(root.path()), Some(false));
    }
    #[test]
    fn mac_uses_power_source_even_when_charge_limit_stops_charging() {
        assert_eq!(
            decode_mac_power("Now drawing from 'AC Power'\n80%; not charging"),
            Some(true)
        );
        assert_eq!(
            decode_mac_power("Now drawing from 'Battery Power'\n79%; discharging"),
            Some(false)
        );
        assert_eq!(decode_mac_power("unavailable"), None);
    }
}

// Recording-only input. Normal builds cannot read simulated power values.
#[cfg(feature = "demo-power")]
fn demo_input() -> Option<PowerSnapshot> {
    #[derive(serde::Deserialize)]
    struct Input {
        percent: Option<u8>,
        external: Option<bool>,
        charging: bool,
    }
    let path = std::env::var_os("PENINSULA_DEMO_POWER_FILE")?;
    let input: Input = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    Some(PowerSnapshot {
        percent: input.percent.filter(|p| *p <= 100),
        external: input.external,
        charging: input.charging && input.external != Some(false),
    })
}

#[cfg(test)]
mod ratio_tests {
    use super::ratio_to_percent;
    #[test]
    fn converts_valid_battery_ratios_to_percent() {
        assert_eq!(ratio_to_percent(0.584), Some(58));
        assert_eq!(ratio_to_percent(1.0), Some(100));
    }
    #[test]
    fn rejects_unavailable_or_invalid_battery_ratios() {
        assert_eq!(ratio_to_percent(-0.1), None);
        assert_eq!(ratio_to_percent(1.1), None);
        assert_eq!(ratio_to_percent(f32::NAN), None);
    }
}
