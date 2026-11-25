import { useCallback, useRef, useState } from "react";

/**
 * Roving focus for a list: one tab stop, arrow keys move within it.
 * Items carry `data-rid` and `tabIndex={tabIndexFor(id)}`; the container gets `ref` and `onKeyDown`.
 */
export function useRoving(ids: string[]) {
  const [active, setActive] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const current = active !== null && ids.includes(active) ? active : (ids[0] ?? null);

  const focusRow = useCallback((id: string) => {
    setActive(id);
    requestAnimationFrame(() => {
      const el = root.current?.querySelector<HTMLElement>(`[data-rid="${CSS.escape(id)}"]`);
      el?.focus();
      el?.scrollIntoView({ block: "nearest" });
    });
  }, []);

  /** Up, Down, Home, End, PageUp, PageDown. Left and Right are left to the item. */
  const onKeyDown = (event: React.KeyboardEvent) => {
    const at = current === null ? -1 : ids.indexOf(current);
    let to: number;
    switch (event.key) {
      case "ArrowDown": to = at + 1; break;
      case "ArrowUp": to = at - 1; break;
      case "PageDown": to = at + 10; break;
      case "PageUp": to = at - 10; break;
      case "Home": to = 0; break;
      case "End": to = ids.length - 1; break;
      default: return;
    }
    event.preventDefault();
    event.stopPropagation(); // never also step the trace
    if (ids.length > 0) focusRow(ids[Math.min(ids.length - 1, Math.max(0, to))]);
  };

  return { root, current, setActive, focusRow, onKeyDown, tabIndexFor: (id: string) => (id === current ? 0 : -1) };
}
