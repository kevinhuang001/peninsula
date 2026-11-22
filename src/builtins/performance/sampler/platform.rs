#[cfg(target_os = "macos")]
use regex::Regex;

#[cfg(target_os = "macos")]
pub(super) fn parse_gpu_utilization(text: &str) -> Option<f32> {
    let re = Regex::new(r#""Device Utilization %"\s*=\s*(\d+)"#).ok()?;
    let value = re.captures(text)?.get(1)?.as_str().parse::<f32>().ok()?;
    (0.0..=100.0).contains(&value).then_some(value)
}

pub(super) fn gpu_utilization(cancel: &crate::sdk::tasks::CancellationToken) -> Option<f32> {
    if cancel.is_cancelled() {
        return None;
    }
    #[cfg(target_os = "macos")]
    {
        let output = crate::sdk::process::ProcessExecutor
            .run_with(
                crate::sdk::process::ProcessSpec::new("ioreg")
                    .args(["-r", "-c", "IOAccelerator", "-d", "1"])
                    .timeout(std::time::Duration::from_secs(2)),
                cancel,
                |_, _| {},
            )
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
        let output = crate::sdk::process::ProcessExecutor.run_with(
            crate::sdk::process::ProcessSpec::new("nvidia-smi")
                .args([
                    "--query-gpu=utilization.gpu",
                    "--format=csv,noheader,nounits",
                ])
                .timeout(std::time::Duration::from_secs(3)),
            cancel,
            |_, _| {},
        );
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
        let output = crate::sdk::process::ProcessExecutor
            .run_with(
                crate::sdk::process::ProcessSpec::new("powershell")
                    .args(["-NoProfile", "-NonInteractive", "-Command", script])
                    .timeout(std::time::Duration::from_secs(5)),
                cancel,
                |_, _| {},
            )
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
