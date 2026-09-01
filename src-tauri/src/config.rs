use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::geometry::Insets;

/// User configuration, read once at startup from `<app config dir>/config.json`.
/// Missing fields fall back to defaults, so the file can stay sparse.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub hud: HudConfig,
    pub shell: ShellConfig,
    pub hotkeys: HotkeyConfig,
    pub metrics: MetricsConfig,
    pub wm: WmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HudConfig {
    /// Space the HUD chrome reserves on every monitor, in logical pixels.
    /// Must match the rail sizes in `src/styles.css`.
    pub reserved: Insets,
    /// Space reserved on non-primary monitors when they only get minimal chrome.
    pub reserved_minimal: Insets,
    /// Draw full chrome (topology rail, dock) on secondary monitors too.
    pub full_chrome_on_secondary: bool,
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            reserved: Insets {
                top: 34,
                right: 0,
                bottom: 34,
                left: 220,
            },
            reserved_minimal: Insets {
                top: 34,
                right: 0,
                bottom: 34,
                left: 0,
            },
            full_chrome_on_secondary: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MetricsConfig {
    /// Sampling tick. 1 s is the sweet spot: fast enough to feel live, slow
    /// enough that the sampler stays invisible in its own CPU graph.
    pub interval_ms: u64,
    /// How many ticks between the expensive refreshes (process table, disks).
    pub slow_tick_every: u32,
    /// Processes shown in the HUD, ranked by CPU.
    pub top_processes: usize,
    /// Samples the HUD keeps for sparklines; 120 at 1 s is two minutes.
    pub history_samples: usize,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            slow_tick_every: 3,
            top_processes: 6,
            history_samples: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ShellConfig {
    /// Put the Windows taskbar into auto-hide while dev-layer runs.
    /// Always restored on exit, including on panic (see `safety`).
    pub hide_taskbar: bool,
}

impl Default for ShellConfig {
    fn default() -> Self {
        Self { hide_taskbar: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HotkeyConfig {
    /// Global escape hatch: restores the shell and exits, from anywhere.
    pub exit: String,
    /// Cycles the layout on the monitor holding the focused window.
    pub cycle_layout: String,
    /// Takes the focused window in or out of tiling.
    pub toggle_float: String,
    /// Forces a re-tile, for when an app has fought its way out of place.
    pub retile: String,
}

impl Default for HotkeyConfig {
    fn default() -> Self {
        Self {
            exit: "Ctrl+Alt+Shift+Q".into(),
            cycle_layout: "Ctrl+Alt+L".into(),
            toggle_float: "Ctrl+Alt+F".into(),
            retile: "Ctrl+Alt+R".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WmConfig {
    /// Tiling on or off. Off leaves every window exactly where it is.
    pub enabled: bool,
    /// Space between tiles, in physical pixels.
    pub gap: i32,
    pub default_layout: crate::wm::LayoutKind,
    /// Share of the region the main pane takes in MainStack.
    pub main_ratio: f32,
    /// Never managed, never listed: shell surfaces and system dialogs.
    pub ignore_processes: Vec<String>,
    /// Managed but never tiled - utilities that are useless at tile size.
    pub float_processes: Vec<String>,
}

impl Default for WmConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            gap: 8,
            default_layout: crate::wm::LayoutKind::MainStack,
            main_ratio: 0.6,
            ignore_processes: [
                "explorer.exe",
                "ApplicationFrameHost.exe",
                "SystemSettings.exe",
                "TextInputHost.exe",
                "ShellExperienceHost.exe",
                "SearchHost.exe",
                "StartMenuExperienceHost.exe",
                "LockApp.exe",
                "PickerHost.exe",
                "SecurityHealthSystray.exe",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
            float_processes: ["Taskmgr.exe", "magnify.exe", "SnippingTool.exe", "mmc.exe"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

impl WmConfig {
    pub fn is_ignored(&self, process: &str) -> bool {
        self.ignore_processes
            .iter()
            .any(|p| p.eq_ignore_ascii_case(process))
    }

    pub fn floats_by_default(&self, process: &str) -> bool {
        self.float_processes
            .iter()
            .any(|p| p.eq_ignore_ascii_case(process))
    }
}

impl Config {
    pub fn path_in(dir: &Path) -> PathBuf {
        dir.join("config.json")
    }

    /// Never fails: an unreadable or malformed config logs and yields defaults,
    /// because a broken config file must not leave the desktop without a shell.
    pub fn load_or_default(dir: &Path) -> Config {
        let path = Self::path_in(dir);
        match std::fs::read_to_string(&path) {
            Ok(raw) => match serde_json::from_str(&raw) {
                Ok(cfg) => cfg,
                Err(e) => {
                    tracing::warn!(?path, error = %e, "malformed config, using defaults");
                    Config::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let cfg = Config::default();
                if let Err(e) = cfg.save(dir) {
                    tracing::warn!(error = %e, "could not write default config");
                }
                cfg
            }
            Err(e) => {
                tracing::warn!(?path, error = %e, "unreadable config, using defaults");
                Config::default()
            }
        }
    }

    pub fn save(&self, dir: &Path) -> crate::error::Result<()> {
        std::fs::create_dir_all(dir).map_err(|e| crate::error::Error::Config(e.to_string()))?;
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| crate::error::Error::Config(e.to_string()))?;
        std::fs::write(Self::path_in(dir), raw)
            .map_err(|e| crate::error::Error::Config(e.to_string()))
    }
}
