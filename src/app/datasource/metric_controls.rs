//! Compact inline switches for each independently managed data-source metric.
use super::*;

impl DataSourceView {
    /// The same controls serve wide right-aligned rows and narrower wrapped
    /// rows. They stay inline instead of creating another nested full-width UI.
    pub(super) fn metric_controls(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        metric: &datasource::MetricSnapshot,
        preferences: &mut super::super::settings::AppPreferences,
        reverse: bool,
    ) {
        if reverse {
            if metric.has_alert {
                self.metric_cooldown(ui, source, metric, preferences, true);
                ui.add_space(14.0);
                self.metric_notification_toggle(ui, source, metric, preferences, true);
                ui.add_space(14.0);
            }
            self.metric_capsule_toggle(ui, source, metric, preferences, true);
        } else {
            self.metric_capsule_toggle(ui, source, metric, preferences, false);
            if metric.has_alert {
                ui.add_space(14.0);
                self.metric_notification_toggle(ui, source, metric, preferences, false);
                ui.add_space(14.0);
                self.metric_cooldown(ui, source, metric, preferences, false);
            }
        }
    }

    fn metric_capsule_toggle(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        metric: &datasource::MetricSnapshot,
        preferences: &mut super::super::settings::AppPreferences,
        reverse: bool,
    ) {
        let Ok(id) = datasource::metric_capsule_id(&source.id, &metric.id) else { return };
        let mut enabled = preferences.is_enabled(&id);
        if !reverse { ui.label("胶囊"); }
        if super::super::settings::core_toggle(ui, &mut enabled, "胶囊显示")
            && preferences.set_capsule_item_enabled(&id, enabled)
        {
            self.preferences_dirty = true;
        }
        if reverse { ui.label("胶囊"); }
    }

    fn metric_notification_toggle(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        metric: &datasource::MetricSnapshot,
        preferences: &mut super::super::settings::AppPreferences,
        reverse: bool,
    ) {
        let Ok(id) = datasource::metric_notification_id(&source.id, &metric.id) else { return };
        let descriptor = preferences.pages.iter()
            .flat_map(|page| &page.descriptor.contributions)
            .find(|entry| entry.id == id).cloned();
        let Some(descriptor) = descriptor else { return };
        let mut enabled = preferences.contribution_enabled(&descriptor);
        if !reverse { ui.label("通知"); }
        if super::super::settings::core_toggle(ui, &mut enabled, "通知提醒") {
            preferences.set_contribution_enabled(id, enabled);
            self.preferences_dirty = true;
        }
        if reverse { ui.label("通知"); }
    }

    fn metric_cooldown(
        &mut self,
        ui: &mut egui::Ui,
        source: &SourceSnapshot,
        metric: &datasource::MetricSnapshot,
        preferences: &mut super::super::settings::AppPreferences,
        reverse: bool,
    ) {
        let Ok(id) = datasource::metric_notification_id(&source.id, &metric.id) else { return };
        let values = preferences.contribution_values.entry(id).or_insert_with(|| {
            let mut values = crate::extension::Settings::new();
            values.insert(
                "cooldown".into(),
                crate::extension::SettingValue::Duration(metric.alert_interval_ms),
            );
            values
        });
        if let Some(crate::extension::SettingValue::Duration(ms)) = values.get_mut("cooldown") {
            if !reverse { ui.label("间隔"); }
            let mut minutes = (*ms).div_ceil(60_000).clamp(1, 43_200);
            if ui.add(
                egui::DragValue::new(&mut minutes).range(1..=43_200).suffix(" 分钟"),
            ).changed() {
                *ms = minutes * 60_000;
                self.preferences_dirty = true;
            }
            if reverse { ui.label("间隔"); }
        }
    }

}
