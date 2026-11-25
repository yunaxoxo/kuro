import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { highlightLine, markRange } from "../lib/highlight";
import type { Highlight } from "../lib/spans";
import type { Diagnostic } from "../trace/types";

interface Props {
  source: string;
  onChange: (value: string) => void;
  diagnostics: Diagnostic[];
  /** Source line the current trace step is on (0 = none). */
  stepLine: number;
  /** The range under the pointer in the Tokens & AST or Bytecode view. */
  highlight: Highlight | null;
  /** A click-to-source request: scroll there, put the caret at its start, pulse its span. */
  nav?: { seq: number; start: number; end: number } | null;
  /** Called with the caret offset whenever it moves. */
  onCaret?: (offset: number) => void;
}

export const Editor = memo(function Editor({ source, onChange, diagnostics, stepLine, highlight, nav, onCaret }: Props) {
  const area = useRef<HTMLTextAreaElement>(null);
  const exit = useRef<HTMLSpanElement>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const [caretLine, setCaretLine] = useState(0);
  const [selecting, setSelecting] = useState(false);
  /** More code lies to the right of the visible part: an edge fade says the lines continue. */
  const [more, setMore] = useState(false);

  const lines = useMemo(() => source.split("\n"), [source]);
  const widest = useMemo(() => lines.reduce((max, l) => Math.max(max, l.length), 0), [lines]);
  const baseSpans = useMemo(() => lines.map(highlightLine), [lines]);
  const lineStarts = useMemo(() => {
    const starts: number[] = [];
    let at = 0;
    for (const l of lines) {
      starts.push(at);
      at += l.length + 1;
    }
    return starts;
  }, [lines]);
  const errorLines = useMemo(() => new Set(diagnostics.filter((d) => d.severity === "error").map((d) => d.line)), [diagnostics]);
  const errorCols = useMemo(() => {
    const cols = new Map<number, number>();
    diagnostics.forEach((d) => d.severity === "error" && !cols.has(d.line) && cols.set(d.line, d.col));
    return cols;
  }, [diagnostics]);
  const [pulse, setPulse] = useState<{ start: number; end: number } | null>(null);

  /** Colour one line, then lay the hover range, the pulse and the error underline over it. */
  const paint = (text: string, i: number) => {
    let spans = baseSpans[i] ?? highlightLine(text);
    if (highlight) spans = markRange(spans, highlight.start - lineStarts[i], highlight.end - lineStarts[i], "hl");
    if (pulse) spans = markRange(spans, pulse.start - lineStarts[i], pulse.end - lineStarts[i], "pulse");
    const col = errorCols.get(i + 1);
    if (col !== undefined) {
      const word = /^\w+/.exec(text.slice(col - 1))?.[0].length ?? 1;
      spans = markRange(spans, col - 1, col - 1 + word, "err");
    }
    return spans;
  };

  const syncCaret = () => {
    const el = area.current;
    if (!el) return;
    setCaretLine(el.value.slice(0, el.selectionStart).split("\n").length);
    setSelecting(el.selectionStart !== el.selectionEnd);
    onCaret?.(el.selectionStart);
  };

  const measure = () => {
    const box = scroller.current;
    if (box) setMore(box.scrollWidth - box.clientWidth - box.scrollLeft > 1);
  };
  useLayoutEffect(measure, [source]);
  useEffect(() => {
    const box = scroller.current;
    if (!box) return;
    const watch = new ResizeObserver(measure);
    watch.observe(box);
    return () => watch.disconnect();
  }, []);

  // Keep the line the trace is on in view.
  useLayoutEffect(() => {
    if (!stepLine) return;
    const row = scroller.current?.querySelector<HTMLElement>(`[data-line="${stepLine}"]`);
    const box = scroller.current;
    if (!row || !box) return;
    const top = row.offsetTop;
    const lh = row.getBoundingClientRect().height; // fractional (14px x 1.65 = 23.1px): a rounded height drifts a pixel every ten lines
    // Whole lines only, so the top row is never cut in half.
    const snap = (y: number) => Math.max(0, Math.round(y / lh) * lh);
    if (top < box.scrollTop + 24) box.scrollTop = snap(top - 48);
    else if (top + lh > box.scrollTop + box.clientHeight - 24) box.scrollTop = snap(top - box.clientHeight + lh + 48);
  }, [stepLine]);

  // Click-to-source: smooth scroll, caret at the span start, a 300ms pulse.
  useEffect(() => {
    if (!nav) return;
    const el = area.current;
    const box = scroller.current;
    if (el) el.setSelectionRange(nav.start, nav.start);
    syncCaret();
    let line = 1;
    for (let i = 0; i < lineStarts.length; i++) if (lineStarts[i] <= nav.start) line = i + 1;
    const row = box?.querySelector<HTMLElement>(`[data-line="${line}"]`);
    if (box && row) {
      const calm = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      const lh = row.getBoundingClientRect().height;
      box.scrollTo({ top: Math.max(0, Math.round((row.offsetTop - box.clientHeight / 3) / lh) * lh), behavior: calm ? "auto" : "smooth" });
    }
    setPulse({ start: nav.start, end: nav.end });
    // Reduced motion: a still highlight that holds for a second instead of a 300ms flash.
    const calmFlash = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const timer = window.setTimeout(() => setPulse(null), calmFlash ? 1000 : 300);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nav?.seq]);

  // Let the diagnostics list move the caret.
  useEffect(() => {
    const handler = (event: Event) => {
      const { line, col } = (event as CustomEvent<{ line: number; col: number }>).detail;
      const el = area.current;
      if (!el) return;
      const offset = lines.slice(0, line - 1).reduce((sum, l) => sum + l.length + 1, 0) + col - 1;
      el.focus();
      el.setSelectionRange(offset, offset);
      syncCaret();
    };
    window.addEventListener("kuro:jump", handler);
    return () => window.removeEventListener("kuro:jump", handler);
  }, [lines]);

  const onKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Escape") {
      // Not blur(): that resets the Tab start point to the top of the page. The marker sits after the textarea, so Tab moves on.
      exit.current?.focus();
    } else if (event.key === "Tab" && !event.shiftKey && !event.metaKey && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      const el = event.currentTarget;
      el.setRangeText("    ", el.selectionStart, el.selectionEnd, "end");
      onChange(el.value);
    }
  };

  return (
    <div className="editor-wrap" data-more={more || undefined} data-selecting={selecting || undefined}>
      <div className="editor" translate="no" ref={scroller} onScroll={measure} style={{ "--cols": Math.max(widest + 2, 40), "--rows": lines.length } as React.CSSProperties}>
        <div className="editor-sheet">
          <div className="editor-lines" aria-hidden="true">
            {lines.map((text, i) => {
              const no = i + 1;
              return (
                <div
                  key={no}
                  className="row"
                  data-line={no}
                  data-caret={no === caretLine || undefined}
                  data-step={no === stepLine || undefined}
                  data-error={errorLines.has(no) || undefined}
                >
                  <span className="no">{no}</span>
                  <span className="code">
                    {paint(text, i).map((span, j) => (
                      <span key={j} className={`tok-${span.cls}${span.hl ? " hl" : ""}${span.pulse ? " pulse" : ""}${span.err ? " err" : ""}`}>
                        {span.text}
                      </span>
                    ))}
                    {text === "" && "​"}
                  </span>
                </div>
              );
            })}
          </div>
          <textarea
            ref={area}
            className="editor-input"
            aria-label="kuro source code"
            value={source}
            name="source"
            autoComplete="off"
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            wrap="off"
            onChange={(event) => {
              onChange(event.target.value);
              syncCaret();
            }}
            onKeyDown={onKeyDown}
            onKeyUp={syncCaret}
            onClick={syncCaret}
            onSelect={syncCaret}
            onBlur={() => setCaretLine(0)}
          />
          <span ref={exit} className="editor-exit sr-only" tabIndex={-1}>
            Left the editor. Tab moves on, Shift Tab returns.
          </span>
        </div>
      </div>
    <i className="editor-more" aria-hidden="true" />
    </div>
  );
});

/** `recorded`: the run replayed a recorded trace, whose notes are about the run, not a place in the code. */
export function Diagnostics({ diagnostics, recorded = false }: { diagnostics: Diagnostic[]; recorded?: boolean }) {
  if (diagnostics.length === 0) return null;
  return (
    <ul className="diagnostics" aria-label="Compiler diagnostics">
      {diagnostics.map((d, i) => (
        <li key={i} className="diagnostic" data-severity={d.severity}>
          <button type="button" onClick={() => window.dispatchEvent(new CustomEvent("kuro:jump", { detail: { line: d.line, col: d.col } }))}>
            <span className="diagnostic-pos">{recorded ? "" : `${d.line}:${d.col}`}</span>
            <span className="diagnostic-msg">
              <span className="diagnostic-kind">{d.severity === "error" ? "Error" : "Note"}</span> {d.message}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
