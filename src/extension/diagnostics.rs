use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum DiagnosticStatus {
    Passed,
    Warning,
    Failed,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub id: String,
    pub title: String,
    pub status: DiagnosticStatus,
    pub detail: String,
    pub remediation: Option<String>,
}

impl DiagnosticCheck {
    pub fn new(
        id: &str,
        title: &str,
        status: DiagnosticStatus,
        detail: impl Into<String>,
        remediation: Option<&str>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status,
            detail: detail.into(),
            remediation: remediation.map(str::to_owned),
        }
    }

    pub fn passed(id: &str, title: &str, detail: impl Into<String>) -> Self {
        Self::new(id, title, DiagnosticStatus::Passed, detail, None)
    }

    pub fn unknown(id: &str, title: &str, detail: impl Into<String>) -> Self {
        Self::new(id, title, DiagnosticStatus::Unknown, detail, None)
    }
}
