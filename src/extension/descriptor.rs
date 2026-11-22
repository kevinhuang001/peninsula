use super::SettingSchema;
use crate::ui::declarative::{RefreshHint, WidgetBounds};
use serde::{Deserialize, Serialize};
use std::{fmt, hash::Hash};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ExtensionId(String);

impl ExtensionId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        let valid = !value.is_empty()
            && !value.trim_matches('.').is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/' | b'-' | b'_')
            });
        if valid {
            Ok(Self(value))
        } else {
            Err(IdError)
        }
    }

    pub fn builtin(value: &'static str) -> Self {
        Self::new(value).expect("built-in extension IDs are valid")
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ExtensionId {
    type Error = IdError;
    fn try_from(value: String) -> Result<Self, IdError> {
        Self::new(value)
    }
}

impl From<ExtensionId> for String {
    fn from(value: ExtensionId) -> Self {
        value.0
    }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdError;

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .write_str("extension ID must be 1–128 ASCII letters, digits, '.', '/', '-', or '_'")
    }
}

impl std::error::Error for IdError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ContributionKind {
    Widget,
    Compact,
    Notification,
    BackgroundService,
    EventHandler,
    MascotExpression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Capability {
    View,
    Action,
    Event,
    Schedule,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContributionDescriptor {
    pub id: ExtensionId,
    pub kind: ContributionKind,
    pub title: String,
    pub icon: String,
    pub default_enabled: bool,
    pub default_order: u32,
    pub size_hint: [f32; 2],
    #[serde(default)]
    pub widget_bounds: WidgetBounds,
    #[serde(default)]
    pub refresh_hint: RefreshHint,
    pub settings_version: u32,
    pub settings: Vec<SettingSchema>,
    pub capabilities: Vec<Capability>,
    pub schema_version: u32,
    pub version: String,
    pub requires_setup: bool,
}

impl ContributionDescriptor {
    pub fn builtin(
        id: &'static str,
        kind: ContributionKind,
        title: &'static str,
        icon: &'static str,
        default_enabled: bool,
        default_order: u32,
    ) -> Self {
        Self {
            id: ExtensionId::builtin(id),
            kind,
            title: title.into(),
            icon: icon.into(),
            default_enabled,
            default_order,
            size_hint: [0.0, 0.0],
            widget_bounds: WidgetBounds::default(),
            refresh_hint: RefreshHint::OnInvalidation,
            settings_version: 1,
            settings: vec![],
            capabilities: match kind {
                ContributionKind::Notification
                | ContributionKind::EventHandler
                | ContributionKind::MascotExpression => vec![Capability::Event],
                ContributionKind::BackgroundService => vec![Capability::Schedule],
                ContributionKind::Widget | ContributionKind::Compact => {
                    vec![Capability::View, Capability::Action]
                }
            },
            schema_version: 1,
            version: env!("CARGO_PKG_VERSION").into(),
            requires_setup: false,
        }
    }

    pub fn with_size_hint(mut self, width: f32, height: f32) -> Self {
        self.size_hint = [width, height];
        self
    }

    pub fn with_widget_bounds(mut self, bounds: WidgetBounds) -> Self {
        self.widget_bounds = bounds;
        self
    }

    pub fn with_settings(mut self, version: u32, settings: Vec<SettingSchema>) -> Self {
        self.settings_version = version;
        self.settings = settings;
        self
    }

    pub fn with_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn with_setup(mut self, requires_setup: bool) -> Self {
        self.requires_setup = requires_setup;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ExecutionPolicy {
    EventDriven,
    Interval(std::time::Duration),
    WhenVisible,
    OnEnable,
}

/// One business module owns every UI, task, notification and expression contribution.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExtensionDescriptor {
    pub id: ExtensionId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub icon: String,
    pub category: String,
    pub default_enabled: bool,
    pub default_order: u32,
    pub contributions: Vec<ContributionDescriptor>,
    pub settings_version: u32,
    pub settings: Vec<SettingSchema>,
    pub execution_policy: ExecutionPolicy,
}
impl ExtensionDescriptor {
    pub fn with_runtime_contributions(
        mut self,
        service: bool,
        events: bool,
        expressions: &[crate::core::mascot::MascotExpression],
    ) -> Self {
        let mut add = |local: &str, kind: ContributionKind, name: &str| {
            self.contributions.push(ContributionDescriptor {
                id: ExtensionId::new(format!("{}/{}", self.id, local))
                    .unwrap_or_else(|_| self.id.clone()),
                kind,
                title: name.into(),
                icon: self.icon.clone(),
                default_enabled: true,
                default_order: 0,
                size_hint: [0.0, 0.0],
                widget_bounds: Default::default(),
                refresh_hint: Default::default(),
                settings_version: 1,
                settings: vec![],
                capabilities: vec![],
                schema_version: 1,
                version: self.version.clone(),
                requires_setup: false,
            });
        };
        if service {
            add(
                "service.background",
                ContributionKind::BackgroundService,
                "后台服务",
            );
        }
        if events {
            add("event.handler", ContributionKind::EventHandler, "事件订阅");
        }
        for expression in expressions {
            add(
                &format!("expression.{}", expression.id),
                ContributionKind::MascotExpression,
                &expression.name,
            );
        }
        self
    }
    pub fn builtin(
        id: &'static str,
        name: &str,
        description: &str,
        mut contributions: Vec<ContributionDescriptor>,
    ) -> Self {
        let settings_version = contributions
            .iter()
            .map(|c| c.settings_version)
            .max()
            .unwrap_or(1);
        let settings = contributions
            .iter_mut()
            .flat_map(|c| std::mem::take(&mut c.settings))
            .collect();
        Self {
            id: ExtensionId::builtin(id),
            name: name.into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: description.into(),
            icon: contributions
                .first()
                .map_or_else(|| "extension".into(), |c| c.icon.clone()),
            category: "内置".into(),
            default_enabled: contributions.iter().any(|c| c.default_enabled),
            default_order: contributions.first().map_or(0, |c| c.default_order),
            contributions,
            settings_version,
            settings,
            execution_policy: ExecutionPolicy::Interval(std::time::Duration::from_millis(250)),
        }
    }
}
