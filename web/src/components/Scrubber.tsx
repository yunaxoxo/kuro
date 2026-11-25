import { memo, useRef, useState } from "react";
import { pad2 } from "../lib/format";
import type { TraceStep } from "../trace/types";

interface Props {
  steps: TraceStep[];
  step: number;
  playing: boolean;
  onStep: (index: number) => void;
  onPause: () => void;
  onToggle: () => void;
}

export const Scrubber = memo(function Scrubber({ steps, step, playing, onStep, onPause, onToggle }: Props) {
  const track = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<number | null>(null);
  const count = steps.length;
  const last = Math.max(0, count - 1);
  const position = (index: number) => (last === 0 ? 0 : (index / last) * 100);

  const nearest = (clientX: number) => {
    const box = track.current!.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientX - box.left) / box.width));
    return Math.round(ratio * last);
  };

  const disabled = count < 2;

  return (
    <div className="scrubber">
      <div className="transport">
        <button type="button" className="ctl ctl-step" disabled={step <= 0} onClick={() => { onPause(); onStep(step - 1); }} aria-label="Previous step" aria-keyshortcuts="ArrowLeft">
          <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true" focusable="false"><path d="M8 1v8L2 5z" fill="currentColor" /></svg>
        </button>
        <button type="button" className="ctl ctl-play" disabled={disabled} onClick={onToggle} aria-label={playing ? "Pause" : "Play"} aria-keyshortcuts="Space">
          <svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true" focusable="false">
            {playing ? <path d="M3 1h2v10H3zM7 1h2v10H7z" fill="currentColor" /> : <path d="M3 1v10l7-5z" fill="currentColor" />}
          </svg>
        </button>
        <button type="button" className="ctl ctl-step" disabled={step >= last} onClick={() => { onPause(); onStep(step + 1); }} aria-label="Next step" aria-keyshortcuts="ArrowRight">
          <svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true" focusable="false"><path d="M2 1v8l6-4z" fill="currentColor" /></svg>
        </button>
      </div>
      <output className="step-readout" aria-hidden="true">
        Step {pad2(count === 0 ? 0 : step + 1)} / {pad2(count)}
      </output>

      <div
        ref={track}
        className="track-line"
        role="slider"
        tabIndex={0}
        aria-label="Execution step"
        aria-valuemin={1}
        aria-valuemax={Math.max(1, count)}
        aria-valuenow={count === 0 ? 0 : step + 1}
        aria-valuetext={steps[step] ? `Step ${step + 1}: ${steps[step].op}` : "No steps"}
        aria-disabled={disabled}
        onPointerDown={(event) => {
          if (disabled) return;
          event.currentTarget.setPointerCapture(event.pointerId);
          onPause();
          onStep(nearest(event.clientX));
        }}
        onPointerMove={(event) => {
          if (disabled) return;
          const index = nearest(event.clientX);
          setHover(index);
          if (event.currentTarget.hasPointerCapture(event.pointerId)) onStep(index);
        }}
        onPointerLeave={() => setHover(null)}
      >
        <span className="rail" />
        <span className="rail-done" style={{ "--done": position(step) / 100 } as React.CSSProperties} />
        {steps.map((s, i) => (
          <span
            key={s.step_index}
            className="mark"
            data-writes={s.mutated_indices.length > 0 || undefined}
            data-past={i <= step || undefined}
            data-current={i === step || undefined}
            data-hover={i === hover || undefined}
            style={{ left: `round(down, ${position(i)}%, 1px)` }}
          />
        ))}
        {hover !== null && steps[hover] && (
          <span className="peek" style={{ left: `${position(hover)}%` }}>
            {pad2(hover + 1)}
          </span>
        )}
      </div>
    </div>
  );
});
