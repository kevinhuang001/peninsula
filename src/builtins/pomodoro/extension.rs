use super::timer::{Phase, Pomodoro};
use crate::extension::{
    Action, CompactView, CompactVisibility, ContributionDescriptor, ContributionKind, Event,
    ExtensionId, ExtensionOutput, Notification, NotificationAction, NotificationAlertView,
    NotificationCenterItemView, NotificationPresentationProvider, NotificationPriority,
    ReloadPolicy, RuntimeExtension, ScheduledTask, SettingSchema, SettingValue, Settings, ViewNode,
    WidgetView,
};
use crate::ui::declarative::*;
use std::time::{Instant, SystemTime};

pub const WIDGET_ID: &str = "builtin.pomodoro/timer";
pub const COMPACT_ID: &str = "builtin.pomodoro/status";
pub const NOTIFICATION_ID: &str = "builtin.pomodoro/notifications";
const TIMER_TASK: &str = "builtin.pomodoro/timer";

pub fn descriptors() -> Vec<ContributionDescriptor> {
    vec![
        ContributionDescriptor::builtin(
            COMPACT_ID,
            ContributionKind::Compact,
            "番茄钟进度",
            "timer",
            true,
            7,
        )
        .with_size_hint(88.0, 28.0),
        ContributionDescriptor::builtin(
            WIDGET_ID,
            ContributionKind::Widget,
            "番茄钟",
            "timer",
            true,
            2,
        )
        .with_size_hint(300.0, 126.0)
        .with_settings(
            1,
            vec![
                SettingSchema::new("focus_minutes", SettingValue::Integer(25), "专注分钟数")
                    .range(1.0, 180.0),
                SettingSchema::new("break_minutes", SettingValue::Integer(5), "休息分钟数")
                    .range(1.0, 180.0),
            ],
        )
        .with_capabilities(vec![
            crate::extension::Capability::View,
            crate::extension::Capability::Action,
            crate::extension::Capability::Schedule,
            crate::extension::Capability::Event,
        ]),
        ContributionDescriptor::builtin(
            NOTIFICATION_ID,
            ContributionKind::Notification,
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

pub struct PomodoroExtension {
    timer: Pomodoro,
    completion_sequence: u64,
    restored: bool,
    auto_start: bool,
}

impl Default for PomodoroExtension {
    fn default() -> Self {
        Self {
            timer: Pomodoro::default(),
            completion_sequence: 0,
            restored: false,
            auto_start: true,
        }
    }
}
impl PomodoroExtension {
    fn phase_color(&self) -> ColorSpec {
        if self.timer.phase == Phase::Focus {
            ColorSpec::Violet
        } else {
            ColorSpec::Success
        }
    }

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
            id: format!(
                "pomodoro-{}-{}",
                SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                self.completion_sequence
            ),
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
    fn descriptor(&self) -> crate::extension::ExtensionDescriptor {
        let mut d = crate::extension::ExtensionDescriptor::builtin(
            "builtin.pomodoro",
            "番茄钟",
            "专注与休息计时",
            descriptors(),
        );
        d.settings.push(SettingSchema::new(
            "auto_start",
            SettingValue::Boolean(true),
            "自动开始下一轮",
        ));
        d.execution_policy = crate::extension::ExecutionPolicy::EventDriven;
        d
    }
    fn on_start(&mut self, context: &crate::extension::ExtensionContext) -> Result<(), String> {
        if !self.restored {
            let path = context
                .extension_data_dir(&ExtensionId::builtin("builtin.pomodoro"))
                .join("timer.json");
            if let Ok(file) = std::fs::File::open(path) {
                if let Ok(saved) = serde_json::from_reader(file) {
                    self.timer.restore(saved);
                }
            }
            self.restored = true;
        }
        Ok(())
    }
    fn on_stop(&mut self, context: &crate::extension::ExtensionContext) -> Result<(), String> {
        if self.timer.running {
            self.timer.toggle_at(Instant::now());
        }
        let directory = context.extension_data_dir(&ExtensionId::builtin("builtin.pomodoro"));
        std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(&directory).map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut file, &self.timer.saved()).map_err(|e| e.to_string())?;
        file.persist(directory.join("timer.json"))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn mascot_state(&self) -> std::collections::BTreeMap<String, String> {
        [(
            "phase".into(),
            if self.timer.running {
                if self.timer.phase == Phase::Focus {
                    "focus"
                } else {
                    "break"
                }
            } else {
                "paused"
            }
            .into(),
        )]
        .into()
    }
    fn mascot_expressions(&self) -> Vec<crate::core::mascot::MascotExpression> {
        use crate::core::mascot::MascotExpression as E;
        vec![
            E::state(
                "focusing",
                "phase",
                "focus",
                crate::core::mascot::MascotMotion::Focused,
            ),
            E::state(
                "resting",
                "phase",
                "break",
                crate::core::mascot::MascotMotion::Sleepy,
            ),
            E::event(
                "celebrating",
                "timer.completed",
                crate::core::mascot::MascotMotion::Celebrate,
            ),
        ]
    }
    fn widget_view(&self, widget_id: &ExtensionId) -> Option<WidgetView> {
        if widget_id.as_str() != WIDGET_ID {
            return None;
        }
        let now = Instant::now();
        let phase = self.timer.phase;
        let button_size = (Size::Fixed(72.0), Size::Fixed(26.0));
        let mut root = ViewNode::row(
            vec![
                ViewNode::ring(
                    self.timer.progress_at(now),
                    format_time(self.timer.remaining_at(now)),
                    66.0,
                    3.0,
                    18.0,
                )
                .color(self.phase_color()),
                ViewNode::column(
                    vec![
                        ViewNode::text(
                            if phase == Phase::Focus {
                                "专注中"
                            } else {
                                "休息中"
                            },
                            TextStyle {
                                font_size: FontSize::Title,
                                font_weight: FontWeight::Bold,
                                align: Align::Center,
                                ..Default::default()
                            },
                        )
                        .sized(Size::Fill(1.0), Size::Auto),
                        ViewNode::row(
                            vec![
                                ViewNode::button(
                                    if self.timer.running {
                                        "暂停"
                                    } else {
                                        "开始"
                                    },
                                    Action::Toggle,
                                )
                                .sized(button_size.0, button_size.1),
                                ViewNode::subtle_button("重置", Action::Reset)
                                    .sized(button_size.0, button_size.1),
                            ],
                            Flow {
                                gap: Some(8.0),
                                justify: Justify::Center,
                                ..Default::default()
                            },
                        )
                        .sized(Size::Fill(1.0), Size::Auto),
                    ],
                    Flow {
                        gap: Some(7.0),
                        ..Default::default()
                    },
                )
                .sized(Size::Fixed(152.0), Size::Auto),
            ],
            Flow {
                gap: Some(22.0),
                justify: Justify::Center,
                ..Default::default()
            },
        );
        root.style.padding = [0.0, 5.0, 0.0, 5.0];
        let mut view = WidgetView::new(root, [300.0, 126.0]);
        view.bounds.fit_height = true;
        if self.timer.running {
            view.refresh_hint = RefreshHint::EverySecond;
        }
        Some(view)
    }

    fn compact_refresh_interval(&self) -> Option<std::time::Duration> {
        self.timer
            .running
            .then_some(std::time::Duration::from_millis(250))
    }

    fn compact_views(&self) -> Vec<CompactView> {
        let now = Instant::now();
        let progress = self.timer.progress_at(now);
        vec![CompactView {
            id: ExtensionId::builtin(COMPACT_ID),
            source_extension_id: ExtensionId::builtin(WIDGET_ID),
            content: ViewNode::compact(
                ViewNode::new(Node::Stack(
                    [Align::Center; 2],
                    vec![
                        ViewNode::ring(progress, "", 26.0, 1.8, 7.0)
                            .color(self.phase_color())
                            .motion(Motion::progress(progress, 190)),
                        ViewNode::text(
                            if self.timer.phase == Phase::Focus {
                                "专注"
                            } else {
                                "休息"
                            },
                            TextStyle {
                                font_size: FontSize::Custom(8.0),
                                font_weight: FontWeight::Bold,
                                ..Default::default()
                            },
                        ),
                    ],
                ))
                .sized(Size::Fixed(26.0), Size::Fixed(26.0)),
                ViewNode::column(
                    vec![
                        super::super::compact_value(
                            format_time(self.timer.remaining_at(now)),
                            true,
                        ),
                        super::super::compact_detail(if self.timer.running {
                            "剩余时间"
                        } else {
                            "已暂停"
                        }),
                    ],
                    Flow {
                        align: Align::Start,
                        gap: Some(0.0),
                        ..Default::default()
                    },
                )
                .color(ColorSpec::PrimaryText),
                CompactLayout {
                    leading_size: [26.0, 26.0],
                    gap: 6.0,
                    ..Default::default()
                },
            )
            .color(self.phase_color()),
            visibility: if self.timer.has_started() {
                CompactVisibility::Visible
            } else {
                CompactVisibility::Hidden
            },
            min_width: 88.0,
            preferred_width: 88.0,
            fit_width: false,
            priority: 7,
            tooltip: Some(
                if self.timer.running {
                    "番茄钟运行中"
                } else {
                    "番茄钟已暂停"
                }
                .into(),
            ),
        }]
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

    fn on_settings_changed(
        &mut self,
        _ctx: &crate::extension::ExtensionContext,
        _id: &ExtensionId,
        settings: &Settings,
        _policy: ReloadPolicy,
    ) -> ExtensionOutput {
        self.auto_start = settings.get("auto_start") == Some(&SettingValue::Boolean(true));
        let (Some(SettingValue::Integer(focus)), Some(SettingValue::Integer(pause))) =
            (settings.get("focus_minutes"), settings.get("break_minutes"))
        else {
            return ExtensionOutput::default();
        };
        let old_deadline = self.timer.deadline();
        if !self.timer.set_durations(*focus as u16, *pause as u16) {
            return ExtensionOutput::default();
        }
        self.action_output(old_deadline)
    }

    fn on_event(&mut self, event: &Event, now: Instant) -> ExtensionOutput {
        if event != &Event::Custom(TIMER_TASK.into()) {
            return ExtensionOutput::default();
        }
        let completed_phase = self.timer.phase;
        let old_deadline = self.timer.deadline();
        if !self.timer.update(now) {
            return ExtensionOutput::default();
        }
        if !self.auto_start {
            self.timer.toggle_at(now);
        }
        let mut output = self.action_output(old_deadline);
        output.mascot_events.push("timer.completed".into());
        output
            .notifications
            .push(self.completion_notification(completed_phase));
        output
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

#[cfg(test)]
mod compact_tests {
    use super::*;

    #[test]
    fn compact_is_hidden_until_start_and_after_reset() {
        let mut extension = PomodoroExtension::default();
        let now = Instant::now();
        assert_eq!(
            extension.compact_views()[0].visibility,
            CompactVisibility::Hidden
        );
        let output = extension.on_action(Action::Toggle, now);
        assert!(output
            .invalidated
            .contains(&ExtensionId::builtin(WIDGET_ID)));
        assert_eq!(
            extension.compact_views()[0].visibility,
            CompactVisibility::Visible
        );
        extension.on_action(Action::Toggle, now);
        assert_eq!(
            extension.compact_views()[0].visibility,
            CompactVisibility::Visible
        );
        extension.on_action(Action::Reset, now);
        assert_eq!(
            extension.compact_views()[0].visibility,
            CompactVisibility::Hidden
        );
    }
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn stopped_timer_is_restored_paused_and_auto_start_uses_a_new_deadline() {
        let root = tempfile::tempdir().unwrap();
        let ctx =
            crate::extension::ExtensionContext::new(root.path().into(), root.path().join("config"));
        let mut extension = PomodoroExtension::default();
        extension.on_start(&ctx).unwrap();
        extension.on_action(Action::Toggle, Instant::now());
        extension.on_stop(&ctx).unwrap();
        let mut restored = PomodoroExtension::default();
        restored.on_start(&ctx).unwrap();
        assert!(!restored.timer.running);
        assert!(restored.timer.remaining_at(Instant::now()) <= 25.0 * 60.0);
        restored.auto_start = true;
        let now = Instant::now();
        restored.on_action(Action::Toggle, now);
        let deadline = restored.timer.deadline().unwrap();
        let output = restored.on_event(&Event::Custom(TIMER_TASK.into()), deadline);
        assert!(restored.timer.running);
        assert_eq!(restored.timer.phase, Phase::Break);
        assert_eq!(output.schedule.len(), 1);
        assert_eq!(output.notifications.len(), 1);
    }
}
