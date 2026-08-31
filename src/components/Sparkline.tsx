/**
 * Single-series history. No legend (the caption names it) and no per-point
 * labels; the live value is shown numerically beside it by the caller.
 */
export function Sparkline({
  values,
  max,
  height = 22,
}: {
  values: number[];
  /** Fixed scale for percentages; omit to auto-scale (rates). */
  max?: number;
  height?: number;
}) {
  if (values.length < 2) return <div className="spark spark--empty" style={{ height }} />;

  const ceiling = max ?? Math.max(...values, 1);
  const step = 100 / (values.length - 1);
  const y = (v: number) => 100 - Math.max(0, Math.min(100, (v / ceiling) * 100));
  const line = values.map((v, i) => `${(i * step).toFixed(2)},${y(v).toFixed(2)}`).join(" ");
  // Deliberately line-only: a filled area under a flat series reads as a bar
  // chart of magnitude, which is not what a sparkline is saying.

  return (
    <svg
      className="spark"
      viewBox="0 0 100 100"
      preserveAspectRatio="none"
      height={height}
      aria-hidden
    >
      <polyline className="spark__line" points={line} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}
