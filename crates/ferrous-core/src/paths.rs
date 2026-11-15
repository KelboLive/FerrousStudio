use std::path::{Path, PathBuf};

/// The settings folder's name under `%APPDATA%`.
pub const APP_DIR: &str = "Ferrous Studio";

/// `%APPDATA%\Ferrous Studio` on Windows (falls back to the working directory).
pub fn config_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join(APP_DIR);
    if !dir.exists() {
        migrate(&base, &dir);
    }
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Carries settings over from the app's earlier names, once: Ferrous (copied, so an older build still
/// finds its own), or Ember Studio before that.
fn migrate(base: &Path, dir: &Path) {
    let ferrous = base.join("Ferrous");
    if ferrous.is_dir() {
        let tmp = base.join(format!("{APP_DIR}.migrating"));
        let _ = std::fs::remove_dir_all(&tmp);
        if copy_settings(&ferrous, &tmp).is_ok() {
            let _ = std::fs::rename(&tmp, dir);
        } else {
            let _ = std::fs::remove_dir_all(&tmp);
        }
        return;
    }
    let ember = base.join("EmberStudio");
    if ember.is_dir() {
        let _ = std::fs::rename(&ember, dir);
    }
}

/// Copies a settings folder, leaving out what is only scratch: replay buffers, logs and builds
/// kept there by hand.
fn copy_settings(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let name = e.file_name();
        let n = name.to_string_lossy();
        if n.starts_with("replay-") || n == "Claude outputs" || n.ends_with(".log") {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if e.file_type()?.is_dir() {
            copy_settings(&src, &dst)?;
        } else {
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

/// Default folder for recordings: the user's Videos folder.
pub fn default_recording_dir() -> PathBuf {
    dirs::video_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_carry_over_from_ferrous_without_scratch() {
        let base = std::env::temp_dir().join(format!("ferrous-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let old = base.join("Ferrous");
        std::fs::create_dir_all(old.join("profiles")).unwrap();
        std::fs::create_dir_all(old.join("replay-42")).unwrap();
        std::fs::write(old.join("profiles").join("Default.json"), "{}").unwrap();
        std::fs::write(old.join("scenes.json"), "[]").unwrap();
        std::fs::write(old.join("ferrous.log"), "x").unwrap();
        let dir = base.join(APP_DIR);
        migrate(&base, &dir);
        assert!(dir.join("profiles").join("Default.json").is_file());
        assert!(dir.join("scenes.json").is_file());
        assert!(!dir.join("replay-42").exists() && !dir.join("ferrous.log").exists());
        assert!(old.join("scenes.json").is_file(), "the old folder is left as it was");
        let _ = std::fs::remove_dir_all(&base);
    }
}
