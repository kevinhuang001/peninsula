use super::*;

pub(super) fn surface(ui: &egui::Ui) -> egui::Frame {
    let fill = ui.visuals().widgets.noninteractive.bg_fill;
    let border = crate::ui::palette::BORDER;
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0_f32, border))
        .corner_radius(10.0)
        .inner_margin(egui::Margin::same(14))
}

pub(super) fn form_surface(ui: &egui::Ui) -> egui::Frame {
    surface(ui).inner_margin(egui::Margin::symmetric(12, 6))
}

pub(super) fn accent(ui: &egui::Ui) -> Color32 {
    crate::ui::palette::accent_tokens(ui.ctx()).accent
}

pub(super) fn panel(ui: &egui::Ui) -> egui::Frame {
    surface(ui).inner_margin(egui::Margin::symmetric(14, 12))
}

pub(super) fn import_badge(ui: &mut egui::Ui, icon: Option<&egui::TextureHandle>) {
    let color = accent(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
    ui.painter().rect_filled(
        rect,
        11.0,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 38),
    );
    if let Some(icon) = icon {
        ui.painter().image(
            icon.id(),
            rect.shrink(2.0),
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "↓",
            Typography::font_id(ui.ctx(), 20.0),
            color,
        );
    }
}

pub(super) fn method_pill(ui: &mut egui::Ui, method: &str) {
    let font = egui::FontId::monospace(10.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(method.to_owned(), font.clone(), Color32::WHITE)
        .size()
        .x;
    let (rect, _) =
        ui.allocate_exact_size(Vec2::new(text_width + 14.0, 18.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 5.0, accent(ui));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        method,
        font,
        Color32::WHITE,
    );
}

pub(super) fn section_title(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(RichText::new(title).text_style(egui::TextStyle::Heading).strong());
    if !subtitle.is_empty() {
        ui.label(
            RichText::new(subtitle)
                .text_style(egui::TextStyle::Small)
                .color(ui.visuals().weak_text_color()),
        );
    }
    ui.add_space(6.0);
}

pub(super) fn switch(ui: &mut egui::Ui, value: &mut bool, label: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), egui::Sense::click());
        if response.clicked() {
            *value = !*value;
            changed = true;
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
        });
        ui.painter().rect_filled(
            rect,
            10.0,
            if *value {
                accent(ui)
            } else {
                ui.visuals().widgets.inactive.bg_fill
            },
        );
        ui.painter().circle_filled(
            egui::pos2(
                if *value {
                    rect.right() - 10.0
                } else {
                    rect.left() + 10.0
                },
                rect.center().y,
            ),
            7.0,
            Color32::WHITE,
        );
        ui.label(label);
    });
    changed
}

pub(super) fn switch_row(ui: &mut egui::Ui, value: &mut bool, label: &str) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(1.0), 28.0),
        egui::Sense::click(),
    );
    if response.clicked() {
        *value = !*value;
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    ui.painter().text(
        rect.left_center(),
        egui::Align2::LEFT_CENTER,
        label,
        Typography::font_id(ui.ctx(), 12.0),
        ui.visuals().text_color(),
    );
    let track = egui::Rect::from_center_size(
        rect.right_center() - Vec2::new(18.0, 0.0),
        Vec2::new(36.0, 20.0),
    );
    ui.painter().rect_filled(
        track,
        10.0,
        if *value {
            accent(ui)
        } else {
            ui.visuals().widgets.inactive.bg_fill
        },
    );
    ui.painter().circle_filled(
        egui::pos2(
            if *value {
                track.right() - 10.0
            } else {
                track.left() + 10.0
            },
            track.center().y,
        ),
        7.0,
        Color32::WHITE,
    );
    response.clicked()
}

/// Keep the setting label and its control in reserved, non-overlapping
/// regions. The surrounding frame fills the source detail pane instead of
/// shrinking to a 490-point column.
pub(super) fn runtime_row(
    ui: &mut egui::Ui,
    label: &str,
    control: impl FnOnce(&mut egui::Ui),
) -> egui::Rect {
    form_surface(ui).show(ui, |ui| {
        let width = ui.available_width().max(1.0);
        ui.set_width(width);
        let control_width = 160.0;
        let gap = ui.spacing().item_spacing.x;
        if width < 330.0 {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).strong());
                ui.allocate_ui_with_layout(
                    Vec2::new(width, 28.0),
                    Layout::right_to_left(Align::Center),
                    control,
                );
            });
        } else {
            ui.horizontal(|ui| {
                let title_width = (width - control_width - gap).max(1.0);
                ui.add_sized(
                    [title_width, 28.0],
                    egui::Label::new(egui::RichText::new(label).strong()).truncate(),
                ).on_hover_text(label);
                ui.allocate_ui_with_layout(
                    Vec2::new(control_width, 28.0),
                    Layout::right_to_left(Align::Center),
                    control,
                );
            });
        }
    }).response.rect
}

pub(super) fn info_card(ui: &mut egui::Ui, label: &str, value: &str, value_size: f32) -> egui::Rect {
    form_surface(ui).show(ui, |ui| {
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width().max(1.0), 28.0),
            egui::Sense::hover(),
        );
        ui.painter().text(
            rect.left_center(),
            egui::Align2::LEFT_CENTER,
            label,
            Typography::font_id(ui.ctx(), 12.0),
            ui.visuals().weak_text_color(),
        );
        ui.painter().text(
            rect.right_center(),
            egui::Align2::RIGHT_CENTER,
            value,
            Typography::font_id(ui.ctx(), value_size),
            ui.visuals().text_color(),
        );
    }).response.rect
}

