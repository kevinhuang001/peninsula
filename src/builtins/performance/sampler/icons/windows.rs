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
            let bgra = render(info.hIcon)?;
            let rgba = bgra
                .chunks_exact(4)
                .flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]])
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

unsafe fn render(icon: HICON) -> Option<Vec<u8>> {
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
    pixels.fill(0);
    let drawn = DrawIconEx(dc, 0, 0, icon, 32, 32, 0, HBRUSH::default(), DI_NORMAL).is_ok();
    let _ = GdiFlush();
    let result = drawn.then(|| pixels.to_vec());
    SelectObject(dc, previous);
    let _ = DeleteObject(bitmap);
    let _ = DeleteDC(dc);
    result
}
