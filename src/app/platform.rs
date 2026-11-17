#[cfg(target_os = "macos")]
pub(super) fn configure_floating_window(frame: &eframe::Frame) -> bool {
    use objc2_app_kit::{NSStatusWindowLevel, NSView, NSWindowCollectionBehavior};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let Ok(handle) = frame.window_handle() else {
        return false;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return false;
    };
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
    let Some(window) = view.window() else {
        return false;
    };
    let current = unsafe { window.collectionBehavior() };
    unsafe {
        window.setCollectionBehavior(
            current
                | NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
    }
    window.setLevel(NSStatusWindowLevel);
    true
}

#[cfg(target_os = "macos")]
pub(super) fn update_hit_region(
    frame: &eframe::Frame,
    rect: Option<eframe::egui::Rect>,
    dragging: bool,
) {
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return;
    };
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
    let Some(window) = view.window() else { return };
    let point = unsafe { window.mouseLocationOutsideOfEventStream() };
    let height = window.frame().size.height;
    let local = eframe::egui::pos2(point.x as f32, (height - point.y) as f32);
    let inside = rect.is_some_and(|bounds| bounds.expand(2.0).contains(local));
    window.setIgnoresMouseEvents(!inside && !dragging);
}

#[cfg(target_os = "macos")]
pub(super) fn pointer_inside(
    frame: &eframe::Frame,
    rect: Option<eframe::egui::Rect>,
    _pointer: Option<eframe::egui::Pos2>,
) -> bool {
    use objc2_app_kit::NSView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = frame.window_handle() else {
        return false;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return false;
    };
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
    let Some(window) = view.window() else {
        return false;
    };
    let point = unsafe { window.mouseLocationOutsideOfEventStream() };
    let local = eframe::egui::pos2(
        point.x as f32,
        (window.frame().size.height - point.y) as f32,
    );
    rect.is_some_and(|bounds| bounds.contains(local))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn pointer_inside(
    _frame: &eframe::Frame,
    rect: Option<eframe::egui::Rect>,
    pointer: Option<eframe::egui::Pos2>,
) -> bool {
    pointer.is_some_and(|point| rect.is_some_and(|bounds| bounds.contains(point)))
}

#[cfg(not(target_os = "macos"))]
pub(super) fn update_hit_region(
    _frame: &eframe::Frame,
    _rect: Option<eframe::egui::Rect>,
    _dragging: bool,
) {
}

#[cfg(not(target_os = "macos"))]
pub(super) fn configure_floating_window(_frame: &eframe::Frame) -> bool {
    true
}
mod drag;
pub use drag::WindowDrag;

#[cfg(target_os = "macos")]
mod scroll;
#[cfg(target_os = "macos")]
pub(super) use scroll::ScrollMonitor;
