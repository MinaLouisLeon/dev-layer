# dev-layer

A fullscreen developer HUD that overlays Windows — system telemetry, a dev-app
launcher, and a tiling window region, wrapped in a "Jarvis"-style interface.

It is a **layer**, not a shell replacement. `explorer.exe` keeps running, the
Winlogon registry is never touched, and every change dev-layer makes to the
desktop is undone on exit — including on crash.

## Status: milestone 2 of 6

| # | Milestone | State |
|---|---|---|
| 1 | Shell, per-monitor HUD windows, safe teardown | **done** |
| 2 | Metrics: CPU / RAM / GPU / net gauges | **done** |
| 3 | App catalog: Start Menu discovery, icons, dock | not started |
| 4 | Window manager: slots, layouts, DWM thumbnails | not started |
| 5 | Native panels: terminal, REST client, git/docker | not started |
| 6 | Command bus + AI/voice layer | bus scaffolded |

## How apps end up "inside" the HUD

Windows does not let one app host another's windows reliably, so dev-layer
never reparents a foreign `HWND` (that path breaks DPI, focus, dialogs, and
every Chromium-based app — which is most dev tooling). Instead:

* Each monitor gets a fullscreen, transparent, **always-on-bottom** HUD window.
* The HUD reserves margins for its chrome and leaves a *window region* free.
* Launched apps stay real, independent top-level windows — the window manager
  (milestone 4) just positions them into that region with `SetWindowPos`.

They look embedded; Windows still owns them, so nothing destabilizes.

## Stack

* **Rust + Tauri v2** — Win32 access via the `windows` crate, WebView2 frontend.
  Chosen over Electron mainly because a tool that reports your RAM should not
  eat 400 MB of it.
* **React + TypeScript**, hand-written CSS for the HUD chrome.
* No native code in the frontend; everything system-level crosses the
  command bus in `src-tauri/src/bus`.

## Running it

Prerequisites on Windows 10/11: Rust (stable), Node 18+, and the WebView2
runtime (preinstalled on Windows 11).

```bash
npm install
npm run tauri dev      # HUD on every connected display
npm run tauri build    # NSIS installer in src-tauri/target/release/bundle
```

**Exit: `Ctrl+Alt+Shift+Q`** — global, works even if the HUD stops responding,
and restores the taskbar. The `SAFE EXIT` button does the same thing.

### Developing off Windows

`cargo` builds on Linux/macOS against a stub platform layer that reports one
synthetic display and never touches the shell, so the frontend can be iterated
anywhere. Type-check the real Win32 path from any OS with:

```bash
rustup target add x86_64-pc-windows-msvc
cargo check --target x86_64-pc-windows-msvc --all-targets
```

## Configuration

Written on first run to `%APPDATA%\dev.devlayer.hud\config.json`:

```json
{
  "hud": {
    "reserved": { "top": 34, "right": 0, "bottom": 34, "left": 280 },
    "reservedMinimal": { "top": 34, "right": 0, "bottom": 34, "left": 0 },
    "fullChromeOnSecondary": false
  },
  "shell": { "hideTaskbar": true },
  "metrics": {
    "intervalMs": 1000,
    "slowTickEvery": 3,
    "topProcesses": 6,
    "historySamples": 120
  },
  "hotkeys": { "exit": "Ctrl+Alt+Shift+Q" }
}
```

`reserved` is in logical (CSS) pixels and must match the rail sizes in
`src/styles.css`. A malformed config logs a warning and falls back to defaults
rather than leaving you without a shell.

## Telemetry

One Rust thread samples on a fixed tick and pushes a whole snapshot to the HUD
as a single event; the frontend owns the history for sparklines. Sampling is
tiered — cheap counters (CPU, memory, GPU, network) every tick, expensive ones
(process table, disks) every third — because a tool that reports your CPU usage
should not show up in its own graph.

GPU is the awkward one, since Windows has no single vendor-neutral API:

| Backend | Covers | Reports |
|---|---|---|
| **NVML** (`nvidia` feature, default on) | NVIDIA only | utilization, VRAM used **and total**, temperature |
| **PDH counters** | any GPU, same source as Task Manager | utilization, VRAM used |

NVML is preferred when present, PDH is the fallback. Utilization from PDH is
the **busiest engine type** (3D, Copy, VideoDecode…), matching Task Manager —
summing engines would cheerfully report 300 %. Anything a backend cannot
measure is shown as `—`, never as `0`.

Build without NVML (no NVIDIA hardware, smaller build):

```bash
npm run tauri build -- --no-default-features
```

## Multi-monitor

Built for 3–4 displays from the start, even on a single-monitor machine:

* One HUD window per display, keyed by the OS device name (`\\.\DISPLAY1`),
  which is stable across hot-plugs and reboots.
* Displays plugged, unplugged, rearranged, or rescaled are picked up from
  `WM_DISPLAYCHANGE`/`WM_DPICHANGED`, with a 3-second poll as a safety net.
* Per-monitor DPI is read individually — mixed-DPI setups scale correctly.
* Full chrome on the primary display, minimal chrome elsewhere by default.

See [docs/architecture.md](docs/architecture.md) for the decisions behind all
of this.
