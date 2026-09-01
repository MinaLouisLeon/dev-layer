import { lazy, Suspense, useEffect, useState } from "react";
import { CoreBars } from "../components/CoreBars";
import { Dock, type Overlay } from "../components/Dock";
import { Launcher } from "../components/Launcher";
import { LayoutSwitcher } from "../components/LayoutSwitcher";
import { TileOutlines } from "../components/TileOutlines";
import { WindowList } from "../components/WindowList";
import { Meter } from "../components/Meter";
import { ProcessTable } from "../components/ProcessTable";
import { Readout } from "../components/Ring";
import { Sparkline } from "../components/Sparkline";
import {
  getHostInfo,
  hudContext,
  launchApp,
  listApps,
  listMonitors,
  listWindows,
  onCatalog,
  onMonitorsChanged,
  onWindows,
  refreshApps,
  setAppPinned,
  setHudOverlay,
  shutdown,
  windowLayouts,
} from "../lib/api";
import { bytes, duration, rate } from "../lib/format";
import { useMetrics } from "../lib/useMetrics";
import type {
  AppEntry,
  HostInfo,
  HudContext,
  ManagedWindow,
  MonitorInfo,
  MonitorLayout,
} from "../types";

// xterm is ~300 KB and only one HUD window is ever showing a terminal; every
// other monitor's HUD should not pay to parse it at startup.
const TerminalPanel = lazy(() => import("../components/TerminalPanel"));
const HttpPanel = lazy(() => import("../components/HttpPanel"));
const AskPanel = lazy(() => import("../components/AskPanel"));

export function Hud({ label }: { label: string }) {
  const [ctx, setCtx] = useState<HudContext | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [host, setHost] = useState<HostInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [windows, setWindows] = useState<ManagedWindow[]>([]);
  const [layouts, setLayouts] = useState<MonitorLayout[]>([]);
  const [overlay, setOverlay] = useState<Overlay>("launcher");
  const [notice, setNotice] = useState<string | null>(null);
  const [clock, setClock] = useState(() => new Date());
  // Insets are logical (CSS) pixels; the viewport is the only logical-pixel
  // measure of this monitor, since `bounds` is physical.
  const [viewport, setViewport] = useState({
    width: window.innerWidth,
    height: window.innerHeight,
  });

  const { snapshot, history } = useMetrics();

  useEffect(() => {
    const onResize = () =>
      setViewport({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", onResize);
    const tick = window.setInterval(() => setClock(new Date()), 1000);
    return () => {
      window.removeEventListener("resize", onResize);
      window.clearInterval(tick);
    };
  }, []);

  useEffect(() => {
    const load = () =>
      Promise.all([hudContext(label), listMonitors()])
        .then(([c, m]) => {
          setCtx(c);
          setMonitors(m);
          setError(null);
        })
        .catch((e) => setError(String(e)));

    load();
    getHostInfo().then(setHost).catch(() => {});
    listApps().then(setApps).catch(() => {});
    listWindows().then(setWindows).catch(() => {});
    windowLayouts().then(setLayouts).catch(() => {});
    const unMonitors = onMonitorsChanged(load);
    const unCatalog = onCatalog(setApps);
    const unWindows = onWindows((next) => {
      setWindows(next);
      windowLayouts().then(setLayouts).catch(() => {});
    });
    return () => {
      unMonitors.then((f) => f());
      unCatalog.then((f) => f());
      unWindows.then((f) => f());
    };
  }, [label]);

  // The HUD lives below every app window, so any panel has to be lifted above
  // them while it is open, and dropped back afterwards.
  useEffect(() => {
    setHudOverlay(label, overlay !== null).catch(() => {});
  }, [label, overlay]);

  const flash = (message: string) => {
    setNotice(message);
    window.setTimeout(() => setNotice(null), 4000);
  };

  const launch = (app: AppEntry) =>
    launchApp(app.id)
      .then(() => flash(`launched ${app.name}`))
      .catch((e) => flash(String(e)));

  const togglePin = (app: AppEntry) =>
    setAppPinned(app.id, !app.pinned)
      .then(setApps)
      .catch((e) => flash(String(e)));

  const refreshWindows = () => {
    listWindows().then(setWindows).catch(() => {});
    windowLayouts().then(setLayouts).catch(() => {});
  };

  if (error) return <div className="hud hud--error">topology error :: {error}</div>;
  if (!ctx) return <div className="hud" />;

  const { monitor, reserved, chrome } = ctx;
  const gpu = snapshot?.gpus[0] ?? null;
  const memoryPercent = snapshot?.memory.total
    ? (snapshot.memory.used / snapshot.memory.total) * 100
    : null;
  const systemDisk = snapshot?.disks[0];
  const mine = windows.filter((w) => w.monitorId === monitor.id);
  const layout = layouts.find((l) => l.monitorId === monitor.id);

  return (
    <div className={`hud hud--${chrome}`}>
      <div className="hud__scanlines" aria-hidden />
      <TileOutlines windows={mine} monitor={monitor} />

      {/* The region the window manager will tile app windows into (milestone 4). */}
      <div
        className="slot"
        style={{
          top: `${reserved.top}px`,
          right: `${reserved.right}px`,
          bottom: `${reserved.bottom}px`,
          left: `${reserved.left}px`,
        }}
      >
        <Bracket corner="tl" />
        <Bracket corner="tr" />
        <Bracket corner="bl" />
        <Bracket corner="br" />
        {/* Only worth saying when the region is empty; once windows are
            tiled, each tile carries its own label. */}
        {mine.length === 0 && overlay === null && (
          <div className="slot__label">
            WINDOW REGION · {viewport.width - reserved.left - reserved.right} ×{" "}
            {viewport.height - reserved.top - reserved.bottom}
            {layout ? ` · ${layout.kind}` : ""}
          </div>
        )}
        {chrome === "full" && overlay === "launcher" && (
          <Launcher
            apps={apps}
            onLaunch={launch}
            onTogglePin={togglePin}
            onClose={() => setOverlay(null)}
            onRefresh={() => {
              refreshApps();
              flash("rescanning Start Menu…");
            }}
          />
        )}
        {chrome === "full" &&
          (overlay === "terminal" || overlay === "http" || overlay === "agent") && (
            <Suspense fallback={<div className="panel panel--loading dim">loading panel…</div>}>
              {overlay === "terminal" ? (
                <TerminalPanel onClose={() => setOverlay(null)} />
              ) : overlay === "http" ? (
                <HttpPanel onClose={() => setOverlay(null)} />
              ) : (
                <AskPanel onClose={() => setOverlay(null)} />
              )}
            </Suspense>
          )}
      </div>

      <header className="rail rail--top">
        <span className="tag">DEV-LAYER</span>
        {host && (
          <span className="dim" title={`${host.os} · ${host.cpuBrand}`}>
            {host.hostname} · {host.logicalCores}T
          </span>
        )}
        {chrome === "minimal" && snapshot && (
          <span className="readouts">
            <Readout label="CPU" value={snapshot.cpu.usage} />
            <Readout label="MEM" value={memoryPercent} />
            <Readout label="GPU" value={gpu?.utilization ?? null} />
          </span>
        )}
        <span className="spacer" />
        {layout && (
          <LayoutSwitcher
            monitorId={monitor.id}
            active={layout.kind}
            onChanged={refreshWindows}
          />
        )}
        <span className="dim">
          {monitor.name} · {monitor.bounds.width}×{monitor.bounds.height} ·{" "}
          {Math.round(monitor.scaleFactor * 100)}%
        </span>
        {monitor.isPrimary && <span className="tag tag--accent">PRIMARY</span>}
        <span className="clock">{clock.toLocaleTimeString([], { hour12: false })}</span>
      </header>

      <aside className="rail rail--left">
        <Meter
          caption="CPU"
          value={snapshot?.cpu.usage ?? null}
          detail={
            snapshot?.cpu.frequencyMhz
              ? `${(snapshot.cpu.frequencyMhz / 1000).toFixed(2)} GHz`
              : "—"
          }
          history={history.cpu}
        />
        <Meter
          caption="MEM"
          value={memoryPercent}
          detail={
            snapshot
              ? `${bytes(snapshot.memory.used)} / ${bytes(snapshot.memory.total)}`
              : "—"
          }
          history={history.memory}
        />
        <Meter
          caption="GPU"
          value={gpu?.utilization ?? null}
          detail={gpuDetail(gpu)}
          history={history.gpu}
        />

        <section className="block">
          <h2>CORES</h2>
          <CoreBars cores={snapshot?.cpu.perCore ?? []} />
        </section>

        <section className="block">
          <h2>NET</h2>
          <Sparkline values={history.net} height={18} />
          <p className="netline">
            <span>↓ {rate(snapshot?.network.rxPerSec ?? 0)}</span>
            <span>↑ {rate(snapshot?.network.txPerSec ?? 0)}</span>
          </p>
        </section>

        <section className="block">
          <h2>
            WINDOWS <span className="dim">· {mine.length}</span>
          </h2>
          <WindowList windows={mine} onChanged={refreshWindows} />
        </section>

        <section className="block block--grow">
          <h2>TOP PROCESSES</h2>
          <ProcessTable processes={snapshot?.processes ?? []} />
        </section>

        <section className="block">
          <h2>TOPOLOGY</h2>
          <ol className="monitors">
            {monitors.map((m) => (
              <li key={m.id} className={m.id === monitor.id ? "is-self" : ""}>
                <b>{m.index}</b>
                <span>{m.name}</span>
                <em>
                  {m.bounds.width}×{m.bounds.height}
                </em>
              </li>
            ))}
          </ol>
        </section>
      </aside>

      <footer className="rail rail--bottom">
        <div className="status">
          <span className="dim">
            {systemDisk
              ? `${systemDisk.mount} ${bytes(systemDisk.used)} / ${bytes(systemDisk.total)}`
              : "disk —"}
          </span>
          <span className="dim">up {duration(snapshot?.uptimeSecs ?? 0)}</span>
          <span className={notice ? "notice" : "dim"}>
            {notice ??
              (snapshot
                ? `sampled ${new Date(snapshot.timestampMs).toLocaleTimeString([], { hour12: false })}`
                : "waiting for sampler…")}
          </span>
        </div>

        {chrome === "full" && (
          <Dock
            apps={apps.filter((a) => a.pinned)}
            onLaunch={launch}
            onUnpin={togglePin}
            overlay={overlay}
            onOverlay={setOverlay}
          />
        )}

        <button className="exit" onClick={() => shutdown()}>
          SAFE EXIT
        </button>
      </footer>
    </div>
  );
}

function gpuDetail(gpu: { memoryUsed: number | null; memoryTotal: number | null; temperatureC: number | null; source: string } | null): string {
  if (!gpu) return "no GPU backend";
  const parts: string[] = [];
  if (gpu.memoryUsed != null) {
    parts.push(gpu.memoryTotal ? `${bytes(gpu.memoryUsed)} / ${bytes(gpu.memoryTotal)}` : bytes(gpu.memoryUsed));
  }
  if (gpu.temperatureC != null) parts.push(`${gpu.temperatureC}°C`);
  // PDH cannot report VRAM capacity or temperature; say so rather than imply zero.
  return parts.length ? parts.join(" · ") : `${gpu.source} · usage only`;
}

function Bracket({ corner }: { corner: "tl" | "tr" | "bl" | "br" }) {
  return <span className={`bracket bracket--${corner}`} aria-hidden />;
}
