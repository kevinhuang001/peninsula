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
