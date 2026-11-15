//! Locating and launching the FFmpeg executable.

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
const EXE: &str = "ffmpeg.exe";
#[cfg(not(windows))]
const EXE: &str = "ffmpeg";

/// Resolution order: explicit setting, `ffmpeg.exe` beside our executable,
/// `ffmpeg\bin\ffmpeg.exe` beside our executable, then whatever is on PATH.
pub fn locate(configured: &str) -> PathBuf {
    // A configured path that no longer exists (a moved or deleted install) falls back to the bundled
    // FFmpeg instead of failing every encode and every hardware Link with "file not found".
    let configured = configured.trim().trim_matches('"');
    if !configured.is_empty() && (Path::new(configured).is_file() || !Path::new(configured).is_absolute()) {
        return PathBuf::from(configured);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for candidate in [dir.join(EXE), dir.join("ffmpeg").join("bin").join(EXE)] {
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from(EXE)
}

/// A `Command` for FFmpeg that won't flash a console window on Windows.
pub fn command(ffmpeg: &Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(ffmpeg);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// `ffmpeg -version` first line, or an error if FFmpeg can't be run.
pub fn version(ffmpeg: &Path) -> anyhow::Result<String> {
    let out = command(ffmpeg).arg("-hide_banner").arg("-version").output()?;
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.lines().next().unwrap_or("ffmpeg").to_string())
}
