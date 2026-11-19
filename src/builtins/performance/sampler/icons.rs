use crate::ui::declarative::RasterImage;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct IconCache {
    images: HashMap<PathBuf, Option<RasterImage>>,
}
impl IconCache {
    pub fn get(&mut self, path: &Path) -> Option<RasterImage> {
        if !self.images.contains_key(path) {
            // Bound memory even on hosts with a frequently changing process list.
            if self.images.len() >= 128 {
                self.images.clear();
            }
            self.images.insert(path.to_owned(), load(path));
        }
        self.images.get(path).cloned().flatten()
    }
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::load;

#[cfg(target_os = "linux")]
fn decode(path: &Path) -> Option<RasterImage> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    if path.extension().is_some_and(|v| v == "svg") {
        let image = egui_extras::image::load_svg_bytes_with_size(
            &bytes,
            Some(eframe::egui::SizeHint::Size(32, 32)),
        )
        .ok()?;
        return Some(RasterImage {
            size: [image.size[0] as u16, image.size[1] as u16],
            rgba: image
                .pixels
                .iter()
                .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                .collect(),
        });
    }
    let image = image::load_from_memory(&bytes)
        .ok()?
        .resize_exact(32, 32, image::imageops::FilterType::Triangle)
        .into_rgba8();
    Some(RasterImage {
        size: [32, 32],
        rgba: image.into_raw(),
    })
}

#[cfg(target_os = "linux")]
fn load(executable: &Path) -> Option<RasterImage> {
    let name = executable.file_stem()?.to_str()?;
    let mut names = vec![name.to_owned()];
    let mut desktop_dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Some(home) = std::env::var_os("HOME") {
        desktop_dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    for dir in desktop_dirs {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if entry.path().extension().is_none_or(|v| v != "desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let exec = text.lines().find_map(|line| line.strip_prefix("Exec="));
            let matches = exec
                .and_then(|v| v.split_whitespace().next())
                .map(|v| Path::new(v.trim_matches('"')).file_stem() == executable.file_stem())
                .unwrap_or(false);
            if matches {
                if let Some(icon) = text.lines().find_map(|line| line.strip_prefix("Icon=")) {
                    if Path::new(icon).is_absolute() {
                        if let Some(image) = decode(Path::new(icon)) {
                            return Some(image);
                        }
                    }
                    names.push(icon.to_owned());
                }
            }
        }
    }
    for name in names {
        // Desktop entries provide catalog names, never directory traversal.
        if name.contains('/') {
            continue;
        }
        for theme in ["hicolor", "Adwaita"] {
            for size in ["32x32", "48x48", "scalable", "256x256"] {
                for extension in ["png", "svg"] {
                    let path = PathBuf::from(format!(
                        "/usr/share/icons/{theme}/{size}/apps/{name}.{extension}"
                    ));
                    if let Some(image) = decode(&path) {
                        return Some(image);
                    }
                }
            }
        }
        if let Some(image) = decode(&PathBuf::from(format!("/usr/share/pixmaps/{name}.png"))) {
            return Some(image);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn load(path: &Path) -> Option<RasterImage> {
    use objc2::rc::autoreleasepool;
    use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSWorkspace};
    use objc2_foundation::{NSDictionary, NSString};

    if !path.extension().is_some_and(|extension| extension == "app") || !path.is_dir() {
        return None;
    }
    autoreleasepool(|_| {
        let path = NSString::from_str(path.to_str()?);
        // iconForFile can run on the sampler thread. Keep all image objects
        // created by this lookup in its autorelease pool.
        let image = unsafe {
            let icon = NSWorkspace::sharedWorkspace().iconForFile(&path);
            let tiff = icon.TIFFRepresentation()?;
            let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
            let png = bitmap.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )?;
            image::load_from_memory_with_format(png.bytes(), image::ImageFormat::Png).ok()?
        };
        let rgba = image
            .resize_exact(32, 32, image::imageops::FilterType::Triangle)
            .into_rgba8();
        Some(RasterImage {
            size: [32, 32],
            rgba: rgba.into_raw(),
        })
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn load(_: &Path) -> Option<RasterImage> {
    None
}
