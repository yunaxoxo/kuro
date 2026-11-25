import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { buildTree, collectIds, defaultOpen, pathAt, type TreeNode } from "../lib/ast";
import { useRoving } from "../hooks/useRoving";
import { disassemble, type Disassembly } from "../lib/disasm";
import { isMac } from "../lib/format";
import { lineColOf, offsetOf, type Highlight } from "../lib/spans";
import type { ExecutionTrace } from "../trace/types";

interface PickProps {
  source: string;
  highlight: Highlight | null;
  /** Click-to-source: scroll the editor there, put the caret at the start, pulse the span. */
  onPick: (next: Highlight) => void;
  /** Hover: paint the span in the editor. */
  onHover: (next: Highlight | null) => void;
  /** Caret offset in the editor, or null. */
  caret: number | null;
}

interface Row {
  node: TreeNode;
  depth: number;
}

function flatten(node: TreeNode, open: Set<string>, depth = 0, out: Row[] = []): Row[] {
  out.push({ node, depth });
  if (open.has(node.id)) node.children.forEach((c) => flatten(c, open, depth + 1, out));
  return out;
}

const nodeText = (n: TreeNode) => [n.role, n.kind, n.ident, n.type].filter(Boolean).join(" ").toLowerCase();

interface FilterProps {
  filter: string;
  onFilter: (value: string) => void;
  filterRef: React.RefObject<HTMLInputElement | null>;
}

export function TokensAst({ trace, source, highlight, onPick, onHover, caret, filter, onFilter, filterRef, stale = false }: { trace: ExecutionTrace; stale?: boolean } & PickProps & FilterProps) {
  const allTokens = useMemo(() => trace.tokens.filter((t) => t.kind !== "Eof"), [trace.tokens]);
  // Spans index the source that was compiled, so the tree is rebuilt per trace, not per keystroke (which would also reset what is open).
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const root = useMemo(() => buildTree(trace.ast_json, source), [trace.ast_json]);
  const [open, setOpen] = useState<Set<string>>(new Set());
  const treeScroll = useRef<HTMLDivElement>(null);
  const tokenScroll = useRef<HTMLDivElement>(null);
  const q = filter.trim().toLowerCase();

  useEffect(() => {
    setOpen(new Set(root ? defaultOpen(root) : []));
  }, [root]);

  const allIds = useMemo(() => (root ? collectIds(root) : []), [root]);

  // Filter: tokens by kind or lexeme; tree nodes by role, kind, name or type, keeping the path down to each match.
  const tokenRows = useMemo(
    () => allTokens.map((t, i) => ({ t, i })).filter(({ t }) => !q || t.kind.toLowerCase().includes(q) || t.lexeme.toLowerCase().includes(q)),
    [allTokens, q],
  );
  const { matches, keep } = useMemo(() => {
    const matches = new Set<string>();
    const keep = new Set<string>();
    if (!root || !q) return { matches, keep };
    const walk = (n: TreeNode): boolean => {
      const own = nodeText(n).includes(q);
      let below = false;
      n.children.forEach((c) => {
        if (walk(c)) below = true;
      });
      if (own) matches.add(n.id);
      if (own || below) keep.add(n.id);
      return own || below;
    };
    walk(root);
    return { matches, keep };
  }, [root, q]);

  const rows = useMemo(() => {
    if (!root) return [];
    if (!q) return flatten(root, open);
    const out: Row[] = [];
    const walk = (n: TreeNode, depth: number) => {
      if (!keep.has(n.id)) return;
      out.push({ node: n, depth });
      n.children.forEach((c) => walk(c, depth + 1));
    };
    walk(root, 0);
    return out;
  }, [root, open, q, keep]);

  const tokenRoving = useRoving(useMemo(() => tokenRows.map(({ i }) => `t${i}`), [tokenRows]));
  const treeRoving = useRoving(useMemo(() => rows.map((r) => r.node.id), [rows]));

  // Editor to tree: the innermost node around the caret, and the token under it.
  const caretPath = useMemo(() => (root && caret !== null ? pathAt(root, caret) : []), [root, caret]);
  const caretNode = caretPath.length > 1 ? caretPath[caretPath.length - 1] : null;
  const tokenStarts = useMemo(() => allTokens.map((t) => offsetOf(source, t.line, t.col)), [allTokens, source]);
  const caretToken = useMemo(() => {
    if (caret === null) return -1;
    return allTokens.findIndex((t, i) => tokenStarts[i] <= caret && caret < tokenStarts[i] + t.lexeme.length);
  }, [allTokens, tokenStarts, caret]);

  // Open the path to the caret's node when the caret moves to a different node, not every time something collapses.
  const openNow = useRef(open);
  openNow.current = open;
  useEffect(() => {
    if (caretPath.length < 2) return;
    const missing = caretPath.slice(0, -1).filter((n) => !openNow.current.has(n.id));
    if (missing.length > 0) setOpen((now) => new Set([...now, ...missing.map((n) => n.id)]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [caretNode?.id]);

  useEffect(() => {
    for (const box of [treeScroll.current, tokenScroll.current]) {
      const el = box?.querySelector<HTMLElement>("[data-caret]");
      if (!box || !el) continue;
      const b = box.getBoundingClientRect();
      const r = el.getBoundingClientRect();
      if (r.top < b.top + 26) box.scrollTop += r.top - b.top - 52;
      else if (r.bottom > b.bottom - 26) box.scrollTop += r.bottom - b.bottom + 52;
    }
  }, [caretNode?.id, caretToken]);

  const flip = (id: string) =>
    setOpen((now) => {
      const next = new Set(now);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  return (
    <>
      <div className="filter">
        <label className="filter-label" htmlFor="kuro-filter">
          Filter
        </label>
        <input
          id="kuro-filter"
          ref={filterRef}
          className="filter-input"
          type="search"
          name="filter"
          autoComplete="off"
          spellCheck={false}
          placeholder="Ident, While, total…"
          value={filter}
          onChange={(e) => onFilter(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.stopPropagation();
              onFilter("");
              e.currentTarget.blur();
            }
          }}
        />
        <kbd className="filter-key">{isMac() ? "⌘F" : "Ctrl F"}</kbd>
      </div>

      <div className="inspect-grid inspect-grid-tree" translate="no">
        <section className="inspect-col" aria-label="Tokens">
          <h2 className="inspect-title">
            Tokens {!stale && <span className="inspect-count">{q ? `${tokenRows.length} / ${allTokens.length}` : allTokens.length}</span>}
          </h2>
          {tokenRows.length === 0 ? (
            <p className="inspect-empty">{q ? "No matches" : "No tokens"}</p>
          ) : (
            <div className="scroll scroll-y" ref={tokenScroll}>
              <div className="tokens" role="grid" aria-label="Tokens" data-stale={stale || undefined} ref={tokenRoving.root} onKeyDown={tokenRoving.onKeyDown} data-roving>
                {tokenRows.map(({ t, i }) => {
                  const id = `t${i}`;
                  const pick: Highlight = { key: id, start: tokenStarts[i], end: tokenStarts[i] + t.lexeme.length };
                  const selected = highlight?.key === pick.key;
                  return (
                    <div
                      key={i}
                      role="row"
                      aria-selected={selected}
                      className="tok-row"
                      data-rid={id}
                      tabIndex={tokenRoving.tabIndexFor(id)}
                      data-selected={selected || undefined}
                      data-caret={i === caretToken || undefined}
                      data-match={q ? true : undefined}
                      onFocus={() => {
                        tokenRoving.setActive(id);
                        onHover(pick);
                      }}
                      onBlur={() => onHover(null)}
                      onMouseEnter={() => onHover(pick)}
                      onMouseLeave={() => onHover(null)}
                      onClick={() => onPick(pick)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          e.stopPropagation();
                          onPick(pick);
                        } else if (e.key === "ArrowLeft" || e.key === "ArrowRight") e.stopPropagation();
                      }}
                    >
                      <span role="gridcell" className="tok-pos">
                        {t.line}:{t.col}
                      </span>
                      <span role="gridcell" className="tok-kind">
                        {t.kind}
                      </span>
                      <span role="gridcell" className="lexeme" data-same={t.kind.toLowerCase() === t.lexeme.toLowerCase() || undefined}>
                        {t.lexeme}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </section>

        <section className="inspect-col" aria-label="Syntax tree">
          <div className="inspect-head">
            <h2 className="inspect-title">
              Syntax Tree {q && <span className="inspect-count">{matches.size}</span>}
            </h2>
            {root && (
              <div className="text-actions">
                <button type="button" className="text-action" disabled={!!q || allIds.every((id) => open.has(id))} onClick={() => setOpen(new Set(allIds))}>
                  Expand All
                </button>
                <button type="button" className="text-action" disabled={!!q || open.size <= 1} onClick={() => setOpen(new Set([root.id]))}>
                  Collapse All
                </button>
              </div>
            )}
          </div>
          {!root ? (
            <p className="inspect-empty">No syntax tree</p>
          ) : rows.length === 0 ? (
            <p className="inspect-empty">No matches</p>
          ) : (
            <div className="scroll scroll-y" ref={treeScroll}>
              <div role="tree" aria-label="Syntax tree" className="tree" data-filtered={q ? true : undefined} data-stale={stale || undefined} ref={treeRoving.root} onKeyDown={treeRoving.onKeyDown} data-roving>
                {rows.map(({ node, depth }, index) => {
                  const branch = node.children.length > 0;
                  const expanded = q ? keep.has(node.id) && node.children.some((c) => keep.has(c.id)) : open.has(node.id);
                  const pick: Highlight | null = node.span ? { key: `n ${node.id}`, start: node.span.start, end: node.span.end } : null;
                  const end = node.span ? lineColOf(source, node.span.end) : null;
                  const selected = !!pick && highlight?.key === pick.key;
                  return (
                    <div
                      key={node.id}
                      role="treeitem"
                      aria-level={depth + 1}
                      aria-expanded={branch ? expanded : undefined}
                      aria-selected={selected}
                      data-rid={node.id}
                      tabIndex={treeRoving.tabIndexFor(node.id)}
                      className="trow"
                      data-selected={selected || undefined}
                      data-caret={node.id === caretNode?.id || undefined}
                      data-match={(q && matches.has(node.id)) || undefined}
                      data-trail={(q && !matches.has(node.id)) || undefined}
                      style={{ paddingLeft: depth * (q ? 10 : 16) /* filtered paths run deep: names keep the room */ }}
                      onMouseEnter={() => pick && onHover(pick)}
                      onMouseLeave={() => onHover(null)}
                      onFocus={() => {
                        treeRoving.setActive(node.id);
                        if (pick) onHover(pick);
                      }}
                      onBlur={() => onHover(null)}
                      onClick={() => pick && onPick(pick)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          e.stopPropagation();
                          if (pick) onPick(pick);
                        } else if (e.key === "ArrowRight") {
                          e.preventDefault();
                          e.stopPropagation();
                          if (branch && !expanded && !q) flip(node.id);
                          else if (branch && expanded && rows[index + 1]) treeRoving.focusRow(rows[index + 1].node.id);
                        } else if (e.key === "ArrowLeft") {
                          e.preventDefault();
                          e.stopPropagation();
                          if (branch && expanded && !q) flip(node.id);
                          else for (let j = index - 1; j >= 0; j--) if (rows[j].depth < depth) { treeRoving.focusRow(rows[j].node.id); break; }
                        }
                      }}
                    >
                      <span className="tri-slot">
                        {branch && (
                          <span
                            className="tri"
                            aria-hidden="true"
                            data-open={expanded || undefined}
                            onClick={(e) => {
                              e.stopPropagation();
                              if (!q) flip(node.id);
                            }}
                          />
                        )}
                      </span>
                      <span className="t-main" title={`${node.role ? `${node.role}: ` : ""}${node.kind}${node.ident !== undefined ? ` ${node.ident}` : ""}${node.type ? ` · ${node.type}` : ""}`}>
                        {node.role && <span className="t-field">{node.role}:</span>}
                        <span className="t-kind">{node.kind}</span>
                        {node.ident !== undefined && <span className="t-val">{node.ident}</span>}
                      </span>
                      <span className="t-rail">
                        {node.type && <span className="t-type">{node.type}</span>}
                        {node.span && end && (
                          <span className="t-span">
                            {node.span.line}:{node.span.col}–{end.line}:{end.col}
                          </span>
                        )}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </section>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// 03 WASM Bytecode
// ---------------------------------------------------------------------------

const hex2 = (n: number) => n.toString(16).padStart(2, "0");
const hex4 = (n: number) => n.toString(16).padStart(4, "0");
const ROW = 8;

interface ByteRange {
  from: number;
  to: number;
}

/** Bytes that belong to the module header or to a section's id and size fields. */
function headerBytes(dis: Disassembly | null): Set<number> {
  const set = new Set<number>();
  for (let i = 0; i < 8; i++) set.add(i);
  dis?.sections.forEach((s) => {
    for (let i = s.offset; i < s.payloadOffset; i++) set.add(i);
  });
  return set;
}

/** One 8-byte row. Memoised on the lit span inside it, so a hover re-renders only the rows it touches. */
const HexRow = memo(function HexRow({ offset, bytes, headers, litFrom, litTo }: { offset: number; bytes: Uint8Array; headers: Set<number>; litFrom: number; litTo: number }) {
  return (
    <div className="hex-row" role="row" data-row={offset / ROW} data-hot={litTo > litFrom || undefined}>
      <span className="hex-off" role="rowheader">
        {hex4(offset)}
      </span>
      <span className="hex-bytes" role="cell">
        {Array.from(bytes.subarray(offset, offset + ROW), (b, i) => {
          const at = offset + i;
          return (
            <span key={i} className={headers.has(at) ? "b b-head" : "b"} data-lit={(at >= litFrom && at < litTo) || undefined} data-gap={i === 4 || undefined}>
              {hex2(b)}
            </span>
          );
        })}
      </span>
    </div>
  );
});

interface InstrListProps {
  dis: Disassembly;
  decls: Map<string, Highlight>;
  pinned: ByteRange | null;
  onLight: (range: ByteRange | null) => void;
  onPin: (range: ByteRange) => void;
  onPick: (next: Highlight) => void;
  onHover: (next: Highlight | null) => void;
}

/** The instruction list. It never reads the hover range, so lighting bytes never re-renders it. */
const InstrList = memo(function InstrList({ dis, decls, pinned, onLight, onPin, onPick, onHover }: InstrListProps) {
  const instrRoving = useRoving(useMemo(() => dis.functions.flatMap((f) => f.instrs.map((i) => `i${i.offset}`)), [dis]));
  return (
    <div
      className="scroll"
      ref={instrRoving.root}
      data-roving
      onKeyDown={(e) => {
        if (e.key === "ArrowLeft" || e.key === "ArrowRight") e.stopPropagation();
        else instrRoving.onKeyDown(e);
      }}
    >
      {dis.functions.map((fn) => (
        <div key={fn.index} className="fn">
          <h3 className="fn-head">
            {decls.has(fn.name) ? (
              <button
                type="button"
                className="fn-jump"
                title="Go to Declaration"
                onMouseEnter={() => onHover(decls.get(fn.name)!)}
                onMouseLeave={() => onHover(null)}
                onFocus={() => onHover(decls.get(fn.name)!)}
                onBlur={() => onHover(null)}
                onClick={() => onPick(decls.get(fn.name)!)}
              >
                <span className="fn-kw">fn</span> <span className="fn-name">{fn.name}</span> <span className="fn-sig">{fn.signature}</span>
              </button>
            ) : (
              <>
                <span className="fn-kw">fn</span> <span className="fn-name">{fn.name}</span> <span className="fn-sig">{fn.signature}</span>
              </>
            )}
          </h3>
          {fn.localsText && <p className="fn-locals">{fn.localsText}</p>}
          <ol className="instrs">
            {fn.instrs.map((ins) => {
              const range = { from: ins.offset, to: ins.offset + ins.bytes.split(" ").length };
              const isPinned = pinned?.from === range.from;
              return (
                <li key={ins.offset}>
                  <button
                    type="button"
                    className="instr"
                    data-rid={`i${ins.offset}`}
                    tabIndex={instrRoving.tabIndexFor(`i${ins.offset}`)}
                    aria-pressed={isPinned}
                    onMouseEnter={() => onLight(range)}
                    onMouseLeave={() => onLight(null)}
                    onFocus={() => {
                      instrRoving.setActive(`i${ins.offset}`);
                      onLight(range);
                    }}
                    onBlur={() => onLight(null)}
                    onClick={() => onPin(range)}
                  >
                    <span className="instr-at">{hex4(ins.offset)}</span>
                    <span className="instr-text" style={{ paddingLeft: ins.depth * 16 }}>{ins.text}</span>
                  </button>
                </li>
              );
            })}
          </ol>
        </div>
      ))}
    </div>
  );
});

export const Bytecode = memo(function Bytecode({ trace, source, onPick, onHover }: { trace: ExecutionTrace; source: string; onPick: (next: Highlight) => void; onHover: (next: Highlight | null) => void }) {
  // Each user function in the module has a declaration in the source; the runtime's `alloc` does not.
  const decls = useMemo(() => {
    const tree = buildTree(trace.ast_json, source);
    const map = new Map<string, Highlight>();
    tree?.children.forEach((f) => f.span && f.ident && map.set(f.ident, { key: `fn ${f.ident}`, start: f.span.start, end: f.span.end }));
    return map;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [trace.ast_json]);

  const bytes = useMemo(() => Uint8Array.from(trace.wasm_bytes), [trace.wasm_bytes]);
  const dis = useMemo(() => (bytes.length ? disassemble(bytes) : null), [bytes]);
  const headers = useMemo(() => headerBytes(dis), [dis]);
  const [pinned, setPinned] = useState<ByteRange | null>(null);
  const [hover, setHover] = useState<ByteRange | null>(null);
  const dump = useRef<HTMLDivElement>(null);
  const hot = hover ?? pinned;

  const rows = useMemo(() => {
    const out: number[] = [];
    for (let o = 0; o < bytes.length; o += ROW) out.push(o);
    return out;
  }, [bytes]);

  useEffect(() => {
    setPinned(null);
    setHover(null);
  }, [bytes]);

  /** Click an instruction: pin (or unpin) its bytes and scroll them into view. */
  const pin = useCallback((range: ByteRange) => {
    setPinned((now) => (now?.from === range.from ? null : range));
    const box = dump.current;
    const row = box?.querySelector<HTMLElement>(`[data-row="${Math.floor(range.from / ROW)}"]`);
    if (!box || !row) return;
    const boxRect = box.getBoundingClientRect();
    const rowRect = row.getBoundingClientRect();
    if (rowRect.top < boxRect.top + 24 || rowRect.bottom > boxRect.bottom - 24) {
      box.scrollTop += rowRect.top - boxRect.top - box.clientHeight / 2;
    }
  }, []);

  if (!dis) return <p className="inspect-empty">No bytecode</p>;

  return (
    <div className="inspect-grid" translate="no">
      <section className="inspect-col" aria-label="Module bytes">
        <h2 className="inspect-title">
          Module Bytes <span className="inspect-count">{bytes.length}</span>
        </h2>
        <ul className="sections">
          {dis.sections.map((s) => (
            <li key={s.offset}>
              <button type="button" className="section-jump" aria-pressed={pinned?.from === s.offset} onClick={() => pin({ from: s.offset, to: s.payloadOffset + s.size })}>
                <span className="num">{s.id}</span> {s.name} <span className="sections-size">{s.size} bytes</span>
              </button>
            </li>
          ))}
        </ul>
        <div className="scroll" ref={dump}>
          <div className="hexdump" role="table" aria-label="Module bytes">
            {rows.map((offset) => {
              const live = hot && hot.from < offset + ROW && hot.to > offset;
              // Clipped to this row, or an empty range, so untouched rows keep equal props and skip rendering.
              return <HexRow key={offset} offset={offset} bytes={bytes} headers={headers} litFrom={live ? Math.max(hot.from, offset) : 0} litTo={live ? Math.min(hot.to, offset + ROW) : 0} />;
            })}
          </div>
        </div>
      </section>

      <section className="inspect-col" aria-label="Instructions">
        <h2 className="inspect-title">Instructions</h2>
        {dis.error && (
          <p className="inspect-error">
            Byte {dis.error.offset}: {dis.error.message}
          </p>
        )}
        <InstrList dis={dis} decls={decls} pinned={pinned} onLight={setHover} onPin={pin} onPick={onPick} onHover={onHover} />
      </section>
    </div>
  );
});
