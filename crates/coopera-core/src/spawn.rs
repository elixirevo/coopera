//! Detached process spawning. Background work started by a hook (distillation,
//! presence pushes) must outlive the hook process: hook runners may clean up
//! the hook's process group once the hook exits, which kills any child still
//! in that group mid-run (observed 2026-08-02: SessionEnd distillation dying
//! silently while the same command succeeds in a foreground shell).

use std::process::Command;

/// Put the child in its own process group so teardown signals aimed at the
/// hook's group cannot reach it. Returns the same `Command` for chaining.
pub fn detach(cmd: &mut Command) -> &mut Command {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
    cmd
}

/// Is `name` an executable on PATH? (Windows also tries `<name>.exe`.)
/// Mirrors what the guarded hook command's `command -v` will conclude.
pub fn find_on_path(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|d| {
        d.join(name).is_file() || (cfg!(windows) && d.join(format!("{name}.exe")).is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn detached_child_still_runs_and_reports_status() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "exit 7"]);
        let status = detach(&mut cmd).status().unwrap();
        assert_eq!(status.code(), Some(7));
    }

    #[test]
    fn find_on_path_sees_git_but_not_nonsense() {
        assert!(find_on_path("git"), "git must be on PATH for these tests");
        assert!(!find_on_path("definitely-not-a-real-binary-name"));
    }
}
