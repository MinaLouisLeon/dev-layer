import { bytes } from "../lib/format";
import type { ProcessMetrics } from "../types";

/** Top consumers, ranked by CPU. Doubles as the table view of the CPU meter. */
export function ProcessTable({ processes }: { processes: ProcessMetrics[] }) {
  if (processes.length === 0) return <p className="dim note">sampling…</p>;

  return (
    <table className="proc">
      <thead>
        <tr>
          <th scope="col">PROCESS</th>
          <th scope="col">CPU</th>
          <th scope="col">MEM</th>
        </tr>
      </thead>
      <tbody>
        {processes.map((p) => (
          <tr key={p.pid}>
            <td title={`${p.name} · pid ${p.pid}`}>{p.name.replace(/\.exe$/i, "")}</td>
            <td className="num">{p.cpu.toFixed(1)}</td>
            <td className="num">{bytes(p.memory)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
