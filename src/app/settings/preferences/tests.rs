use super::CompactPreferences;
use crate::extension::ExtensionId;

fn id(value: &'static str) -> ExtensionId {
    ExtensionId::builtin(value)
}

#[test]
fn preferences_round_trip_and_migrate_legacy_flags_order_and_quiet_hours() {
    let legacy = CompactPreferences::parse(
        "mascot=false\nbattery=true\ncpu=true\ngpu=false\nnetwork=true\ndownloads=true\ntime=true\ncompact_order=1,0,2,3,4,6,5\nquiet_manual=true\nquiet_periods=true,37,1395,390;false,96,720,840\n",
    );
    assert!(!legacy.is_enabled(&id("builtin.mascot/face")));
    assert!(legacy.is_enabled(&id("builtin.system-monitor/cpu")));
    assert_eq!(
        legacy
            .ordered_compact_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        [
            "builtin.system-monitor/battery",
            "builtin.mascot/face",
            "builtin.system-monitor/cpu",
            "builtin.system-monitor/gpu",
            "builtin.system-monitor/network",
            "builtin.clock/time",
            "builtin.downloads/status",
        ]
    );
    assert!(legacy.quiet_hours.manual_enabled);
    assert_eq!(CompactPreferences::parse(&legacy.encode()), legacy);
    let defaults = CompactPreferences::parse("cpu=true\n");
    assert!(defaults.is_enabled(&id("builtin.system-monitor/cpu")));
    assert!(defaults.is_enabled(&id("builtin.mascot/face")));
    assert!(!defaults.is_enabled(&id("builtin.clock/time")));
}

#[test]
fn duplicate_and_invalid_ids_are_ignored_and_items_can_be_reordered() {
    let mut preferences = CompactPreferences::parse(
        "compact_order=3,3,99,1\nextension=builtin.test/widget|true|20\nextension= bad id |true|1\n",
    );
    assert_eq!(preferences.compact_items.len(), 8);
    assert_eq!(
        preferences.ordered_compact_items()[0].id,
        id("builtin.system-monitor/gpu")
    );
    assert!(preferences.move_compact_item(&id("builtin.clock/time"), -1));
    let ordered = preferences.ordered_compact_items();
    assert_eq!(ordered[5].id, id("builtin.clock/time"));
    assert!(!preferences.move_compact_item(&id("builtin.system-monitor/gpu"), -1));
}

#[test]
fn compact_width_tracks_enabled_items_and_active_download_content() {
    let default = CompactPreferences::default();
    let mut wide = default.clone();
    wide.set_enabled(&id("builtin.system-monitor/cpu"), true);
    wide.set_enabled(&id("builtin.system-monitor/network"), true);
    assert_eq!(default.compact_width(false), 114.0);
    assert!(wide.compact_width(false) > default.compact_width(false));
    assert!(default.compact_width(true) == default.compact_width(false));

    let mut downloads = default;
    downloads.set_enabled(&id("builtin.downloads/status"), true);
    assert_eq!(downloads.compact_width(false), 114.0);
    assert!(downloads.compact_width(true) > downloads.compact_width(false));
}

#[test]
fn saves_preferences_to_disk_and_replaces_existing_content() {
    let folder =
        std::env::temp_dir().join(format!("peninsula-settings-test-{}", std::process::id()));
    let path = folder.join("preferences.conf");
    let mut preferences = CompactPreferences::default();
    preferences.save_to(&path).unwrap();
    preferences.set_enabled(&id("builtin.system-monitor/cpu"), true);
    preferences.save_to(&path).unwrap();
    assert_eq!(
        CompactPreferences::parse(&std::fs::read_to_string(&path).unwrap()),
        preferences
    );
    std::fs::remove_dir_all(folder).unwrap();
}
