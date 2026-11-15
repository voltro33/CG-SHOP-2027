//! Processes that outlive the menus: started detached from the terminal,
//! checked for by process id, and stopped with their children.

use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};

/// Starts `program args` detached, output to `log`, and returns its pid.
pub fn spawn_detached(program: &Path, args: &[String], cwd: &Path, log: &Path) -> Result<u32, String> {
    let out = File::create(log).map_err(|e| format!("cannot create {}: {e}", log.display()))?;
    let err = out.try_clone().map_err(|e| e.to_string())?;

    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err);
    detach(&mut command);

    let child = command
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", program.display()))?;
    Ok(child.id())
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt;

    // its own hidden console and process group, so closing the terminal
    // or Ctrl-C in it does not reach the run
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    // its own process group: the terminal's hangup and Ctrl-C miss it, and
    // stopping signals the group
    command.process_group(0);
}

/// True if a process with this id runs.
pub fn alive(pid: u32) -> bool {
    if cfg!(windows) {
        let filter = format!("PID eq {pid}");
        let Ok(out) = Command::new("tasklist")
            .args(["/FI", &filter, "/NH", "/FO", "CSV"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        else {
            return false;
        };
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
    } else {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
}

/// The ids of every running process, from one listing, for checking many
/// runs at once.
pub fn running_pids() -> std::collections::HashSet<u32> {
    let (program, args): (&str, &[&str]) = if cfg!(windows) {
        ("tasklist", &["/NH", "/FO", "CSV"])
    } else {
        ("ps", &["-e", "-o", "pid="])
    };
    let Ok(out) = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return std::collections::HashSet::new();
    };

    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            let field = if cfg!(windows) {
                line.split(',').nth(1)?
            } else {
                line
            };
            field.trim().trim_matches('"').parse().ok()
        })
        .collect()
}

/// Stops the process and everything it started.
pub fn stop_tree(pid: u32) -> Result<(), String> {
    let status = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        // the run leads its own process group
        Command::new("kill")
            .args(["-TERM", &format!("-{pid}")])
            .stderr(Stdio::null())
            .status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) if !alive(pid) => Ok(()),
        Ok(s) => Err(format!("could not stop process {pid} ({s})")),
        Err(e) => Err(format!("could not stop process {pid}: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_is_alive_and_a_made_up_one_is_not() {
        assert!(alive(std::process::id()));
        assert!(!alive(u32::MAX - 7));
        assert!(running_pids().contains(&std::process::id()));
    }
}
