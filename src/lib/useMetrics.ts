import { useEffect, useRef, useState } from "react";
import { latestMetrics, onMetrics } from "./api";
import type { MetricsSnapshot } from "../types";

/** Series the HUD sparklines draw, kept as fixed-length ring buffers. */
export interface MetricsHistory {
  cpu: number[];
  memory: number[];
  gpu: number[];
  net: number[];
}

const EMPTY: MetricsHistory = { cpu: [], memory: [], gpu: [], net: [] };

/**
 * Subscribes to the sampler. Rust owns sampling; the frontend owns history,
 * so the tick payload stays small and the backend stays stateless.
 */
export function useMetrics(historyLength = 120) {
  const [snapshot, setSnapshot] = useState<MetricsSnapshot | null>(null);
  const [history, setHistory] = useState<MetricsHistory>(EMPTY);
  // Mutated in place, then published as a fresh object once per tick.
  const buffers = useRef<MetricsHistory>({ ...EMPTY });

  useEffect(() => {
    const push = (series: number[], value: number) => {
      series.push(value);
      if (series.length > historyLength) series.splice(0, series.length - historyLength);
      return series;
    };

    const accept = (next: MetricsSnapshot | null) => {
      if (!next) return;
      const b = buffers.current;
      push(b.cpu, next.cpu.usage);
      push(b.memory, next.memory.total ? (next.memory.used / next.memory.total) * 100 : 0);
      push(b.gpu, next.gpus[0]?.utilization ?? 0);
      push(b.net, next.network.rxPerSec + next.network.txPerSec);

      setSnapshot(next);
      setHistory({ cpu: [...b.cpu], memory: [...b.memory], gpu: [...b.gpu], net: [...b.net] });
    };

    // Seed from the last sample so a window opened mid-session is never blank.
    latestMetrics().then(accept).catch(() => {});
    const un = onMetrics(accept);
    return () => {
      un.then((f) => f());
    };
  }, [historyLength]);

  return { snapshot, history };
}
