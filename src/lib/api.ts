import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  HostInfo,
  HudContext,
  MetricsSnapshot,
  MonitorChange,
  MonitorInfo,
} from "../types";

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

export const latestMetrics = () =>
  invoke<MetricsSnapshot | null>("latest_metrics");

export const getHostInfo = () => invoke<HostInfo>("host_info");

export const onMetrics = (
  cb: (snapshot: MetricsSnapshot) => void,
): Promise<UnlistenFn> =>
  listen<MetricsSnapshot>("metrics::tick", (e) => cb(e.payload));
