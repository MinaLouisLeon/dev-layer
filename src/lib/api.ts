import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AgentEvent,
  AgentStatus,
  AppEntry,
  DirEntry,
  FilePayload,
  HostInfo,
  HttpRequest,
  HttpResponse,
  HudContext,
  LayoutKind,
  ManagedWindow,
  MetricsSnapshot,
  MonitorChange,
  MonitorInfo,
  MonitorLayout,
  TerminalClosed,
  TerminalOutput,
  WorkbenchState,
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

export const listApps = () => invoke<AppEntry[]>("list_apps");

export const launchApp = (id: string) => invoke<void>("launch_app", { id });

export const setAppPinned = (id: string, pinned: boolean) =>
  invoke<AppEntry[]>("set_app_pinned", { id, pinned });

export const refreshApps = () => invoke<void>("refresh_apps");

export const onCatalog = (cb: (apps: AppEntry[]) => void): Promise<UnlistenFn> =>
  listen<AppEntry[]>("apps::catalog", (e) => cb(e.payload));

export const listWindows = () => invoke<ManagedWindow[]>("list_windows");

export const windowLayouts = () => invoke<MonitorLayout[]>("window_layouts");

export const setWindowLayout = (monitorId: string, kind: LayoutKind) =>
  invoke<void>("set_window_layout", { monitorId, kind });

export const focusWindow = (id: number) => invoke<void>("focus_window", { id });

export const toggleWindowFloat = (id: number) =>
  invoke<boolean>("toggle_window_float", { id });

export const promoteWindow = (id: number) => invoke<void>("promote_window", { id });

export const closeWindow = (id: number) => invoke<void>("close_window", { id });

export const setWmEnabled = (enabled: boolean) =>
  invoke<void>("set_wm_enabled", { enabled });

/**
 * The HUD sits below every app window, so any panel opened over the window
 * region has to be lifted first or it is simply invisible.
 */
export const setHudOverlay = (label: string, on: boolean) =>
  invoke<void>("set_hud_overlay", { label, on });

export const onWindows = (
  cb: (windows: ManagedWindow[]) => void,
): Promise<UnlistenFn> =>
  listen<ManagedWindow[]>("wm::windows", (e) => cb(e.payload));

export const terminalOpen = (cols: number, rows: number) =>
  invoke<string>("terminal_open", { cols, rows });

export const terminalWrite = (id: string, data: string) =>
  invoke<void>("terminal_write", { id, data });

export const terminalResize = (id: string, cols: number, rows: number) =>
  invoke<void>("terminal_resize", { id, cols, rows });

export const terminalClose = (id: string) => invoke<void>("terminal_close", { id });

export const onTerminalOutput = (
  cb: (output: TerminalOutput) => void,
): Promise<UnlistenFn> =>
  listen<TerminalOutput>("terminal::output", (e) => cb(e.payload));

export const onTerminalClosed = (
  cb: (closed: TerminalClosed) => void,
): Promise<UnlistenFn> =>
  listen<TerminalClosed>("terminal::closed", (e) => cb(e.payload));

export const httpSend = (request: HttpRequest) =>
  invoke<HttpResponse>("http_send", { request });

export const httpHistory = () => invoke<HttpRequest[]>("http_history");

export const agentAsk = (prompt: string) => invoke<string>("agent_ask", { prompt });

export const agentStatus = () => invoke<AgentStatus>("agent_status");

export const agentReset = () => invoke<void>("agent_reset");

export const onAgentEvent = (cb: (event: AgentEvent) => void): Promise<UnlistenFn> =>
  listen<AgentEvent>("agent::event", (e) => cb(e.payload));

export const workbenchState = () => invoke<WorkbenchState>("workbench_state");

export const workbenchOpen = (path?: string) =>
  invoke<WorkbenchState>("workbench_open", { path: path ?? null });

export const workbenchListDir = (path: string) =>
  invoke<DirEntry[]>("workbench_list_dir", { path });

export const workbenchReadFile = (path: string) =>
  invoke<FilePayload>("workbench_read_file", { path });
