import { AppIcon } from "./AppIcon";
import type { AppEntry } from "../types";

export type Overlay = "launcher" | "terminal" | "http" | "agent" | null;

const PANELS: { kind: Exclude<Overlay, null>; label: string; hint: string }[] = [
  { kind: "launcher", label: "APPS", hint: "All applications" },
  { kind: "terminal", label: "TERM", hint: "Terminal session" },
  { kind: "http", label: "HTTP", hint: "HTTP client" },
  { kind: "agent", label: "ASK", hint: "Command layer — natural language" },
];

/** Pinned apps only. Everything else lives in the launcher. */
export function Dock({
  apps,
  onLaunch,
  onUnpin,
  overlay,
  onOverlay,
}: {
  apps: AppEntry[];
  onLaunch: (app: AppEntry) => void;
  onUnpin: (app: AppEntry) => void;
  overlay: Overlay;
  onOverlay: (overlay: Overlay) => void;
}) {
  return (
    <nav className="dock" aria-label="Pinned applications">
      <span className="dock__panels">
        {PANELS.map(({ kind, label, hint }) => (
          <button
            key={kind}
            className={`dock__apps ${overlay === kind ? "is-open" : ""}`}
            onClick={() => onOverlay(overlay === kind ? null : kind)}
            title={hint}
          >
            {label}
          </button>
        ))}
      </span>

      {apps.length === 0 ? (
        <span className="dim note">scanning Start Menu…</span>
      ) : (
        apps.map((app) => (
          <button
            key={app.id}
            className="dock__item"
            onClick={() => onLaunch(app)}
            title={app.name}
          >
            <AppIcon app={app} size={34} />
            <span className="dock__name">{app.name}</span>
            <span
              className="dock__pin"
              role="button"
              tabIndex={0}
              title={`Unpin ${app.name}`}
              onClick={(e) => {
                e.stopPropagation();
                onUnpin(app);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.stopPropagation();
                  onUnpin(app);
                }
              }}
            >
              ×
            </span>
          </button>
        ))
      )}
    </nav>
  );
}
