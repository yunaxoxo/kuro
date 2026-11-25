/** Turns the compiler's serde-encoded AST (`trace.ast_json`) into a readable tree of nodes. */

export interface AstSpan {
  start: number;
  end: number;
  line: number;
  col: number;
}

export interface TreeNode {
  id: string;
  /** Role of the node in its parent, e.g. "cond", "lhs". */
  role?: string;
  /** Node kind, e.g. "FnDecl", "BinaryExpr". */
  kind: string;
  /** Name, operator or literal the node carries. */
  ident?: string;
  /** Declared or inferred type text, e.g. "int[]", or a function signature. */
  type?: string;
  span?: AstSpan;
  children: TreeNode[];
}

type Json = Record<string, unknown>;

const OPS: Record<string, string> = {
  Add: "+", Sub: "-", Mul: "*", Div: "/", Rem: "%", Eq: "==", Ne: "!=", Lt: "<", Le: "<=", Gt: ">", Ge: ">=", And: "&&", Or: "||", Neg: "-", Not: "!",
};

const TYPES: Record<string, string> = { Int: "int", Float: "float", Bool: "bool", IntArray: "int[]", Void: "void" };
const tyName = (ty: unknown): string | undefined => (typeof ty === "string" ? (TYPES[ty] ?? ty) : undefined);

const isObj = (v: unknown): v is Json => typeof v === "object" && v !== null && !Array.isArray(v);
const asSpan = (v: unknown): AstSpan | undefined =>
  isObj(v) && typeof v.start === "number" && typeof v.end === "number" && typeof v.line === "number" && typeof v.col === "number"
    ? { start: v.start, end: v.end, line: v.line, col: v.col }
    : undefined;

/** `{ Variant: payload }` → ["Variant", payload]; a bare string variant → [name, null]. */
function variant(v: unknown): [string, unknown] | null {
  if (typeof v === "string") return [v, null];
  if (isObj(v)) {
    const key = Object.keys(v)[0];
    if (key !== undefined) return [key, v[key]];
  }
  return null;
}

function expr(node: unknown, id: string, role?: string): TreeNode {
  const e = isObj(node) ? node : {};
  const span = asSpan(e.span);
  const type = tyName(e.ty);
  const [kind, body] = variant(e.kind) ?? ["Expr", null];
  const make = (k: string, ident: string | undefined, children: TreeNode[]): TreeNode => ({ id, role, kind: k, ident, type, span, children });
  const b = isObj(body) ? body : {};
  switch (kind) {
    case "IntLit": return make("IntLit", String(body), []);
    case "FloatLit": return make("FloatLit", String(body), []);
    case "BoolLit": return make("BoolLit", String(body), []);
    case "Var": return make("VarExpr", String(body), []);
    case "Unary": return make("UnaryExpr", OPS[String(b.op)] ?? String(b.op), [expr(b.operand, `${id}.0`, "operand")]);
    case "Binary": return make("BinaryExpr", OPS[String(b.op)] ?? String(b.op), [expr(b.lhs, `${id}.0`, "lhs"), expr(b.rhs, `${id}.1`, "rhs")]);
    case "Call": return make("CallExpr", String(b.callee), ((b.args as unknown[]) ?? []).map((a, i) => expr(a, `${id}.${i}`, "arg")));
    case "Index": return make("IndexExpr", undefined, [expr(b.array, `${id}.0`, "array"), expr(b.index, `${id}.1`, "index")]);
    default: return make(kind, undefined, []);
  }
}

function block(b: unknown, id: string, role: string): TreeNode {
  const o = isObj(b) ? b : {};
  return { id, role, kind: "Block", span: asSpan(o.span), children: stmts(o.stmts, id) };
}

function stmts(list: unknown, parent: string): TreeNode[] {
  return ((list as unknown[]) ?? []).map((s, i) => stmt(s, `${parent}.${i}`));
}

function stmt(node: unknown, id: string): TreeNode {
  const [kind, body] = variant(node) ?? ["Stmt", null];
  const b = isObj(body) ? body : {};
  const span = asSpan(kind === "Expr" ? (b as Json).span : b.span);
  switch (kind) {
    case "Let": {
      const init = expr(b.init, `${id}.0`, "init");
      return { id, kind: "LetStmt", ident: String(b.name), type: tyName(b.declared_ty) ?? init.type, span, children: [init] };
    }
    case "Assign": return { id, kind: "AssignStmt", ident: String(b.name), span, children: [expr(b.value, `${id}.0`, "value")] };
    case "IndexAssign":
      return { id, kind: "IndexAssign", span, children: [expr(b.array, `${id}.0`, "array"), expr(b.index, `${id}.1`, "index"), expr(b.value, `${id}.2`, "value")] };
    case "If": {
      const children = [expr(b.cond, `${id}.0`, "cond"), block(b.then_block, `${id}.1`, "then")];
      if (b.else_block) children.push(block(b.else_block, `${id}.2`, "else"));
      return { id, kind: "IfStmt", span, children };
    }
    case "While": return { id, kind: "WhileStmt", span, children: [expr(b.cond, `${id}.0`, "cond"), block(b.body, `${id}.1`, "body")] };
    case "Return": return { id, kind: "ReturnStmt", span, children: b.value ? [expr(b.value, `${id}.0`, "value")] : [] };
    case "Expr": {
      const inner = expr(node && isObj(node) ? (node as Json).Expr : null, `${id}.0`);
      return { id, kind: "ExprStmt", span: inner.span, children: [inner] };
    }
    default: return { id, kind, span, children: [] };
  }
}

function fnDecl(f: unknown, id: string): TreeNode {
  const o = isObj(f) ? f : {};
  const rawParams = ((o.params as unknown[]) ?? []).filter(isObj);
  const params: TreeNode[] = rawParams.map((p, i) => ({
    id: `${id}.p${i}`,
    kind: "Param",
    ident: String(p.name),
    type: tyName(p.ty),
    span: asSpan(p.span),
    children: [],
  }));
  const ret = tyName(o.return_type) ?? "void";
  const types = params.map((p) => p.type ?? "?");
  const input = types.length === 1 ? types[0] : `(${types.join(", ")})`;
  const body = isObj(o.body) ? o.body : {};
  const children: TreeNode[] = [];
  if (params.length > 0) {
    const first = params[0].span;
    const last = params[params.length - 1].span;
    children.push({
      id: `${id}.params`,
      role: "params",
      kind: "Params",
      span: first && last ? { start: first.start, end: last.end, line: first.line, col: first.col } : undefined,
      children: params,
    });
  }
  children.push(block(body, `${id}.body`, "body"));
  return { id, kind: "FnDecl", ident: String(o.name), type: `${input} → ${ret}`, span: asSpan(o.span), children };
}

/**
 * The tree for a trace's `ast_json`, or null when there is none. The compiler's `start`/`end` are UTF-8 byte offsets;
 * the editor indexes JavaScript string offsets, so they are converted here, once. `line`/`col` need no
 * conversion: non-ASCII may only appear in comments, which run to the end of a line, so the text before any node's
 * start on its line is ASCII.
 */
export function buildTree(ast: unknown, source: string): TreeNode | null {
  if (!isObj(ast) || !Array.isArray(ast.functions)) return null;
  const tree: TreeNode = { id: "p", kind: "Program", children: ast.functions.map((f, i) => fnDecl(f, `p${i}`)) };
  const toChar = byteToChar(source);
  if (!toChar) return tree;
  // New objects, never in place: a statement can share its span object with the expression it wraps.
  const convert = (n: TreeNode): TreeNode => ({
    ...n,
    span: n.span && { ...n.span, start: toChar(n.span.start), end: toChar(n.span.end) },
    children: n.children.map(convert),
  });
  return convert(tree);
}

/** Byte offset → string offset for `source`, or null when the two agree (pure ASCII). */
export function byteToChar(source: string): ((byte: number) => number) | null {
  if (!/[^\x00-\x7f]/.test(source)) return null;
  const at: number[] = []; // at[byte] = string offset of the character that byte belongs to
  let offset = 0;
  for (const ch of source) {
    const cp = ch.codePointAt(0)!;
    const bytes = cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
    for (let k = 0; k < bytes; k++) at.push(offset);
    offset += ch.length;
  }
  at.push(offset);
  return (byte) => at[Math.max(0, Math.min(byte, at.length - 1))];
}

export function collectIds(node: TreeNode, out: string[] = []): string[] {
  if (node.children.length > 0) out.push(node.id);
  node.children.forEach((c) => collectIds(c, out));
  return out;
}

/** Ids of the nodes shown open when a tree first appears: the program, its functions, and their bodies. */
export function defaultOpen(root: TreeNode): string[] {
  const ids: string[] = [];
  const walk = (node: TreeNode, depth: number) => {
    if (depth <= 2 && node.children.length > 0) ids.push(node.id);
    node.children.forEach((c) => walk(c, depth + 1));
  };
  walk(root, 0);
  return ids;
}

/** The chain of nodes enclosing `offset`, outermost first; the last entry is the innermost. */
export function pathAt(root: TreeNode, offset: number): TreeNode[] {
  const path: TreeNode[] = [root];
  let node = root;
  for (;;) {
    const next = node.children.find((c) => c.span && c.span.start <= offset && offset <= c.span.end);
    if (!next) return path;
    path.push(next);
    node = next;
  }
}
