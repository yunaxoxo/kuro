import type { Workbench } from "../hooks/useWorkbench";
import { isMac } from "../lib/format";
import { Mark } from "./Mark";

export function Header({ wb, onRun, replayOnly }: { wb: Workbench; onRun: () => void; replayOnly: boolean }) {
  const shortcut = isMac() ? "⌘↵" : "Ctrl↵";
  const mode = wb.trace?.mode ?? null;
  return (
    <header className="header">
      <div className="brand">
        <h1 className="wordmark" translate="no">
          <Mark />
          kuro
          {/* Visible on wide screens so a cold visitor knows what this is; read out everywhere. */}
          <span className="tagline">
            <span className="tagline-full">Rust → WebAssembly compiler</span>
            <span className="tagline-short" aria-hidden="true">
              Rust → Wasm
            </span>
          </span>
        </h1>
        {mode && (
          <span className="brand-mode" data-mode={mode} title={wb.note ?? "Compiled and run by the Rust backend"}>
            <span className="mode-dot" aria-hidden="true" />
            {mode === "live" ? "Live WASM" : "Recorded Run"}
          </span>
        )}
      </div>

      <nav className="samples" aria-label="Sample programs">
        {wb.samples.map((sample) => (
          <button
            key={sample.id}
            type="button"
            className="sample"
            aria-pressed={sample.id === wb.sample.id}
            onClick={() => wb.chooseSample(sample)}
          >
            <span className="sample-no">{sample.number}</span>
            {sample.title}
          </button>
        ))}
      </nav>

      <div className="run-zone">
        <button
          type="button"
          className="run"
          onClick={onRun}
          aria-busy={wb.busy}
          aria-keyshortcuts="Control+Enter Meta+Enter"
          title={replayOnly ? "Your edits can't be compiled in this build: this replays the recorded run" : undefined}
        >
          {/* One inline run, so the sans label and the mono keycap sit on one baseline. */}
          <span className="run-label">
            <span className="run-full">Compile &amp; Run</span>
            <span className="run-short">Run</span>
            <kbd className="run-keys">{shortcut}</kbd>
          </span>
        </button>
      </div>
    </header>
  );
}
