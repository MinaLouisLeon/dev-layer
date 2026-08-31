import { Ring } from "./Ring";
import { Sparkline } from "./Sparkline";

/** Ring + live detail + history, the HUD's standard telemetry block. */
export function Meter({
  caption,
  value,
  detail,
  history,
}: {
  caption: string;
  value: number | null;
  detail: string;
  history: number[];
}) {
  return (
    <section className="meter">
      <Ring value={value} caption={caption} />
      <div className="meter__body">
        <span className="meter__detail" title={detail}>
          {detail}
        </span>
        <Sparkline values={history} max={100} />
      </div>
    </section>
  );
}
