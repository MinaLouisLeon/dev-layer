import { useEffect, useRef, useState } from "react";
import { agentAsk, agentReset, agentStatus, onAgentEvent } from "../lib/api";
import type { AgentEvent, AgentStatus } from "../types";

type Entry = { role: "you" } & { text: string } | ({ role: "agent" } & AgentEvent);

const SUGGESTIONS = [
  "what's eating my CPU?",
  "open postman and chrome side by side",
  "switch this display to grid",
  "how much VRAM is free?",
];

/** Compact one-line rendering of tool arguments. */
function argsOf(input: unknown): string {
  if (!input || typeof input !== "object") return "";
  const entries = Object.entries(input as Record<string, unknown>);
  if (entries.length === 0) return "";
  return entries.map(([k, v]) => `${k}: ${String(v)}`).join(", ");
}

export function AskPanel({ onClose }: { onClose: () => void }) {
  const [prompt, setPrompt] = useState("");
  const [entries, setEntries] = useState<Entry[]>([]);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const tail = useRef<HTMLDivElement>(null);

  useEffect(() => {
    agentStatus().then(setStatus).catch(() => {});
    const un = onAgentEvent((event: AgentEvent) => {
      // "done" carries no content worth a row; it just ends the turn.
      if (event.kind !== "done") {
        setEntries((rows) => [...rows, { role: "agent", ...event }]);
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    tail.current?.scrollIntoView({ block: "end" });
  }, [entries]);

  const ask = (text: string) => {
    const question = text.trim();
    if (!question || busy) return;

    setEntries((rows) => [...rows, { role: "you", text: question }]);
    setPrompt("");
    setBusy(true);
    agentAsk(question)
      .catch(() => {
        // The error already arrived as an agent::event; nothing to add.
      })
      .finally(() => {
        setBusy(false);
        agentStatus().then(setStatus).catch(() => {});
      });
  };

  return (
    <section className="panel" aria-label="Command layer">
      <header className="panel__bar">
        <span className="tag">ASK</span>
        {status && (
          <span className="dim">
            {status.configured ? status.model : "no API key"}
            {status.allowGuarded ? " · guarded on" : ""}
            {status.turns > 0 ? ` · ${status.turns} in context` : ""}
          </span>
        )}
        <span className="spacer" />
        <button
          className="ghost"
          onClick={() => {
            agentReset().catch(() => {});
            setEntries([]);
            agentStatus().then(setStatus).catch(() => {});
          }}
        >
          CLEAR
        </button>
        <button className="ghost" onClick={onClose}>
          CLOSE
        </button>
      </header>

      <div className="panel__body ask">
        <div className="ask__log">
          {entries.length === 0 && (
            <div className="ask__empty">
              {status && !status.configured ? (
                <p className="dim">
                  No API key. Set <code>ANTHROPIC_API_KEY</code> in the environment and
                  restart, or put <code>agent.apiKey</code> in config.json.
                </p>
              ) : (
                <>
                  <p className="dim">Ask for something. It has dev-layer's own commands.</p>
                  <ul className="ask__suggestions">
                    {SUGGESTIONS.map((suggestion) => (
                      <li key={suggestion}>
                        <button onClick={() => ask(suggestion)}>{suggestion}</button>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </div>
          )}

          {entries.map((entry, index) => {
            if (entry.role === "you") {
              return (
                <p key={index} className="ask__you">
                  <b>›</b> {entry.text}
                </p>
              );
            }
            switch (entry.kind) {
              case "thinking":
                return (
                  <p key={index} className="ask__thinking">
                    {entry.text}
                  </p>
                );
              case "text":
                return (
                  <p key={index} className="ask__text">
                    {entry.text}
                  </p>
                );
              case "toolUse":
                return (
                  <p key={index} className="ask__tool">
                    <span className="ask__toolname">{entry.name}</span>
                    <span className="dim">{argsOf(entry.input)}</span>
                  </p>
                );
              case "toolResult":
                return (
                  <p key={index} className={`ask__result ${entry.ok ? "" : "is-error"}`}>
                    {entry.ok ? "✓" : "✗"} {entry.detail.slice(0, 160)}
                  </p>
                );
              case "error":
                return (
                  <p key={index} className="ask__error">
                    {entry.message}
                  </p>
                );
              default:
                return null;
            }
          })}
          <div ref={tail} />
        </div>

        <form
          className="ask__form"
          onSubmit={(e) => {
            e.preventDefault();
            ask(prompt);
          }}
        >
          <input
            className="ask__input"
            value={prompt}
            onChange={(e) => setPrompt(e.target.value)}
            placeholder={busy ? "working…" : "ask dev-layer"}
            disabled={busy}
            spellCheck={false}
            autoFocus
          />
          <button className="ghost ghost--go" type="submit" disabled={busy || !prompt.trim()}>
            {busy ? "…" : "ASK"}
          </button>
        </form>
      </div>
    </section>
  );
}

export default AskPanel;
