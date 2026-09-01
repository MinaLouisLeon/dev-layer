import { useEffect, useState } from "react";
import { CoreBars } from "../components/CoreBars";
import { Dock } from "../components/Dock";
import { Launcher } from "../components/Launcher";
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
  onCatalog,
  onMonitorsChanged,
  refreshApps,
  setAppPinned,
  shutdown,
} from "../lib/api";
import { bytes, duration, rate } from "../lib/format";
import { useMetrics } from "../lib/useMetrics";
import type { AppEntry, HostInfo, HudContext, MonitorInfo } from "../types";

export function Hud({ label }: { label: string }) {
  const [ctx, setCtx] = useState<HudContext | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [host, setHost] = useState<HostInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [apps, setApps] = useState<AppEntry[]>([]);
  const [launcherOpen, setLauncherOpen] = useState(true);
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
    const unMonitors = onMonitorsChanged(load);
    const unCatalog = onCatalog(setApps);
    return () => {
      unMonitors.then((f) => f());
      unCatalog.then((f) => f());
    };
  }, [label]);

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

  if (error) return <div className="hud hud--error">topology error :: {error}</div>;
  if (!ctx) return <div className="hud" />;

  const { monitor, reserved, chrome } = ctx;
  const gpu = snapshot?.gpus[0] ?? null;
  const memoryPercent = snapshot?.memory.total
    ? (snapshot.memory.used / snapshot.memory.total) * 100
    : null;
  const systemDisk = snapshot?.disks[0];

  return (
    <div className={`hud hud--${chrome}`}>
      <div className="hud__scanlines" aria-hidden />

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
        <div className="slot__label">
          WINDOW REGION · {viewport.width - reserved.left - reserved.right} ×{" "}
          {viewport.height - reserved.top - reserved.bottom}
        </div>
        {chrome === "full" && launcherOpen && (
          <Launcher
            apps={apps}
            onLaunch={launch}
            onTogglePin={togglePin}
            onClose={() => setLauncherOpen(false)}
            onRefresh={() => {
              refreshApps();
              flash("rescanning Start Menu…");
            }}
          />
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
            onOpenLauncher={() => setLauncherOpen((open) => !open)}
            launcherOpen={launcherOpen}
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
