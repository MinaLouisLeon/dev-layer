import { useEffect, useState } from "react";
import { httpHistory, httpSend } from "../lib/api";
import { bytes } from "../lib/format";
import type { HttpHeader, HttpRequest, HttpResponse } from "../types";

const METHODS = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

/** Pretty-prints JSON, leaves anything else alone. */
function formatBody(response: HttpResponse): string {
  if (response.bodyIsBase64) {
    return `[binary body, ${bytes(response.size)}, base64]\n\n${response.body}`;
  }
  try {
    return JSON.stringify(JSON.parse(response.body), null, 2);
  } catch {
    return response.body;
  }
}

function statusBand(status: number): string {
  if (status >= 500) return "critical";
  if (status >= 400) return "warning";
  return "nominal";
}

/**
 * The "don't open Postman for one call" panel: a request, a response, and the
 * three numbers that matter — status, time, size.
 */
export function HttpPanel({ onClose }: { onClose: () => void }) {
  const [method, setMethod] = useState("GET");
  const [url, setUrl] = useState("http://localhost:3000/");
  const [headers, setHeaders] = useState<HttpHeader[]>([{ name: "", value: "" }]);
  const [body, setBody] = useState("");
  const [tab, setTab] = useState<"headers" | "body">("headers");
  const [response, setResponse] = useState<HttpResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [sending, setSending] = useState(false);
  const [history, setHistory] = useState<HttpRequest[]>([]);

  useEffect(() => {
    httpHistory().then(setHistory).catch(() => {});
  }, []);

  const send = () => {
    setSending(true);
    setError(null);
    httpSend({
      method,
      url,
      headers: headers.filter((h) => h.name.trim()),
      body: method === "GET" || method === "HEAD" ? null : body || null,
      timeoutMs: null,
    })
      .then((result) => {
        setResponse(result);
        httpHistory().then(setHistory).catch(() => {});
      })
      .catch((e) => {
        setResponse(null);
        setError(String(e));
      })
      .finally(() => setSending(false));
  };

  const recall = (request: HttpRequest) => {
    setMethod(request.method);
    setUrl(request.url);
    setHeaders(request.headers.length ? request.headers : [{ name: "", value: "" }]);
    setBody(request.body ?? "");
  };

  const editHeader = (index: number, patch: Partial<HttpHeader>) =>
    setHeaders((rows) => {
      const next = rows.map((row, i) => (i === index ? { ...row, ...patch } : row));
      // Always keep one empty row to type into.
      if (next[next.length - 1].name || next[next.length - 1].value) {
        next.push({ name: "", value: "" });
      }
      return next;
    });

  return (
    <section className="panel" aria-label="HTTP client">
      <header className="panel__bar">
        <span className="tag">HTTP</span>
        <select
          className="http__method"
          value={method}
          onChange={(e) => setMethod(e.target.value)}
        >
          {METHODS.map((m) => (
            <option key={m}>{m}</option>
          ))}
        </select>
        <input
          className="http__url"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && send()}
          placeholder="https://…"
          spellCheck={false}
        />
        <button className="ghost ghost--go" onClick={send} disabled={sending}>
          {sending ? "…" : "SEND"}
        </button>
        <button className="ghost" onClick={onClose}>
          CLOSE
        </button>
      </header>

      <div className="panel__body http">
        <aside className="http__history">
          <h2>RECENT</h2>
          {history.length === 0 ? (
            <p className="dim note">nothing yet</p>
          ) : (
            <ul>
              {history.map((request, i) => (
                <li key={`${request.method}-${request.url}-${i}`}>
                  <button onClick={() => recall(request)} title={request.url}>
                    <b>{request.method}</b>
                    <span>{request.url.replace(/^https?:\/\//, "")}</span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </aside>

        <div className="http__request">
          <nav className="http__tabs">
            <button
              className={tab === "headers" ? "is-active" : ""}
              onClick={() => setTab("headers")}
            >
              HEADERS
            </button>
            <button className={tab === "body" ? "is-active" : ""} onClick={() => setTab("body")}>
              BODY
            </button>
          </nav>

          {tab === "headers" ? (
            <div className="http__headers">
              {headers.map((header, index) => (
                <div key={index} className="http__headerrow">
                  <input
                    value={header.name}
                    onChange={(e) => editHeader(index, { name: e.target.value })}
                    placeholder="Header"
                    spellCheck={false}
                  />
                  <input
                    value={header.value}
                    onChange={(e) => editHeader(index, { value: e.target.value })}
                    placeholder="Value"
                    spellCheck={false}
                  />
                </div>
              ))}
            </div>
          ) : (
            <textarea
              className="http__body"
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder='{ "json": true }'
              spellCheck={false}
            />
          )}
        </div>

        <div className="http__response">
          {error && <p className="http__error">{error}</p>}
          {response && (
            <>
              <div className="http__stats">
                <span className={`readout readout--${statusBand(response.status)}`}>
                  <em>STATUS</em> {response.status} {response.statusText}
                </span>
                <span className="dim">
                  <em>TIME</em> {response.elapsedMs} ms
                </span>
                <span className="dim">
                  <em>SIZE</em> {bytes(response.size)}
                </span>
              </div>
              <pre className="http__out">{formatBody(response)}</pre>
              <details className="http__resheaders">
                <summary className="dim">{response.headers.length} response headers</summary>
                <table className="proc">
                  <tbody>
                    {response.headers.map((header) => (
                      <tr key={header.name}>
                        <td>{header.name}</td>
                        <td>{header.value}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </details>
            </>
          )}
          {!response && !error && <p className="dim note">no response yet</p>}
        </div>
      </div>
    </section>
  );
}

export default HttpPanel;
