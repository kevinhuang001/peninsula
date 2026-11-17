//! Construct all application-launched commands through this module.
use std::{ffi::OsStr, process::Command};

pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    hide_console(&mut command);
    command
}

#[cfg(target_os = "windows")]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    // CREATE_NO_WINDOW only suppresses console allocation; normal GUI apps
    // (Explorer / browser) retain their visible windows and redirected I/O works.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(target_os = "windows"))]
fn hide_console(_command: &mut Command) {}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    #[test]
    fn background_console_program_has_no_console_window_and_keeps_output() {
        let script = r#"Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public static class ConsoleProbe { [DllImport("kernel32.dll")] public static extern IntPtr GetConsoleWindow(); }'; if ([ConsoleProbe]::GetConsoleWindow() -ne [IntPtr]::Zero) { exit 9 }; [Console]::Out.Write('hidden')"#;
        let output = super::command("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout), "hidden");
    }
}
