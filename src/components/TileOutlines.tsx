import type { ManagedWindow, MonitorInfo } from "../types";

/**
 * Where the window manager actually put each window, drawn on the HUD.
 *
 * The HUD is always-on-bottom, so these outlines are only visible through the
 * gaps between tiles and wherever the region is empty — which is exactly when
 * they are useful.
 */
export function TileOutlines({
  windows,
  monitor,
}: {
  windows: ManagedWindow[];
  monitor: MonitorInfo;
}) {
  // Window rects are physical and virtual-desktop absolute; the HUD's CSS is
  // logical and monitor-relative.
  const toLocal = (value: number) => value / monitor.scaleFactor;

  return (
    <div className="tiles" aria-hidden>
      {windows
        .filter((w) => !w.minimized && !w.floating && w.slot != null)
        .map((w) => (
          <div
            key={w.id}
            className={`tiles__tile ${w.focused ? "is-focused" : ""}`}
            style={{
              left: toLocal(w.rect.x - monitor.bounds.x),
              top: toLocal(w.rect.y - monitor.bounds.y),
              width: toLocal(w.rect.width),
              height: toLocal(w.rect.height),
            }}
          >
            <span className="tiles__label">
              {(w.slot ?? 0) + 1} · {w.process.replace(/\.exe$/i, "")}
            </span>
          </div>
        ))}
    </div>
  );
}
