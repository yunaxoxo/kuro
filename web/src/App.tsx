import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Header } from "./components/Header";
import { Bytecode, TokensAst } from "./components/Inspector";
import { Diagnostics, Editor } from "./components/Editor";
import { Memory } from "./components/Memory";
import { Scrubber } from "./components/Scrubber";
import { Stdout } from "./components/Stdout";
import { StatusBar } from "./components/StatusBar";
import { useWorkbench, type ViewTab } from "./hooks/useWorkbench";
import type { Highlight } from "./lib/spans";
import type { Diagnostic } from "./trace/types";
import { readUrlState } from "./trace/samples";

const TABS: { id: ViewTab; number: string; title: string; short: string }[] = [
  { id: "memory", number: "01", title: "Memory Visualizer", short: "Memory" },
  { id: "tokens", number: "02", title: "Tokens & AST", short: "Tokens & AST" },
  { id: "bytecode", number: "03", title: "WASM Bytecode", short: "Bytecode" },
];

const isTyping = (target: EventTarget | null) => {
  const el = target as HTMLElement | null;
  return !!el && (el.tagName === "TEXTAREA" || el.tagName === "INPUT" || el.tagName === "SELECT" || el.isContentEditable);
};

// Space presses a focused control; the play shortcut only applies elsewhere.
const ownsSpace = (target: EventTarget | null) =>
  !!(target as HTMLElement | null)?.closest?.("button, a, [role=tab], [role=treeitem], [role=row]");

const NO_DIAGNOSTICS: Diagnostic[] = [];
const isPhone = () => window.matchMedia("(max-width: 767px)").matches;

export function App() {
  const wb = useWorkbench();
  const [pane, setPane] = useState<"editor" | "telemetry">("telemetry");
  const [filter, setFilter] = useState(() => readUrlState().filter);
  const filterRef = useRef<HTMLInputElement>(null);
  const { trace, step, stepCount } = wb;
  const { compile, setStep, togglePlay, pause, setTab } = wb;
  // A recorded trace is standing in for edited code (the only warning a recorded trace carries). Its positions index
  // the original sample, not this text, so nothing may link the two: no painted spans, no step line, no jumps.
  const ignoredEdits = trace?.mode === "mock" && trace.diagnostics.some((d) => d.severity === "warning");
  // The compiler is absent (a recorded run is on screen) and the source differs from the sample: Run can only replay.
  const replayOnly = wb.edited && trace?.mode === "mock";

  // On a phone only one pane is visible, and the result of a run lives in the telemetry pane.
  const run = useCallback(() => {
    compile();
    if (isPhone()) setPane("telemetry");
  }, [compile]);

  // The sample, the view and the filter live in the address bar, so any state can be shared or reloaded.
  const sampleId = wb.sample.id;
  const viewTab = wb.tab;
  useEffect(() => {
    const params = new URLSearchParams({ sample: sampleId, view: viewTab });
    if (filter.trim()) params.set("q", filter.trim());
    window.history.replaceState(null, "", `?${params}${window.location.hash}`);
  }, [sampleId, viewTab, filter]);

  // The view tabs are one tab stop; arrows move between them.
  const onTabKeys = (event: React.KeyboardEvent) => {
    const at = TABS.findIndex((t) => t.id === wb.tab);
    let to: number;
    if (event.key === "ArrowRight") to = (at + 1) % TABS.length;
    else if (event.key === "ArrowLeft") to = (at + TABS.length - 1) % TABS.length;
    else if (event.key === "Home") to = 0;
    else if (event.key === "End") to = TABS.length - 1;
    else return;
    event.preventDefault();
    event.stopPropagation();
    setTab(TABS[to].id);
    requestAnimationFrame(() => document.getElementById(`tab-${TABS[to].id}`)?.focus());
  };

  // Click-to-source. On a phone the editor is a separate pane, so bring it forward first.
  const { navigate } = wb;
  const goTo = useCallback(
    (target: Highlight) => {
      if (ignoredEdits) return;
      if (isPhone()) setPane("editor");
      navigate(target);
    },
    [navigate, ignoredEdits],
  );


  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        run();
        return;
      }
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "f" && !event.shiftKey && wb.tab === "tokens") {
        event.preventDefault();
        filterRef.current?.focus();
        filterRef.current?.select();
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey || isTyping(event.target)) return;
      switch (event.key) {
        case "ArrowLeft":
          event.preventDefault();
          pause();
          setStep(step - (event.shiftKey ? 5 : 1));
          break;
        case "ArrowRight":
          event.preventDefault();
          pause();
          setStep(step + (event.shiftKey ? 5 : 1));
          break;
        case "Home":
          event.preventDefault();
          pause();
          setStep(0);
          break;
        case "End":
          event.preventDefault();
          pause();
          setStep(stepCount - 1);
          break;
        case " ":
          if (ownsSpace(event.target)) break;
          event.preventDefault();
          togglePlay();
          break;
        case "1":
        case "2":
        case "3":
          setTab(TABS[Number(event.key) - 1].id);
          break;
      }
    };
    // Where Space played or paused, stop the key's release from also pressing anything.
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key === " " && !isTyping(event.target) && !ownsSpace(event.target)) event.preventDefault();
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("keyup", onKeyUp);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("keyup", onKeyUp);
    };
  }, [run, setStep, togglePlay, pause, setTab, step, stepCount, wb.tab]);

  const diagnostics = trace?.diagnostics ?? NO_DIAGNOSTICS;
  const hasErrors = diagnostics.some((d) => d.severity === "error");
  // A program that never writes linear memory (it never allocates) gets a plain message instead of an empty plot.
  const neverWritesMemory = useMemo(
    () => !!trace && trace.steps.length > 0 && trace.steps.every((s) => s.mutated_indices.length === 0 && s.memory_i32.every((v) => v === 0)),
    [trace],
  );

  return (
    <div className="app">
      <a className="skip" href="#workspace">
        Skip to Workspace
      </a>
      <Header wb={wb} onRun={run} replayOnly={replayOnly} />

      <div className="pane-switch" role="group" aria-label="Workspace">
        <button type="button" className="tab" aria-pressed={pane === "editor"} onClick={() => setPane("editor")}>
          Editor
        </button>
        <button type="button" className="tab" aria-pressed={pane === "telemetry"} onClick={() => setPane("telemetry")}>
          Telemetry
        </button>
      </div>

      <main className="zones" id="workspace" tabIndex={-1}>
        <section className="zone zone-code" aria-label="Source code" data-active={pane === "editor"}>
          <div className="pane-head">
            <span className="pane-file">{wb.sample.id}.kr</span>
            <span className="pane-meta">
              {wb.edited && (
                <>
                  <span className="pane-edited">Edited{replayOnly ? " · won't compile" : ""}</span>
                  <button
                    type="button"
                    className="text-action pane-revert"
                    onClick={() => window.confirm(`Discard your edits to ${wb.sample.id}.kr?`) && wb.revert()}
                  >
                    Revert
                  </button>
                </>
              )}
              {wb.source.split("\n").length} lines
            </span>
          </div>
          <Editor
            source={wb.source}
            onChange={wb.setSource}
            diagnostics={diagnostics}
            stepLine={wb.tab === "memory" && !ignoredEdits ? (wb.current?.pc ?? 0) : 0}
            highlight={wb.tab === "memory" || ignoredEdits ? null : wb.hover}
            nav={wb.nav}
            onCaret={wb.setCaret}
          />
          <Diagnostics diagnostics={ignoredEdits ? NO_DIAGNOSTICS : diagnostics} recorded={trace?.mode === "mock"} />
        </section>

        <section className="zone zone-data" aria-label="Execution" data-active={pane === "telemetry"}>
          <div className="pane-head tabs" role="tablist" aria-label="Views" onKeyDown={onTabKeys}>
            {TABS.map((t) => (
              <button
                key={t.id}
                type="button"
                role="tab"
                id={`tab-${t.id}`}
                aria-selected={wb.tab === t.id}
                aria-controls="view-panel"
                tabIndex={wb.tab === t.id ? 0 : -1}
                className="tab"
                onClick={() => wb.setTab(t.id)}
              >
                <span className="tab-no">{t.number}</span>
                <span className="tab-full">{t.title}</span>
                <span className="tab-short" aria-hidden="true">
                  {t.short}
                </span>
              </button>
            ))}
          </div>

          <div className="panel" role="tabpanel" id="view-panel" aria-labelledby={`tab-${wb.tab}`}>
            {replayOnly && (
              <p className="recorded-note" key={wb.runs}>
                {ignoredEdits
                  ? `Recorded trace of the original ${wb.sample.title} · your edits were not compiled, so errors in them won't show`
                  : `Edits aren't compiled in this build · Run replays the recorded ${wb.sample.title}`}
              </p>
            )}
            {wb.tab === "memory" && (
              <>
                <Memory step={wb.current} previous={wb.previous} index={wb.step} count={stepCount} hasErrors={hasErrors} playing={wb.playing} silent={neverWritesMemory} onBytecode={() => setTab("bytecode")} />
                <Scrubber steps={trace?.steps ?? []} step={wb.step} playing={wb.playing} onStep={wb.setStep} onPause={wb.pause} onToggle={wb.togglePlay} />
              </>
            )}
            {wb.tab === "tokens" && trace && <TokensAst trace={trace} source={wb.source} highlight={wb.highlight} onPick={goTo} onHover={wb.setHover} caret={ignoredEdits ? null : wb.caret} filter={filter} onFilter={setFilter} filterRef={filterRef} stale={ignoredEdits} />}
            {wb.tab === "bytecode" && trace && <Bytecode trace={trace} source={wb.source} onPick={goTo} onHover={wb.setHover} />}
          </div>

          <Stdout trace={trace} step={wb.step} />
        </section>
      </main>

      <StatusBar source={wb.source} caret={wb.caret} note={trace?.mode === "mock" && !ignoredEdits ? wb.note : null} />
    </div>
  );
}
