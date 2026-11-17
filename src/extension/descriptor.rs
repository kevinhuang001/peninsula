use std::{fmt, hash::Hash};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExtensionId(String);

impl ExtensionId {
    pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        let valid = !value.is_empty()
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionKind {
    Widget,
    Compact,
    Notification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Capability {
    View,
    Action,
    Event,
    Schedule,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtensionDescriptor {
    pub id: ExtensionId,
    pub kind: ExtensionKind,
    pub title: String,
    pub icon: String,
    pub default_enabled: bool,
    pub default_order: u32,
    pub size_hint: [f32; 2],
    pub capabilities: Vec<Capability>,
    pub schema_version: u32,
    pub version: String,
    pub requires_setup: bool,
}

impl ExtensionDescriptor {
    pub fn builtin(
        id: &'static str,
        kind: ExtensionKind,
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
            capabilities: match kind {
                ExtensionKind::Notification => vec![Capability::Event],
                ExtensionKind::Widget | ExtensionKind::Compact => {
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
