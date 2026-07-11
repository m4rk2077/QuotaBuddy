use std::{ffi::OsStr, process::Command};

#[cfg(target_os = "windows")]
const fn platform_creation_flags() -> u32 {
    // CREATE_NO_WINDOW from WinBase.h. Keeping the value local avoids adding
    // a Win32 dependency to process launching while still documenting the API.
    0x0800_0000
}

#[cfg(all(test, not(target_os = "windows")))]
const fn platform_creation_flags() -> u32 {
    0
}

/// Creates a child-process command using QuotaBuddy's platform launch policy.
pub(crate) fn command(program: impl AsRef<OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        command.creation_flags(platform_creation_flags());
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    #[test]
    fn child_process_policy_uses_create_no_window_on_windows() {
        // CREATE_NO_WINDOW from WinBase.h.
        assert_eq!(platform_creation_flags(), 0x0800_0000);
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn child_process_policy_adds_no_windows_flags_elsewhere() {
        assert_eq!(platform_creation_flags(), 0);
    }
}
