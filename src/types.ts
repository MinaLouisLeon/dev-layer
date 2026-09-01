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

/** Mirrors `src-tauri/src/metrics/`. */

export interface CpuMetrics {
  usage: number;
  perCore: number[];
  frequencyMhz: number;
}

export interface MemoryMetrics {
  used: number;
  total: number;
  swapUsed: number;
  swapTotal: number;
}

export interface GpuMetrics {
  name: string;
  /** `null` means "not measurable on this machine", never "zero". */
  utilization: number | null;
  memoryUsed: number | null;
  memoryTotal: number | null;
  temperatureC: number | null;
  source: "nvml" | "pdh";
}

export interface NetworkMetrics {
  rxPerSec: number;
  txPerSec: number;
  rxTotal: number;
  txTotal: number;
}

export interface DiskMetrics {
  name: string;
  mount: string;
  used: number;
  total: number;
}

export interface ProcessMetrics {
  pid: number;
  name: string;
  cpu: number;
  memory: number;
}

export interface MetricsSnapshot {
  timestampMs: number;
  cpu: CpuMetrics;
  memory: MemoryMetrics;
  gpus: GpuMetrics[];
  network: NetworkMetrics;
  disks: DiskMetrics[];
  processes: ProcessMetrics[];
  uptimeSecs: number;
}

export interface HostInfo {
  hostname: string;
  os: string;
  kernel: string;
  cpuBrand: string;
  physicalCores: number;
  logicalCores: number;
  totalMemory: number;
}

/** Mirrors `src-tauri/src/apps/`. */
export interface AppEntry {
  id: string;
  name: string;
  launchPath: string;
  target: string | null;
  /** Absolute path to the cached PNG; needs convertFileSrc() to render. */
  icon: string | null;
  isDevTool: boolean;
  pinned: boolean;
}

/** Mirrors `src-tauri/src/wm/`. */
export type LayoutKind = "mainStack" | "columns" | "grid" | "monocle" | "float";

export interface ManagedWindow {
  /** Native window handle. Unique while the window lives. */
  id: number;
  title: string;
  process: string;
  monitorId: string;
  floating: boolean;
  focused: boolean;
  minimized: boolean;
  /** Physical, virtual-desktop coordinates. */
  rect: Rect;
  slot: number | null;
}

export interface MonitorLayout {
  monitorId: string;
  kind: LayoutKind;
  region: Rect;
  windowCount: number;
}

/** Mirrors `src-tauri/src/panels/`. */
export interface TerminalOutput {
  id: string;
  data: string;
}

export interface TerminalClosed {
  id: string;
  exitCode: number;
}

export interface HttpHeader {
  name: string;
  value: string;
}

export interface HttpRequest {
  method: string;
  url: string;
  headers: HttpHeader[];
  body: string | null;
  timeoutMs: number | null;
}

export interface HttpResponse {
  status: number;
  statusText: string;
  headers: HttpHeader[];
  body: string;
  bodyIsBase64: boolean;
  size: number;
  elapsedMs: number;
  finalUrl: string;
}

/** Mirrors `src-tauri/src/agent/`. */
export type AgentEvent =
  | { kind: "thinking"; text: string }
  | { kind: "text"; text: string }
  | { kind: "toolUse"; name: string; input: unknown }
  | { kind: "toolResult"; name: string; ok: boolean; detail: string }
  | { kind: "done"; turns: number }
  | { kind: "error"; message: string };

export interface AgentStatus {
  enabled: boolean;
  /** An API key was found, in the environment or config. */
  configured: boolean;
  model: string;
  allowGuarded: boolean;
  turns: number;
}
