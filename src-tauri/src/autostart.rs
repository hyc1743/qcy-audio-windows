//! Windows per-user startup registration helper (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`).
//! Requires zero admin privileges and provides a safe no-op stub on non-Windows targets.

use std::path::Path;

pub const RUN_KEY_PATH: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
pub const RUN_VALUE_NAME: &str = "QcyAudioDesktop";

/// Formats a Windows Registry Run command line, properly quoting paths that may contain spaces.
pub fn format_startup_command(exe_path: &Path, start_minimized: bool) -> String {
    let raw = exe_path.to_string_lossy().replace('"', "");
    if start_minimized {
        format!("\"{raw}\" --minimized")
    } else {
        format!("\"{raw}\"")
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{format_startup_command, RUN_KEY_PATH, RUN_VALUE_NAME};
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    pub fn is_autostart_enabled() -> bool {
        Command::new("reg.exe")
            .args(["query", RUN_KEY_PATH, "/v", RUN_VALUE_NAME])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    pub fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
        if enabled {
            let exe = std::env::current_exe()
                .map_err(|e| format!("failed to locate current executable: {e}"))?;
            let cmd_line = format_startup_command(&exe, true);

            let output = Command::new("reg.exe")
                .args([
                    "add",
                    RUN_KEY_PATH,
                    "/v",
                    RUN_VALUE_NAME,
                    "/t",
                    "REG_SZ",
                    "/d",
                    &cmd_line,
                    "/f",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("failed to invoke reg.exe: {e}"))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("reg add failed: {}", stderr.trim()));
            }
            Ok(())
        } else {
            let output = Command::new("reg.exe")
                .args(["delete", RUN_KEY_PATH, "/v", RUN_VALUE_NAME, "/f"])
                .creation_flags(CREATE_NO_WINDOW)
                .output()
                .map_err(|e| format!("failed to invoke reg.exe: {e}"))?;

            // Treat already-absent key as cleanly disabled.
            if !output.status.success() && is_autostart_enabled() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(format!("reg delete failed: {}", stderr.trim()));
            }
            Ok(())
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub fn is_autostart_enabled() -> bool {
        false
    }

    pub fn set_autostart_enabled(_enabled: bool) -> Result<(), String> {
        Ok(())
    }
}

/// Checks whether QCY Desktop is registered in `HKCU\...\Run\QcyAudioDesktop`.
pub fn is_autostart_enabled() -> bool {
    imp::is_autostart_enabled()
}

/// Enables or disables launch-at-Windows-startup for the current executable.
pub fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    imp::set_autostart_enabled(enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn startup_command_quotes_spaces_and_flags() {
        let with_spaces = PathBuf::from(r"C:\Program Files\QCY Desktop\qcy-desktop.exe");
        assert_eq!(
            format_startup_command(&with_spaces, true),
            "\"C:\\Program Files\\QCY Desktop\\qcy-desktop.exe\" --minimized"
        );
        assert_eq!(
            format_startup_command(&with_spaces, false),
            "\"C:\\Program Files\\QCY Desktop\\qcy-desktop.exe\""
        );

        let simple = PathBuf::from(r"D:\qcy\qcy-desktop.exe");
        assert_eq!(
            format_startup_command(&simple, true),
            "\"D:\\qcy\\qcy-desktop.exe\" --minimized"
        );
    }

    #[test]
    fn autostart_query_does_not_panic() {
        // Read-only query; never mutates the developer host's registry during unit tests.
        let _enabled = is_autostart_enabled();
    }
}
