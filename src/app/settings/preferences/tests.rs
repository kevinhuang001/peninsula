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
