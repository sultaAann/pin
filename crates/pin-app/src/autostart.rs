//! Launch at login: a per-user `LaunchAgent` on macOS, an XDG autostart entry on Linux. Plain
//! files, so no system API or approval is involved and removing the file turns it off.

use std::io;
use std::path::{Path, PathBuf};

/// Passed by the login entry so pin starts hidden, waiting for its hotkey.
pub const BACKGROUND_ARG: &str = "--background";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("launch at login needs the installed Pin app")]
    NotInstalled,
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    #[error("launch at login is not supported on this system")]
    Unsupported,
    #[error("home directory not found")]
    NoHome,
    #[error("cannot update the login entry: {0}")]
    Io(#[from] io::Error),
}

/// Whether pin is set to start at login.
pub fn enabled() -> bool {
    entry_path().is_ok_and(|path| path.exists())
}

/// Whether this copy of pin can be started at login; a build run from the source tree can't.
pub fn available() -> bool {
    installed_exe().is_ok()
}

/// Turns launch at login on or off for this copy of pin.
pub fn set(on: bool) -> Result<(), Error> {
    let path = entry_path()?;
    if !on {
        return match std::fs::remove_file(&path) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        };
    }
    let exe = installed_exe()?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let contents = entry(&exe);
    if std::fs::read_to_string(&path).is_ok_and(|old| old == contents) {
        return Ok(());
    }
    // Written beside and renamed, so a crash never leaves a half-written entry.
    let partial = path.with_extension("partial");
    std::fs::write(&partial, contents)?;
    std::fs::rename(&partial, &path)?;
    Ok(())
}

/// Points an enabled entry at this copy, e.g. after the app was moved or updated.
pub fn refresh() {
    if enabled()
        && let Err(err) = set(true)
    {
        tracing::warn!(%err, "launch at login not updated");
    }
}

fn installed_exe() -> Result<PathBuf, Error> {
    let exe = std::env::current_exe()?.canonicalize()?;
    if is_installed(&exe) {
        Ok(exe)
    } else {
        Err(Error::NotInstalled)
    }
}

#[cfg(target_os = "macos")]
const LABEL: &str = "io.github.sultaann.pin";

#[cfg(target_os = "macos")]
fn entry_path() -> Result<PathBuf, Error> {
    let dirs = directories::BaseDirs::new().ok_or(Error::NoHome)?;
    Ok(dirs
        .home_dir()
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist")))
}

#[cfg(target_os = "macos")]
fn is_installed(exe: &Path) -> bool {
    exe.to_string_lossy().contains(".app/Contents/MacOS/")
}

#[cfg(target_os = "macos")]
fn entry(exe: &Path) -> String {
    let exe = xml_escape(&exe.to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>{BACKGROUND_ARG}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#
    )
}

#[cfg(any(target_os = "macos", test))]
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(target_os = "linux")]
fn entry_path() -> Result<PathBuf, Error> {
    let dirs = directories::BaseDirs::new().ok_or(Error::NoHome)?;
    Ok(dirs.config_dir().join("autostart/pin.desktop"))
}

#[cfg(target_os = "linux")]
fn is_installed(exe: &Path) -> bool {
    // A cargo build in the source tree would start a stale debug binary at every login.
    !exe.components().any(|part| part.as_os_str() == "target")
}

#[cfg(target_os = "linux")]
fn entry(exe: &Path) -> String {
    let exe = exec_quote(&exe.to_string_lossy());
    format!(
        "[Desktop Entry]\nType=Application\nName=pin\nComment=Clipboard manager\n\
         Exec={exe} {BACKGROUND_ARG}\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n"
    )
}

/// Quotes an argument for a desktop entry's Exec key, as the XDG spec requires.
#[cfg(any(target_os = "linux", test))]
fn exec_quote(arg: &str) -> String {
    let mut quoted = String::with_capacity(arg.len() + 2);
    quoted.push('"');
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    quoted.push('"');
    quoted
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn entry_path() -> Result<PathBuf, Error> {
    Err(Error::Unsupported)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn is_installed(_exe: &Path) -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn entry(_exe: &Path) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_escaped_for_their_format() {
        assert_eq!(xml_escape("/A & B/<Pin>.app"), "/A &amp; B/&lt;Pin&gt;.app");
        assert_eq!(exec_quote("/opt/my pin/$x\"q"), r#""/opt/my pin/\$x\"q""#);
    }
}
