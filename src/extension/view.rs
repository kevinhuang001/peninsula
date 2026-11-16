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
    Button {
        label: String,
        action: Action,
    },
    ProgressRing {
        fraction: f32,
        label: Option<String>,
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
pub enum Event {
    SnapshotUpdated,
    TimerElapsed,
    DownloadCompleted,
    NotificationPublished,
    Custom(String),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActionResult {
    pub invalidated: bool,
    pub schedule: Vec<ScheduledTask>,
    pub cancel_tasks: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledTask {
    pub task_id: String,
    pub deadline: std::time::Instant,
}

pub trait WidgetExtension<C> {
    fn descriptor(&self) -> super::ExtensionDescriptor;
    fn view(&self, context: &C) -> WidgetView;
    fn on_action(&mut self, action: Action, context: &mut C) -> ActionResult;
}

pub trait CompactExtension<C> {
    fn descriptor(&self) -> super::ExtensionDescriptor;
    fn view(&self, context: &C) -> Option<CompactView>;
    fn on_action(&mut self, action: Action, context: &mut C) -> ActionResult;
}

pub trait NotificationPublisher {
    fn publish(&mut self, notification: super::Notification);
}

pub trait HostScheduler {
    fn invalidate(&mut self, extension_id: &super::ExtensionId);
    fn schedule_at(&mut self, task_id: &str, deadline: std::time::Instant);
    fn cancel(&mut self, task_id: &str);
    fn on_event(&mut self, event: Event);
}

#[cfg(test)]
mod tests {
    use super::{CompactNode, CompactView, ViewNode};

    #[test]
    fn compact_contract_carries_layout_preferences_without_ui_objects() {
        let view = CompactView {
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
