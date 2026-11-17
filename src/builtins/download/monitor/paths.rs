use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Browser {
    Chromium,
    Firefox,
    #[cfg(target_os = "macos")]
    Safari,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub browser: Browser,
    pub label: &'static str,
    pub path: PathBuf,
}

pub fn discover() -> Vec<Source> {
    let Some(home) = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
    else {
        return Vec::new();
    };
    let mut sources = Vec::new();
    #[cfg(target_os = "macos")]
    let roots = vec![
        (
            "Chrome",
            home.join("Library/Application Support/Google/Chrome"),
        ),
        (
            "Chromium",
            home.join("Library/Application Support/Chromium"),
        ),
        (
            "Edge",
            home.join("Library/Application Support/Microsoft Edge"),
        ),
        (
            "Brave",
            home.join("Library/Application Support/BraveSoftware/Brave-Browser"),
        ),
        (
            "Arc",
            home.join("Library/Application Support/Arc/User Data"),
        ),
    ];
    #[cfg(target_os = "linux")]
    let roots = {
        let config = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        vec![
            ("Chrome", config.join("google-chrome")),
            ("Chromium", config.join("chromium")),
            ("Edge", config.join("microsoft-edge")),
            ("Brave", config.join("BraveSoftware/Brave-Browser")),
        ]
    };
    #[cfg(target_os = "windows")]
    let roots = {
        let local = env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Local"));
        vec![
            ("Chrome", local.join("Google/Chrome/User Data")),
            ("Chromium", local.join("Chromium/User Data")),
            ("Edge", local.join("Microsoft/Edge/User Data")),
            ("Brave", local.join("BraveSoftware/Brave-Browser/User Data")),
        ]
    };
    for (label, root) in roots {
        for profile in directories(&root) {
            let history = profile.join("History");
            if history.is_file() {
                sources.push(Source {
                    browser: Browser::Chromium,
                    label,
                    path: history,
                });
            }
        }
    }
    #[cfg(target_os = "macos")]
    let firefox = home.join("Library/Application Support/Firefox");
    #[cfg(target_os = "linux")]
    let firefox = home.join(".mozilla/firefox");
    #[cfg(target_os = "windows")]
    let firefox = env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join("AppData/Roaming"))
        .join("Mozilla/Firefox");
    let mut profiles = directories(&firefox.join("Profiles"));
    profiles.extend(directories(&firefox));
    profiles.extend(profile_ini_paths(&firefox));
    profiles.sort();
    profiles.dedup();
    for profile in profiles {
        let history = profile.join("places.sqlite");
        if history.is_file() {
            sources.push(Source {
                browser: Browser::Firefox,
                label: "Firefox",
                path: history,
            });
        }
    }
    #[cfg(target_os = "macos")]
    {
        let path = home.join("Library/Safari/Downloads.plist");
        if path.exists() {
            sources.push(Source {
                browser: Browser::Safari,
                label: "Safari",
                path,
            });
        }
    }
    sources
}

fn profile_ini_paths(root: &Path) -> Vec<PathBuf> {
    let Ok(contents) = fs::read_to_string(root.join("profiles.ini")) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let mut relative = true;
    let mut path = None;
    for line in contents.lines().chain(std::iter::once("[end]")) {
        let line = line.trim();
        if line.starts_with('[') {
            if let Some(value) = path.take() {
                result.push(if relative {
                    root.join(value)
                } else {
                    PathBuf::from(value)
                });
            }
            relative = true;
        } else if let Some(value) = line.strip_prefix("IsRelative=") {
            relative = value.trim() != "0";
        } else if let Some(value) = line.strip_prefix("Path=") {
            path = Some(value.trim().to_owned());
        }
    }
    result
}

fn directories(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovers_relative_and_absolute_firefox_profiles() {
        let folder = tempfile::tempdir().unwrap();
        fs::write(
            folder.path().join("profiles.ini"),
            "[Profile0]\nIsRelative=1\nPath=Profiles/a\n[Profile1]\nIsRelative=0\nPath=/custom/b\n",
        )
        .unwrap();
        let paths = profile_ini_paths(folder.path());
        assert_eq!(paths[0], folder.path().join("Profiles/a"));
        assert_eq!(paths[1], PathBuf::from("/custom/b"));
    }
}
