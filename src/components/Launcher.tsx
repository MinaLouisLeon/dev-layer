import { useMemo, useState } from "react";
import { AppIcon } from "./AppIcon";
import type { AppEntry } from "../types";

/**
 * Every discovered application, searchable. Lives inside the window region —
 * from milestone 4 that region also holds tiled app windows, so this is an
 * overlay rather than a permanent panel.
 */
export function Launcher({
  apps,
  onLaunch,
  onTogglePin,
  onClose,
  onRefresh,
}: {
  apps: AppEntry[];
  onLaunch: (app: AppEntry) => void;
  onTogglePin: (app: AppEntry) => void;
  onClose: () => void;
  onRefresh: () => void;
}) {
  const [query, setQuery] = useState("");

  const results = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return apps;
    return apps.filter(
      (app) =>
        app.name.toLowerCase().includes(needle) ||
        (app.target ?? "").toLowerCase().includes(needle),
    );
  }, [apps, query]);

  return (
    <section className="launcher" aria-label="Application launcher">
      <header className="launcher__bar">
        <input
          className="launcher__search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="SEARCH APPLICATIONS"
          spellCheck={false}
          autoFocus
        />
        <span className="dim">
          {results.length} / {apps.length}
        </span>
        <button className="ghost" onClick={onRefresh} title="Rescan the Start Menu">
          RESCAN
        </button>
        <button className="ghost" onClick={onClose} title="Close launcher">
          CLOSE
        </button>
      </header>

      {apps.length === 0 ? (
        <p className="launcher__empty dim">
          scanning the Start Menu — this takes a few seconds the first time,
          while icons are rendered and cached
        </p>
      ) : (
        <div className="launcher__grid">
          {results.map((app) => (
            <button
              key={app.id}
              className={`tile ${app.pinned ? "is-pinned" : ""}`}
              onClick={() => onLaunch(app)}
              onContextMenu={(e) => {
                e.preventDefault();
                onTogglePin(app);
              }}
              title={`${app.name}\n${app.target ?? app.launchPath}\nright-click to ${app.pinned ? "unpin" : "pin"}`}
            >
              <AppIcon app={app} size={44} />
              <span className="tile__name">{app.name}</span>
              {app.pinned && <span className="tile__pin" aria-label="pinned" />}
            </button>
          ))}
        </div>
      )}
    </section>
  );
}
