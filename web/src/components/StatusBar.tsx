import { isMac } from "../lib/format";
import { lineColOf } from "../lib/spans";

/** Cursor, recorded-run note and key hints. The run mode lives in the header. */
export function StatusBar({ source, caret, note }: { source: string; caret: number | null; note: string | null }) {
  const { line, col } = lineColOf(source, caret ?? 0);
  const run = isMac() ? "⌘↵" : "Ctrl↵";
  return (
    <footer className="statusbar">
      {/* Only a real caret: before the editor is used, "Ln 1, Col 1" would contradict the highlighted step line. */}
      {caret !== null && (
        <span className="seg seg-cursor">
          Ln {line}, Col {col}
        </span>
      )}
      {note && (
        <span className="seg seg-note" title={note}>
          Replaying a recorded run
        </span>
      )}
      <span className="seg seg-hints seg-hints-edit">
        <span>
          <kbd>{run}</kbd> Run
        </span>
        <span>
          <kbd>Tab</kbd> Indent
        </span>
        <span>
          <kbd>Esc</kbd> Leave Editor
        </span>
      </span>
      <span className="seg seg-hints seg-hints-nav">
        <span>
          <kbd>{run}</kbd> Run
        </span>
        <span>
          <kbd>←/→</kbd> Step
        </span>
        <span>
          <kbd>Space</kbd> Play
        </span>
      </span>
      <span className="seg seg-target">wasm32-mvp</span>
    </footer>
  );
}
