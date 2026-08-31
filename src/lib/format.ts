const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Compact byte formatting: 3 significant figures, never more. */
export function bytes(value: number | null | undefined): string {
  if (value == null) return "—";
  let v = value;
  let unit = 0;
  while (v >= 1024 && unit < UNITS.length - 1) {
    v /= 1024;
    unit++;
  }
  return `${v >= 100 || unit === 0 ? Math.round(v) : v.toFixed(1)} ${UNITS[unit]}`;
}

export const rate = (bytesPerSec: number): string => `${bytes(bytesPerSec)}/s`;

export function percent(value: number | null | undefined, digits = 0): string {
  return value == null ? "—" : `${value.toFixed(digits)}%`;
}

export function duration(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  return d > 0 ? `${d}d ${h}h` : h > 0 ? `${h}h ${m}m` : `${m}m`;
}

/** Status band shared by every meter, so 82% means the same thing everywhere. */
export type Band = "nominal" | "warning" | "critical";

export function band(value: number | null | undefined): Band {
  if (value == null) return "nominal";
  if (value >= 90) return "critical";
  if (value >= 75) return "warning";
  return "nominal";
}
