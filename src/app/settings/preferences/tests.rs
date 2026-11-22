use super::*;
#[test]
fn atomic_preferences_save_replaces_existing_content() {
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("preferences.conf");
    let mut prefs = CompactPreferences::default();
    prefs.save_to(&path).unwrap();
    prefs.mascot_enabled = false;
    prefs.save_to(&path).unwrap();
    assert!(!CompactPreferences::parse(&fs::read_to_string(path).unwrap()).mascot_enabled);
}

#[test]
fn obsolete_mascot_size_is_not_saved_again() {
    let prefs =
        CompactPreferences::parse("mascot_size=64\nmascot_enabled=false\nmascot_motion=Reduced");
    assert!(!prefs.mascot_enabled);
    assert_eq!(prefs.mascot_motion, "Reduced");
    assert!(!prefs.encode().contains("mascot_size"));
}

#[test]
fn widget_switches_round_trip_and_sort_only_enabled_widgets() {
    let mut prefs = CompactPreferences::default();
    let ids = prefs.widget_order.clone();
    assert!(ids.len() >= 3);
    assert!(prefs.set_widget_enabled(&ids[1], false));
    assert!(!prefs.set_widget_enabled(&ids[1], false));
    assert!(!prefs.widget_enabled(&ids[1]));
    let active_before = prefs.ordered_enabled_widgets();
    assert!(prefs.move_widget(&ids[2], -1));
    let active_after = prefs.ordered_enabled_widgets();
    assert_eq!(active_after[0], ids[2]);
    assert_eq!(active_after[1], active_before[0]);
    assert!(!active_after.contains(&ids[1]));
    let restored = CompactPreferences::parse(&prefs.encode());
    assert_eq!(restored.widget_disabled, prefs.widget_disabled);
    assert_eq!(restored.widget_order, prefs.widget_order);
    assert!(restored.set_widget_enabled(&ids[1], true));
    assert!(restored.widget_enabled(&ids[1]));
}

#[test]
fn compact_reordering_skips_disabled_items_without_enabling_them() {
    let mut prefs = CompactPreferences::default();
    let items: Vec<_> = prefs
        .ordered_compact_items()
        .into_iter()
        .take(3)
        .map(|item| item.id.clone())
        .collect();
    assert_eq!(items.len(), 3);
    for id in &items {
        prefs.set_compact_item_enabled(id, true);
    }
    prefs.set_compact_item_enabled(&items[1], false);
    assert!(prefs.move_compact_item(&items[2], -1));
    let active: Vec<_> = prefs
        .ordered_compact_items()
        .into_iter()
        .filter(|item| item.enabled)
        .map(|item| item.id.clone())
        .collect();
    assert!(active.iter().position(|id| id == &items[2]).unwrap()
        < active.iter().position(|id| id == &items[0]).unwrap());
    assert!(!prefs.is_enabled(&items[1]));
}
