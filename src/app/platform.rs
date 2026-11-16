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
pub(super) fn resize_anchored(frame: &eframe::Frame, size: [f32; 2]) -> bool {
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
    let next = anchored_frame(window.frame(), size);
    unsafe { window.setFrame_display_animate(next, true, false) };
    true
}

#[cfg(target_os = "macos")]
pub(super) fn update_hit_region(frame: &eframe::Frame, rect: Option<eframe::egui::Rect>) {
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
    window.setIgnoresMouseEvents(!inside);
}

#[cfg(not(target_os = "macos"))]
pub(super) fn update_hit_region(_frame: &eframe::Frame, _rect: Option<eframe::egui::Rect>) {}

#[cfg(target_os = "macos")]
fn anchored_frame(mut frame: objc2_foundation::NSRect, size: [f32; 2]) -> objc2_foundation::NSRect {
    let width = f64::from(size[0]);
    let height = f64::from(size[1]);
    frame.origin.x += (frame.size.width - width) * 0.5;
    frame.origin.y += frame.size.height - height;
    frame.size.width = width;
    frame.size.height = height;
    frame
}

#[cfg(not(target_os = "macos"))]
pub(super) fn configure_floating_window(_frame: &eframe::Frame) -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
pub(super) fn resize_anchored(_frame: &eframe::Frame, _size: [f32; 2]) -> bool {
    false
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::anchored_frame;
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    #[test]
    fn resizing_preserves_horizontal_center_and_top_edge_atomically() {
        let original = NSRect::new(NSPoint::new(620.0, 800.0), NSSize::new(190.0, 64.0));
        let expanded = anchored_frame(original, [420.0, 180.0]);
        assert_eq!(expanded.origin.x + expanded.size.width * 0.5, 715.0);
        assert_eq!(expanded.origin.y + expanded.size.height, 864.0);
    }
}
