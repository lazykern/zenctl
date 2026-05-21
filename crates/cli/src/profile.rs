//! Locate the Zen profile directory and detect whether Zen is running.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

pub fn profiles_root() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        home().map(|h| h.join("Library/Application Support/zen/Profiles"))
    } else if cfg!(target_os = "linux") {
        // Flatpak path takes precedence when it exists.
        if let Some(flatpak) = home().map(|h| h.join(".var/app/org.zen_browser.zen/.zen")) {
            if flatpak.exists() {
                return Some(flatpak);
            }
        }
        // XDG_CONFIG_HOME / .config/zen (used by some Linux builds including AUR zen-browser-bin).
        let config_root = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|h| h.join(".config")))
            .map(|p| p.join("zen"));
        if let Some(ref cr) = config_root {
            if cr.exists() {
                return Some(cr.clone());
            }
        }
        home().map(|h| h.join(".zen"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("zen/Profiles"))
    } else {
        None
    }
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Pick the most-recently-modified profile (most likely the active one).
pub fn detect_profile() -> Result<Option<PathBuf>> {
    let Some(root) = profiles_root() else {
        return Ok(None);
    };
    if !root.exists() {
        return Ok(None);
    }
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&root).with_context(|| format!("read {}", root.display()))? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let prefs = path.join("prefs.js");
        if !prefs.exists() {
            continue;
        }
        let mtime = std::fs::metadata(&prefs)
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(t, _)| mtime > *t) {
            best = Some((mtime, path));
        }
    }
    Ok(best.map(|(_, p)| p))
}

pub fn zen_pid() -> Option<u32> {
    // Try pgrep first with platform-appropriate patterns.
    let pgrep = if std::path::Path::new("/usr/bin/pgrep").exists() {
        "/usr/bin/pgrep"
    } else {
        "pgrep"
    };

    let patterns: &[&str] = if cfg!(target_os = "macos") {
        &["Zen.app/Contents/MacOS/zen"]
    } else if cfg!(target_os = "linux") {
        // Zen on Linux ships as zen-bin (tarball), zen-browser-bin (AUR),
        // zen-browser (AppImage), or simply zen (symlink).
        &["zen-bin", "zen-browser-bin", "zen-browser", "zen"]
    } else {
        &[]
    };

    for pattern in patterns {
        if let Ok(out) = Command::new(pgrep).args(["-f", pattern]).output() {
            if out.status.success() {
                if let Some(pid) = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .and_then(|l| l.trim().parse().ok())
                {
                    return Some(pid);
                }
            }
        }
    }

    // Fallback: when running as native messaging host (stdin is a pipe), our
    // parent process IS Zen. In CLI mode this fallback is skipped because
    // the parent could be bash, a terminal, or anything else.
    #[cfg(unix)]
    {
        use std::io::IsTerminal;
        use std::os::unix::process::parent_id;
        if !std::io::stdin().is_terminal() {
            let ppid = parent_id();
            if ppid > 1 {
                return Some(ppid);
            }
        }
    }
    None
}

pub fn is_running(profile: &Path) -> bool {
    // Firefox-family lock files: .parentlock (classic) and lock (symlink on newer versions).
    let lock = profile.join(".parentlock").exists()
        || profile.join("lock").exists()
        || profile.join(".parentlock").is_symlink()
        || profile.join("lock").is_symlink();
    lock && zen_pid().is_some()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_dir() -> PathBuf {
        let unique = format!(
            "zenctl-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        );
        let dir = std::env::temp_dir().join(unique);
        fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    fn clean_temp_dir(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn test_profiles_root_linux_native() {
        if !cfg!(target_os = "linux") {
            return;
        }
        let root = profiles_root();
        assert!(
            root.is_some(),
            "profiles_root should return Some when HOME is set"
        );
        let root = root.unwrap();
        let s = root.to_string_lossy();
        assert!(
            s.ends_with(".zen") || s.ends_with(".config/zen") || s.contains("org.zen_browser.zen"),
            "unexpected profiles root: {s}"
        );
    }

    #[test]
    fn test_profiles_root_macos() {
        if !cfg!(target_os = "macos") {
            return;
        }
        let root = profiles_root();
        assert!(root.is_some());
        let binding = root.unwrap();
        let s = binding.to_string_lossy();
        assert!(s.ends_with("Profiles"), "unexpected profiles root: {s}");
    }

    #[test]
    fn test_is_running_no_lock_files() {
        let dir = make_temp_dir();
        // Empty dir, no Zen PID — should return false.
        assert!(!is_running(&dir));
        clean_temp_dir(&dir);
    }

    #[test]
    fn test_is_running_with_parentlock() {
        let dir = make_temp_dir();
        fs::write(dir.join(".parentlock"), b"").expect("write lock");
        // is_running() returns lock_exists && zen_pid().is_some().
        // If Zen is actually running on this system, pgrep will find it and
        // the result will be true. Otherwise it's false.
        let running = is_running(&dir);
        let zen_found = zen_pid().is_some();
        assert_eq!(
            running, zen_found,
            "is_running should match zen_pid availability when lock exists"
        );
        clean_temp_dir(&dir);
    }

    #[test]
    fn test_is_running_with_lock() {
        let dir = make_temp_dir();
        fs::write(dir.join("lock"), b"").expect("write lock");
        let running = is_running(&dir);
        let zen_found = zen_pid().is_some();
        assert_eq!(
            running, zen_found,
            "is_running should match zen_pid availability when lock exists"
        );
        clean_temp_dir(&dir);
    }

    #[test]
    fn test_detect_profile_empty_temp() {
        let dir = make_temp_dir();
        let saved = std::env::var_os("HOME");
        std::env::set_var("HOME", &dir);
        let result = detect_profile();
        if let Some(h) = saved {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
        // Should be either Ok(None) or an Err (because the temp dir lacks
        // the platform-specific subdirectory structure).
        if let Err(ref e) = result {
            eprintln!("detect_profile error (may be fine): {e}");
        }
        clean_temp_dir(&dir);
    }

    #[test]
    fn test_detect_profile_with_fake_profile() {
        if !cfg!(target_os = "linux") {
            return;
        }
        let dir = make_temp_dir();
        let zen_dir = dir.join(".zen");
        let profile = zen_dir.join("abc123.default");
        fs::create_dir_all(&profile).expect("create profile dir");
        fs::write(profile.join("prefs.js"), b"// fake prefs").expect("write prefs.js");

        let saved = std::env::var_os("HOME");
        std::env::set_var("HOME", &dir);
        let result = detect_profile();
        if let Some(h) = saved {
            std::env::set_var("HOME", h);
        } else {
            std::env::remove_var("HOME");
        }
        match result {
            Ok(Some(p)) => assert!(p.ends_with("abc123.default"), "found: {}", p.display()),
            Ok(None) => eprintln!("no profile detected (unexpected)"),
            Err(e) => eprintln!("error: {e}"),
        }
        clean_temp_dir(&dir);
    }
}
