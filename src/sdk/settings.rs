//! Typed settings contracts shared by the host and built-in extensions.
pub use crate::extension::{
    validate_schema, Constraints, ReloadPolicy, SearchOption, SearchSchema, SearchSelection,
    SettingSchema, SettingValue, Settings, ValueType,
};

#[derive(Clone, Debug, Default)]
pub struct SettingsApi {
    values: Settings,
}

impl SettingsApi {
    pub fn from_values(values: Settings) -> Self {
        Self { values }
    }
    pub fn values(&self) -> &Settings {
        &self.values
    }
    pub fn boolean(&self, key: &str, default: bool) -> bool {
        match self.values.get(key) {
            Some(SettingValue::Boolean(value)) => *value,
            _ => default,
        }
    }
    pub fn integer(&self, key: &str, default: i64) -> i64 {
        match self.values.get(key) {
            Some(SettingValue::Integer(value)) => *value,
            _ => default,
        }
    }
    pub fn string(&self, key: &str, default: &str) -> String {
        match self.values.get(key) {
            Some(SettingValue::String(value) | SettingValue::Enum(value)) => value.clone(),
            _ => default.to_owned(),
        }
    }
    pub fn search(&self, key: &str) -> Option<&SearchSelection> {
        match self.values.get(key) {
            Some(SettingValue::Search(Some(selection))) => Some(selection),
            _ => None,
        }
    }
}
