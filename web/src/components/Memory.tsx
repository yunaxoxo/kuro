import { memo, useLayoutEffect, useMemo, useRef } from "react";
import { cellAddress, cellValue } from "../lib/format";
import { RESERVED_CELLS, stepLabel } from "../lib/step";
import type { TraceStep } from "../trace/types";

interface Props {
  step: TraceStep | null;
  previous: TraceStep | null;
  index: number;
  count: number;
  hasErrors: boolean;
  /** While the trace plays, announcements pause; they resume on pause or manual stepping. */
  playing: boolean;
  /** The whole trace never writes memory: show a message in place of the stems. */
  silent: boolean;
  /** Open the WASM Bytecode view (the empty state points a program with no heap writes there). */
  onBytecode: () => void;
}

/** Slot name under an address: the runtime slots by name, the first array's elements by index. */
function slotName(cells: number[], i: number): string {
  if (i === 0) return "PTR";
  if (i === 1) return "RSV";
  const allocated = (cells[0] ?? 0) > 0;
  if (i === RESERVED_CELLS - 1) return allocated ? "LEN" : "";
  const element = i - RESERVED_CELLS;
  return allocated && element < (cells[2] ?? 0) ? `[${element}]` : "";
}

export const Memory = memo(function Memory({ step, previous, index, count, hasErrors, playing, silent, onBytecode }: Props) {
  const cells = step?.memory_i32 ?? [];
  const mutated = useMemo(() => new Set(step?.mutated_indices ?? []), [step]);
  const plot = useRef<HTMLDivElement>(null);

  // When the plot is wider than its pane (phones), keep the cell just written in view.
  useLayoutEffect(() => {
    const box = plot.current;
    const written = box ? [...box.querySelectorAll<HTMLElement>(".cell[data-changed]")] : [];
    if (!box || written.length === 0) return;
    // Every cell this step wrote (a swap writes two), from the first one's left edge to the last one's right edge.
    const left = written[0].offsetLeft;
    const right = written[written.length - 1].offsetLeft + written[written.length - 1].offsetWidth;
    if (right > box.scrollLeft + box.clientWidth) box.scrollLeft = right - box.clientWidth;
    if (left < box.scrollLeft) box.scrollLeft = left;
  }, [step]);

  // Scale to the array elements; the reserved slots are clamped so they never dwarf the data.
  const scale = useMemo(() => {
    const data = cells.slice(RESERVED_CELLS).map(Math.abs);
    return Math.max(...(data.length ? data : cells.map(Math.abs)), 1);
  }, [cells]);

  const written = cells.flatMap((value, i) => (mutated.has(i) ? [`${cellAddress(i)} = ${cellValue(i, value)}`] : []));
  // The last step of a run: the sentence that closes it gets full ink and an invitation to what to look at next.
  const finished = !!step && count > 1 && index === count - 1;
  const summary = step
    ? `Linear memory after step ${index + 1} of ${count}, ${cells.length} cells.${written.length ? ` Written: ${written.join(", ")}.` : " Nothing written."}`
    : "No memory snapshot";

  return (
    <section className="memory" aria-label="Linear memory" data-final={finished || undefined}>
      <header className="readout">
        <h2 className="readout-title">Memory · Int32</h2>
        <p className="readout-step">
          {step ? stepLabel(step) : hasErrors ? "No run" : "…"}
        </p>
        {/* Only instrumented runs carry these; recorded runs and plain modules show nothing extra. */}
        {step?.call_stack && (
          <p className="readout-meta">
            Stack <span>{step.call_stack.join(" → ")}</span>
          </p>
        )}
        {step?.gas && (
          <p className="readout-meta">
            Gas <span>{step.gas.alu + step.gas.branch} ops · {step.gas.load + step.gas.store} mem</span>
          </p>
        )}
      </header>
      {/* What the step did, in words: visible for everyone, announced only when playback is paused or stepped by hand. */}
      <p className="narration" aria-live={playing ? "off" : "polite"} title={step?.narration}>
        {step?.narration || "\u00a0"}
        {finished && (
          <span className="narration-next">
            <span className="key-hint"> · ← steps back</span> · 02 and 03 show how it was built
          </span>
        )}
      </p>

      {silent ? (
        <div className="plot plot-empty">
          <p className="plot-note">
            No heap writes. This program never allocates. Follow it in STDOUT below, or read its instructions in{" "}
            <button type="button" className="text-action plot-link" onClick={onBytecode}>
              03 WASM Bytecode
            </button>
            .
          </p>
        </div>
      ) : (
        <div className="plot" ref={plot} role="img" aria-label={summary} translate="no">
          <div className="cells" style={{ "--n": cells.length } as React.CSSProperties}>
            <i className="axis" aria-hidden="true" />
            {cells.map((value, i) => {
              const changed = mutated.has(i);
              const before = previous?.memory_i32[i] ?? 0;
              const style = {
                "--v": Math.min(1, Math.abs(value) / scale),
                "--p": Math.min(1, Math.abs(before) / scale),
                "--min": value === 0 ? "0px" : "2px",
                "--min-p": before === 0 ? "0px" : "2px",
              } as React.CSSProperties;
              return (
                <div
                  key={i}
                  className="cell"
                  style={style}
                  data-changed={changed || undefined}
                  data-reserved={i < RESERVED_CELLS || undefined}
                  data-divider={i === RESERVED_CELLS || undefined}
                  title={`${cellAddress(i)} = ${cellValue(i, value)}${changed ? ` (was ${cellValue(i, before)})` : ""}`}
                >
                  <div className="track">
                    <i className="cursor" />
                    <i className="peak" />
                    <i className="stem" />
                    <span className="value">{cellValue(i, value)}</span>
                  </div>
                  <span className="addr">{cellAddress(i)}</span>
                  <span className="role">{slotName(cells, i) || "\u00a0"}</span>
                </div>
              );
            })}
          </div>
        </div>
      )}
      {!silent && (
        <p className="legend" aria-hidden="true">
          <span className="legend-item">
            <i className="swatch swatch-peak" />
            previous value
          </span>
          <span className="legend-item">
            <i className="swatch swatch-dim" />
            runtime header: allocator pointer, reserved, array length
          </span>
          <span className="legend-item">
            <i className="swatch swatch-tick" />
            tall tick: memory written
          </span>
        </p>
      )}
    </section>
  );
});
