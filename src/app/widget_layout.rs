/// Widths are based on logical monitor points, so HiDPI scaling is already
/// accounted for. Square-root growth keeps large desktops from producing
/// disproportionately wide minimums; upper and lower scale caps bound it.
pub(super) fn width_range(screen_width: f32, minimum_scale: Option<f32>) -> [f32; 2] {
    let screen = if screen_width.is_finite() {
        screen_width.max(1.0)
    } else {
        1440.0
    };
    let scale = (screen / 1440.0).sqrt().clamp(0.85, 1.25);
    let available = (screen - 32.0).max(1.0);
    let minimum_scale = minimum_scale
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(1.0)
        .clamp(0.5, 1.0);
    let maximum = (560.0 * scale).round().min(available);
    let minimum = (280.0 * scale * minimum_scale).round().min(maximum);
    [minimum, maximum]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn large_screens_grow_gently_and_have_bounded_minimum_and_maximum_widths() {
        assert_eq!(width_range(1440.0, None), [280.0, 560.0]);
        let hd = width_range(1920.0, None);
        let qhd = width_range(2560.0, None);
        let uhd = width_range(3840.0, None);
        assert!(hd[0] > 280.0 && hd[1] > 560.0);
        assert!(qhd[0] >= hd[0] && qhd[1] >= hd[1]);
        assert_eq!(uhd, [350.0, 700.0]);
        assert!(uhd[0] < 3840.0 * 0.1);
    }
    #[test]
    fn narrower_widgets_share_the_same_screen_cap_and_small_screens_never_overflow() {
        for screen in [320.0, 1024.0, 1440.0, 1920.0, 3840.0] {
            let standard = width_range(screen, None);
            let narrow = width_range(screen, Some(0.85));
            assert!(narrow[0] <= standard[0]);
            assert_eq!(narrow[1], standard[1]);
            assert!(narrow[0] <= narrow[1] && narrow[1] <= screen - 32.0);
        }
        assert_eq!(
            width_range(1440.0, Some(f32::NAN)),
            width_range(1440.0, None)
        );
    }
}
