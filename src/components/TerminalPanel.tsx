import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { useEffect, useRef, useState } from "react";
import {
  onTerminalClosed,
  onTerminalOutput,
  terminalClose,
  terminalOpen,
  terminalResize,
  terminalShell,
  terminalWrite,
} from "../lib/api";
import type { ShellProbe } from "../types";

/** HUD palette, so the terminal reads as part of the interface. */
const THEME = {
  background: "#02080c",
  foreground: "#cdf4ff",
  cursor: "#5eeaff",
  cursorAccent: "#02080c",
  selectionBackground: "rgba(94, 234, 255, 0.25)",
  black: "#0a1016",
  red: "#ff5a5a",
  green: "#5effa8",
  yellow: "#ffb347",
  blue: "#5eb0ff",
  magenta: "#c98bff",
  cyan: "#5eeaff",
  white: "#cdf4ff",
};

export function TerminalPanel({ onClose }: { onClose: () => void }) {
  const host = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState("starting…");
  const [shell, setShell] = useState<ShellProbe | null>(null);

  useEffect(() => {
    terminalShell().then(setShell).catch(() => {});
  }, []);

  useEffect(() => {
    const element = host.current;
    if (!element) return;

    // Guards the async open: React can unmount before the session id arrives
    // (StrictMode double-mounts in development), and an orphaned shell would
    // outlive the panel.
    let disposed = false;
    let sessionId: string | null = null;
    const unlisteners: Array<() => void> = [];

    const term = new Terminal({
      theme: THEME,
      fontFamily: '"Consolas", "JetBrains Mono", ui-monospace, monospace',
      fontSize: 13,
      cursorBlink: true,
      scrollback: 5000,
      allowTransparency: false,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(element);
    fit.fit();

    terminalOpen(term.cols, term.rows)
      .then(async (id) => {
        if (disposed) {
          // Opened after unmount: close it immediately rather than leaking.
          terminalClose(id);
          return;
        }
        sessionId = id;
        setStatus(id);

        unlisteners.push(
          await onTerminalOutput((output) => {
            if (output.id === id) term.write(output.data);
          }),
          await onTerminalClosed((closed) => {
            if (closed.id !== id) return;
            sessionId = null;
            term.write(`\r\n\x1b[38;5;208m— session ended (${closed.exitCode}) —\x1b[0m\r\n`);
            setStatus(`exited ${closed.exitCode}`);
          }),
        );
      })
      .catch((e) => {
        setStatus(String(e));
        term.write(`\r\n\x1b[31m${String(e)}\x1b[0m\r\n`);
      });

    const typed = term.onData((data) => {
      if (sessionId) terminalWrite(sessionId, data).catch(() => {});
    });

    const observer = new ResizeObserver(() => {
      fit.fit();
      if (sessionId) terminalResize(sessionId, term.cols, term.rows).catch(() => {});
    });
    observer.observe(element);

    return () => {
      disposed = true;
      observer.disconnect();
      typed.dispose();
      unlisteners.forEach((un) => un());
      if (sessionId) terminalClose(sessionId);
      term.dispose();
    };
  }, []);

  return (
    <section className="panel" aria-label="Terminal">
      <header className="panel__bar">
        <span className="tag">TERMINAL</span>
        {shell && <span className="tag tag--shell">{shell.label}</span>}
        <span className="dim">{status}</span>
        {/* A fallback is never silent: if nushell is not the shell, say what
            is running and how to get nushell. */}
        {shell && !shell.nuAvailable && !shell.configured && (
          <span className="dim">
            nushell not found — <code>winget install nushell</code>
          </span>
        )}
        <span className="spacer" />
        <button className="ghost" onClick={onClose}>
          CLOSE
        </button>
      </header>
      <div className="panel__body term" ref={host} />
    </section>
  );
}

export default TerminalPanel;
