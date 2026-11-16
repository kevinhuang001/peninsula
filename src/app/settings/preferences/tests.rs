use super::CompactPreferences;

#[test]
fn compact_preferences_round_trip_and_keep_defaults_for_missing_keys() {
    let mut quiet_hours = super::super::quiet_hours::QuietHours::default();
    quiet_hours.manual_enabled = true;
    quiet_hours.add_period(super::super::quiet_hours::QuietPeriod {
        enabled: true,
        days: 0b0010_0101,
        start_minute: 23 * 60 + 15,
        end_minute: 6 * 60 + 30,
    });
    quiet_hours.add_period(super::super::quiet_hours::QuietPeriod {
        enabled: false,
        days: 0b0110_0000,
        start_minute: 12 * 60,
        end_minute: 14 * 60,
    });
    let preferences = CompactPreferences {
        mascot: false,
        battery: true,
        cpu: true,
        gpu: false,
        network: true,
        downloads: true,
        time: true,
        compact_order: [1, 0, 2, 3, 4, 6, 5],
        quiet_hours,
    };
    assert_eq!(
        CompactPreferences::parse(&preferences.encode()),
        preferences
    );
    assert!(CompactPreferences::parse("cpu=true\n").cpu);
    assert!(CompactPreferences::parse("cpu=true\n").mascot);
    assert!(!CompactPreferences::parse("cpu=true\n").time);
}

#[test]
fn compact_order_repairs_duplicates_and_moves_items_one_slot_at_a_time() {
    let mut preferences = CompactPreferences::parse("compact_order=3,3,99,1\n");
    assert_eq!(preferences.compact_order, [3, 1, 0, 2, 4, 5, 6]);
    assert!(preferences.move_compact_item(super::super::OptionItem::Time, -1));
    assert_eq!(preferences.compact_order, [3, 1, 0, 2, 4, 6, 5]);
    assert!(!preferences.move_compact_item(super::super::OptionItem::Gpu, -1));
}

#[test]
fn compact_width_accounts_for_enabled_content() {
    let default = CompactPreferences::default();
    let wide = CompactPreferences {
        cpu: true,
        network: true,
        ..default
    };
    assert_eq!(default.compact_width(false), 114.0);
    assert!(wide.compact_width(false) > default.compact_width(false));
    assert!(
        CompactPreferences {
            time: true,
            ..default
        }
        .compact_width(false)
            > default.compact_width(false)
    );
}

#[test]
fn download_indicator_only_consumes_width_while_active() {
    let preferences = CompactPreferences {
        downloads: true,
        cpu: true,
        ..Default::default()
    };
    assert_eq!(
        preferences.compact_width(false),
        CompactPreferences {
            downloads: false,
            ..preferences
        }
        .compact_width(false)
    );
    assert!(preferences.compact_width(true) > preferences.compact_width(false));
}

#[test]
fn saves_preferences_to_disk_and_replaces_existing_content() {
    let folder =
        std::env::temp_dir().join(format!("peninsula-settings-test-{}", std::process::id()));
    let path = folder.join("preferences.conf");
    let mut preferences = CompactPreferences::default();
    preferences.save_to(&path).unwrap();
    preferences.cpu = true;
    preferences.save_to(&path).unwrap();
    assert_eq!(
        CompactPreferences::parse(&std::fs::read_to_string(&path).unwrap()),
        preferences
    );
    std::fs::remove_dir_all(folder).unwrap();
}
