//! Extension-owned configuration metadata, independent of any settings UI.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

pub type Settings = BTreeMap<String, SettingValue>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValueType {
    Boolean,
    Integer,
    Float,
    String,
    Enum,
    Color,
    Duration,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SettingValue {
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Enum(String),
    /// Unmultiplied sRGBA channels.
    Color([u8; 4]),
    /// Milliseconds.
    Duration(u64),
}

impl SettingValue {
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::Boolean(_) => ValueType::Boolean,
            Self::Integer(_) => ValueType::Integer,
            Self::Float(_) => ValueType::Float,
            Self::String(_) => ValueType::String,
            Self::Enum(_) => ValueType::Enum,
            Self::Color(_) => ValueType::Color,
            Self::Duration(_) => ValueType::Duration,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReloadPolicy {
    Live,
    Reload,
    Restart,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingSchema {
    pub key: String,
    pub value_type: ValueType,
    pub default: Option<SettingValue>,
    pub description: String,
    pub constraints: Constraints,
    pub options: Vec<String>,
    pub required: bool,
    pub reload_policy: ReloadPolicy,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Constraints {
    /// Numeric limits; durations use milliseconds.
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Unicode scalar count for strings.
    pub max_length: Option<usize>,
}

impl SettingSchema {
    pub fn new(key: &str, default: SettingValue, description: &str) -> Self {
        Self {
            key: key.into(),
            value_type: default.value_type(),
            default: Some(default),
            description: description.into(),
            constraints: Constraints::default(),
            options: vec![],
            required: true,
            reload_policy: ReloadPolicy::Live,
        }
    }

    pub fn range(mut self, min: f64, max: f64) -> Self {
        self.constraints.min = Some(min);
        self.constraints.max = Some(max);
        self
    }

    pub fn validate(&self, value: &SettingValue) -> Result<(), String> {
        let invalid = || format!("invalid value for setting {}", self.key);
        if self.value_type != value.value_type() {
            return Err(invalid());
        }
        // Integral comparisons stay integral even above the precision of f64.
        let outside_integer = |number: i128| {
            self.constraints.min.is_some_and(|min| number < min as i128)
                || self.constraints.max.is_some_and(|max| number > max as i128)
        };
        let outside = match value {
            SettingValue::Integer(v) => outside_integer(i128::from(*v)),
            SettingValue::Duration(v) => outside_integer(i128::from(*v)),
            SettingValue::Float(v) => {
                !v.is_finite()
                    || self.constraints.min.is_some_and(|min| *v < min)
                    || self.constraints.max.is_some_and(|max| *v > max)
            }
            _ => false,
        };
        if outside {
            return Err(invalid());
        }
        match value {
            SettingValue::String(v)
                if self
                    .constraints
                    .max_length
                    .is_some_and(|max| v.chars().count() > max) =>
            {
                Err(invalid())
            }
            SettingValue::Enum(v) if !self.options.contains(v) => Err(invalid()),
            _ => Ok(()),
        }
    }
}

pub fn validate_schema(schema: &[SettingSchema]) -> Result<(), String> {
    let mut keys = HashSet::new();
    for field in schema {
        let invalid = || format!("invalid setting schema: {}", field.key);
        if field.key.is_empty()
            || !field
                .key
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || !keys.insert(&field.key)
            || (field.required && field.default.is_none())
        {
            return Err(invalid());
        }
        let Constraints {
            min,
            max,
            max_length,
        } = field.constraints;
        if min.is_some_and(|v| !v.is_finite())
            || max.is_some_and(|v| !v.is_finite())
            || min.zip(max).is_some_and(|(a, b)| a > b)
            || (matches!(field.value_type, ValueType::Integer | ValueType::Duration)
                && [min, max].into_iter().flatten().any(|v| v.fract() != 0.0))
            || (field.value_type == ValueType::Duration
                && [min, max].into_iter().flatten().any(|v| v < 0.0))
            || ((min.is_some() || max.is_some())
                && !matches!(
                    field.value_type,
                    ValueType::Integer | ValueType::Float | ValueType::Duration
                ))
            || (max_length.is_some() && field.value_type != ValueType::String)
        {
            return Err(invalid());
        }
        if field.value_type == ValueType::Enum {
            let options: HashSet<_> = field.options.iter().collect();
            if options.len() != field.options.len()
                || options.is_empty()
                || options.iter().any(|v| v.is_empty())
            {
                return Err(invalid());
            }
        } else if !field.options.is_empty() {
            return Err(invalid());
        }
        if let Some(value) = &field.default {
            field.validate(value)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_strings_floats_colors_and_millisecond_durations() {
        let string = SettingSchema {
            constraints: Constraints {
                max_length: Some(2),
                ..Constraints::default()
            },
            ..SettingSchema::new("name", SettingValue::String("网络".into()), "")
        };
        let float = SettingSchema::new("ratio", SettingValue::Float(0.3), "").range(0.0, 1.0);
        let color = SettingSchema::new("color", SettingValue::Color([1, 2, 3, 4]), "");
        let duration =
            SettingSchema::new("interval", SettingValue::Duration(250), "").range(100.0, 500.0);
        assert!(validate_schema(&[
            string.clone(),
            float.clone(),
            color.clone(),
            duration.clone()
        ])
        .is_ok());
        assert!(string
            .validate(&SettingValue::String("网络速".into()))
            .is_err());
        assert!(float.validate(&SettingValue::Float(f64::INFINITY)).is_err());
        assert!(color
            .validate(&SettingValue::String("#01020304".into()))
            .is_err());
        assert!(duration.validate(&SettingValue::Duration(999)).is_err());
        assert!(validate_schema(&[color.clone(), color]).is_err());
    }
}
