//! The application catalog: what the dock and launcher can open.
//!
//! Discovery is a background job (COM, disk walking and icon rendering are all
//! too slow for startup), so the HUD paints first and the dock fills in when
//! the scan lands.

mod scan;

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

pub use scan::{scan_catalog, spawn_scan, CATALOG_EVENT};

/// One launchable application.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppEntry {
    /// Stable across rescans: derived from the launch target, not the scan order.
    pub id: String,
    pub name: String,
    /// What gets handed to the shell — usually the `.lnk`, so the shortcut's
    /// own arguments and working directory apply.
    pub launch_path: PathBuf,
    /// The executable the shortcut resolves to; used for grouping and for
    /// matching windows to apps in milestone 4.
    pub target: Option<PathBuf>,
    /// Cached PNG on disk, served to the HUD over the asset protocol.
    pub icon: Option<PathBuf>,
    /// Recognized developer tooling, pinned to the dock by default.
    pub is_dev_tool: bool,
    pub pinned: bool,
}

/// Substrings matched against the executable stem. Deliberately short and
/// lowercase; this is a "put it in the dock by default" heuristic, not an
/// allowlist — anything not matched is still discoverable in the launcher.
const DEV_TOOL_PATTERNS: &[&str] = &[
    "code",
    "devenv",
    "idea",
    "webstorm",
    "pycharm",
    "rider",
    "clion",
    "goland",
    "phpstorm",
    "rustrover",
    "sublime_text",
    "postman",
    "insomnia",
    "bruno",
    "chrome",
    "firefox",
    "msedge",
    "brave",
    "docker",
    "windowsterminal",
    "wt",
    "powershell",
    "pwsh",
    "cmd",
    "git-bash",
    "gitkraken",
    "sourcetree",
    "dbeaver",
    "pgadmin",
    "mysqlworkbench",
    "mongodbcompass",
    "redisinsight",
    "figma",
    "obsidian",
    "notion",
    "slack",
    "discord",
    "warp",
    "alacritty",
    "wezterm",
    "cursor",
    "zed",
    "fork",
    "tableplus",
    "androidstudio",
    "unity",
    "godot",
    "blender",
    "virtualbox",
];

pub fn is_dev_tool(name: &str, target: Option<&Path>) -> bool {
    let stem = target
        .and_then(|t| t.file_stem())
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let name = name.to_lowercase();

    DEV_TOOL_PATTERNS
        .iter()
        .any(|pattern| stem == *pattern || stem.contains(pattern) || name.contains(pattern))
}

/// Identity that survives rescans, reinstalls that keep the same path, and
/// reordering — but changes if the app actually moves.
pub fn entry_id(launch_path: &Path) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    launch_path
        .to_string_lossy()
        .to_lowercase()
        .hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// User overrides, persisted next to the config so a rescan never loses them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppPreferences {
    /// True once the user has pinned or unpinned anything. Until then the
    /// dev-tool defaults apply; afterwards their list is authoritative, so
    /// unpinning one tool does not re-pin it on the next scan.
    pub configured: bool,
    pub pinned: Vec<String>,
    pub hidden: Vec<String>,
}

impl AppPreferences {
    fn path_in(dir: &Path) -> PathBuf {
        dir.join("apps.json")
    }

    pub fn load_or_default(dir: &Path) -> Self {
        std::fs::read_to_string(Self::path_in(dir))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> crate::error::Result<()> {
        std::fs::create_dir_all(dir).map_err(|e| crate::error::Error::Config(e.to_string()))?;
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::Error::Config(e.to_string()))?;
        std::fs::write(Self::path_in(dir), raw)
            .map_err(|e| crate::error::Error::Config(e.to_string()))
    }
}

/// The scanned catalog plus the user's pins.
#[derive(Default)]
pub struct AppCatalog {
    entries: RwLock<Vec<AppEntry>>,
    preferences: RwLock<AppPreferences>,
}

impl AppCatalog {
    pub fn load_preferences(&self, dir: &Path) {
        *self.preferences.write() = AppPreferences::load_or_default(dir);
    }

    /// Visible entries, pins applied, dev tools first then alphabetical.
    pub fn entries(&self) -> Vec<AppEntry> {
        self.entries.read().clone()
    }

    pub fn get(&self, id: &str) -> Option<AppEntry> {
        self.entries.read().iter().find(|e| e.id == id).cloned()
    }

    pub fn replace(&self, mut entries: Vec<AppEntry>) {
        let preferences = self.preferences.read().clone();
        let hidden: HashSet<&String> = preferences.hidden.iter().collect();
        let pinned: HashSet<&String> = preferences.pinned.iter().collect();

        entries.retain(|entry| !hidden.contains(&entry.id));
        for entry in &mut entries {
            // An explicit pin wins; otherwise dev tools start pinned, so the
            // dock is useful before the user has configured anything.
            entry.pinned = if preferences.configured {
                pinned.contains(&entry.id)
            } else {
                entry.is_dev_tool
            };
        }

        entries.sort_by(|a, b| {
            b.is_dev_tool
                .cmp(&a.is_dev_tool)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        *self.entries.write() = entries;
    }

    /// Returns the updated entry, or `None` if the id is unknown.
    pub fn set_pinned(&self, id: &str, pinned: bool, dir: &Path) -> Option<AppEntry> {
        let mut entries = self.entries.write();
        let entry = entries.iter_mut().find(|e| e.id == id)?;
        entry.pinned = pinned;
        let updated = entry.clone();
        drop(entries);

        let mut preferences = self.preferences.write();
        if !preferences.configured {
            // First user choice: freeze the current defaults into the list so
            // the rest of the dock survives.
            preferences.configured = true;
            preferences.pinned = self
                .entries
                .read()
                .iter()
                .filter(|e| e.pinned && e.id != id)
                .map(|e| e.id.clone())
                .collect();
        }
        preferences.pinned.retain(|p| p != id);
        if pinned {
            preferences.pinned.push(id.to_string());
        }
        if let Err(e) = preferences.save(dir) {
            tracing::warn!(error = %e, "could not persist app preferences");
        }
        Some(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, target: &str, id: &str) -> AppEntry {
        let target = PathBuf::from(target);
        AppEntry {
            id: id.into(),
            name: name.into(),
            launch_path: PathBuf::from(format!("C:\\{name}.lnk")),
            is_dev_tool: is_dev_tool(name, Some(&target)),
            target: Some(target),
            icon: None,
            pinned: false,
        }
    }

    #[test]
    fn recognizes_dev_tools_by_executable_stem() {
        assert!(is_dev_tool(
            "Visual Studio Code",
            Some(Path::new("C:\\x\\Code.exe"))
        ));
        assert!(is_dev_tool(
            "Postman",
            Some(Path::new("C:\\x\\Postman.exe"))
        ));
        assert!(!is_dev_tool(
            "Solitaire",
            Some(Path::new("C:\\x\\Solitaire.exe"))
        ));
    }

    #[test]
    fn dev_tools_are_pinned_and_sorted_first_before_any_user_choice() {
        let catalog = AppCatalog::default();
        catalog.replace(vec![
            entry("Solitaire", "C:\\g\\Solitaire.exe", "a"),
            entry("Postman", "C:\\d\\Postman.exe", "b"),
        ]);

        let entries = catalog.entries();
        assert_eq!(entries[0].name, "Postman");
        assert!(entries[0].pinned);
        assert!(!entries[1].pinned);
    }

    #[test]
    fn ids_are_stable_and_case_insensitive() {
        assert_eq!(
            entry_id(Path::new("C:\\Apps\\Code.lnk")),
            entry_id(Path::new("c:\\apps\\code.lnk"))
        );
        assert_ne!(
            entry_id(Path::new("C:\\a.lnk")),
            entry_id(Path::new("C:\\b.lnk"))
        );
    }
}
