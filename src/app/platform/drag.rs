#[cfg(target_os = "macos")]
use objc2_app_kit::{NSEvent, NSView};
#[cfg(target_os = "macos")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[derive(Default)]
pub struct WindowDrag {
    #[cfg(target_os = "macos")]
    start: Option<([f64; 2], [f64; 2])>,
}

impl WindowDrag {
    pub fn begin(&mut self, frame: &eframe::Frame, ctx: &eframe::egui::Context) {
        #[cfg(target_os = "macos")]
        if let Some(window) = get_window(frame) {
            let cursor = unsafe { NSEvent::mouseLocation() };
            let origin = window.frame().origin;
            self.start = Some(([cursor.x, cursor.y], [origin.x, origin.y]));
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = frame;
            ctx.send_viewport_cmd(eframe::egui::ViewportCommand::StartDrag);
        }
        #[cfg(target_os = "macos")]
        let _ = ctx;
    }

    pub fn update(&mut self, frame: &eframe::Frame) -> bool {
        #[cfg(target_os = "macos")]
        {
            let Some((cursor_start, origin)) = self.start else {
                return false;
            };
            if unsafe { NSEvent::pressedMouseButtons() } & 1 == 0 {
                self.start = None;
                return false;
            }
            if let Some(window) = get_window(frame) {
                let cursor = unsafe { NSEvent::mouseLocation() };
                let new_origin = moved_origin(origin, cursor_start, [cursor.x, cursor.y]);
                let point = objc2_foundation::NSPoint::new(new_origin[0], new_origin[1]);
                unsafe { window.setFrameOrigin(point) };
            }
            true
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = frame;
            false
        }
    }
}

#[cfg(any(target_os = "macos", test))]
fn moved_origin(origin: [f64; 2], start: [f64; 2], current: [f64; 2]) -> [f64; 2] {
    [
        origin[0] + current[0] - start[0],
        origin[1] + current[1] - start[1],
    ]
}

#[cfg(target_os = "macos")]
pub(super) fn get_window(frame: &eframe::Frame) -> Option<objc2::rc::Retained<objc2_app_kit::NSWindow>> {
    let handle = frame.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
    view.window()
}

#[cfg(test)]
mod tests {
    use super::moved_origin;

    #[test]
    fn drag_can_move_canvas_below_visible_screen_to_place_capsule_over_dock() {
        let origin = moved_origin([400.0, 200.0], [500.0, 250.0], [550.0, 20.0]);
        assert_eq!(origin, [450.0, -30.0]);
    }
}
