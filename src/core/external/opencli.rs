use std::{
    env,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
static CLI_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct OpenCli {
    executable: PathBuf,
}

#[derive(Clone, Debug)]
pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub status: i32,
}

impl OpenCli {
    pub fn resolve(configured: Option<&Path>) -> Result<Self, String> {
        if let Some(path) = configured.filter(|path| !path.as_os_str().is_empty()) {
            return if path.is_file() {
                Ok(Self {
                    executable: path.to_owned(),
                })
            } else {
                Err(format!("OpenCLI executable not found: {}", path.display()))
            };
        }
        let mut candidates = Vec::new();
        if let Some(path) = env::var_os("OPENCLI_PATH") {
            candidates.push(PathBuf::from(path));
        }
        if let Some(paths) = env::var_os("PATH") {
            candidates.extend(path_candidates(&paths));
        }
        for path in common_paths() {
            candidates.push(path);
        }
        candidates
            .into_iter()
            .find(|path| path.is_file())
            .map(|executable| Self { executable })
            .ok_or_else(|| {
                "OpenCLI was not found on PATH or in common installation locations".into()
            })
    }

    pub fn version(&self, timeout: Duration) -> Result<String, String> {
        let output = self.run(&["--version"], timeout)?;
        if output.status != 0 {
            return Err(command_error("opencli --version", &output));
        }
        Ok(output.stdout.trim().to_owned())
    }

    pub fn run(&self, args: &[&str], timeout: Duration) -> Result<CommandOutput, String> {
        self.run_with_browser_timeout(args, timeout, None)
    }

    pub fn run_browser(&self, args: &[&str], timeout: Duration) -> Result<CommandOutput, String> {
        self.run_with_browser_timeout(
            args,
            timeout,
            Some(timeout.saturating_sub(Duration::from_secs(10))),
        )
    }

    fn run_with_browser_timeout(
        &self,
        args: &[&str],
        timeout: Duration,
        browser_timeout: Option<Duration>,
    ) -> Result<CommandOutput, String> {
        let _guard = CLI_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .map_err(|_| "OpenCLI execution lock poisoned".to_owned())?;
        let mut command = crate::sdk::process::ProcessSpec::new(self.executable.clone())
            .args(args.iter().copied())
            .timeout(timeout)
            .max_output_bytes(OUTPUT_LIMIT);
        // Finder does not load NVM's shell initialization. Its OpenCLI script
        // uses /usr/bin/env node, so make the sibling Node runtime discoverable.
        if let Some(parent) = self
            .executable
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            let inherited = env::var_os("PATH").unwrap_or_default();
            if let Ok(paths) = env::join_paths(
                std::iter::once(parent.to_owned()).chain(env::split_paths(&inherited)),
            ) {
                command = command.env("PATH", paths);
            }
        }
        if let Some(timeout) = browser_timeout {
            command = command.env(
                "OPENCLI_BROWSER_COMMAND_TIMEOUT",
                timeout.as_secs().to_string(),
            );
        }
        let output = crate::sdk::process::ProcessExecutor
            .run(command)
            .map_err(|error| format!("OpenCLI process failed: {error}"))?;
        Ok(CommandOutput {
            stdout: output.stdout_text(),
            stderr: output.stderr_text(),
            status: output.status.code().unwrap_or(-1),
        })
    }
}

fn command_error(command: &str, output: &CommandOutput) -> String {
    format!(
        "{command} failed ({}): {}",
        output.status,
        output.stderr.trim()
    )
}

fn executable_names(name: &str) -> Vec<OsString> {
    #[cfg(target_os = "windows")]
    {
        ["exe", "cmd", "bat"]
            .map(|extension| format!("{name}.{extension}").into())
            .to_vec()
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![name.into()]
    }
}

fn path_candidates(paths: &OsStr) -> Vec<PathBuf> {
    env::split_paths(paths)
        .flat_map(|dir| {
            executable_names("opencli")
                .into_iter()
                .map(move |name| dir.join(name))
        })
        .collect()
}

fn common_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    #[cfg(target_os = "windows")]
    if let Some(appdata) = env::var_os("APPDATA") {
        let npm = PathBuf::from(appdata).join("npm");
        paths.extend(
            executable_names("opencli")
                .into_iter()
                .map(|name| npm.join(name)),
        );
    }
    #[cfg(target_os = "macos")]
    paths.extend(
        [
            "/opt/homebrew/bin/opencli",
            "/usr/local/bin/opencli",
            "/opt/local/bin/opencli",
        ]
        .map(PathBuf::from),
    );
    #[cfg(target_os = "linux")]
    paths.extend(
        [
            "/usr/local/bin/opencli",
            "/usr/bin/opencli",
            "/snap/bin/opencli",
        ]
        .map(PathBuf::from),
    );
    if let Some(home) = env::var_os("HOME").or_else(|| env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        paths.extend([
            home.join(".local/bin/opencli"),
            home.join(".npm-global/bin/opencli"),
            home.join(".nvm/current/bin/opencli"),
            home.join(".volta/bin")
                .join(&executable_names("opencli")[0]),
            home.join(".fnm/current/bin/opencli"),
        ]);
        if let Ok(versions) = std::fs::read_dir(home.join(".nvm/versions/node")) {
            paths.extend(
                versions
                    .flatten()
                    .map(|entry| entry.path().join("bin/opencli")),
            );
        }
    }
    paths
}

pub fn parse_json(output: &CommandOutput) -> Result<serde_json::Value, String> {
    if output.status != 0 {
        return Err(command_error("OpenCLI", output));
    }
    serde_json::from_str(&output.stdout)
        .map_err(|error| format!("OpenCLI returned invalid JSON: {error}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_json, CommandOutput};

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_npm_cmd_and_its_console_child_have_no_visible_console() {
        let root = tempfile::Builder::new()
            .prefix("npm shim ")
            .tempdir()
            .unwrap();
        let shim = root.path().join("opencli.cmd");
        use base64::Engine as _;
        // Passively check window visibility in a real console child. Opening
        // the CON device can itself allocate a console and is not a popup test.
        let script: Vec<_> = super::super::process::CONSOLE_VISIBILITY_PROBE
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        let encoded = base64::engine::general_purpose::STANDARD.encode(script);
        std::fs::write(&shim, format!(
            "@echo off\r\npowershell.exe -NoProfile -NonInteractive -EncodedCommand {encoded}\r\nexit /b %errorlevel%\r\n"
        )).unwrap();
        let paths = std::env::join_paths([root.path()]).unwrap();
        let executable = super::path_candidates(&paths)
            .into_iter()
            .find(|path| path.is_file())
            .unwrap();
        assert_eq!(executable, shim);
        let cli = super::OpenCli::resolve(Some(&executable)).unwrap();
        let output = cli.run(&[], std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(output.status, 0, "{}", output.stderr);
        assert_eq!(output.stdout.trim(), "hidden");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_batch_stdout_stderr_and_exit_status_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let shim = root.path().join("opencli.cmd");
        std::fs::write(
            &shim,
            "@echo off\r\necho stdout-ready\r\necho stderr-ready 1>&2\r\nexit /b 7\r\n",
        )
        .unwrap();
        let cli = super::OpenCli::resolve(Some(&shim)).unwrap();
        let output = cli.run(&[], std::time::Duration::from_secs(10)).unwrap();
        assert_eq!(output.status, 7);
        assert_eq!(output.stdout.trim(), "stdout-ready");
        assert_eq!(output.stderr.trim(), "stderr-ready");
    }

    #[test]
    fn command_json_parser_rejects_failed_commands_and_malformed_output() {
        let successful = CommandOutput {
            stdout: r#"{"ok":true}"#.into(),
            stderr: String::new(),
            status: 0,
        };
        assert!(parse_json(&successful).unwrap()["ok"].as_bool().unwrap());
        let failed = CommandOutput {
            status: 1,
            ..successful.clone()
        };
        assert!(parse_json(&failed).is_err());
        let malformed = CommandOutput {
            stdout: "not json".into(),
            ..successful
        };
        assert!(parse_json(&malformed).is_err());
    }
}
