use crate::{
    app::icons::Icons,
    extension::{
        Action, AlertInteraction, ExtensionId, NotificationAction, NotificationAlertView,
        NotificationKey,
    },
    ui::{declarative::*, renderer},
};
use eframe::egui::{self, vec2, Rect};

const ALERT_PADDING: egui::Vec2 = vec2(18.0, 12.0);
const MIN_ALERT_HEIGHT: f32 = 76.0;

pub struct AlertSurface {
    source: ExtensionId,
    view: WidgetView,
    actions: Vec<NotificationAction>,
}

impl AlertSurface {
    pub fn new(key: &NotificationKey, data: NotificationAlertView, max_actions: usize) -> Self {
        let actions: Vec<_> = data.actions.into_iter().take(max_actions.min(4)).collect();
        let fill = |node: ViewNode| node.sized(Size::Fill(1.0), Size::Auto);
        let title = fill(ViewNode::text(
            data.title,
            TextStyle {
                align: Align::Start,
                font_size: FontSize::Title,
                ..Default::default()
            },
        ));
        let close = ViewNode::subtle_button("×", Action::Dismiss)
            .sized(Size::Fixed(24.0), Size::Fixed(23.0));
        let mut header = Vec::new();
        if let Some(icon) = data.icon {
            header.push(ViewNode::icon(icon, 16.0).color(ColorSpec::Info));
        }
        header.extend([title, close]);
        let body = fill(ViewNode::text(
            data.body,
            TextStyle {
                align: Align::Start,
                font_size: FontSize::Body,
                wrap: true,
                ..Default::default()
            },
        ))
        .color(ColorSpec::SecondaryText);
        let mut buttons: Vec<_> = actions
            .iter()
            .enumerate()
            .map(|(i, action)| button(&action.label, Action::Custom(format!("run:{i}"))))
            .collect();
        let confirm = button("确定", Action::Custom("acknowledge".into()));
        buttons.push(if actions.is_empty() {
            confirm.sized(Size::Fixed(80.0), Size::Fixed(26.0))
        } else {
            confirm
        });
        let mut root = ViewNode::column(
            vec![
                fill(ViewNode::row(
                    header,
                    Flow {
                        gap: Some(7.0),
                        ..Default::default()
                    },
                )),
                body,
                fill(ViewNode::row(
                    buttons,
                    Flow {
                        gap: Some(5.0),
                        justify: Justify::End,
                        ..Default::default()
                    },
                )),
            ],
            Flow {
                gap: Some(6.0),
                justify: Justify::SpaceBetween,
                ..Default::default()
            },
        );
        root.key = Some(key.stable_id());
        Self {
            source: key.source_extension_id.clone(),
            view: WidgetView::new(root, [crate::app::capsule::ALERT_WIDTH, MIN_ALERT_HEIGHT]),
            actions,
        }
    }

    pub fn extra_height(&self, context: &egui::Context, icons: &Icons) -> f32 {
        let width = self.view.preferred_size[0] - 2.0 * ALERT_PADDING.x;
        let content = renderer::content_height(context, &self.view.root, width, icons);
        (content + 2.0 * ALERT_PADDING.y - crate::app::capsule::EXPANDED_BASE_HEIGHT)
            .ceil()
            .max(MIN_ALERT_HEIGHT)
    }

    pub fn show(&self, ui: &mut egui::Ui, rect: Rect, icons: &Icons) -> Option<AlertInteraction> {
        let rect = rect.shrink2(ALERT_PADDING);
        let invocation = renderer::show_widget(ui, rect, &self.source, &self.view, icons, 1.0)?;
        match invocation.action {
            Action::Dismiss => Some(AlertInteraction::Close),
            Action::Custom(id) if id == "acknowledge" => Some(AlertInteraction::Acknowledge),
            Action::Custom(id) => {
                let index = id.strip_prefix("run:")?.parse::<usize>().ok()?;
                self.actions
                    .get(index)
                    .cloned()
                    .map(AlertInteraction::RunAction)
            }
            _ => None,
        }
    }
}

fn button(label: &str, action: Action) -> ViewNode {
    ViewNode::button(label, action).sized(Size::Fill(1.0), Size::Fixed(26.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, Event, Modifiers, PointerButton, RawInput};
    use std::cell::Cell;

    #[test]
    fn alert_buttons_preserve_close_acknowledge_and_extension_action_semantics() {
        let ctx = egui::Context::default();
        let icons = Icons::load(&ctx);
        let key = NotificationKey {
            source_extension_id: ExtensionId::builtin("builtin.downloads/notifications"),
            notification_id: "download:12".into(),
        };
        let action = NotificationAction {
            id: "reveal".into(),
            label: "显示所在位置".into(),
            data: Some("/path/to/download".into()),
        };
        let surface = AlertSurface::new(
            &key,
            NotificationAlertView {
                title: "下载完成".into(),
                body: "report.pdf".into(),
                icon: Some("download".into()),
                actions: vec![action.clone()],
            },
            4,
        );
        let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(300.0, 124.0));
        let result = Cell::new(None);
        let draw = |events| {
            let _ = ctx.run(
                RawInput {
                    events,
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default()
                        .show(ctx, |ui| result.set(surface.show(ui, rect, &icons)));
                },
            );
            result.take()
        };
        let click = |x, y| {
            let pos = rect.min + vec2(x, y);
            draw(vec![Event::PointerMoved(pos)]);
            let button = |pressed| Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            draw(vec![button(true)]);
            draw(vec![button(false)])
        };
        assert_eq!(click(270.0, 23.0), Some(AlertInteraction::Close));
        assert_eq!(click(215.0, 100.0), Some(AlertInteraction::Acknowledge));
        assert_eq!(
            click(90.0, 100.0),
            Some(AlertInteraction::RunAction(action))
        );
    }
}
