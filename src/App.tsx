import { getCurrentWindow } from "@tauri-apps/api/window";
import { Hud } from "./views/Hud";

/**
 * One bundle serves every window; the window label picks the view.
 * Labels are assigned in `src-tauri/src/hud/mod.rs` (`hud--<monitor id>`).
 */
export function App() {
  const label = getCurrentWindow().label;

  if (label.startsWith("hud--")) return <Hud label={label} />;

  // `command` and `overlay` windows land here until milestones 3+ fill them in.
  return <div className="placeholder">dev-layer :: {label}</div>;
}
