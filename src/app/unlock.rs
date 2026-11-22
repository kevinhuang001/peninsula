use super::{capsule, capsule_layout, IslandApp, WINDOW_TOP_PADDING};
use crate::{
    core::mascot::{MascotMotion, MotionPolicy, UnlockMascotState},
    sdk::secrets::{SecretStore, SecretsError},
};
use eframe::egui;
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

pub(super) const WIDTH: f32 = 270.0;
const TYPING_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub(super) struct UnlockView {
    password: String,
    task: Option<Receiver<Result<(), SecretsError>>>,
    focused_once: bool,
    last_edit: Option<Instant>,
    failed: Option<(Instant, &'static str)>,
    success_at: Option<Instant>,
}

impl UnlockView {
    pub(super) fn visible(&self, locked: bool) -> bool {
        locked || self.task.is_some()
    }

    pub(super) fn handoff(&self, now: Instant, policy: MotionPolicy) -> Option<UnlockHandoff> {
        self.success_at.map(|start| {
            let duration = match policy {
                MotionPolicy::Full => Duration::from_millis(440),
                MotionPolicy::Reduced => Duration::from_millis(200),
                MotionPolicy::Off => Duration::ZERO,
            };
            let amount = if duration.is_zero() {
                1.0
            } else {
                (now.saturating_duration_since(start).as_secs_f32() / duration.as_secs_f32())
                    .min(1.0)
            };
            UnlockHandoff { amount }
        })
    }

    fn mascot_state(&self, focused: bool, now: Instant) -> UnlockMascotState {
        if self.task.is_some() {
            UnlockMascotState::Verifying
        } else if self.failed.is_some() {
            UnlockMascotState::Failed
        } else if focused {
            if self
                .last_edit
                .is_some_and(|last| now < last + TYPING_TIMEOUT)
            {
                UnlockMascotState::Typing
            } else {
                UnlockMascotState::Attentive
            }
        } else {
            UnlockMascotState::Idle
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct UnlockHandoff {
    amount: f32,
}

impl UnlockHandoff {
    pub(super) fn finished(self) -> bool {
        self.amount >= 1.0
    }

    pub(super) fn width(self, capsule_width: f32) -> f32 {
        egui::lerp(WIDTH..=capsule_width, self.travel())
    }

    pub(super) fn travel(self) -> f32 {
        ease(self.amount)
    }

    pub(super) fn status_opacity(self) -> f32 {
        1.0 - ease((self.amount - 0.12) / 0.55)
    }

    pub(super) fn items_opacity(self) -> f32 {
        ease((self.amount - 0.24) / 0.76)
    }
}

fn ease(amount: f32) -> f32 {
    let amount = amount.clamp(0.0, 1.0);
    amount * amount * (3.0 - 2.0 * amount)
}

pub(super) fn mascot_rect(rect: egui::Rect) -> egui::Rect {
    egui::Rect::from_center_size(
        rect.left_center() + egui::vec2(27.0, 0.0),
        egui::vec2(
            crate::core::mascot::SLOT_WIDTH - 6.0,
            capsule_layout::CHIP_HEIGHT,
        ),
    )
}

impl IslandApp {
    pub(super) fn mascot_policy(&self) -> MotionPolicy {
        if self.preferences.motion_off || self.preferences.mascot_motion == "Off" {
            MotionPolicy::Off
        } else if self.preferences.reduced_motion || self.preferences.mascot_motion == "Reduced" {
            MotionPolicy::Reduced
        } else {
            MotionPolicy::Full
        }
    }

    pub(super) fn poll_unlock_task(&mut self, context: &egui::Context, now: Instant) {
        let Some(receiver) = &self.unlock.task else {
            return;
        };
        let message = match receiver.try_recv() {
            Ok(Ok(())) => {
                self.unlock.password.clear();
                self.unlock.success_at = Some(now);
                None
            }
            Ok(Err(SecretsError::InvalidPassword)) => Some("密码错误或验证数据已损坏"),
            Ok(Err(_)) | Err(TryRecvError::Disconnected) => Some("无法读取或验证安全存储"),
            Err(TryRecvError::Empty) => return,
        };
        self.unlock.task = None;
        self.unlock.failed = message.map(|message| (now, message));
        if message.is_some() {
            self.unlock.focused_once = false;
        }
        context.request_repaint();
    }

    pub(super) fn show_unlock_capsule(&mut self, context: &egui::Context, now: Instant) {
        let policy = self.mascot_policy();
        let state = &mut self.unlock;
        let height = capsule_layout::CAPSULE_HEIGHT;
        let verifying = state.task.is_some();
        let editable = !verifying;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(context, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(WINDOW_TOP_PADDING);
                    let (base, _) =
                        ui.allocate_exact_size(egui::vec2(WIDTH, height), egui::Sense::hover());
                    let mut rect = base;
                    if let Some((start, _)) = state.failed {
                        let elapsed = now.saturating_duration_since(start);
                        let duration = MascotMotion::Error.duration();
                        if elapsed >= duration {
                            state.failed = None;
                        } else {
                            context.request_repaint_after(duration - elapsed);
                            let magnitude = match policy {
                                MotionPolicy::Full => 9.0,
                                MotionPolicy::Reduced => 2.5,
                                MotionPolicy::Off => 0.0,
                            };
                            if magnitude > 0.0 {
                                let seconds = elapsed.as_secs_f32();
                                let decay = (1.0 - seconds / duration.as_secs_f32()).powi(3);
                                rect = base.translate(egui::vec2(
                                    magnitude * decay * (seconds * 42.0).sin(),
                                    0.0,
                                ));
                            }
                        }
                    }
                    capsule::paint_shell(ui, rect, 0.0, false);
                    let input_rect = egui::Rect::from_center_size(
                        rect.center() + egui::vec2(-16.0, 0.0),
                        egui::vec2(130.0, 24.0),
                    );
                    ui.painter()
                        .rect_filled(input_rect, 5.0, ui.visuals().extreme_bg_color);
                    let response = ui.put(
                        input_rect,
                        egui::TextEdit::singleline(&mut state.password)
                            .id_salt("vault-password")
                            .password(true)
                            .interactive(editable)
                            .font(egui::FontId::proportional(12.0))
                            .frame(false)
                            .hint_text("输入密码解锁"),
                    );
                    if !state.focused_once {
                        state.focused_once = true;
                        response.request_focus();
                    }
                    if response.changed() {
                        state.failed = None;
                        state.last_edit = Some(now);
                    }
                    if let Some((_, message)) = state.failed {
                        let color = egui::Color32::from_rgb(228, 118, 125);
                        ui.painter().rect_stroke(
                            input_rect.expand(1.5),
                            6.0,
                            egui::Stroke::new(2.0_f32, color),
                            egui::StrokeKind::Inside,
                        );
                        ui.painter().text(
                            input_rect.center_bottom() + egui::vec2(0.0, 2.0),
                            egui::Align2::CENTER_TOP,
                            message,
                            egui::FontId::proportional(10.0),
                            color,
                        );
                    }
                    let confirm = egui::Rect::from_center_size(
                        rect.center() + egui::vec2(79.0, 0.0),
                        egui::vec2(50.0, 22.0),
                    );
                    let mut controls = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("vault-confirm")
                            .max_rect(confirm),
                    );
                    if !editable {
                        controls.disable();
                    }
                    let button = controls.put(
                        confirm,
                        egui::Button::new(
                            egui::RichText::new(if verifying { "验证中" } else { "确认" })
                                .size(12.0),
                        ),
                    );
                    let enter = (response.has_focus() || response.lost_focus())
                        && context.input(|input| input.key_pressed(egui::Key::Enter));
                    if editable && (button.clicked() || enter) {
                        state.task = Some(submit(
                            self.secret_store.clone(),
                            state.password.clone(),
                            context.clone(),
                        ));
                        state.failed = None;
                        context.request_repaint();
                    }

                    if self.preferences.mascot_enabled
                        && ui.is_rect_visible(rect)
                        && !context.input(|input| input.viewport().minimized.unwrap_or(false))
                    {
                        let mascot_state = state.mascot_state(response.has_focus(), now);
                        if let Some(delay) = self.mascot.update_unlock(mascot_state, now, policy) {
                            context.request_repaint_after(delay);
                        }
                        if let Some(edit) = state
                            .last_edit
                            .filter(|_| mascot_state == UnlockMascotState::Typing)
                        {
                            context.request_repaint_after(
                                (edit + TYPING_TIMEOUT).saturating_duration_since(now),
                            );
                        }
                        let _ = crate::ui::renderer::show_capsule(
                            ui,
                            mascot_rect(rect),
                            &crate::extension::ExtensionId::builtin(
                                crate::core::mascot::MASCOT_SLOT_ID,
                            ),
                            &self.mascot.node(),
                            &self.icons,
                            1.0,
                        );
                    }
                });
            });
    }
}

fn submit(
    store: SecretStore,
    password: String,
    context: egui::Context,
) -> Receiver<Result<(), SecretsError>> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(store.unlock(&password));
        context.request_repaint();
    });
    receiver
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlock_interactions_have_precedence_and_do_not_expose_password() {
        let now = Instant::now();
        let mut view = UnlockView {
            last_edit: Some(now),
            ..UnlockView::default()
        };
        assert_eq!(view.mascot_state(false, now), UnlockMascotState::Idle);
        assert_eq!(view.mascot_state(true, now), UnlockMascotState::Typing);
        assert_eq!(
            view.mascot_state(true, now + TYPING_TIMEOUT),
            UnlockMascotState::Attentive
        );
        view.failed = Some((now, "error"));
        assert_eq!(view.mascot_state(true, now), UnlockMascotState::Failed);
        let (_sender, receiver) = mpsc::channel();
        view.task = Some(receiver);
        assert!(view.visible(false));
        assert_eq!(view.mascot_state(true, now), UnlockMascotState::Verifying);
        view.task = None;
        assert!(!view.visible(false));
    }

    #[test]
    fn handoff_uses_the_unlock_and_capsule_endpoints_and_respects_motion_off() {
        let now = Instant::now();
        let view = UnlockView {
            success_at: Some(now),
            ..UnlockView::default()
        };
        let start = view.handoff(now, MotionPolicy::Full).unwrap();
        assert_eq!(start.width(190.0), WIDTH);
        assert_eq!(start.items_opacity(), 0.0);
        assert_eq!(start.status_opacity(), 1.0);
        let middle = view
            .handoff(now + Duration::from_millis(220), MotionPolicy::Full)
            .unwrap();
        assert!(middle.width(190.0) < WIDTH && middle.width(190.0) > 190.0);
        assert!(middle.items_opacity() > 0.0 && middle.status_opacity() > 0.0);
        let end = view
            .handoff(now + Duration::from_millis(440), MotionPolicy::Full)
            .unwrap();
        assert!(end.finished());
        assert_eq!(end.width(190.0), 190.0);
        let off = view.handoff(now, MotionPolicy::Off).unwrap();
        assert!(off.finished());
        assert_eq!(off.width(190.0), 190.0);
    }
}
