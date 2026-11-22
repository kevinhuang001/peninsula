use super::{
    DiagnosticCheck, ExtensionId, Notification, NotificationAction, NotificationAlertView,
    NotificationCenterItemView, ReloadPolicy, SearchOption, Settings, WidgetView,
};

pub type SettingSearchTask = Box<
    dyn FnOnce(crate::sdk::tasks::CancellationToken) -> Result<Vec<SearchOption>, String> + Send,
>;

/// Runtime visibility is evaluated separately from the user's Capsule preference.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CapsuleVisibility {
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
pub struct CapsuleView {
    /// Preference key for a capsule component. One extension can provide
    /// multiple components under its own extension ID.
    pub id: ExtensionId,
    pub source_extension_id: ExtensionId,
    pub content: super::ViewNode,
    #[serde(default)]
    pub visibility: CapsuleVisibility,
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

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
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
    pub prompts: Vec<Prompt>,
}

/// A one-shot, exclusive Capsule presentation. The host queues prompts in the
/// order it receives them and restores ordinary Capsule items when the queue
/// becomes empty.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Prompt {
    pub source_extension_id: ExtensionId,
    pub contribution_id: ExtensionId,
    pub content: super::ViewNode,
    pub duration: std::time::Duration,
    pub min_width: f32,
    pub preferred_width: f32,
    /// Independently positioned content on the whole capsule surface. An empty
    /// list keeps the original centered `content` presentation.
    #[serde(default)]
    pub layers: Vec<PromptLayer>,
    /// Optional color for the host's animated ambient light and outline.
    #[serde(default)]
    pub accent: Option<[u8; 3]>,
}

/// A Prompt layer occupies a fractional rectangle of the capsule, including
/// its end caps. Motion on the layer's ViewNode is independent of its position.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PromptLayer {
    pub frame: [f32; 4], // left, top, width, height; each in 0..=1
    pub content: super::ViewNode,
}

impl PromptLayer {
    pub fn new(frame: [f32; 4], content: super::ViewNode) -> Self {
        Self { frame, content }
    }
}

impl Prompt {
    pub fn new(
        source_extension_id: ExtensionId,
        contribution_id: ExtensionId,
        content: super::ViewNode,
        duration: std::time::Duration,
    ) -> Self {
        Self {
            source_extension_id,
            contribution_id,
            content,
            duration,
            min_width: 112.0,
            preferred_width: 152.0,
            layers: Vec::new(),
            accent: None,
        }
    }

    pub fn with_width(mut self, min_width: f32, preferred_width: f32) -> Self {
        self.min_width = min_width.max(1.0);
        self.preferred_width = preferred_width.max(self.min_width);
        self
    }

    pub fn with_layers(mut self, layers: Vec<PromptLayer>) -> Self {
        self.layers = layers;
        self
    }

    pub fn with_accent(mut self, accent: [u8; 3]) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn capsule_view(&self) -> CapsuleView {
        CapsuleView {
            id: self.contribution_id.clone(),
            source_extension_id: self.source_extension_id.clone(),
            content: self.content.clone(),
            visibility: CapsuleVisibility::Visible,
            min_width: self.min_width,
            preferred_width: self.preferred_width,
            fit_width: false,
            priority: 0,
            tooltip: None,
        }
    }
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
    fn capsule_refresh_interval(&self) -> Option<std::time::Duration> {
        None
    }

    fn capsule_views(&self) -> Vec<CapsuleView> {
        Vec::new()
    }
    /// Complete dynamic registration snapshot. None retains the last snapshot;
    /// an empty snapshot explicitly unregisters all dynamic contributions.
    fn dynamic_contributions(&self) -> Option<Vec<super::ContributionDescriptor>> {
        None
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
    /// Read retired extension-level settings during a contribution migration.
    fn import_legacy_settings(&mut self, _settings: &Settings) {}

    fn contribution_preference_migration(
        &self,
        _previous: &Settings,
    ) -> std::collections::BTreeMap<ExtensionId, bool> {
        Default::default()
    }

    /// Upgrade saved settings for the extension or one of its contributions.
    fn migrate_settings(
        &self,
        _id: &ExtensionId,
        old_version: u32,
        _old: Settings,
    ) -> Result<Settings, String> {
        Err(format!("no migration for settings version {old_version}"))
    }

    fn contribution_setup_complete(&self, _id: &ExtensionId, _settings: &Settings) -> bool {
        false
    }

    fn on_contribution_start(
        &mut self,
        _ctx: &super::ExtensionContext,
        _id: &ExtensionId,
    ) -> Result<(), String> {
        Ok(())
    }
    fn on_contribution_stop(
        &mut self,
        _ctx: &super::ExtensionContext,
        _id: &ExtensionId,
    ) -> Result<(), String> {
        Ok(())
    }
    fn on_contribution_update(
        &mut self,
        _ctx: &super::ExtensionContext,
        _id: &ExtensionId,
        _now: std::time::Instant,
    ) -> ExtensionOutput {
        ExtensionOutput::default()
    }

    fn on_contribution_event(
        &mut self,
        _ctx: &super::ExtensionContext,
        _id: &ExtensionId,
        _event: &Event,
        _now: std::time::Instant,
    ) -> ExtensionOutput {
        ExtensionOutput::default()
    }
    fn on_contribution_action(
        &mut self,
        _ctx: &super::ExtensionContext,
        _id: &ExtensionId,
        action: Action,
        now: std::time::Instant,
    ) -> ExtensionOutput {
        self.on_action(action, now)
    }

    /// Receive the host's current active contributions and validated settings.
    /// Inactive or unconfigured contributions are absent from the snapshot.
    fn on_contribution_state(
        &mut self,
        _ctx: &super::ExtensionContext,
        _active: &std::collections::BTreeMap<ExtensionId, Settings>,
    ) {
    }
    fn status_message(&self) -> Option<String> {
        None
    }
    /// Return cached checks only; probes and network calls belong on extension
    /// tasks and should update this snapshot when they finish.
    fn doctor(&self) -> Vec<DiagnosticCheck> {
        vec![DiagnosticCheck::passed(
            "extension-loaded",
            "扩展状态",
            "扩展已加载，暂无额外依赖检查。",
        )]
    }
    /// Construct a detached search task for the provider declared in a setting
    /// schema. The host runs it away from the UI thread.
    fn setting_search(&self, _provider: &str, _query: &str) -> Option<SettingSearchTask> {
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
