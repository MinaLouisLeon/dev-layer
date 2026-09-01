import { closeWindow, focusWindow, promoteWindow, toggleWindowFloat } from "../lib/api";
import type { ManagedWindow } from "../types";

/** Windows on this monitor, in tiling order. Row order is slot order. */
export function WindowList({
  windows,
  onChanged,
}: {
  windows: ManagedWindow[];
  onChanged: () => void;
}) {
  if (windows.length === 0) {
    return <p className="dim note">no managed windows</p>;
  }

  return (
    <ul className="wins">
      {windows.map((w) => (
        <li
          key={w.id}
          className={`wins__row ${w.focused ? "is-focused" : ""} ${w.floating ? "is-floating" : ""}`}
        >
          <button className="wins__focus" onClick={() => focusWindow(w.id).catch(() => {})}>
            <b>{w.slot != null ? w.slot + 1 : w.floating ? "~" : "–"}</b>
            <span className="wins__title" title={`${w.title}\n${w.process}`}>
              {w.title}
            </span>
          </button>
          <span className="wins__actions">
            <button
              title="Promote to the main pane"
              onClick={() => promoteWindow(w.id).then(onChanged)}
            >
              ▲
            </button>
            <button
              title={w.floating ? "Return to tiling" : "Float this window"}
              onClick={() => toggleWindowFloat(w.id).then(onChanged)}
            >
              {w.floating ? "▣" : "▢"}
            </button>
            <button title="Close window" onClick={() => closeWindow(w.id).then(onChanged)}>
              ×
            </button>
          </span>
        </li>
      ))}
    </ul>
  );
}
