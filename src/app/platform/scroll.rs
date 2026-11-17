use super::drag::get_window;
use crate::app::trackpad::ScrollPhase;
use eframe::egui::{vec2, Vec2};
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSEvent, NSEventMask, NSEventPhase};
use std::{cell::RefCell, ptr::NonNull, rc::Rc};

pub(in crate::app) struct ScrollMonitor {
    monitor: Retained<AnyObject>,
    samples: Rc<RefCell<Vec<(ScrollPhase, Vec2)>>>,
}

impl ScrollMonitor {
    pub fn new(frame: &eframe::Frame) -> Option<Self> {
        let window = get_window(frame)?;
        let window_number = unsafe { window.windowNumber() };
        let samples = Rc::new(RefCell::new(Vec::new()));
        let pending = Rc::clone(&samples);
        let handler = block2::RcBlock::new(move |event: NonNull<NSEvent>| {
            let native = unsafe { event.as_ref() };
            if unsafe { native.windowNumber() } == window_number {
                let phase = unsafe { native.phase() };
                let momentum = unsafe { native.momentumPhase() };
                let scroll_phase = if !momentum.is_empty() {
                    Some(ScrollPhase::Momentum)
                } else if phase.intersects(NSEventPhase::Ended | NSEventPhase::Cancelled) {
                    Some(ScrollPhase::Ended)
                } else if phase.intersects(NSEventPhase::Began | NSEventPhase::MayBegin) {
                    Some(ScrollPhase::Began)
                } else if !phase.is_empty() {
                    Some(ScrollPhase::Changed)
                } else {
                    None
                };
                if let Some(phase) = scroll_phase {
                    let delta = unsafe {
                        vec2(
                            native.scrollingDeltaX() as f32,
                            native.scrollingDeltaY() as f32,
                        )
                    };
                    let mut pending = pending.borrow_mut();
                    if pending.len() < 256 {
                        pending.push((phase, delta));
                    }
                }
            }
            // Keep delivering scroll events to egui so vertical scrolling works normally.
            event.as_ptr()
        });
        let monitor = unsafe {
            NSEvent::addLocalMonitorForEventsMatchingMask_handler(
                NSEventMask::ScrollWheel,
                &handler,
            )
        }?;
        Some(Self { monitor, samples })
    }

    pub fn drain(&self) -> Vec<(ScrollPhase, Vec2)> {
        std::mem::take(&mut *self.samples.borrow_mut())
    }
}

impl Drop for ScrollMonitor {
    fn drop(&mut self) {
        unsafe { NSEvent::removeMonitor(&self.monitor) }
    }
}
