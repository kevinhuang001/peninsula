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
    pub fn get(&mut self, executable: &Path) -> Option<RasterImage> {
        if !self.images.contains_key(executable) {
            // Bound memory even on hosts with a frequently changing process list.
            if self.images.len() >= 128 {
                self.images.clear();
            }
            self.images.insert(executable.to_owned(), load(executable));
        }
        self.images.get(executable).cloned().flatten()
    }
}

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::load;

#[cfg(not(target_os = "windows"))]
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
fn load(executable: &Path) -> Option<RasterImage> {
    let bundle = executable
        .ancestors()
        .find(|p| p.extension().is_some_and(|v| v == "app"))?;
    let info = plist::Value::from_file(bundle.join("Contents/Info.plist")).ok()?;
    let name = info.as_dictionary()?.get("CFBundleIconFile")?.as_string()?;
    let mut icon = bundle.join("Contents/Resources").join(name);
    if icon.extension().is_none() {
        icon.set_extension("icns");
    }
    if icon.extension().is_some_and(|v| v != "icns") {
        return decode(&icon);
    }
    let output = tempfile::Builder::new().suffix(".png").tempfile().ok()?;
    let status = std::process::Command::new("/usr/bin/sips")
        .args(["-s", "format", "png"])
        .arg(icon)
        .arg("--out")
        .arg(output.path())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    status.success().then(|| decode(output.path())).flatten()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn load(_: &Path) -> Option<RasterImage> {
    None
}
