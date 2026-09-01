//! dev-layer — a fullscreen developer HUD that overlays Windows.
//!
//! Milestone 1 covers the parts everything else depends on:
//!   * one HUD window per monitor, reconciled on every topology change;
//!   * reversible shell mutation (taskbar auto-hide);
//!   * a teardown path that survives panics, Ctrl-C, and a global hotkey.
//!
//! Deliberately *not* here yet: metrics (m2), app catalog/dock (m3), the
//! window manager (m4).

pub mod agent;
pub mod apps;
pub mod bus;
pub mod config;
pub mod error;
pub mod geometry;
pub mod hud;
pub mod metrics;
pub mod monitors;
pub mod panels;
pub mod platform;
pub mod safety;
pub mod shell;
pub mod wm;

use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::agent::AgentSession;
use crate::apps::AppCatalog;
use crate::config::Config;
use crate::hud::HudManager;
use crate::metrics::MetricsStore;
use crate::monitors::MonitorRegistry;
use crate::panels::TerminalSessions;
use crate::wm::WindowManager;

/// Shared, read-mostly application state.
pub struct AppState {
    pub config: Config,
    pub monitors: MonitorRegistry,
    pub hud: HudManager,
    pub metrics: MetricsStore,
    pub apps: AppCatalog,
    pub wm: std::sync::Arc<WindowManager>,
    pub terminals: std::sync::Arc<TerminalSessions>,
    pub agent: AgentSession,
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
            bus::list_apps,
            bus::launch_app,
            bus::set_app_pinned,
            bus::refresh_apps,
            bus::list_windows,
            bus::window_layouts,
            bus::set_window_layout,
            bus::cycle_window_layout,
            bus::focus_window,
            bus::toggle_window_float,
            bus::promote_window,
            bus::close_window,
            bus::set_wm_enabled,
            bus::retile,
            bus::set_hud_overlay,
            bus::terminal_open,
            bus::terminal_write,
            bus::terminal_resize,
            bus::terminal_close,
            bus::http_send,
            bus::http_history,
            bus::agent_ask,
            bus::agent_status,
            bus::agent_reset,
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
        apps: AppCatalog::default(),
        wm: std::sync::Arc::new(WindowManager::new(config.wm.clone())),
        terminals: std::sync::Arc::new(TerminalSessions::default()),
        agent: AgentSession::default(),
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

    // 5. Discover installed applications in the background; the dock fills in
    //    when the scan lands rather than delaying first paint.
    app.state::<AppState>().apps.load_preferences(&config_dir);
    allow_icon_cache(app)?;
    apps::spawn_scan(app.clone());

    // 6. Take over window placement. The restore action is registered *before*
    //    the watcher, so teardown runs in the right order: stop watching, then
    //    put every window back.
    {
        let manager = app.state::<AppState>().wm.clone();
        safety::register("restore windows", move || manager.restore_all());
    }
    wm::spawn_watcher(app.clone())?;

    // 7. Panel sessions are processes we own: they must die with us, or the
    //    user is left with orphaned shells after every exit.
    {
        let terminals = app.state::<AppState>().terminals.clone();
        safety::register("close terminals", move || terminals.close_all());
    }

    // 8. The escape hatch, available even if the HUD stops responding.
    register_hotkeys(app, &config)?;

    tracing::info!(
        exit = %config.hotkeys.exit,
        cycle_layout = %config.hotkeys.cycle_layout,
        "dev-layer ready"
    );
    Ok(())
}

/// Icons are served to the HUD over the asset protocol, which is deny-by-default;
/// grant exactly the cache directory the scanner writes to.
fn allow_icon_cache(app: &AppHandle) -> error::Result<()> {
    let icon_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| error::Error::Config(e.to_string()))?
        .join("icons");
    std::fs::create_dir_all(&icon_dir).map_err(|e| error::Error::Config(e.to_string()))?;

    app.asset_protocol_scope()
        .allow_directory(&icon_dir, false)
        .map_err(|e| error::Error::Config(format!("icon cache scope: {e}")))?;
    Ok(())
}

fn register_hotkeys(app: &AppHandle, config: &Config) -> error::Result<()> {
    // The exit hotkey is the one that must never fail to register: it is the
    // way out if anything else misbehaves.
    bind(app, &config.hotkeys.exit, "exit", |app| {
        tracing::info!("exit hotkey pressed");
        shutdown(app);
    })?;

    // The rest are conveniences; a clash with another app's binding should not
    // stop dev-layer from starting.
    let optional: [(&str, fn(&AppHandle)); 3] = [
        (&config.hotkeys.cycle_layout, |app| {
            if let Some(monitor) = active_monitor(app) {
                let kind = app.state::<AppState>().wm.cycle_layout(&monitor);
                tracing::info!(?kind, monitor, "layout cycled");
                wm::retile(app);
            }
        }),
        (&config.hotkeys.toggle_float, |app| {
            if let Some(id) = platform::sys::foreground_window() {
                app.state::<AppState>().wm.toggle_float(id as i64);
                wm::retile(app);
            }
        }),
        (&config.hotkeys.retile, |app| wm::retile(app)),
    ];

    for (binding, action) in optional {
        if let Err(e) = bind(app, binding, "window manager", action) {
            tracing::warn!(binding, error = %e, "hotkey unavailable; continuing without it");
        }
    }
    Ok(())
}

fn bind(app: &AppHandle, binding: &str, what: &str, action: fn(&AppHandle)) -> error::Result<()> {
    let shortcut: Shortcut = binding
        .parse()
        .map_err(|e| error::Error::Config(format!("bad {what} hotkey {binding:?}: {e}")))?;

    app.global_shortcut()
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                action(app);
            }
        })
        .map_err(|e| error::Error::Platform(format!("could not register {what} hotkey: {e}")))
}

/// The monitor holding the focused window, falling back to the primary.
fn active_monitor(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    state
        .wm
        .windows()
        .iter()
        .find(|w| w.focused)
        .map(|w| w.monitor_id.clone())
        .or_else(|| {
            state
                .monitors
                .snapshot()
                .into_iter()
                .find(|m| m.is_primary)
                .map(|m| m.id)
        })
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
