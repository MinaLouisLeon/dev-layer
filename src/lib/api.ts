import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { HudContext, MonitorChange, MonitorInfo } from "../types";

export const listMonitors = () => invoke<MonitorInfo[]>("list_monitors");

/** Resolves the monitor this HUD window belongs to, from its own window label. */
export const hudContext = (label: string) =>
  invoke<HudContext>("hud_context", { label });

/** Graceful shutdown: runs every registered teardown action, then exits. */
export const shutdown = () => invoke<void>("shutdown");

export const onMonitorsChanged = (
  cb: (change: MonitorChange) => void,
): Promise<UnlistenFn> =>
  listen<MonitorChange>("monitors::changed", (e) => cb(e.payload));
