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
pub(super) fn windows_battery_status() -> Option<crate::builtins::performance::power::PowerSnapshot>
{
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;
    Some(decode_windows_battery_status(
        status.ACLineStatus,
        status.BatteryFlag,
        status.BatteryLifePercent,
    ))
}

#[cfg(any(target_os = "windows", test))]
fn decode_windows_battery_status(
    ac_line: u8,
    flags: u8,
    percent: u8,
) -> crate::builtins::performance::power::PowerSnapshot {
    use crate::builtins::performance::power::PowerSnapshot;
    let external = match ac_line {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    };
    if flags != 255 && flags & 128 != 0 {
        return PowerSnapshot {
            external,
            ..Default::default()
        };
    }
    PowerSnapshot {
        percent: (percent <= 100).then_some(percent),
        external,
        charging: flags != 255 && flags & 8 != 0 && external != Some(false),
    }
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
    fn windows_distinguishes_charging_full_charge_limit_and_unknown_data() {
        use crate::builtins::performance::power::PowerSnapshot as Power;
        let decode = super::decode_windows_battery_status;
        assert_eq!(
            decode(1, 8, 60),
            Power {
                percent: Some(60),
                external: Some(true),
                charging: true
            }
        );
        for percent in [80, 100] {
            assert_eq!(
                decode(1, 1, percent),
                Power {
                    percent: Some(percent),
                    external: Some(true),
                    charging: false
                }
            );
        }
        assert!(!decode(0, 8, 60).charging);
        assert!(!decode(1, 255, 60).charging);
        assert_eq!(decode(1, 0, 255).external, Some(true));
        assert_eq!(decode(1, 0, 255).percent, None);
        assert_eq!(decode(255, 255, 255), Power::default());
        assert_eq!(decode(1, 128, 255).percent, None);
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
