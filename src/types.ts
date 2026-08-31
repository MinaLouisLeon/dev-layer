/** Mirrors `src-tauri/src/geometry.rs` and `src-tauri/src/monitors/mod.rs`. */

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Insets {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface MonitorInfo {
  /** Stable across sessions and hot-plugs: the OS display device name. */
  id: string;
  /** Friendly name from the display adapter, e.g. "Dell U2720Q". */
  name: string;
  isPrimary: boolean;
  index: number;
  /** Full monitor rect in virtual-desktop physical coordinates. */
  bounds: Rect;
  /** Monitor rect minus taskbar/appbars, physical coordinates. */
  workArea: Rect;
  /** Physical pixels per logical pixel (DPI / 96). */
  scaleFactor: number;
}

/** What the HUD reserves on a monitor; the window manager may not use this space. */
export interface HudContext {
  monitor: MonitorInfo;
  reserved: Insets;
  /** Chrome (dock, gauges) is only drawn on monitors flagged as "full". */
  chrome: "full" | "minimal";
}

export interface MonitorChange {
  added: MonitorInfo[];
  removed: string[];
  changed: MonitorInfo[];
}
