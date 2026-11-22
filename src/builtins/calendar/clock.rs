use crate::extension::{
    CompactView, ContributionDescriptor, ContributionKind, ExtensionId,
};
use crate::ui::declarative::{ColorSpec, CompactLayout, ViewNode};

pub(super) const CLOCK_ID: &str = "builtin.calendar/time";

pub(super) fn clock_descriptor() -> ContributionDescriptor {
    ContributionDescriptor::builtin(
        CLOCK_ID,
        ContributionKind::Compact,
        "时间",
        "clock",
        false,
        6,
    )
    .with_size_hint(64.0, 28.0)
}


pub(super) fn compact_view(twelve_hour: bool, show_date: bool) -> CompactView {
        let time =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        CompactView {
            id: ExtensionId::builtin(CLOCK_ID),
            source_extension_id: ExtensionId::builtin(CLOCK_ID),
            content: ViewNode::compact(
                ViewNode::svg(clock_svg(time.hour(), time.minute()), 18.0)
                    .color(ColorSpec::Warning),
                super::super::compact_value(
                    format!(
                        "{:02}:{:02}{}{}",
                        if twelve_hour {
                            (time.hour() + 11) % 12 + 1
                        } else {
                            time.hour()
                        },
                        time.minute(),
                        if twelve_hour {
                            if time.hour() >= 12 {
                                " PM"
                            } else {
                                " AM"
                            }
                        } else {
                            ""
                        },
                        if show_date {
                            format!("  {:02}-{:02}", u8::from(time.month()), time.day())
                        } else {
                            String::new()
                        }
                    ),
                    true,
                ),
                CompactLayout {
                    leading_size: [18.0, 20.0],
                    gap: 4.0,
                    ..Default::default()
                },
            ),
            visibility: Default::default(),
            min_width: 64.0,
            preferred_width: 64.0,
            fit_width: true,
            priority: 0,
            tooltip: None,
        }
    }
fn clock_angles(hour: u8, minute: u8) -> [f32; 2] {
    let turn = std::f32::consts::TAU;
    let noon = -std::f32::consts::FRAC_PI_2;
    [
        noon + turn * (f32::from(hour % 12) / 12.0 + f32::from(minute) / 720.0),
        noon + turn * (f32::from(minute) / 60.0),
    ]
}

fn clock_svg(hour: u8, minute: u8) -> String {
    let angles = clock_angles(hour, minute);
    let mut svg = String::from("inline:<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-linecap=\"round\"><circle cx=\"12\" cy=\"12\" r=\"10\" stroke-width=\"1.3\"/>");
    let hands = [(angles[0], 4.8, 1.8), (angles[1], 7.3, 1.3)];
    for (angle, length, width) in hands {
        svg.push_str(&format!(
            "<path d=\"M12 12 L{:.2} {:.2}\" stroke-width=\"{width}\"/>",
            12.0 + angle.cos() * length,
            12.0 + angle.sin() * length
        ));
    }
    svg.push_str("</svg>");
    svg
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn analog_hands_follow_extension_time() {
        assert!(clock_angles(3, 0)[0].abs() < 0.00001);
        let svg = clock_svg(12, 30);
        assert!(svg.starts_with("inline:<svg"));
        assert_eq!(svg.matches("<path ").count(), 2);
    }
}
