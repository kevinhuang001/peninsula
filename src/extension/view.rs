use super::{
    ExtensionId, Notification, NotificationAction, NotificationAlertView,
    NotificationCenterItemView, ReloadPolicy, Settings, WidgetView,
};

/// Runtime visibility is evaluated separately from the user's Compact preference.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CompactVisibility {
    #[default]
    Visible,
    Hidden,
}

/// A Widget with a hidden runtime state is omitted from both the carousel and dashboard.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WidgetVisibility {
    #[default]
    Visible,
    Hidden,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CompactView {
    /// Preference key for a compact component. One extension can provide
    /// multiple components under its own extension ID.
    pub id: ExtensionId,
    pub source_extension_id: ExtensionId,
    pub content: super::ViewNode,
    #[serde(default)]
    pub visibility: CompactVisibility,
    pub min_width: f32,
    pub preferred_width: f32,
    /// Grow to the measured content width instead of compressing this item.
    #[serde(default)]
    pub fit_width: bool,
    pub priority: i32,
    pub tooltip: Option<String>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Toggle,
    Advance,
    Reset,
    Select(String),
    SetText {
        key: String,
        value: String,
    },
    /// Normalized slider position, from 0 to 10,000 basis points.
    SetProgress {
        key: String,
        basis_points: u16,
    },
    Dismiss,
    Custom(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AlertInteraction {
    Close,
    Acknowledge,
    RunAction(NotificationAction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActionInvocation {
    pub source_extension_id: ExtensionId,
    pub action: Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Event {
    NotificationPublished,
    Custom(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtensionOutput {
    pub events: Vec<Event>,
    pub mascot_events: Vec<String>,
    pub invalidated: Vec<ExtensionId>,
    pub notifications: Vec<Notification>,
    pub resolved_notifications: Vec<super::NotificationKey>,
    pub schedule: Vec<ScheduledTask>,
    pub cancel_tasks: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledTask {
    pub task_id: String,
    pub deadline: std::time::Instant,
}

/// Notification extensions supply data for both surfaces. The App owns the
/// egui layout and never invokes extension business code while painting.
pub trait NotificationPresentationProvider {
    fn alert_view(&self, notification: &Notification) -> NotificationAlertView {
        super::NotificationPresentation::from_notification(notification).alert
    }
    fn center_item_view(&self, notification: &Notification) -> NotificationCenterItemView {
        super::NotificationPresentation::from_notification(notification).center_item
    }
    fn on_notification_action(
        &mut self,
        _notification: &Notification,
        _action: &NotificationAction,
    ) -> Result<(), String> {
        Ok(())
    }
    fn handles_notification(&self, _source_id: &ExtensionId) -> bool {
        false
    }
}

/// In-process extension runtime used by the built-in extensions. Its surface
/// is data-only so the App can render views without knowing feature IDs.
pub trait RuntimeExtension: NotificationPresentationProvider {
    fn descriptor(&self) -> super::ExtensionDescriptor;
    fn registration_descriptor(&self) -> super::ExtensionDescriptor {
        let d = self.descriptor();
        let service = !matches!(
            d.execution_policy,
            super::ExecutionPolicy::EventDriven | super::ExecutionPolicy::WhenVisible
        ) || d
            .contributions
            .iter()
            .any(|c| c.capabilities.contains(&super::Capability::Schedule));
        let events = d.contributions.iter().any(|c| {
            c.capabilities.contains(&super::Capability::Event)
                || c.capabilities.contains(&super::Capability::Schedule)
        });
        d.with_runtime_contributions(service, events, &self.mascot_expressions())
    }
    fn set_motion_policy(&mut self, _policy: crate::core::mascot::MotionPolicy) {}
    fn mascot_state(&self) -> std::collections::BTreeMap<String, String> {
        Default::default()
    }
    fn mascot_expressions(&self) -> Vec<crate::core::mascot::MascotExpression> {
        Vec::new()
    }
    fn on_install(&mut self, _ctx: &super::ExtensionContext) -> Result<(), String> {
        Ok(())
    }
    fn on_upgrade(
        &mut self,
        _ctx: &super::ExtensionContext,
        _old_version: &str,
    ) -> Result<(), String> {
        Ok(())
    }
    fn on_enable(&mut self, _ctx: &super::ExtensionContext) -> Result<(), String> {
        Ok(())
    }
    fn on_start(&mut self, _ctx: &super::ExtensionContext) -> Result<(), String> {
        Ok(())
    }
    fn on_stop(&mut self, _ctx: &super::ExtensionContext) -> Result<(), String> {
        Ok(())
    }
    fn on_disable(&mut self, _ctx: &super::ExtensionContext) -> Result<(), String> {
        Ok(())
    }
    fn on_uninstall(&mut self, _ctx: &super::ExtensionContext) -> Result<(), String> {
        Ok(())
    }
    fn update(&mut self, _now: std::time::Instant) -> ExtensionOutput {
        ExtensionOutput::default()
    }
    fn on_event(&mut self, _event: &Event, _now: std::time::Instant) -> ExtensionOutput {
        ExtensionOutput::default()
    }
    fn widget_view(&self, _widget_id: &ExtensionId) -> Option<WidgetView> {
        None
    }
    fn widget_visibility(&self, _widget_id: &ExtensionId) -> WidgetVisibility {
        WidgetVisibility::Visible
    }
    fn compact_refresh_interval(&self) -> Option<std::time::Duration> {
        None
    }

    fn compact_views(&self) -> Vec<CompactView> {
        Vec::new()
    }
    fn on_action(&mut self, _action: Action, _now: std::time::Instant) -> ExtensionOutput {
        ExtensionOutput::default()
    }
    /// Receives a validated snapshot. Restart-only changes take effect at the
    /// next launch; Reload lets the extension reload any owned resources.
    fn on_settings_changed(
        &mut self,
        _ctx: &super::ExtensionContext,
        _id: &ExtensionId,
        _settings: &Settings,
        _policy: ReloadPolicy,
    ) -> ExtensionOutput {
        ExtensionOutput::default()
    }
    fn status_message(&self) -> Option<String> {
        None
    }
    fn on_notifications_committed(&mut self, _notifications: &[super::Notification]) {}
}

pub trait HostScheduler {
    fn invalidate(&mut self, extension_id: &super::ExtensionId);
    fn schedule_at(&mut self, task_id: &str, deadline: std::time::Instant);
    fn cancel(&mut self, task_id: &str);
    fn on_event(&mut self, event: Event);
}
