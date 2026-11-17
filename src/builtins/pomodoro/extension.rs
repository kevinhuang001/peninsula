use super::timer::{Phase, Pomodoro};
use crate::extension::{
    Action, Event, ExtensionDescriptor, ExtensionId, ExtensionKind, ExtensionOutput, Notification,
    NotificationAction, NotificationAlertView, NotificationCenterItemView,
    NotificationPresentationProvider, NotificationPriority, RefreshHint, RuntimeExtension,
    ScheduledTask, ViewNode, WidgetView,
};
use std::time::{Instant, SystemTime};

pub const WIDGET_ID: &str = "builtin.pomodoro/timer";
pub const NOTIFICATION_ID: &str = "builtin.pomodoro/notifications";
const TIMER_TASK: &str = "builtin.pomodoro/timer";

pub fn descriptors() -> Vec<ExtensionDescriptor> {
    vec![
        ExtensionDescriptor::builtin(WIDGET_ID, ExtensionKind::Widget, "番茄钟", "timer", true, 2)
            .with_size_hint(300.0, 126.0)
            .with_capabilities(vec![
                crate::extension::Capability::View,
                crate::extension::Capability::Action,
                crate::extension::Capability::Schedule,
                crate::extension::Capability::Event,
            ]),
        ExtensionDescriptor::builtin(
            NOTIFICATION_ID,
            ExtensionKind::Notification,
            "番茄钟提醒",
            "timer",
            true,
            1,
        )
        .with_capabilities(vec![
            crate::extension::Capability::Event,
            crate::extension::Capability::Action,
        ]),
    ]
}

#[derive(Default)]
pub struct PomodoroExtension {
    timer: Pomodoro,
    completion_sequence: u64,
}

impl PomodoroExtension {
    fn action_output(&self, old_deadline: Option<Instant>) -> ExtensionOutput {
        let deadline = self.timer.deadline();
        let mut output = ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            ..ExtensionOutput::default()
        };
        if old_deadline.is_some() && old_deadline != deadline {
            output.cancel_tasks.push(TIMER_TASK.into());
        }
        if let Some(deadline) = deadline.filter(|deadline| Some(*deadline) != old_deadline) {
            output.schedule.push(ScheduledTask {
                task_id: TIMER_TASK.into(),
                deadline,
            });
        }
        output
    }

    fn completion_notification(&mut self, completed_phase: Phase) -> Notification {
        self.completion_sequence += 1;
        let (title, body) = match completed_phase {
            Phase::Focus => ("专注完成", "专注时段已结束，可以休息一下。"),
            Phase::Break => ("休息结束", "休息时段已结束，可以开始下一轮专注。"),
        };
        Notification {
            source_id: ExtensionId::builtin(NOTIFICATION_ID),
            id: format!("pomodoro-{}", self.completion_sequence),
            title: title.into(),
            body: body.into(),
            icon: Some("timer".into()),
            priority: NotificationPriority::Normal,
            actions: Vec::new(),
            created_at: SystemTime::now(),
        }
    }
}

impl RuntimeExtension for PomodoroExtension {
    fn descriptors(&self) -> Vec<ExtensionDescriptor> {
        descriptors()
    }

    fn widget_view(&self, widget_id: &ExtensionId) -> Option<WidgetView> {
        if widget_id.as_str() != WIDGET_ID {
            return None;
        }
        let now = Instant::now();
        let phase = self.timer.phase;
        Some(WidgetView {
            root: ViewNode::Column(vec![
                ViewNode::ProgressRing {
                    fraction: self.timer.progress_at(now),
                    label: Some(format_time(self.timer.remaining_at(now))),
                },
                ViewNode::Text {
                    text: if phase == Phase::Focus {
                        "专注中"
                    } else {
                        "休息中"
                    }
                    .into(),
                    style: crate::extension::TextStyle::Small,
                },
                ViewNode::Row(vec![
                    ViewNode::Button {
                        label: "专注".into(),
                        action: Action::Select("focus".into()),
                    },
                    ViewNode::Button {
                        label: "休息".into(),
                        action: Action::Select("break".into()),
                    },
                    ViewNode::Button {
                        label: if self.timer.running {
                            "暂停"
                        } else {
                            "开始"
                        }
                        .into(),
                        action: Action::Toggle,
                    },
                    ViewNode::Button {
                        label: "重置".into(),
                        action: Action::Reset,
                    },
                ]),
            ]),
            preferred_size: [300.0, 126.0],
            refresh_hint: RefreshHint::EverySecond,
        })
    }

    fn on_action(&mut self, action: Action, now: Instant) -> ExtensionOutput {
        let old_deadline = self.timer.deadline();
        match action {
            Action::Toggle => self.timer.toggle_at(now),
            Action::Reset => self.timer.reset(),
            Action::Select(phase) if phase == "focus" => self.timer.select_phase(Phase::Focus),
            Action::Select(phase) if phase == "break" => self.timer.select_phase(Phase::Break),
            _ => return ExtensionOutput::default(),
        }
        self.action_output(old_deadline)
    }

    fn set_pomodoro_durations(
        &mut self,
        focus_minutes: u16,
        break_minutes: u16,
    ) -> ExtensionOutput {
        let old_deadline = self.timer.deadline();
        if !self.timer.set_durations(focus_minutes, break_minutes) {
            return ExtensionOutput::default();
        }
        self.action_output(old_deadline)
    }

    fn on_event(&mut self, event: &Event, now: Instant) -> ExtensionOutput {
        if event != &Event::Custom(TIMER_TASK.into()) {
            return ExtensionOutput::default();
        }
        let completed_phase = self.timer.phase;
        if !self.timer.update(now) {
            return ExtensionOutput::default();
        }
        ExtensionOutput {
            invalidated: vec![ExtensionId::builtin(WIDGET_ID)],
            notifications: vec![self.completion_notification(completed_phase)],
            ..ExtensionOutput::default()
        }
    }
}

impl NotificationPresentationProvider for PomodoroExtension {
    fn alert_view(&self, notification: &Notification) -> NotificationAlertView {
        NotificationAlertView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("timer".into()),
            actions: notification.actions.clone(),
        }
    }

    fn center_item_view(&self, notification: &Notification) -> NotificationCenterItemView {
        NotificationCenterItemView {
            title: notification.title.clone(),
            body: notification.body.clone(),
            icon: Some("timer".into()),
        }
    }

    fn on_notification_action(
        &mut self,
        _notification: &Notification,
        _action: &NotificationAction,
    ) -> Result<(), String> {
        Ok(())
    }

    fn handles_notification(&self, source_id: &ExtensionId) -> bool {
        source_id.as_str() == NOTIFICATION_ID
    }
}

fn format_time(seconds: f32) -> String {
    let total = seconds.max(0.0).ceil() as u32;
    format!("{:02}:{:02}", total / 60, total % 60)
}
