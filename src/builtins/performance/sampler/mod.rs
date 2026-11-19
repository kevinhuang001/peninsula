mod icons;
mod metrics;
mod network;
mod platform;

#[cfg(test)]
pub(super) use metrics::ApplicationSnapshot;
pub use metrics::{MetricsSampler, Snapshot};

pub fn format_speed(bytes_per_second: f64) -> String {
    if bytes_per_second >= 1_000_000.0 {
        let value = bytes_per_second / 1_000_000.0;
        format!("{:.1} MB/s", (value * 10.0).round() / 10.0)
    } else if bytes_per_second >= 1_000.0 {
        format!("{:.0} KB/s", bytes_per_second / 1_000.0)
    } else {
        format!("{:.0} B/s", bytes_per_second.max(0.0))
    }
}

/// Driver queries share the owning extension's stop/join lifecycle.
pub(super) fn start_gpu(interval: std::time::Duration) -> crate::core::worker::Worker<Option<f32>> {
    crate::core::worker::Worker::spawn_latest("gpu", interval, platform::gpu_utilization)
}

#[cfg(test)]
mod tests {
    use super::format_speed;

    #[test]
    fn speed_uses_readable_si_units() {
        assert_eq!(format_speed(800.0), "800 B/s");
        assert_eq!(format_speed(1_500.0), "2 KB/s");
        assert_eq!(format_speed(1_260_000.0), "1.3 MB/s");
    }

    #[test]
    fn speed_does_not_display_negative_rates() {
        assert_eq!(format_speed(-1.0), "0 B/s");
    }
}
