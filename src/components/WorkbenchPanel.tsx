import { useCallback, useEffect, useState } from "react";
import {
  workbenchListDir,
  workbenchOpen,
  workbenchReadFile,
  workbenchState,
} from "../lib/api";
import { bytes } from "../lib/format";
import type { DirEntry, FilePayload } from "../types";

/** One expandable row. Children are fetched the first time it is opened. */
function TreeNode({
  entry,
  depth,
  selected,
  onSelect,
}: {
  entry: DirEntry;
  depth: number;
  selected: string | null;
  onSelect: (entry: DirEntry) => void;
}) {
  const [open, setOpen] = useState(false);
  const [children, setChildren] = useState<DirEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const isDir = entry.kind === "directory";

  const toggle = () => {
    if (!isDir) {
      onSelect(entry);
      return;
    }
    const next = !open;
    setOpen(next);
    // Lazy, one directory at a time — never a recursive walk.
    if (next && children === null) {
      workbenchListDir(entry.path)
        .then(setChildren)
        .catch((e) => setError(String(e)));
    }
  };

  return (
    <>
      <button
        className={`tree__row ${selected === entry.path ? "is-selected" : ""} ${entry.hidden ? "is-hidden" : ""}`}
        style={{ paddingLeft: 6 + depth * 12 }}
        onClick={toggle}
        title={entry.path}
      >
        <span className="tree__mark">{isDir ? (open ? "▾" : "▸") : "·"}</span>
        <span className="tree__name">{entry.name}</span>
        {!isDir && <span className="tree__size">{bytes(entry.size)}</span>}
      </button>
      {error && <p className="tree__error" style={{ paddingLeft: 18 + depth * 12 }}>{error}</p>}
      {open &&
        children?.map((child) => (
          <TreeNode
            key={child.path}
            entry={child}
            depth={depth + 1}
            selected={selected}
            onSelect={onSelect}
          />
        ))}
    </>
  );
}

/**
 * Mino Workbench, hosted in dev-layer: a lazy file tree and a viewer over
 * mino-core's transport. The same panel serves a remote host once the SSH
 * target is wired through.
 */
export function WorkbenchPanel({ onClose }: { onClose: () => void }) {
  const [root, setRoot] = useState<string | null>(null);
  const [entries, setEntries] = useState<DirEntry[]>([]);
  const [file, setFile] = useState<FilePayload | null>(null);
  const [pathInput, setPathInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback((where: string) => {
    setBusy(true);
    workbenchListDir(where)
      .then((listing) => {
        setEntries(listing);
        setError(null);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  }, []);

  const open = useCallback(
    (path?: string) => {
      setBusy(true);
      setFile(null);
      workbenchOpen(path)
        .then((state) => {
          setRoot(state.root);
          setPathInput(state.root ?? "");
          if (state.root) load(state.root);
        })
        .catch((e) => {
          setError(String(e));
          setBusy(false);
        });
    },
    [load],
  );

  useEffect(() => {
    // Reuse the session if one is already rooted; otherwise open the home dir.
    workbenchState()
      .then((state) => {
        if (state.root) {
          setRoot(state.root);
          setPathInput(state.root);
          load(state.root);
        } else {
          open();
        }
      })
      .catch(() => open());
  }, [load, open]);

  const select = (entry: DirEntry) => {
    workbenchReadFile(entry.path)
      .then((payload) => {
        setFile(payload);
        setError(null);
      })
      .catch((e) => setError(String(e)));
  };

  return (
    <section className="panel" aria-label="Workbench">
      <header className="panel__bar">
        <span className="tag">WORKBENCH</span>
        <input
          className="http__url"
          value={pathInput}
          onChange={(e) => setPathInput(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && open(pathInput)}
          placeholder="folder to open"
          spellCheck={false}
        />
        <button className="ghost" onClick={() => open(pathInput)} disabled={busy}>
          OPEN
        </button>
        <button className="ghost" onClick={() => open()} disabled={busy}>
          HOME
        </button>
        <button className="ghost" onClick={onClose}>
          CLOSE
        </button>
      </header>

      <div className="panel__body work">
        <aside className="work__tree">
          {error && <p className="tree__error">{error}</p>}
          {entries.length === 0 && !error && <p className="dim note">{busy ? "reading…" : "empty"}</p>}
          {entries.map((entry) => (
            <TreeNode
              key={entry.path}
              entry={entry}
              depth={0}
              selected={file?.path ?? null}
              onSelect={select}
            />
          ))}
        </aside>

        <div className="work__viewer">
          {file ? (
            <>
              <div className="work__meta">
                <span className="dim">{file.path}</span>
                <span className="spacer" />
                <span className="dim">
                  {bytes(file.size)}
                  {file.extension ? ` · ${file.extension}` : ""}
                </span>
              </div>
              <pre className="http__out">{file.content}</pre>
            </>
          ) : (
            <p className="dim note">
              {root ? "select a file" : "no folder open"}
            </p>
          )}
        </div>
      </div>
    </section>
  );
}

export default WorkbenchPanel;
