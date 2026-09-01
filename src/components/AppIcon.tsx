import { convertFileSrc } from "@tauri-apps/api/core";
import { useState } from "react";
import type { AppEntry } from "../types";

/**
 * Cached PNGs are served over the asset protocol (scoped to the icon cache in
 * `lib.rs`). Anything without a usable icon falls back to an initial, so the
 * dock never shows a broken-image box.
 */
export function AppIcon({ app, size = 32 }: { app: AppEntry; size?: number }) {
  const [failed, setFailed] = useState(false);

  if (!app.icon || failed) {
    return (
      <span className="appicon appicon--fallback" style={{ width: size, height: size }}>
        {app.name.charAt(0).toUpperCase()}
      </span>
    );
  }

  return (
    <img
      className="appicon"
      style={{ width: size, height: size }}
      src={convertFileSrc(app.icon)}
      alt=""
      draggable={false}
      onError={() => setFailed(true)}
    />
  );
}
