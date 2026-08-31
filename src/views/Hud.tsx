import { useEffect, useState } from "react";
import { hudContext, listMonitors, onMonitorsChanged, shutdown } from "../lib/api";
import type { HudContext, MonitorInfo } from "../types";

/**
 * Milestone 1 HUD: proves the per-monitor window model, the reserved-inset
 * contract the window manager will consume, and the safe-exit path.
 * Real gauges arrive in milestone 2, the dock in milestone 3.
 */
export function Hud({ label }: { label: string }) {
  const [ctx, setCtx] = useState<HudContext | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  // Insets are logical (CSS) pixels; the viewport is the only logical-pixel
  // measure of this monitor, since `bounds` is physical.
  const [viewport, setViewport] = useState({
    width: window.innerWidth,
    height: window.innerHeight,
  });

  useEffect(() => {
    const onResize = () =>
      setViewport({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
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
    const un = onMonitorsChanged(load);
    return () => {
      un.then((f) => f());
    };
  }, [label]);

  if (error) return <div className="hud hud--error">topology error :: {error}</div>;
  if (!ctx) return <div className="hud" />;

  const { monitor, reserved, chrome } = ctx;
  const slot = {
    top: `${reserved.top}px`,
    right: `${reserved.right}px`,
    bottom: `${reserved.bottom}px`,
    left: `${reserved.left}px`,
  };

  return (
    <div className={`hud hud--${chrome}`}>
      <div className="hud__scanlines" aria-hidden />

      {/* The region the window manager will tile app windows into (milestone 4). */}
      <div className="slot" style={slot}>
        <Bracket corner="tl" />
        <Bracket corner="tr" />
        <Bracket corner="bl" />
        <Bracket corner="br" />
        <div className="slot__label">
          WINDOW REGION · {viewport.width - reserved.left - reserved.right} ×{" "}
          {viewport.height - reserved.top - reserved.bottom}
        </div>
      </div>

      <header className="rail rail--top">
        <span className="tag">DEV-LAYER</span>
        <span className="dim">v0.1.0 · milestone 1</span>
        <span className="spacer" />
        <span className="dim">
          {monitor.name} · {monitor.bounds.width}×{monitor.bounds.height} ·{" "}
          {Math.round(monitor.scaleFactor * 100)}%
        </span>
        {monitor.isPrimary && <span className="tag tag--accent">PRIMARY</span>}
      </header>

      <aside className="rail rail--left">
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
        <p className="dim note">
          {monitors.length} display{monitors.length === 1 ? "" : "s"} · hot-plug live
        </p>
      </aside>

      <footer className="rail rail--bottom">
        <span className="dim">
          metrics (m2) · dock (m3) · window manager (m4) — not wired yet
        </span>
        <span className="spacer" />
        <button className="exit" onClick={() => shutdown()}>
          SAFE EXIT
        </button>
      </footer>
    </div>
  );
}

function Bracket({ corner }: { corner: "tl" | "tr" | "bl" | "br" }) {
  return <span className={`bracket bracket--${corner}`} aria-hidden />;
}
