use crate::ui::declarative::RasterImage;
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HANDLE,
        Graphics::Gdi::*,
        Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES,
        UI::{
            Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_SMALLICON},
            WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL, HICON},
        },
    },
};

struct ComApartment(bool);
impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { windows::Win32::System::Com::CoUninitialize() };
        }
    }
}

pub(super) fn load(executable: &Path) -> Option<RasterImage> {
    let path: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    unsafe {
        let _com = ComApartment(
            windows::Win32::System::Com::CoInitializeEx(
                None,
                windows::Win32::System::Com::COINIT_APARTMENTTHREADED,
            )
            .is_ok(),
        );
        let mut info = SHFILEINFOW::default();
        if SHGetFileInfoW(
            PCWSTR(path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_SMALLICON,
        ) == 0
        {
            return None;
        }
        let result = (|| {
            // Black/white compositing recovers alpha even for legacy mask icons.
            let black = render(info.hIcon, 0)?;
            let white = render(info.hIcon, 255)?;
            let rgba = black
                .chunks_exact(4)
                .zip(white.chunks_exact(4))
                .flat_map(|(b, w)| {
                    let alpha = 255 - (0..3).map(|i| w[i].saturating_sub(b[i])).max().unwrap_or(0);
                    let channel = |i: usize| {
                        if alpha == 0 {
                            0
                        } else {
                            (u32::from(b[i]) * 255 / u32::from(alpha)).min(255) as u8
                        }
                    };
                    [channel(2), channel(1), channel(0), alpha]
                })
                .collect();
            Some(RasterImage {
                size: [32, 32],
                rgba,
            })
        })();
        let _ = DestroyIcon(info.hIcon);
        result
    }
}

unsafe fn render(icon: HICON, background: u8) -> Option<Vec<u8>> {
    let dc = CreateCompatibleDC(HDC::default());
    if dc.0.is_null() {
        return None;
    }
    let mut bits = std::ptr::null_mut();
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: 32,
            biHeight: -32,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let bitmap = match CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, HANDLE::default(), 0)
    {
        Ok(bitmap) => bitmap,
        Err(_) => {
            let _ = DeleteDC(dc);
            return None;
        }
    };
    let previous = SelectObject(dc, bitmap);
    let pixels = std::slice::from_raw_parts_mut(bits.cast::<u8>(), 32 * 32 * 4);
    pixels.fill(background);
    let drawn = DrawIconEx(dc, 0, 0, icon, 32, 32, 0, HBRUSH::default(), DI_NORMAL).is_ok();
    let _ = GdiFlush();
    let result = drawn.then(|| pixels.to_vec());
    SelectObject(dc, previous);
    let _ = DeleteObject(bitmap);
    let _ = DeleteDC(dc);
    result
}
