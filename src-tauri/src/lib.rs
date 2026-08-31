//! dev-layer — a fullscreen developer HUD that overlays Windows.
//!
//! Milestone 1 covers the parts everything else depends on:
//!   * one HUD window per monitor, reconciled on every topology change;
//!   * reversible shell mutation (taskbar auto-hide);
//!   * a teardown path that survives panics, Ctrl-C, and a global hotkey.
//!
//! Deliberately *not* here yet: metrics (m2), app catalog/dock (m3), the
//! window manager (m4).

pub mod bus;
pub mod config;
pub mod error;
pub mod geometry;
pub mod hud;
pub mod metrics;
pub mod monitors;
pub mod platform;
pub mod safety;
pub mod shell;

use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::config::Config;
use crate::hud::HudManager;
use crate::metrics::MetricsStore;
use crate::monitors::MonitorRegistry;

/// Shared, read-mostly application state.
pub struct AppState {
    pub config: Config,
    pub monitors: MonitorRegistry,
    pub hud: HudManager,
    pub metrics: MetricsStore,
}

pub fn run() {
    init_tracing();

    // Before anything mutates the desktop.
    safety::install_hooks();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            bus::list_monitors,
            bus::get_config,
            bus::hud_context,
            bus::refresh_monitors,
            bus::latest_metrics,
            bus::host_info,
            bus::shutdown,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            if let Err(e) = start(&handle) {
                // Never leave a half-applied desktop behind.
                tracing::error!(error = %e, "startup failed");
                safety::run_all();
                return Err(Box::new(e) as Box<dyn std::error::Error>);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build dev-layer");

    app.run(|_app, event| {
        if let RunEvent::Exit = event {
            safety::run_all();
        }
    });
}

fn start(app: &AppHandle) -> error::Result<()> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| error::Error::Config(e.to_string()))?;
    let config = Config::load_or_default(&config_dir);
    tracing::info!(?config_dir, "config loaded");

    app.manage(AppState {
        config: config.clone(),
        monitors: MonitorRegistry::default(),
        hud: HudManager::default(),
        metrics: MetricsStore::default(),
    });

    // 1. Discover displays and put a HUD on each one.
    let state = app.state::<AppState>();
    state.monitors.refresh()?;
    let monitors = state.monitors.snapshot();
    tracing::info!(count = monitors.len(), "displays detected");
    state.hud.reconcile(app, &monitors, &config)?;

    // 2. Mutate the shell only once the HUD is actually up.
    shell::apply(&config)?;

    // 3. React to displays being plugged, unplugged, or rearranged.
    monitors::spawn_watcher(app.clone())?;

    // 4. Telemetry: the HUD's reason to exist.
    metrics::spawn_sampler(app.clone(), config.metrics.clone())?;

    // 5. The escape hatch, available even if the HUD stops responding.
    register_exit_hotkey(app, &config)?;

    tracing::info!(exit = %config.hotkeys.exit, "dev-layer ready");
    Ok(())
}

fn register_exit_hotkey(app: &AppHandle, config: &Config) -> error::Result<()> {
    let shortcut: Shortcut = config.hotkeys.exit.parse().map_err(|e| {
        error::Error::Config(format!("bad exit hotkey {:?}: {e}", config.hotkeys.exit))
    })?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                tracing::info!("exit hotkey pressed");
                shutdown(app);
            }
        })
        .map_err(|e| error::Error::Platform(format!("could not register exit hotkey: {e}")))?;

    Ok(())
}

/// Single shutdown path: restore everything we changed, then exit.
pub fn shutdown(app: &AppHandle) {
    safety::run_all();
    app.exit(0);
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_env("DEV_LAYER_LOG")
        .unwrap_or_else(|_| EnvFilter::new("dev_layer_lib=debug,warn"));

    let _ = fmt().with_env_filter(filter).with_target(true).try_init();
}
