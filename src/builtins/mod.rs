mod battery;
pub mod bilibili;
mod clock;
pub mod download;
mod health;
pub mod kuaidi;
mod market;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod music;
pub mod performance;
pub mod pomodoro;
mod rss;
pub mod weather;
use crate::extension::{Action, ExtensionRegistry, RuntimeExtension};
use crate::ui::declarative::ViewNode;
fn compact_value(text: impl Into<String>, monospace: bool) -> ViewNode {
    use crate::ui::declarative::*;
    ViewNode::text(
        text,
        TextStyle {
            font_size: FontSize::Custom(11.0),
            font_weight: FontWeight::Bold,
            align: Align::Start,
            monospace,
            ..Default::default()
        },
    )
}

fn compact_detail(text: impl Into<String>) -> ViewNode {
    use crate::ui::declarative::*;
    ViewNode::text(
        text,
        TextStyle {
            font_size: FontSize::Custom(10.0),
            align: Align::Start,
            ..Default::default()
        },
    )
    .color(ColorSpec::SecondaryText)
}

fn refresh_button(refreshing: bool) -> ViewNode {
    use crate::ui::declarative::{Node, Size};
    let label = if refreshing {
        "刷新中…"
    } else {
        "立即刷新"
    };
    let mut button =
        ViewNode::subtle_button(label, Action::Advance).sized(Size::Fixed(66.0), Size::Fixed(22.0));
    if let Node::Control { enabled, .. } = &mut button.kind {
        *enabled = !refreshing;
    }
    button
}

pub fn runtimes() -> Vec<Box<dyn RuntimeExtension>> {
    vec![
        Box::new(performance::PerformanceExtension::default()),
        Box::new(download::DownloadExtension::default()),
        Box::new(pomodoro::PomodoroExtension::default()),
        Box::new(health::HealthExtension::default()),
        Box::new(rss::RssExtension::default()),
        Box::new(market::MarketExtension::new()),
        Box::new(bilibili::BilibiliExtension::default()),
        Box::new(kuaidi::KuaidiExtension::default()),
        Box::new(weather::WeatherExtension::new(
            weather::WeatherPreferences::default(),
        )),
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        Box::new(music::MusicExtension::default()),
        Box::new(clock::ClockExtension::default()),
        Box::new(battery::BatteryExtension::default()),
    ]
}
pub fn registry() -> ExtensionRegistry {
    let mut r = ExtensionRegistry::default();
    for runtime in runtimes() {
        r.register(runtime.registration_descriptor())
            .expect("valid builtin");
    }
    r
}
