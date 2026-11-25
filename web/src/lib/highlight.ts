/** Display-only colouring of kuro source for the editor overlay. Not a lexer. */
export interface Span {
  text: string;
  /** k keyword, t type, n literal, o operator or punctuation, i identifier, c comment, s space */
  cls: string;
  /** Painted by hover in the Tokens & AST or Bytecode view. */
  hl?: boolean;
  /** The 300ms flash after a click-to-source jump. */
  pulse?: boolean;
  /** A compiler error span. */
  err?: boolean;
}

const KEYWORDS = new Set(["fn", "let", "if", "else", "while", "return"]);
const LITERAL_WORDS = new Set(["true", "false"]);
const PIECE = /(\/\/.*)|(\b(?:int|float|bool)\b(?:\[\])?)|([A-Za-z_][A-Za-z0-9_]*)|(\d+(?:\.\d+)?)|(\s+)|(.)/gy;

export function highlightLine(line: string): Span[] {
  const spans: Span[] = [];
  PIECE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while (PIECE.lastIndex < line.length && (match = PIECE.exec(line))) {
    const [text, comment, type, word, number, space] = match;
    let cls = "o";
    if (comment) cls = "c";
    else if (type) cls = "t";
    else if (word) cls = KEYWORDS.has(word) ? "k" : LITERAL_WORDS.has(word) ? "n" : "i";
    else if (number) cls = "n";
    else if (space) cls = "s";
    spans.push({ text, cls });
  }
  return spans;
}

/** Split `spans` so the characters in [from, to) carry `flag`. */
export function markRange(spans: Span[], from: number, to: number, flag: "hl" | "pulse" | "err"): Span[] {
  if (to <= from) return spans;
  const out: Span[] = [];
  let pos = 0;
  for (const span of spans) {
    const end = pos + span.text.length;
    if (end <= from || pos >= to) {
      out.push(span);
    } else {
      const a = Math.max(from, pos) - pos;
      const b = Math.min(to, end) - pos;
      if (a > 0) out.push({ ...span, text: span.text.slice(0, a) });
      out.push({ ...span, text: span.text.slice(a, b), [flag]: true });
      if (b < span.text.length) out.push({ ...span, text: span.text.slice(b) });
    }
    pos = end;
  }
  return out;
}
