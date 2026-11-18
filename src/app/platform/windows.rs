use eframe::egui::Rect;
#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "windows")]
use std::cell::Cell;
#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::HWND,
    Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn},
};

// SetWindowRgn affects both drawing and native hit testing. Transparent canvas
// outside the capsule must not intercept clicks or window drags in other apps.
#[cfg(target_os = "windows")]
pub(super) fn update_hit_region(
    frame: &eframe::Frame,
    rect: Option<Rect>,
    _dragging: bool,
    pixels_per_point: f32,
) {
    let Ok(handle) = frame.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let Some(rect) = rect else { return };
    let bounds = physical_region(rect, pixels_per_point);
    thread_local! {
        static PREVIOUS: Cell<Option<(isize, [i32; 5])>> = const { Cell::new(None) };
    }
    let key = (handle.hwnd.get(), bounds);
    PREVIOUS.with(|previous| {
        if previous.get() == Some(key) {
            return;
        }
        unsafe {
            let region = CreateRoundRectRgn(
                bounds[0], bounds[1], bounds[2], bounds[3], bounds[4], bounds[4],
            );
            if region.0.is_null() {
                return;
            }
            if SetWindowRgn(HWND(handle.hwnd.get() as *mut _), region, true) == 0 {
                // Windows owns the region only after a successful call.
                let _ = DeleteObject(region);
            } else {
                previous.set(Some(key));
            }
        }
    });
}

fn physical_region(rect: Rect, scale: f32) -> [i32; 5] {
    [
        (rect.left() * scale).floor() as i32,
        (rect.top() * scale).floor() as i32,
        (rect.right() * scale).ceil() as i32 + 1,
        (rect.bottom() * scale).ceil() as i32 + 1,
        (rect.height().min(64.0) * scale).round() as i32,
    ]
}

#[cfg(target_os = "windows")]
pub(super) fn pointer_inside(
    frame: &eframe::Frame,
    rect: Option<Rect>,
    _pointer: Option<eframe::egui::Pos2>,
    pixels_per_point: f32,
) -> bool {
    use windows::Win32::{
        Foundation::POINT, Graphics::Gdi::ScreenToClient, UI::WindowsAndMessaging::GetCursorPos,
    };
    let Ok(handle) = frame.window_handle() else {
        return false;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return false;
    };
    let mut point = POINT::default();
    unsafe {
        if GetCursorPos(&mut point).is_err()
            || !ScreenToClient(HWND(handle.hwnd.get() as *mut _), &mut point).as_bool()
        {
            return false;
        }
    }
    rect.is_some_and(|rect| {
        capsule_contains(
            rect,
            eframe::egui::pos2(
                point.x as f32 / pixels_per_point,
                point.y as f32 / pixels_per_point,
            ),
        )
    })
}

fn capsule_contains(rect: Rect, point: eframe::egui::Pos2) -> bool {
    if !rect.contains(point) {
        return false;
    }
    let radius = (rect.height().min(rect.width()) * 0.5).min(32.0);
    let center = eframe::egui::pos2(
        point.x.clamp(rect.left() + radius, rect.right() - radius),
        point.y.clamp(rect.top() + radius, rect.bottom() - radius),
    );
    point.distance_sq(center) <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_region_matches_capsule_at_fractional_dpi() {
        let rect = Rect::from_min_max(
            eframe::egui::pos2(100.5, 4.0),
            eframe::egui::pos2(380.5, 48.0),
        );
        assert_eq!(physical_region(rect, 1.5), [150, 6, 572, 73, 66]);
        assert!(capsule_contains(rect, rect.center()));
        assert!(!capsule_contains(rect, rect.min));
        assert!(!capsule_contains(
            rect,
            rect.min - eframe::egui::vec2(5.0, 5.0)
        ));
    }
}
