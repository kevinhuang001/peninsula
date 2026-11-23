use super::*;
#[test]
fn atomic_preferences_save_replaces_existing_content() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("preferences.conf");
    let mut prefs = AppPreferences::default();
    prefs.save_to(&path).unwrap();
    prefs.mascot_enabled = false;
    prefs.save_to(&path).unwrap();
    assert!(!AppPreferences::parse(&fs::read_to_string(path).unwrap()).mascot_enabled);
}

#[test]
fn obsolete_mascot_size_is_not_saved_again() {
    let prefs =
        AppPreferences::parse("mascot_size=64\nmascot_enabled=false\nmascot_motion=Reduced");
    assert!(!prefs.mascot_enabled);
    assert_eq!(prefs.mascot_motion, "Reduced");
    assert!(!prefs.encode().contains("mascot_size"));
}

#[test]
fn only_enabled_widgets_appear_in_order_and_switches_survive_serialization() {
    let mut prefs = AppPreferences::default();
    let ids = prefs.widget_order.clone();
    assert!(ids.len() >= 3);
    let disabled = ids[1].clone();
    prefs.set_contribution_enabled(disabled.clone(), false);
    let before = prefs.ordered_enabled_widgets();
    assert!(!before.contains(&disabled));
    let moving = before[1].clone();
    assert!(prefs.move_widget(&moving, -1));
    let after = prefs.ordered_enabled_widgets();
    assert_eq!(after[0], moving);
    let mut restored = AppPreferences::parse(&prefs.encode());
    assert_eq!(restored.ordered_enabled_widgets(), after);
    restored.set_contribution_enabled(disabled.clone(), true);
    assert!(restored.ordered_enabled_widgets().contains(&disabled));
}

#[test]
fn capsule_sorting_skips_disabled_items() {
    let mut prefs = AppPreferences::default();
    let ids: Vec<_> = prefs.ordered_capsule_items().into_iter().take(3)
        .map(|item| item.id.clone()).collect();
    assert_eq!(ids.len(), 3);
    for id in &ids { prefs.set_capsule_item_enabled(id, true); }
    prefs.set_capsule_item_enabled(&ids[1], false);
    assert!(prefs.move_capsule_item(&ids[2], -1));
    assert!(!prefs.is_enabled(&ids[1]));
    let active: Vec<_> = prefs.ordered_capsule_items().into_iter()
        .filter(|item| item.enabled).map(|item| item.id.clone()).collect();
    assert!(active.iter().position(|id| id == &ids[2]).unwrap()
        < active.iter().position(|id| id == &ids[0]).unwrap());
}


#[test]
fn previous_prefs_upgrade_without_losing_capsule_order_or_user_accent() {
    let id = AppPreferences::default().capsule_items[0].id.clone();
    let source = format!(
        "extension={id}|false|2\ncompact_text_size=16\ncompact_text_color=#ffffff\nwidget_theme=light\nwidget_accent=#6d45a0\n"
    );
    let prefs = AppPreferences::parse(&source);
    assert!(!prefs.is_enabled(&id));
    assert_eq!(prefs.capsule_text_size, 16.0);
    assert_eq!(prefs.capsule_text_color, Some([255, 255, 255]));
    assert_eq!(prefs.appearance_theme, AppearanceTheme::Light);
    assert_eq!(prefs.accent_color, [109, 69, 160]);
    let encoded = prefs.encode();
    assert!(encoded.contains("capsule_item="));
    assert!(encoded.contains("capsule_text_size=16.0"));
    assert!(encoded.contains("appearance_theme=light"));
    assert!(encoded.contains("accent_color=#6d45a0"));
    assert!(!encoded.contains("\nextension="));
    assert!(!encoded.contains("widget_accent="));
    assert_eq!(AppPreferences::parse(&encoded).encode(), encoded);
}
