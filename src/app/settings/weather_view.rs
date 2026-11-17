use super::{
    theme::{ROW, SECONDARY_TEXT, SWITCH_OFF, SWITCH_ON, TEXT},
    CompactPreferences,
};
use crate::builtins::weather::{search_cities, CityResult, WeatherLocation, MAX_REFRESH_MINUTES};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Vec2};
use std::sync::mpsc::{self, Receiver, TryRecvError};

const AUTO_ROW_TOP: f32 = 0.0;
const ALERTS_ROW_TOP: f32 = 34.0;
const INTERVAL_ROW_TOP: f32 = 68.0;
const LOCATION_TOP: f32 = 104.0;
const SEARCH_LABEL_TOP: f32 = 130.0;
const SEARCH_FIELD_TOP: f32 = 146.0;
const RESULTS_TOP: f32 = 180.0;
const RESULT_STEP: f32 = 27.0;
const MAX_RESULTS: usize = 3;
pub(super) const CONTENT_HEIGHT: f32 = RESULTS_TOP + RESULT_STEP * MAX_RESULTS as f32;

#[derive(Debug, Default)]
pub(super) struct WeatherSearch {
    query: String,
    receiver: Option<Receiver<Result<Vec<CityResult>, String>>>,
    results: Vec<CityResult>,
    error: Option<String>,
    searched: bool,
}

impl WeatherSearch {
    fn poll(&mut self) {
        let mut outcome = None;
        if let Some(receiver) = &self.receiver {
            match receiver.try_recv() {
                Ok(result) => outcome = Some(result),
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    outcome = Some(Err("城市搜索服务意外结束".into()));
                }
            }
        }
        let Some(outcome) = outcome else {
            return;
        };
        self.receiver = None;
        self.searched = true;
        match outcome {
            Ok(results) => {
                self.results = results;
                self.error = None;
            }
            Err(error) => {
                self.results.clear();
                self.error = Some(error);
            }
        }
    }

    fn start(&mut self) {
        if self.receiver.is_some() {
            return;
        }
        let query = self.query.trim().to_owned();
        if query.is_empty() {
            return;
        }
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.error = None;
        self.results.clear();
        self.searched = false;
        std::thread::spawn(move || {
            let _ = sender.send(search_cities(&query));
        });
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    panel: Rect,
    preferences: &mut CompactPreferences,
    search: &mut WeatherSearch,
) -> bool {
    search.poll();
    ui.visuals_mut().extreme_bg_color = ROW;
    ui.visuals_mut().text_cursor.stroke.color = TEXT;
    let mut changed = false;

    let auto_row = row_rect(panel, AUTO_ROW_TOP);
    draw_row(ui, auto_row);
    ui.painter().text(
        Pos2::new(auto_row.left() + 11.0, auto_row.center().y),
        Align2::LEFT_CENTER,
        "自动定位",
        FontId::proportional(11.0),
        TEXT,
    );
    if toggle(
        ui,
        auto_row,
        "weather-auto-locate",
        preferences.weather.auto_locate,
    ) {
        preferences.weather.auto_locate = !preferences.weather.auto_locate;
        changed = true;
    }

    let alerts_row = row_rect(panel, ALERTS_ROW_TOP);
    draw_row(ui, alerts_row);
    ui.painter().text(
        Pos2::new(alerts_row.left() + 11.0, alerts_row.center().y),
        Align2::LEFT_CENTER,
        "极端天气风险提醒",
        FontId::proportional(11.0),
        TEXT,
    );
    if toggle(
        ui,
        alerts_row,
        "weather-alerts",
        preferences.weather.alerts_enabled,
    ) {
        preferences.weather.alerts_enabled = !preferences.weather.alerts_enabled;
        changed = true;
    }

    let interval = row_rect(panel, INTERVAL_ROW_TOP);
    draw_row(ui, interval);
    ui.painter().text(
        interval.left_center() + Vec2::new(11.0, 0.0),
        Align2::LEFT_CENTER,
        "更新间隔",
        FontId::proportional(11.0),
        TEXT,
    );
    changed |= ui
        .put(
            Rect::from_center_size(
                interval.right_center() - Vec2::new(49.0, 0.0),
                Vec2::new(82.0, 22.0),
            ),
            egui::DragValue::new(&mut preferences.weather.refresh_minutes)
                .range(1..=MAX_REFRESH_MINUTES)
                .suffix(" 分钟"),
        )
        .changed();

    let location = if preferences.weather.auto_locate {
        "自动（网络位置）".to_owned()
    } else {
        preferences
            .weather
            .location
            .as_ref()
            .map_or_else(|| "尚未选择".to_owned(), |location| location.name.clone())
    };
    ui.painter().text(
        Pos2::new(panel.left() + 14.0, panel.top() + LOCATION_TOP),
        Align2::LEFT_CENTER,
        "当前城市",
        FontId::proportional(10.0),
        SECONDARY_TEXT,
    );
    ui.painter().text(
        Pos2::new(panel.right() - 14.0, panel.top() + LOCATION_TOP),
        Align2::RIGHT_CENTER,
        truncate(&location, 16),
        FontId::proportional(10.5),
        TEXT,
    );

    ui.painter().text(
        Pos2::new(panel.left() + 14.0, panel.top() + SEARCH_LABEL_TOP),
        Align2::LEFT_CENTER,
        "搜索城市",
        FontId::proportional(10.0),
        SECONDARY_TEXT,
    );
    let input = Rect::from_min_size(
        Pos2::new(panel.left() + 14.0, panel.top() + SEARCH_FIELD_TOP),
        Vec2::new((panel.width() - 92.0).max(80.0), 26.0),
    );
    let field = ui.put(
        input,
        egui::TextEdit::singleline(&mut search.query)
            .hint_text("输入城市名")
            .font(FontId::proportional(11.0))
            .desired_width(f32::INFINITY),
    );
    let enter = field.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
    let button = Rect::from_min_size(
        Pos2::new(panel.right() - 70.0, panel.top() + SEARCH_FIELD_TOP),
        Vec2::new(56.0, 26.0),
    );
    let searching = search.receiver.is_some();
    if ui
        .put(
            button,
            egui::Button::new(
                RichText::new(if searching { "搜索中" } else { "搜索" })
                    .color(Color32::WHITE)
                    .size(10.5),
            )
            .fill(ROW)
            .corner_radius(7.0),
        )
        .clicked()
        || enter
    {
        search.start();
    }

    if let Some(error) = &search.error {
        ui.painter().text(
            Pos2::new(panel.left() + 14.0, panel.top() + RESULTS_TOP + 8.0),
            Align2::LEFT_CENTER,
            truncate(error, 38),
            FontId::proportional(9.5),
            Color32::from_rgb(255, 118, 128),
        );
    } else if search.searched && search.results.is_empty() {
        ui.painter().text(
            Pos2::new(panel.left() + 14.0, panel.top() + RESULTS_TOP + 8.0),
            Align2::LEFT_CENTER,
            "没有找到匹配的城市",
            FontId::proportional(9.5),
            SECONDARY_TEXT,
        );
    }

    let mut selected = None;
    for (index, city) in search.results.iter().take(MAX_RESULTS).enumerate() {
        let row = Rect::from_min_size(
            Pos2::new(
                panel.left() + 14.0,
                panel.top() + RESULTS_TOP + index as f32 * RESULT_STEP,
            ),
            Vec2::new(panel.width() - 28.0, 24.0),
        );
        let response = ui.interact(
            row,
            egui::Id::new(("weather-city", &city.name, index)),
            Sense::click(),
        );
        if response.hovered() {
            ui.painter()
                .rect_filled(row, 7.0, Color32::from_white_alpha(14));
        }
        ui.painter().text(
            Pos2::new(row.left() + 8.0, row.top() + 4.0),
            Align2::LEFT_TOP,
            truncate(&city.name, 12),
            FontId::proportional(10.0),
            TEXT,
        );
        ui.painter().text(
            Pos2::new(row.right() - 8.0, row.top() + 4.0),
            Align2::RIGHT_TOP,
            truncate(&city.detail(), 20),
            FontId::proportional(8.6),
            SECONDARY_TEXT,
        );
        if response.clicked() {
            selected = Some((city.name.clone(), city.latitude, city.longitude));
        }
    }
    if let Some((name, latitude, longitude)) = selected {
        preferences.weather.location = Some(WeatherLocation::new(name, latitude, longitude));
        preferences.weather.auto_locate = false;
        search.results.clear();
        search.searched = false;
        changed = true;
    }
    changed
}

fn row_rect(panel: Rect, top: f32) -> Rect {
    Rect::from_min_size(
        Pos2::new(panel.left() + 8.0, panel.top() + top),
        Vec2::new(panel.width() - 16.0, 28.0),
    )
}

fn draw_row(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_filled(rect, 7.0, ROW);
}

fn toggle(ui: &egui::Ui, row: Rect, id: &'static str, enabled: bool) -> bool {
    let switch = Rect::from_center_size(
        Pos2::new(row.right() - 20.0, row.center().y),
        Vec2::new(30.0, 18.0),
    );
    let response = ui.interact(switch, egui::Id::new(id), Sense::click());
    ui.painter()
        .rect_filled(switch, 9.0, if enabled { SWITCH_ON } else { SWITCH_OFF });
    ui.painter().circle_filled(
        Pos2::new(
            if enabled {
                switch.right() - 9.0
            } else {
                switch.left() + 9.0
            },
            switch.center().y,
        ),
        7.0,
        TEXT,
    );
    response.clicked()
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        format!(
            "{}…",
            text.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    }
}
