import { band } from "../lib/format";

/** One thin bar per logical core. Tooltips carry the exact value. */
export function CoreBars({ cores }: { cores: number[] }) {
  return (
    <div className="cores" role="img" aria-label={`${cores.length} logical cores`}>
      {cores.map((usage, i) => (
        <div key={i} className="cores__slot" title={`core ${i}: ${usage.toFixed(0)}%`}>
          <div
            className={`cores__bar cores__bar--${band(usage)}`}
            style={{ height: `${Math.max(2, Math.min(100, usage))}%` }}
          />
        </div>
      ))}
    </div>
  );
}
