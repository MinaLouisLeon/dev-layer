import { setWindowLayout } from "../lib/api";
import type { LayoutKind } from "../types";

const LAYOUTS: { kind: LayoutKind; label: string; hint: string }[] = [
  { kind: "mainStack", label: "MAIN", hint: "One main pane, the rest stacked" },
  { kind: "columns", label: "COLS", hint: "Equal vertical columns" },
  { kind: "grid", label: "GRID", hint: "Roughly square grid" },
  { kind: "monocle", label: "MONO", hint: "One window, full region" },
  { kind: "float", label: "FREE", hint: "Tiling off for this display" },
];

export function LayoutSwitcher({
  monitorId,
  active,
  onChanged,
}: {
  monitorId: string;
  active: LayoutKind;
  onChanged: () => void;
}) {
  return (
    <span className="layouts" role="group" aria-label="Layout">
      {LAYOUTS.map(({ kind, label, hint }) => (
        <button
          key={kind}
          className={kind === active ? "is-active" : ""}
          title={hint}
          onClick={() => setWindowLayout(monitorId, kind).then(onChanged)}
        >
          {label}
        </button>
      ))}
    </span>
  );
}
