export interface Highlight {
  /** Identifies what was clicked, e.g. "t12" for a token or "n p0.2" for an AST node. */
  key: string;
  /** Half-open character range in the source text. */
  start: number;
  end: number;
}

/** Character offset of a 1-based line:col in `source`. */
export function offsetOf(source: string, line: number, col: number): number {
  let offset = 0;
  for (let l = 1; l < line; l++) {
    const next = source.indexOf("\n", offset);
    if (next === -1) return source.length;
    offset = next + 1;
  }
  return Math.min(offset + col - 1, source.length);
}

/** 1-based line and column of a character offset in `source`. */
export function lineColOf(source: string, offset: number): { line: number; col: number } {
  const upto = source.slice(0, Math.min(offset, source.length));
  const line = upto.split("\n").length;
  return { line, col: offset - (upto.lastIndexOf("\n") + 1) + 1 };
}
