use super::{compact_layout, IslandApp};
use crate::{
    core::mascot::{MascotMotion, MotionPolicy, UnlockMascotState},
    sdk::secrets::{SecretStore, SecretsError},
};
use eframe::egui;
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    time::{Duration, Instant},
};

const WIDTH: f32 = 270.0;
const TYPING_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Default)]
pub(super) struct UnlockView {
    password: String,
    task: Option<Receiver<Result<(), SecretsError>>>,
    focused_once: bool,
    last_edit: Option<Instant>,
    failed: Option<(Instant, &'static str)>,
    success_until: Option<Instant>,
}

impl UnlockView {
    pub(super) fn visible(&self, locked: bool, now: Instant) -> bool {
        locked || self.task.is_some() || self.success_until.is_some_and(|until| now < until)
    }

    fn mascot_state(&self, focused: bool, now: Instant) -> UnlockMascotState {
        if self.success_until.is_some() {
            UnlockMascotState::Succeeded
        } else if self.task.is_some() {
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
        if let Some(until) = self.unlock.success_until {
            if now >= until {
                self.unlock = UnlockView::default();
                self.state.begin_unlock_transition(WIDTH);
            } else {
                context.request_repaint_after(until - now);
            }
        }
        let Some(receiver) = &self.unlock.task else {
            return;
        };
        let message = match receiver.try_recv() {
            Ok(Ok(())) => {
                self.unlock.password.clear();
                self.unlock.success_until = Some(now + MascotMotion::Success.duration());
                context.request_repaint_after(MascotMotion::Success.duration());
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
        let height = compact_layout::CAPSULE_HEIGHT;
        let verifying = state.task.is_some();
        let succeeded = state.success_until.is_some();
        let editable = !verifying && !succeeded;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(context, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
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
                    ui.painter()
                        .rect_filled(rect, height * 0.5, ui.visuals().panel_fill);
                    ui.painter().rect_stroke(
                        rect,
                        height * 0.5,
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(43, 56, 76)),
                        egui::StrokeKind::Inside,
                    );
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
                            .hint_text(if succeeded {
                                "已解锁"
                            } else {
                                "输入密码解锁"
                            }),
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
                            egui::RichText::new(if verifying {
                                "验证中"
                            } else if succeeded {
                                "已解锁"
                            } else {
                                "确认"
                            })
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
                        let size = (crate::core::mascot::SLOT_WIDTH - 6.0).min(height - 6.0);
                        let mascot_rect = egui::Rect::from_center_size(
                            rect.left_center() + egui::vec2(27.0, 0.0),
                            egui::Vec2::splat(size),
                        );
                        let _ = crate::ui::renderer::show_compact(
                            ui,
                            mascot_rect,
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
        assert!(view.visible(false, now));
        assert_eq!(view.mascot_state(true, now), UnlockMascotState::Verifying);
        view.task = None;
        view.success_until = Some(now + MascotMotion::Success.duration());
        assert!(view.visible(false, now));
        assert_eq!(view.mascot_state(true, now), UnlockMascotState::Succeeded);
        assert!(!view.visible(false, now + MascotMotion::Success.duration()));
    }
}
