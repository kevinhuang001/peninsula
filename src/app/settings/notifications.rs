//! Host-only rendering of declarative Notification contribution settings.
//! This file must not check extension IDs, asset types or business field keys.
use super::{
    forms,
    theme::{BORDER, SECONDARY_TEXT},
    AppPreferences,
};
use crate::extension::{
    ActivationRule, ContributionDescriptor, ContributionKind, ExtensionId, SettingSchema,
};
use eframe::egui::{self, Stroke};
use std::collections::HashMap;

/// Derive the second-level menu from declared notification contributions.
pub(super) fn owners(prefs: &AppPreferences) -> Vec<(ExtensionId, String)> {
    prefs.pages.iter()
        .filter(|page| page.descriptor.contributions.iter()
            .any(|entry| entry.kind == ContributionKind::Notification))
        .map(|page| (page.descriptor.id.clone(), page.descriptor.name.clone()))
        .collect()
}

pub(super) fn options(
    ui: &mut egui::Ui,
    prefs: &mut AppPreferences,
    selected: Option<&ExtensionId>,
    number_drafts: &mut HashMap<(ExtensionId, String), String>,
) -> bool {
    // The selected extension is already identified by Core's second-level
    // navigation. Do not render its name again in the configuration pane.
    let Some((owner, shared, entries)) = prefs.pages.iter().filter_map(|page| {
        if Some(&page.descriptor.id) != selected {
            return None;
        }
        let entries: Vec<ContributionDescriptor> = page.descriptor.contributions.iter()
            .filter(|entry| entry.kind == ContributionKind::Notification)
            .cloned().collect();
        if entries.is_empty() { return None; }
        let shared: Vec<SettingSchema> = page.descriptor.settings.iter()
            .filter(|field| field.presentation.in_notifications)
            .cloned().collect();
        Some((page.descriptor.id.clone(), shared, entries))
    }).next() else {
        forms::empty(ui, "没有可配置的通知");
        return false;
    };
    let mut changed = false;
        if let Some(page) = prefs.pages.iter_mut().find(|page| page.descriptor.id == owner) {
            for field in shared {
                if let Some(default) = &field.default {
                    let value = page.values.entry(field.key.clone())
                        .or_insert_with(|| default.clone());
                    ui.push_id(("notification-shared", &owner, &field.key), |ui| {
                        changed |= forms::setting_control_with_draft(
                            ui, &field, value, &owner, number_drafts,
                        );
                    });
                }
            }
        }
        let mut last_group: Vec<String> = vec![];
        for entry in entries {
            if entry.group_path != last_group {
                for segment in &entry.group_path {
                    forms::section(ui, segment);
                }
                last_group.clone_from(&entry.group_path);
            }
            ui.add_space(4.0);
            egui::Frame::new()
                .fill(super::theme::ROW)
                .stroke(Stroke::new(1.0_f32, BORDER))
                .corner_radius(9.0)
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.push_id(&entry.id, |ui| {
                        ui.label(egui::RichText::new(&entry.title)
                            .text_style(egui::TextStyle::Body).strong().color(SECONDARY_TEXT));
                        ui.add_space(2.0);
                        let mut enabled = prefs.contribution_enabled(&entry);
                        if forms::boolean(ui, "启用提醒", &mut enabled) {
                            prefs.set_contribution_enabled(entry.id.clone(), enabled);
                            changed = true;
                        }
                        let values = prefs.contribution_values.entry(entry.id.clone())
                            .or_insert_with(|| crate::core::settings::SettingsStore::defaults(&entry));
                        for field in &entry.settings {
                            if let Some(default) = &field.default {
                                values.entry(field.key.clone()).or_insert_with(|| default.clone());
                            }
                        }
                        for field in &entry.settings {
                            if let Some(value) = values.get_mut(&field.key) {
                                ui.push_id(&field.key, |ui| {
                                    changed |= forms::setting_control_with_draft(
                                        ui, field, value, &entry.id, number_drafts,
                                    );
                                });
                                // Validate after editing to clear the warning immediately.
                                if enabled && entry.requires_setup
                                    && field.activation.is_some()
                                    && !field.activation_ready(value)
                                {
                                    let detail = match field.activation {
                                        Some(ActivationRule::Positive) => "需填写大于 0 的数值",
                                        None => unreachable!("checked activation above"),
                                    };
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}：{detail}", field.description
                                        ))
                                        .text_style(egui::TextStyle::Small)
                                        .color(ui.visuals().warn_fg_color),
                                    );
                                }
                            }
                        }
                    });
                });
        }
    changed
}
