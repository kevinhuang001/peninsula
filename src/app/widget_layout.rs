/// Widths are based on logical monitor points, so HiDPI scaling is already
/// accounted for. Square-root growth keeps large desktops from producing
/// disproportionately wide minimums; upper and lower scale caps bound it.
pub(super) fn width_range(screen_width: f32) -> [f32; 2] {
    let screen = if screen_width.is_finite() {
        screen_width.max(1.0)
    } else {
        1440.0
    };
    let scale = (screen / 1440.0).sqrt().clamp(0.85, 1.25);
    let available = (screen - 32.0).max(1.0);
    let maximum = (560.0 * scale).round().min(available);
    let minimum = (280.0 * scale).round().min(maximum);
    [minimum, maximum]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_screens_grow_gently_and_have_bounded_minimum_and_maximum_widths() {
        assert_eq!(width_range(1440.0), [280.0, 560.0]);
        let hd = width_range(1920.0);
        let qhd = width_range(2560.0);
        let uhd = width_range(3840.0);
        assert!(hd[0] > 280.0 && hd[1] > 560.0);
        assert!(qhd[0] >= hd[0] && qhd[1] >= hd[1]);
        assert_eq!(uhd, [350.0, 700.0]);
        assert!(uhd[0] < 3840.0 * 0.1);
    }
    #[test]
    fn small_screens_never_overflow_and_invalid_screen_sizes_use_reference_widths() {
        for screen in [320.0, 1024.0, 1440.0, 1920.0, 3840.0] {
            let [minimum, maximum] = width_range(screen);
            assert!(minimum <= maximum && maximum <= screen - 32.0);
        }
        assert_eq!(width_range(f32::NAN), width_range(1440.0));
    }
}
