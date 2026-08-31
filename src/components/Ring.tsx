import { band, percent } from "../lib/format";

/**
 * Single-value magnitude meter. The number is always drawn, so the status
 * colour is reinforcement rather than the only encoding.
 */
export function Ring({
  value,
  caption,
  size = 62,
}: {
  value: number | null;
  caption: string;
  size?: number;
}) {
  const known = value != null;
  const status = band(value);
  // pathLength normalizes the circumference to 100, so the dash array is
  // literally the percentage — no 2πr arithmetic to get wrong.
  const arc = known ? Math.max(0, Math.min(100, value)) : 0;

  return (
    <div className={`ring ring--${status}`} style={{ width: size, height: size }}>
      <svg viewBox="0 0 100 100" width={size} height={size} aria-hidden>
        <circle className="ring__track" cx="50" cy="50" r="42" pathLength={100} />
        {known && (
          <circle
            className="ring__value"
            cx="50"
            cy="50"
            r="42"
            pathLength={100}
            strokeDasharray={`${arc} ${100 - arc}`}
            transform="rotate(-90 50 50)"
          />
        )}
      </svg>
      <div className="ring__readout">
        <b>{known ? Math.round(value!) : "—"}</b>
        <span>{caption}</span>
      </div>
    </div>
  );
}

/** Compact text form of the same reading, for rails too small for a ring. */
export function Readout({ label, value }: { label: string; value: number | null }) {
  return (
    <span className={`readout readout--${band(value)}`}>
      <em>{label}</em> {percent(value)}
    </span>
  );
}
