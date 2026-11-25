import { memo, useLayoutEffect, useRef } from "react";
import { printedThrough, stdoutLines } from "../lib/stdout";
import type { ExecutionTrace } from "../trace/types";

export const Stdout = memo(function Stdout({ trace, step }: { trace: ExecutionTrace | null; step: number }) {
  const strip = useRef<HTMLOListElement>(null);
  const lines = trace ? stdoutLines(trace) : [];
  const shown = trace ? printedThrough(trace, step) : 0;

  useLayoutEffect(() => {
    const el = strip.current;
    if (!el || shown === 0) return;
    const item = el.children[shown - 1] as HTMLElement | undefined;
    if (!item) return;
    const items = [...el.children] as HTMLElement[];
    let left = el.scrollLeft;
    if (item.offsetLeft + item.offsetWidth > left + el.clientWidth) left = item.offsetLeft + item.offsetWidth - el.clientWidth;
    else if (item.offsetLeft < left) left = item.offsetLeft;
    // Start on a whole value: the first one at or after the needed offset, so the left edge never shows half a number.
    el.scrollLeft = items.find((li) => li.offsetLeft >= left)?.offsetLeft ?? left;
  }, [shown]);

  return (
    <div className="stdout">
      <h2 className="stdout-label">
        STDOUT
        {lines.length > 0 && (
          <span className="stdout-count">
            {shown} / {lines.length}
          </span>
        )}
      </h2>
      {lines.length > 0 && (
        <ol className="stdout-lines" ref={strip} aria-label="Program output">
          {lines.map((line, i) => (
            <li key={i} className="stdout-line" data-pending={i >= shown || undefined} data-latest={i === shown - 1 || undefined} aria-hidden={i >= shown || undefined}>
              {line}
            </li>
          ))}
        </ol>
      )}
    </div>
  );
});
