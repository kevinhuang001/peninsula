use super::{
    ExtensionId, Notification, NotificationAction, NotificationAlertView,
    NotificationCenterItemView,
};

#[derive(Clone, Debug, PartialEq)]
pub struct WidgetView {
    pub root: ViewNode,
    pub preferred_size: [f32; 2],
    pub refresh_hint: RefreshHint,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RefreshHint {
    #[default]
    OnInvalidation,
    EverySecond,
    Static,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StatCell {
    pub label: String,
    pub value: String,
    pub icon: Option<String>,
    pub color: ColorRole,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ViewNode {
    Row(Vec<ViewNode>),
    Column(Vec<ViewNode>),
    Text {
        text: String,
        style: TextStyle,
    },
    Icon {
        name: String,
        color: ColorRole,
    },
    Metric {
        label: String,
        value: String,
        color: ColorRole,
    },
    MetricCard {
        label: String,
        value: String,
        subtitle: Option<String>,
        icon: Option<String>,
        color: ColorRole,
    },
    /// Small rounded label, useful for status and warning summaries.
    Badge {
        text: String,
        icon: Option<String>,
        color: ColorRole,
    },
    /// A compact grid of labelled values, three columns wide.
    StatGrid {
        cells: Vec<StatCell>,
    },
    /// A range chart with a filled band between a high and a low series.
    RangeChart {
        labels: Vec<String>,
        high: Vec<f32>,
        low: Vec<f32>,
    },
    /// A single rounded bar of compact labelled values.
    InfoBar {
        cells: Vec<StatCell>,
    },
    /// Forces a child to a fixed height inside a column.
    Sized {
        height: f32,
        child: Box<ViewNode>,
    },
    Divider,
    NotificationItem {
        stable_id: String,
        title: String,
        body: String,
        icon: Option<String>,
    },
    Button {
        label: String,
        action: Action,
    },
    ProgressRing {
        fraction: f32,
        label: Option<String>,
        color: ColorRole,
    },
    Sprite {
        asset: String,
        size: [f32; 2],
    },
    Spacer {
        size: [f32; 2],
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TextStyle {
    #[default]
    Body,
    Display,
    Title,
    Heading,
    Muted,
    Small,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ColorRole {
    #[default]
    Primary,
    Muted,
    Accent,
    Success,
    Warning,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompactView {
    /// Preference key for a compact component. One extension can provide
    /// multiple components under its own extension ID.
    pub id: ExtensionId,
    pub source_extension_id: ExtensionId,
    pub content: CompactNode,
    pub min_width: f32,
    pub preferred_width: f32,
    pub priority: i32,
    pub tooltip: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CompactNode {
    Icon {
        name: String,
        color: ColorRole,
    },
    Text {
        text: String,
        color: ColorRole,
    },
    IconText {
        icon: String,
        text: String,
        color: ColorRole,
    },
    Progress {
        fraction: f32,
        label: String,
        color: ColorRole,
    },
    Sprite {
        asset: String,
        size: [f32; 2],
    },
    Row(Vec<CompactNode>),
    Column(Vec<CompactNode>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Toggle,
    Advance,
    Reset,
    Select(String),
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
    SnapshotUpdated,
    TimerElapsed,
    DownloadCompleted,
    NotificationPublished,
    Custom(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtensionOutput {
    pub invalidated: Vec<ExtensionId>,
    pub notifications: Vec<Notification>,
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
    fn descriptors(&self) -> Vec<super::ExtensionDescriptor>;
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
    fn compact_views(&self) -> Vec<CompactView> {
        Vec::new()
    }
    fn on_action(&mut self, _action: Action, _now: std::time::Instant) -> ExtensionOutput {
        ExtensionOutput::default()
    }
    fn set_pomodoro_durations(
        &mut self,
        _focus_minutes: u16,
        _break_minutes: u16,
    ) -> ExtensionOutput {
        ExtensionOutput::default()
    }
    fn configure_bilibili(
        &mut self,
        _ctx: &super::ExtensionContext,
        _opencli_path: &str,
        _dm_enabled: bool,
        _likes_enabled: bool,
        _show_dm_summary: bool,
        _poll_seconds: u16,
    ) -> Result<(), String> {
        Ok(())
    }
    fn status_message(&self) -> Option<String> {
        None
    }
    fn request_bilibili_check(&mut self, _ctx: &super::ExtensionContext) -> Result<(), String> {
        Err("this extension does not support manual checks".into())
    }
    fn on_notifications_committed(&mut self, _notifications: &[super::Notification]) {}
    /// Optional typed access used by the built-in host to push preferences
    /// into a feature without making the extension contract feature-specific.
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

pub trait HostScheduler {
    fn invalidate(&mut self, extension_id: &super::ExtensionId);
    fn schedule_at(&mut self, task_id: &str, deadline: std::time::Instant);
    fn cancel(&mut self, task_id: &str);
    fn on_event(&mut self, event: Event);
}

#[cfg(test)]
mod tests {
    use super::{CompactNode, CompactView, ExtensionId, ViewNode};

    #[test]
    fn compact_contract_carries_layout_preferences_without_ui_objects() {
        let view = CompactView {
            id: ExtensionId::builtin("builtin.system-monitor/cpu"),
            source_extension_id: ExtensionId::builtin("builtin.system-monitor/overview"),
            content: CompactNode::IconText {
                icon: "cpu".into(),
                text: "12%".into(),
                color: Default::default(),
            },
            min_width: 40.0,
            preferred_width: 56.0,
            priority: 1,
            tooltip: Some("CPU usage".into()),
        };
        assert_eq!(view.preferred_width, 56.0);
        assert!(matches!(
            ViewNode::Spacer { size: [8.0, 0.0] },
            ViewNode::Spacer { .. }
        ));
    }
}
