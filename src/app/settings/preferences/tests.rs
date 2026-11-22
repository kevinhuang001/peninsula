use super::*;
use crate::extension::SettingValue as V;
fn id(value: &'static str) -> ExtensionId {
    ExtensionId::builtin(value)
}
#[test]
fn legacy_layout_migrates_to_independent_core_mascot_and_battery_extension() {
    let prefs = CompactPreferences::parse("mascot=false\nbattery=true\ncpu=true\ncompact_order=1,0,2,3,4,6,5\nquiet_manual=true\nquiet_periods=true,37,1395,390\n");
    assert!(!prefs.mascot_enabled);
    assert!(prefs.is_enabled(&id("builtin.battery/status")));
    assert!(prefs.is_enabled(&id("builtin.system-monitor/cpu")));
    assert_eq!(
        prefs.ordered_compact_items()[0].id,
        id("builtin.battery/status")
    );
    assert!(!prefs
        .compact_items
        .iter()
        .any(|i| i.id.as_str().contains("mascot")));
    assert!(prefs.quiet_hours.manual_enabled);
    assert_eq!(prefs.quiet_hours.periods().len(), 1);
    let round = CompactPreferences::parse(&prefs.encode());
    assert_eq!(round, prefs);
}
#[test]
fn legacy_business_values_are_only_inputs_to_schema_migration() {
    let source = "pomodoro_focus_minutes=42\npomodoro_break_minutes=8\nweather_auto=false\nweather_location=上海|31.2|121.4\nbilibili_enabled=true\nbilibili_poll_seconds=90\n";
    let values = super::super::migration::legacy_settings(source);
    assert_eq!(
        values[&id("builtin.pomodoro")]["focus_minutes"],
        V::Integer(42)
    );
    assert_eq!(
        values[&id("builtin.weather")]["location_name"],
        V::String("上海".into())
    );
    assert_eq!(
        values[&id("builtin.bilibili")]["poll_interval"],
        V::Duration(90_000)
    );
    let prefs = CompactPreferences::parse(source);
    assert!(
        prefs
            .pages
            .iter()
            .find(|p| p.descriptor.id == id("builtin.bilibili"))
            .unwrap()
            .enabled
    );
    let encoded = prefs.encode();
    for key in [
        "pomodoro_focus_minutes",
        "weather_location",
        "bilibili_poll_seconds",
    ] {
        assert!(!encoded.contains(key));
    }
}
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
