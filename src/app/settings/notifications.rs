//! Host-only rendering of declarative Notification contribution settings.
//! This file must not check extension IDs, asset types or business field keys.
use super::{
    forms,
    theme::{ACCENT, BORDER, SECONDARY_TEXT, TEXT},
    CompactPreferences,
};
use crate::extension::{
    ActivationRule, ContributionDescriptor, ContributionKind, ExtensionId, SettingSchema,
    SettingValue,
};
use eframe::egui::{self, Color32, Sense, Stroke};
use std::collections::HashMap;

pub(super) fn options(
    ui: &mut egui::Ui,
    prefs: &mut CompactPreferences,
    number_drafts: &mut HashMap<(ExtensionId, String), String>,
) -> bool {
    // Snapshot the catalog so UI changes never mutate registration metadata.
    let groups: Vec<_> = prefs.pages.iter().filter_map(|page| {
        let entries: Vec<ContributionDescriptor> = page.descriptor.contributions.iter()
            .filter(|entry| entry.kind == ContributionKind::Notification)
            .cloned().collect();
        if entries.is_empty() { return None; }
        let shared: Vec<SettingSchema> = page.descriptor.settings.iter()
            .filter(|field| field.presentation.in_notifications)
            .cloned().collect();
        Some((page.descriptor.id.clone(), page.descriptor.name.clone(), shared, entries))
    }).collect();
    let mut changed = false;
    for (owner, name, shared, entries) in groups {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let (marker, _) = ui.allocate_exact_size(egui::vec2(3.0, 16.0), Sense::hover());
            ui.painter().rect_filled(marker, 2.0, ACCENT);
            ui.label(egui::RichText::new(&name).size(14.0).strong().color(TEXT));
        });
        ui.add_space(7.0);
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
            ui.add_space(9.0);
            egui::Frame::new()
                .fill(Color32::from_rgb(30, 33, 39))
                .stroke(Stroke::new(1.0, BORDER))
                .corner_radius(10.0)
                .inner_margin(egui::Margin::symmetric(10, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.push_id(&entry.id, |ui| {
                        ui.label(egui::RichText::new(&entry.title)
                            .size(12.0).strong().color(SECONDARY_TEXT));
                        ui.add_space(6.0);
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
                                // The switch records user intent; Core starts a
                                // contribution only when all activation rules
                                // pass. Indicate an incomplete value beside its
                                // own field, not in a contradictory card banner.
                                // Evaluate AFTER editing so feedback disappears
                                // immediately when a valid number is entered.
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
                                        .size(10.5)
                                        .color(ui.visuals().warn_fg_color),
                                    );
                                }
                            }
                        }
                    });
                });
        }
    }
    changed
}
