//! Base update task — the job behind the user's updater .bat, generalized:
//! a title string plus a list of shell commands (default: two xcopy runs
//! from the network share). Config: %APPDATA%\zakonexport\update.toml
//! (editable both in the GUI dialog and by hand); log: update.log next to it.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_TITLE: &str = "Поновлення Законодавства";

/// Profile of CREATE_NO_WINDOW — keep cmd.exe invisible while copying.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    #[serde(default = "default_title")]
    pub title: String,
    /// one shell command per line (e.g. xcopy /D/E/Q/R/Y \\srv\dbZakon\... dest)
    #[serde(default)]
    pub commands: Vec<String>,
}

fn default_title() -> String {
    DEFAULT_TITLE.to_owned()
}

impl Task {
    /// Default task straight from the user's updater .bat: the banner echo
    /// boxes and `pause` lines are dropped, only the sync commands remain.
    pub fn default_task() -> Self {
        Task {
            title: DEFAULT_TITLE.to_owned(),
            commands: vec![
                r#"xcopy /D/E/Q/R/Y \\192.168.52.19\dbZakon\ZAKON.2\Baza.GK\*.* "C:\Zakon.Local\Baza.GK""#.to_owned(),
                r#"xcopy /D/E/Q/R/Y \\192.168.52.19\dbZakon\ZAKON.2\baza\*.* "C:\Zakon.Local\BAZA\""#.to_owned(),
            ],
        }
    }
}

/// update.toml next to the remembered-base config (same %APPDATA% folder).
pub fn config_path() -> Option<PathBuf> {
    crate::config_path()
        .and_then(|p| p.parent().map(|d| d.join("update.toml")))
}

/// update.log next to update.toml (append-only, UTF-8).
pub fn log_path() -> Option<PathBuf> {
    crate::config_path()
        .and_then(|p| p.parent().map(|d| d.join("update.log")))
}

/// Load the update task; a missing file is seeded with the defaults so the
/// user can also edit it by hand outside the GUI.
pub fn load() -> Task {
    let def = Task::default_task();
    let Some(path) = config_path() else {
        return def;
    };
    if !path.exists() {
        let _ = save(&def);
        return def;
    }
    match std::fs::read_to_string(&path) {
        Ok(raw) => match toml::from_str::<Task>(&raw) {
            Ok(mut t) => {
                if t.commands.is_empty() {
                    t = def;
                }
                if t.title.trim().is_empty() {
                    t.title = DEFAULT_TITLE.to_owned();
                }
                t
            }
            Err(_) => def,
        },
        Err(_) => def,
    }
}

pub fn save(t: &Task) -> Result<()> {
    let path = config_path()
        .ok_or_else(|| anyhow::anyhow!("APPDATA is not set"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string(t).context("serialize update task")?;
    std::fs::write(&path, text)?;
    Ok(())
}

/// Result of one shell command.
#[derive(Debug, Clone)]
pub struct CmdResult {
    pub ok: bool,
    /// process exit code (xcopy: 0 = nothing new, 1 = copied, >=4 = error)
    pub code: i32,
    /// short human note (decoded OEM console tail or the OS error)
    pub note: String,
}

fn oem_decode(bytes: &[u8]) -> String {
    let (cow, _, _) = encoding_rs::IBM866.decode(bytes);
    cow.into_owned()
}

/// Run ONE command hidden (CREATE_NO_WINDOW): command is passed to
/// `cmd /C`. xcopy exit codes: 0 = nothing to copy, 1 = copied, 2/3 =
/// aborted, >=4 = errors — treated as failure. The stdout/stderr tail is
/// decoded from OEM codepage and appended to update.log.
pub fn run_one(command: &str) -> CmdResult {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};

    let res = Command::new("cmd")
        .args(["/C", command])
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    let (code, note) = match res {
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            let mut all = out.stdout.clone();
            all.extend_from_slice(&out.stderr);
            // last non-empty decoded line = the informative tail
            let decoded = oem_decode(&all);
            let tail = decoded
                .lines()
                .rev()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .unwrap_or("")
                .to_owned();
            log_line(command, code, &tail);
            (code, if tail.is_empty() { format!("код {code}") } else { tail })
        }
        Err(e) => {
            let msg = format!("{e}");
            log_line(command, -1, &msg);
            (-1, msg)
        }
    };
    CmdResult {
        ok: (0..=1).contains(&code),
        code,
        note,
    }
}

fn log_line(command: &str, code: i32, tail: &str) {
    if let Some(path) = log_path() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let line = format!(
            "[{}] kod {code} :: {command} :: {tail}\n",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs().to_string())
                .unwrap_or_else(|_| "?".into())
        );
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_roundtrip() {
        let t = Task::default_task();
        let s = toml::to_string(&t).unwrap();
        assert!(s.contains("192.168.52.19"), "UNC path survives the roundtrip");
        let back: Task = toml::from_str(&s).unwrap();
        assert_eq!(back.title, t.title);
        assert_eq!(back.commands, t.commands);
    }
}
