#[cfg(target_os = "macos")]
use regex::Regex;

#[cfg(target_os = "macos")]
pub(super) fn parse_gpu_utilization(text: &str) -> Option<f32> {
    let re = Regex::new(r#""Device Utilization %"\s*=\s*(\d+)"#).ok()?;
    let value = re.captures(text)?.get(1)?.as_str().parse::<f32>().ok()?;
    (0.0..=100.0).contains(&value).then_some(value)
}

pub(super) fn gpu_utilization() -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        let output = crate::core::external::process::command("ioreg")
            .args(["-r", "-c", "IOAccelerator", "-d", "1"])
            .output()
            .ok()?;
        return parse_gpu_utilization(&String::from_utf8_lossy(&output.stdout));
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(cards) = std::fs::read_dir("/sys/class/drm") {
            for card in cards
                .flatten()
                .filter(|entry| entry.file_name().to_string_lossy().starts_with("card"))
            {
                let path = card.path().join("device/gpu_busy_percent");
                if let Ok(value) = std::fs::read_to_string(path) {
                    if let Ok(value) = value.trim().parse::<f32>() {
                        return (0.0..=100.0).contains(&value).then_some(value);
                    }
                }
            }
        }
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        let output = crate::core::external::process::command("nvidia-smi")
            .args([
                "--query-gpu=utilization.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output();
        if let Ok(output) = output {
            if let Some(value) = String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .and_then(parse_percent)
            {
                return Some(value);
            }
        }
        let script = "$s=Get-Counter '\\GPU Engine(*)\\Utilization Percentage' -ErrorAction Stop; ($s.CounterSamples | Where-Object {$_.InstanceName -match 'engtype_3D'} | Measure-Object -Property CookedValue -Maximum).Maximum";
        let output = crate::core::external::process::command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .ok()?;
        return String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|v| (0.0..=100.0).contains(v));
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(target_os = "windows")]
pub(super) fn windows_battery_status() -> Option<(Option<u8>, bool)> {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    let mut status = SYSTEM_POWER_STATUS::default();
    // Windows fills the structure on success.
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    decode_windows_battery_status(
        status.ACLineStatus,
        status.BatteryFlag,
        status.BatteryLifePercent,
    )
}

#[cfg(any(target_os = "windows", test))]
fn decode_windows_battery_status(
    ac_line: u8,
    battery_flags: u8,
    percent: u8,
) -> Option<(Option<u8>, bool)> {
    const AC_ONLINE: u8 = 1;
    const CHARGING: u8 = 8;
    const NO_BATTERY: u8 = 128;
    const UNKNOWN: u8 = 255;

    let flags = (battery_flags != UNKNOWN)
        .then_some(battery_flags)
        .unwrap_or_default();
    if flags & NO_BATTERY != 0 {
        return Some((None, false));
    }
    // Let the per-battery sampler handle an unavailable aggregate percentage.
    let percent = (percent <= 100).then_some(percent)?;
    Some((Some(percent), ac_line == AC_ONLINE || flags & CHARGING != 0))
}

#[cfg(any(target_os = "windows", test))]
fn parse_percent(text: &str) -> Option<f32> {
    let value = text.trim().parse::<f32>().ok()?;
    (0.0..=100.0).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::parse_gpu_utilization;

    #[test]
    fn windows_reports_charging_or_external_power_without_marking_desktops() {
        let decode = super::decode_windows_battery_status;
        assert_eq!(decode(1, 0, 60), Some((Some(60), true)));
        assert_eq!(decode(1, 1, 100), Some((Some(100), true)));
        assert_eq!(decode(255, 8 | 1, 92), Some((Some(92), true)));
        assert_eq!(decode(0, 0, 60), Some((Some(60), false)));
        assert_eq!(decode(0, 255, 60), Some((Some(60), false)));
        assert_eq!(decode(1, 128, 255), Some((None, false)));
        assert_eq!(decode(255, 255, 255), None);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn parses_macos_accelerator_statistics() {
        let sample =
            r#""PerformanceStatistics" = {"Device Utilization %"=24,"Renderer Utilization %"=21}"#;
        assert_eq!(parse_gpu_utilization(sample), Some(24.0));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn rejects_missing_or_out_of_range_gpu_values() {
        assert_eq!(parse_gpu_utilization("no accelerator"), None);
        assert_eq!(parse_gpu_utilization(r#""Device Utilization %"=140"#), None);
    }

    #[test]
    fn parses_gpu_percentage_from_windows_driver_output() {
        assert_eq!(super::parse_percent("37\r\n"), Some(37.0));
        assert_eq!(super::parse_percent("N/A"), None);
        assert_eq!(super::parse_percent("101"), None);
    }
}
