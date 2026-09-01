//! Start Menu discovery.
//!
//! Walks the machine-wide and per-user Start Menu trees, resolves every
//! `.lnk`, renders its icon once to a PNG cache, and publishes the result.
//! Runs on its own thread: COM, disk walking, and icon rasterization together
//! take seconds on a machine with a lot installed.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter, Manager};

use crate::apps::{entry_id, is_dev_tool, AppEntry};
use crate::AppState;

pub const CATALOG_EVENT: &str = "apps::catalog";

/// Shortcuts that are documentation, not applications.
const NAME_BLOCKLIST: &[&str] = &[
    "uninstall",
    "readme",
    "read me",
    "help",
    "documentation",
    "license",
    "changelog",
    "release notes",
    "website",
    "web site",
    "homepage",
    "support",
    "manual",
    "user guide",
];

/// Start Menus nest a few levels (vendor → product → shortcut); beyond that is
/// almost always noise.
const MAX_DEPTH: usize = 5;

pub fn spawn_scan(app: AppHandle) {
    std::thread::Builder::new()
        .name("dev-layer/app-scan".into())
        .spawn(move || {
            let icon_dir = match app.path().app_cache_dir() {
                Ok(dir) => dir.join("icons"),
                Err(e) => {
                    tracing::error!(error = %e, "no cache directory; skipping app scan");
                    return;
                }
            };
            if let Err(e) = std::fs::create_dir_all(&icon_dir) {
                tracing::warn!(error = %e, "could not create icon cache; icons will be missing");
            }

            let started = std::time::Instant::now();
            let entries = scan_catalog(&icon_dir);
            tracing::info!(count = entries.len(), elapsed = ?started.elapsed(), "app catalog scanned");

            let state = app.state::<AppState>();
            state.apps.replace(entries);
            if let Err(e) = app.emit(CATALOG_EVENT, state.apps.entries()) {
                tracing::warn!(error = %e, "could not emit app catalog");
            }
        })
        .map(|_| ())
        .unwrap_or_else(|e| tracing::error!(error = %e, "could not start app scan"));
}

#[cfg(windows)]
pub fn scan_catalog(icon_dir: &Path) -> Vec<AppEntry> {
    use crate::platform::sys::{extract_icon_png, resolve_shortcut, ComGuard};

    // One apartment for the whole scan; every shell call below needs it.
    let _com = match ComGuard::new() {
        Ok(guard) => guard,
        Err(e) => {
            tracing::error!(error = %e, "COM unavailable; no apps discovered");
            return Vec::new();
        }
    };

    let mut shortcuts = Vec::new();
    for root in start_menu_roots() {
        collect_shortcuts(&root, 0, &mut shortcuts);
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut entries = Vec::new();

    for lnk in shortcuts {
        let name = match lnk.file_stem() {
            Some(stem) => stem.to_string_lossy().to_string(),
            None => continue,
        };
        if is_blocked(&name) {
            continue;
        }

        let shortcut = match resolve_shortcut(&lnk) {
            Ok(shortcut) => shortcut,
            Err(e) => {
                tracing::debug!(path = %lnk.display(), error = %e, "unresolved shortcut");
                continue;
            }
        };

        // Only real executables: Start Menus are full of .url and .chm links.
        let is_executable = shortcut
            .target
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("exe"))
            .unwrap_or(false);
        if !is_executable || !shortcut.target.exists() {
            continue;
        }

        // The same app usually appears in both the machine and user Start
        // Menus; first one wins.
        let fingerprint = format!(
            "{}|{}",
            shortcut.target.to_string_lossy().to_lowercase(),
            shortcut.arguments.to_lowercase()
        );
        if !seen.insert(fingerprint) {
            continue;
        }

        let id = entry_id(&lnk);
        entries.push(AppEntry {
            icon: cache_icon(&id, &lnk, icon_dir, extract_icon_png),
            is_dev_tool: is_dev_tool(&name, Some(&shortcut.target)),
            id,
            name,
            launch_path: lnk,
            target: Some(shortcut.target),
            pinned: false,
        });
    }

    entries
}

#[cfg(not(windows))]
pub fn scan_catalog(_icon_dir: &Path) -> Vec<AppEntry> {
    // No Start Menu to walk. Returning nothing (rather than fixtures) keeps
    // fake apps out of anything that might treat them as real.
    tracing::debug!("app discovery is Windows-only; catalog is empty");
    Vec::new()
}

#[cfg(windows)]
fn start_menu_roots() -> Vec<PathBuf> {
    ["ProgramData", "AppData"]
        .iter()
        .filter_map(|var| std::env::var_os(var))
        .map(|base| PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs"))
        .filter(|path| path.is_dir())
        .collect()
}

#[cfg(windows)]
fn collect_shortcuts(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in read.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => collect_shortcuts(&path, depth + 1, out),
            Ok(_)
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("lnk")) =>
            {
                out.push(path)
            }
            _ => {}
        }
    }
}

/// Renders the icon once and reuses it forever after; re-rendering every icon
/// on every start would dominate scan time.
#[cfg(windows)]
fn cache_icon(
    id: &str,
    source: &Path,
    icon_dir: &Path,
    extract: impl Fn(&Path) -> crate::error::Result<Vec<u8>>,
) -> Option<PathBuf> {
    let cached = icon_dir.join(format!("{id}.png"));
    if cached.is_file() {
        return Some(cached);
    }

    match extract(source) {
        Ok(png) => match std::fs::write(&cached, png) {
            Ok(()) => Some(cached),
            Err(e) => {
                tracing::debug!(error = %e, "could not cache icon");
                None
            }
        },
        Err(e) => {
            tracing::debug!(path = %source.display(), error = %e, "no icon");
            None
        }
    }
}

fn is_blocked(name: &str) -> bool {
    let lower = name.to_lowercase();
    NAME_BLOCKLIST.iter().any(|blocked| lower.contains(blocked))
}

#[cfg(test)]
mod tests {
    use super::is_blocked;

    #[test]
    fn filters_documentation_shortcuts_only() {
        assert!(is_blocked("Uninstall Postman"));
        assert!(is_blocked("Node.js Documentation"));
        assert!(!is_blocked("Postman"));
        assert!(!is_blocked("Visual Studio Code"));
    }
}
