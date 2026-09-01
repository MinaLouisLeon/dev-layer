# Architecture

Decisions, and why they went that way. Written during the design discussion
that produced milestone 1; update it when a decision changes.

## 1. Overlay, not shell replacement, not reparenting

Three ways to make other apps appear inside a custom fullscreen UI on Windows:

| Approach | Verdict |
|---|---|
| `SetParent` foreign windows into ours | **Rejected.** Best-looking in theory, but breaks DPI scaling, focus/activation, modal dialogs and context menus, and misbehaves badly with Chromium apps — which covers Chrome, VS Code and Postman, i.e. the actual targets. A child crash takes the HUD with it. |
| Replace `explorer.exe` as the shell (Winlogon registry) | **Rejected.** Irreversible-feeling, breaks the tray/notifications, and a bug means booting to a blank desktop. |
| Overlay + window manager | **Chosen.** The HUD is a backdrop; apps stay real top-level windows, positioned by us. Same approach [Seelen UI](https://github.com/eythaann/Seelen-UI) takes (AGPL-3.0 — studied, not copied). |

Consequence: the HUD window is **always-on-bottom**. App windows naturally
stack above it and land inside the reserved region, which reads as "embedded"
without any window surgery.

## 2. Monitor identity is the OS device name

`MonitorInfo::id` is `\\.\DISPLAY1`, not an `HMONITOR` (recycled on hot-plug),
not an index (shifts when a display is unplugged). Everything downstream —
HUD window labels, per-monitor layout state, future per-monitor workspaces —
keys off this id, so unplugging monitor 2 in a 4-monitor setup does not
silently re-target monitor 3's state.

`index` is recomputed left-to-right on every topology change and is for display
only. Never persist it.

## 3. Two signals feed one topology refresh

`WM_DISPLAYCHANGE` / `WM_DPICHANGED` / `WM_DEVICECHANGE` give instant reaction;
a 3-second poll catches anything missed. Both feed a debounced refresh that
diffs old against new topology, so a redundant signal costs one cheap
enumeration and produces no event.

Gotcha encoded in `platform/win.rs`: the notifier window is a **hidden
top-level window**, not a message-only (`HWND_MESSAGE`) window, because
message-only windows are excluded from broadcast messages.

## 4. Physical vs logical pixels

* **Physical pixels** (`Rect`) for anything crossing into Win32 — monitor
  bounds, window positions. On a mixed-DPI multi-monitor desktop this is the
  only unambiguous coordinate space.
* **Logical pixels** (`Insets`) for HUD chrome, because CSS is logical. Convert
  with `Insets::to_physical(scale_factor)` at the boundary.

Getting this backwards is the classic multi-monitor bug: a HUD that is correct
on the 100 %-scaled external and half-sized on the 150 %-scaled laptop panel.

## 5. Teardown is a first-class subsystem

`safety::register` pairs every desktop mutation with its inverse, at the moment
the mutation succeeds. `safety::run_all` runs them in reverse order and is
wired to:

* normal exit (`RunEvent::Exit`),
* the `shutdown` command (SAFE EXIT button),
* the global exit hotkey,
* `Ctrl-C`,
* the panic hook.

A panicking teardown action is caught so one failure cannot strand the rest.
Rule for every future feature: **if it changes the desktop, it registers its
undo in the same function.** This matters far more once milestone 4 starts
restyling other people's windows.

## 6. Everything goes through the command bus

`src-tauri/src/bus` is the only IPC surface. Milestone 6 points the AI/voice
layer at the same commands, so capabilities added there are automatically
addressable by natural language. New capability → new bus command, not ad-hoc
frontend logic.

## 7. Telemetry: tiered sampling, and honest gaps

One sampler thread, one event per tick carrying a whole snapshot. Rust holds
only what it must (rate deltas, the last snapshot so a newly opened HUD paints
immediately); the frontend owns the history buffers.

Sampling is tiered because the cost is wildly uneven: reading CPU and memory
counters is nearly free, while enumerating every process is not. Cheap counters
run every tick, `slowTickEvery` gates the expensive ones, and the last result is
carried forward in between.

**GPU has two backends** because Windows exposes no single vendor-neutral API:
NVML for NVIDIA (utilization, VRAM total, temperature) and PDH performance
counters for everything else (utilization and VRAM used only). Two decisions
worth keeping:

* PDH utilization is aggregated as *sum within an engine type, max across
  engine types* — the same thing Task Manager shows. Summing all engines can
  exceed 100 %; averaging them understates load.
* Unmeasurable values are `None` all the way to the UI, which renders `—`.
  A HUD that confidently prints "GPU 0 %" on a machine it cannot read is worse
  than one that admits it does not know.

## 8. Visual encoding

Meters share one status band — nominal / warning (>= 75) / critical (>= 90) — so a
number means the same thing wherever it appears. The palette was checked with
the visualization validator: chroma, CVD separation (worst adjacent pair
ΔE 13.7 deutan), and contrast against the HUD surface all pass. The lightness
band check fails by design: it is calibrated for a `#1a1a19` chart surface,
while the HUD sits on near-black `#02080c` and deliberately uses bright, glowing
marks. Every meter prints its value, so colour is reinforcement and never the
sole encoding — and the process table doubles as the table view of the CPU
meter.

Sparklines are line-only. An area fill under a near-constant series (memory
sits at a stable percentage for hours) reads as a magnitude bar rather than a
trend, which is the wrong claim.

## 9. The application catalog

Discovery is a background thread, not a startup step: COM initialization, two
directory walks, `IShellLink` resolution per shortcut and icon rasterization
together take seconds on a machine with a lot installed. The HUD paints
immediately and the dock fills in when the scan lands (`apps::catalog`).

Decisions worth keeping:

* **Identity is the launch path**, hashed. Not the scan index (shifts), not the
  display name (duplicates across vendors). Pins therefore survive rescans.
* **Launch through the shell**, on the `.lnk` rather than the resolved target.
  Re-implementing argument and working-directory handling would only diverge
  from what the Start Menu does — including elevation prompts, which is
  behaviour we specifically do not want to reinvent.
* **Icons are rendered once.** The shell hands back a 256 px HICON; we convert
  BGRA to RGBA, crop the transparent padding jumbo icons come with, and write a
  PNG keyed by entry id. Serving them over the asset protocol (scoped to that
  one directory) keeps them out of IPC entirely and lets the webview cache them.
* **Defaults stop applying once the user chooses.** `AppPreferences.configured`
  flips on the first pin or unpin and the current defaults are frozen into the
  list, so unpinning one tool cannot resurrect it — or re-pin the others — on
  the next scan.
* **Only executables.** Start Menus are full of `.url`, `.chm` and uninstaller
  shortcuts; entries are filtered to `.exe` targets that exist on disk, minus a
  name blocklist for documentation shortcuts.

Known gap: UWP/Store apps are not enumerated — they are not `.lnk` files but
entries in the `AppsFolder` shell namespace, needing `IShellItem` enumeration
and AUMID launching. Worth adding when something you actually use lives there.

## 10. The window manager

The reconcile loop is the whole design: enumerate every window, assign each to
a monitor, ask the pure layout engine for rectangles, apply them. Events from
`SetWinEventHook` and a 2-second poll both funnel into it, debounced.

**Terminating the feedback loop.** Our own `SetWindowPos` calls generate the
same location-change events we listen to. The loop terminates because
`WindowManager::place` remembers the last rectangle it *asked for* per window
and skips a repeat. That also handles the nastier case: a window with a
minimum size that can never reach its target would otherwise be re-issued the
same move forever, one event per attempt.

**Restore is an invariant, not a feature.** A window's geometry is recorded
before the first time we move it, and `restore_all` is registered with
`safety` — so a panic puts every window back. Registration order matters and is
deliberate: the restore action is registered *before* the watcher, so teardown
(which runs in reverse) stops watching first and only then restores. Restoring
while the watcher was still live would race a re-tile against the restore.

**One source of truth for reserved space.** The tiling region is the monitor
bounds inset by the HUD's own per-monitor context (`HudManager::context`),
converted to physical pixels. The chrome and the tiling cannot disagree about
how much room the HUD takes, because they read the same number.

**Ordering is stable.** New windows are appended to the tiling order rather
than promoted, so opening something does not shuffle the window you are working
in; `promote` moves one to the main pane explicitly.

**Windows belong to the monitor holding their centre**, with an offscreen
fallback to the primary — a window left behind by an unplugged display is
adopted rather than lost.

The HUD sits *below* app windows, which makes any panel drawn over the window
region invisible once something is tiled there. The launcher therefore lifts
its HUD window above the stack while open (`set_hud_overlay`) and drops it back
on close.

Deferred: live DWM thumbnails. `DwmRegisterThumbnail` renders into a region of
*our* window, and ours is always-on-bottom — so thumbnails only make sense
alongside an overview mode that raises the HUD, which is a feature of its own
rather than a detail of this one.

## 11. Native panels

**The PTY is not hand-rolled.** ConPTY setup — pseudo-console handles, the
`PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE` dance, pipe lifetimes — is fiddly and
thoroughly solved by `portable-pty` (wezterm's). Using it also means the PTY
plumbing runs on Linux, which is how it got tested at all from a Linux host.

Each session owns two threads: one draining the PTY into events, one waiting on
the child so an exited shell removes its own session. The slave handle is
dropped immediately after spawn — hold it and the reader never sees EOF when
the shell exits, and the panel hangs on a dead session.

**Chunk-boundary decoding.** PTY reads split wherever the buffer fills, which
lands mid-character often enough to matter. `from_utf8_lossy` per chunk would
sprinkle replacement characters through any non-ASCII output (every progress
bar cargo draws). The decoder keeps an incomplete trailing sequence for the
next chunk, and still emits the valid prefix immediately so output never
stalls waiting for a continuation byte.

**Terminals die with us.** `close_all` is registered with `safety`: a crash
that left shells running would be a resource leak the user cannot see.

**HTTP over schannel, not rustls.** `native-tls` on Windows means the OS
certificate store applies, so internal HTTPS endpoints signed by a corporate CA
work without configuration — the common case for the developer this is built
for. (It also sidesteps needing a C toolchain to cross-compile `ring`.)

History de-duplicates by method+URL so re-running a call moves it to the top
rather than filling the list, and never stores response bodies.

**xterm is lazily imported.** It is ~330 KB, and with a HUD window per monitor
every display would otherwise pay to parse it at startup for a panel only one
of them will ever show.

## 12. The command layer

This is what the bus was for. `bus::registry` is a single catalog of typed
capabilities — name, description, JSON schema — consumed by the HUD through
Tauri commands and by the model as tool definitions. Adding a capability there
gives it to both; there is no second list to keep in sync.

Decisions worth keeping:

* **Guarded by omission, not by refusal.** Commands that reach outside
  dev-layer's UI (closing a window, making a network request) are filtered out
  of the catalog unless `allowGuarded` is set. A tool the model cannot see is
  one it cannot be argued into using — strictly better than a runtime refusal
  it can retry against.
* **Live state as a mid-conversation system message.** The static system
  prompt and tool list stay byte-identical between turns, so they cache; the
  volatile machine state (displays, window titles, usage) goes in a
  `{"role": "system"}` entry appended after the user's message, which is where
  the API allows it. Editing the top-level `system` each turn would invalidate
  the cached prefix on every request.
* **All tool results in one user message.** The API treats results split across
  several messages as a signal to stop making parallel calls, so a turn that
  runs three tools returns three `tool_result` blocks in a single message.
* **Tool failures are results, not errors.** A failed dispatch comes back as
  `tool_result` with `is_error: true`, so the model can apologise, retry, or
  choose another tool. Raising would end the turn with nothing useful.
* **Refusals are HTTP 200.** `stop_reason` is checked before reading content,
  and server-side fallbacks are enabled so a policy decline re-runs on a
  fallback model inside the same call rather than the turn simply stopping.
* **The loop is bounded.** `maxIterations` caps tool rounds; the failure mode
  of an unbounded agent loop is a bill, not an error message.
* **Secrets stay out of logs.** The API key is resolved from the environment
  (config is an escape hatch), and no request body is ever logged — it carries
  both the key header and the user's prompt.

Rust has no official Anthropic SDK, so this speaks the Messages API over HTTPS
with `reqwest`, already present for the HTTP panel.

Not built: voice. WebView2 has no dependable speech recognition, so it needs a
bundled local model rather than a browser API.

## Module map

```
src-tauri/src/
├─ lib.rs          app bootstrap, startup order, shutdown path
├─ safety.rs       teardown registry, panic/Ctrl-C hooks
├─ config.rs       config.json, defaults that never fail
├─ geometry.rs     Rect (physical) / Insets (logical)
├─ error.rs        error type, IPC-friendly conversion
├─ agent/         Claude tool-use loop over the command catalog
├─ apps/          Start Menu scan, catalog, pins
├─ bus/            all Tauri commands + the command catalog (registry.rs)
├─ metrics/        sampler thread, snapshot types, GPU backends
├─ monitors/       topology registry, diffing, watcher thread
├─ panels/         terminal sessions (PTY) and the HTTP client
├─ hud/            per-monitor HUD windows, reserved-region contract
├─ shell/          reversible shell mutation (taskbar auto-hide)
├─ wm/             window manager: pure layout engine, reconcile loop
└─ platform/       win/ (display, shell, apps) | stub.rs (everything else)
```

Startup order in `lib.rs::start` is deliberate: discover displays → put HUDs up
→ *only then* mutate the shell → start the watcher → register the escape hatch.
The taskbar is never hidden before there is something on screen to replace it.

## Known limits (accepted for now)

* **Elevated windows.** A non-elevated dev-layer cannot move or restyle windows
  of processes running as administrator. Milestone 4 must degrade gracefully
  rather than retry-loop.
* **Monitor names.** `EnumDisplayDevicesW` returns adapter descriptions like
  "Generic PnP Monitor". Real product names need EDID parsing — worth doing
  when the dock starts labelling displays.
* **Chromium apps** re-apply their own frames and remember their own bounds;
  milestone 4 needs per-app rules and a debounced apply loop rather than
  fighting them frame by frame.
* **GPU metrics** (milestone 2) are vendor-split: NVML for NVIDIA, PDH
  `GPU Engine` counters as the universal fallback. Task Manager reports the
  *busiest engine*, not the sum — match that or the numbers will look wrong.
