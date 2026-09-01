# dev-layer

A fullscreen developer HUD that overlays Windows — system telemetry, a dev-app
launcher, and a tiling window region, wrapped in a "Jarvis"-style interface.

It is a **layer**, not a shell replacement. `explorer.exe` keeps running, the
Winlogon registry is never touched, and every change dev-layer makes to the
desktop is undone on exit — including on crash.

## Status: milestone 6 of 6

| # | Milestone | State |
|---|---|---|
| 1 | Shell, per-monitor HUD windows, safe teardown | **done** |
| 2 | Metrics: CPU / RAM / GPU / net gauges | **done** |
| 3 | App catalog: Start Menu discovery, icons, dock | **done** |
| 4 | Window manager: tiling, layouts, per-app rules | **done** |
| 5 | Native panels: terminal, HTTP client | **done** |
| 6 | Command bus + AI command layer | **done** (voice not built) |

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

## Installing

Grab `dev-layer_<version>_x64-setup.exe` from
[Releases](../../releases) and run it. The installer is per-user, so there is no
administrator prompt. The build is unsigned, so Windows SmartScreen will warn
the first time — "More info" then "Run anyway".

Exit with `Ctrl+Alt+Shift+Q`; it restores the taskbar and every window it moved.

### Releases are automatic

| Trigger | Result |
|---|---|
| Push to `main` | The `nightly` prerelease is replaced with a fresh installer built at that commit |
| Push a `v*` tag | A permanent release for that version |
| Pull request into `main` | The installer is built and attached to the run, so a change can be tried before merging |

Every path runs `cargo test` on Windows first and stops if it fails — that is
the only place the tests run, since the rest of CI is on Linux where the Tauri
dependency chain cannot build.

To cut a stable release, bump `version` in `src-tauri/tauri.conf.json` (and
`package.json` to match), commit, then:

```bash
git tag v0.2.0 && git push origin v0.2.0
```

## Building it yourself

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
    "reserved": { "top": 34, "right": 0, "bottom": 92, "left": 280 },
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

## Applications

The dock and launcher are populated by scanning both Start Menu trees (machine
and user) on a background thread at startup: every `.lnk` is resolved through
`IShellLink`, filtered down to real executables, de-duplicated by target, and
its icon rendered once to a PNG cache.

* **Launching** goes through the shell (`ShellExecuteW`) on the `.lnk` itself,
  so the shortcut's own arguments, working directory and elevation behaviour
  apply — exactly as if you had clicked it in the Start Menu.
* **Icons** are cached under the app cache directory and served to the HUD over
  Tauri's asset protocol, scoped at runtime to that one directory. Jumbo
  (256 px) icons are cropped to their visible content, so the dock can size
  them itself.
* **Pins** live in `apps.json` beside the config. Recognized developer tools
  are pinned by default; the moment you pin or unpin anything, your list
  becomes authoritative and the defaults stop applying.
* The first scan takes a few seconds while icons are rasterized. Later starts
  reuse the cache. `RESCAN` in the launcher re-runs it.

Not yet covered: UWP/Store apps (they live in the `AppsFolder` shell namespace
rather than as `.lnk` files) and manually added entries.

## Window management

Launched apps stay real, independent top-level windows; dev-layer positions
them into the region the HUD reserves. Nothing is reparented, so nothing
destabilizes — see [docs/architecture.md](docs/architecture.md) for why.

| Layout | Shape |
|---|---|
| `mainStack` (default) | one large pane, the rest stacked beside it |
| `columns` | equal vertical columns |
| `grid` | roughly square; a short final row stretches |
| `monocle` | one window at a time, full region |
| `float` | tiling off for that display |

Layout is **per monitor**, so a 4-display setup can tile the main screen and
leave a reference display free.

| Hotkey | Action |
|---|---|
| `Ctrl+Alt+Shift+Q` | exit and restore everything |
| `Ctrl+Alt+L` | cycle layout on the focused monitor |
| `Ctrl+Alt+F` | float / unfloat the focused window |
| `Ctrl+Alt+R` | force a re-tile |

Only the exit hotkey is required; if another app already owns one of the
others, dev-layer logs it and starts without that binding.

Per-app rules live in `config.json`:

```json
"wm": {
  "enabled": true,
  "gap": 8,
  "defaultLayout": "mainStack",
  "mainRatio": 0.6,
  "ignoreProcesses": ["explorer.exe", "SearchHost.exe", "..."],
  "floatProcesses": ["Taskmgr.exe", "SnippingTool.exe", "..."]
}
```

**Every window dev-layer moves has its original geometry recorded first**, and
it is put back on exit — including on panic, on Ctrl-C, and when you turn
tiling off from the HUD.

Known limits: windows of elevated (administrator) processes cannot be moved by
a non-elevated dev-layer, and are left alone rather than retried. Chromium-based
apps sometimes restore their own bounds; `Ctrl+Alt+R` re-tiles. Live DWM
thumbnails for an overview mode are not built yet.

## Command layer

`ASK` puts a prompt in front of dev-layer's own commands. Claude is given the
bus catalog as tools and runs a tool-use loop against it — the same capabilities
the HUD's buttons call, which is why the bus was built in milestone 1.

```
› what's eating my cpu, and put this screen in columns
  system_metrics → set_layout
  rustc.exe is taking 24.6% — that's your cargo build, and memory is at 67%.
  Switched this display to columns.
```

Set `ANTHROPIC_API_KEY` in the environment before starting (a key can go in
`config.json` instead, but it sits there in plaintext).

```json
"agent": {
  "enabled": true,
  "model": "claude-opus-5",
  "effort": "high",
  "maxIterations": 8,
  "allowGuarded": false,
  "apiKey": null
}
```

**Guarded commands are off by default.** Closing windows and making HTTP
requests reach outside dev-layer's own UI, so they are not even shown to the
model unless you set `allowGuarded` — a tool it cannot see is one it cannot
talk itself into using. Everything else (launching apps, layouts, focus,
telemetry, displays) is always available.

**What leaves the machine:** your prompt, a short state summary (display list,
open window *titles* and process names, CPU/memory/GPU figures, app count), and
the results of any tools the model calls. Window titles routinely contain file
paths and branch names — if that matters for a given session, close the panel
or set `enabled: false`.

Voice input is **not built**. WebView2 has no reliable speech recognition, so
it needs a bundled local model (whisper.cpp) rather than a browser API — a
milestone of its own rather than a detail of this one.

## The workbench

`WORK` opens [Mino Workbench](https://github.com/MinaLouisLeon/mino-workbench)
inside dev-layer: a lazily-loaded file tree and a file viewer, rooted at a
folder you choose. One app, one installer — the workbench is not a second
program to install.

Its `mino-core` crate is vendored into `src-tauri/crates/mino-core` (MIT, see
that directory's `LICENSE`). Nothing in dev-layer touches the filesystem
directly: every call goes through that crate's `Transport`, which means its
path guard applies — a session is confined to the folder it was opened on —
and the same panel will serve an SSH host once the remote target is wired
through.

## Native panels

Two tools live *inside* the HUD rather than as more windows to tile, on the
rule from the original design discussion: heavyweight apps you cannot replace
stay real windows; small tools you reach for constantly are better as panels.

**Terminal** (`TERM`) — a real shell on a pseudo-console, rendered with xterm.
PowerShell 7 if installed, then Windows PowerShell, then `COMSPEC`; override in
config. Sessions are killed on exit, including on panic, so dev-layer never
leaves orphaned shells behind.

**HTTP client** (`HTTP`) — the "don't open Postman for one call" panel. Method,
URL, headers, body; response with status, elapsed time, size, pretty-printed
JSON and a collapsible header list. The last 25 requests are remembered in
`requests.json` and recalled with one click. TLS goes through schannel, so the
Windows certificate store applies — internal endpoints with a corporate CA just
work.

```json
"panels": {
  "shell": null,
  "startupDir": null
}
```

Panels open over the window region and lift the HUD above the tiled apps while
open. Not built: git and docker panels.

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
