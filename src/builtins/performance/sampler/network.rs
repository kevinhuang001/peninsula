pub(super) fn rate(previous: u64, current: u64, seconds: f64) -> f64 {
    if seconds <= 0.0 || current < previous {
        return 0.0;
    }
    (current - previous) as f64 / seconds
}

#[cfg(test)]
mod tests {
    use super::rate;

    #[test]
    fn calculates_bytes_per_second_from_counter_delta() {
        assert_eq!(rate(100, 600, 2.0), 250.0);
    }

    #[test]
    fn handles_counter_reset_and_invalid_elapsed_time() {
        assert_eq!(rate(600, 10, 1.0), 0.0);
        assert_eq!(rate(10, 20, 0.0), 0.0);
    }
}
