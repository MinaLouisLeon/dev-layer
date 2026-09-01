import { AppIcon } from "./AppIcon";
import type { AppEntry } from "../types";

/** Pinned apps only. Everything else lives in the launcher. */
export function Dock({
  apps,
  onLaunch,
  onUnpin,
  onOpenLauncher,
  launcherOpen,
}: {
  apps: AppEntry[];
  onLaunch: (app: AppEntry) => void;
  onUnpin: (app: AppEntry) => void;
  onOpenLauncher: () => void;
  launcherOpen: boolean;
}) {
  return (
    <nav className="dock" aria-label="Pinned applications">
      <button
        className={`dock__apps ${launcherOpen ? "is-open" : ""}`}
        onClick={onOpenLauncher}
        title="All applications"
      >
        APPS
      </button>

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
