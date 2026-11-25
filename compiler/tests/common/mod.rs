//! Helpers shared by the integration tests.
//!
//! Nothing in here is part of the compiler. It is test plumbing: thin
//! wrappers that run the pipeline up to a given stage and fail loudly, with
//! the source attached, when an earlier stage breaks.

#![allow(dead_code)]

use kuro::ast::{Block, Expr, ExprKind, Program, Stmt, UnaryOp};
use kuro::error::CompileError;
use kuro::token::{Token, TokenKind};
use kuro::{lexer, parser, typeck};

pub const FIBONACCI: &str = include_str!("../../../examples/fibonacci.kr");
pub const ARRAY_SUM: &str = include_str!("../../../examples/array_sum.kr");
pub const BUBBLE_SORT: &str = include_str!("../../../examples/bubble_sort.kr");

/// The three sample programs shipped with the playground, as `(name, source)`.
pub const EXAMPLES: [(&str, &str); 3] = [
    ("fibonacci", FIBONACCI),
    ("array_sum", ARRAY_SUM),
    ("bubble_sort", BUBBLE_SORT),
];

// ----- lexer -----------------------------------------------------------------

/// Lex `source`, panicking with a readable message on a lexical error.
pub fn lex(source: &str) -> Vec<Token> {
    lexer::tokenize(source)
        .unwrap_or_else(|err| panic!("unexpected lex error: {err}\n--- source ---\n{source}"))
}

/// Lex `source` and keep only the token kinds (including the final `Eof`).
pub fn kinds(source: &str) -> Vec<TokenKind> {
    lex(source).into_iter().map(|token| token.kind).collect()
}

/// Lex `source`, which must fail, and return the error.
pub fn lex_err(source: &str) -> CompileError {
    match lexer::tokenize(source) {
        Ok(tokens) => panic!(
            "expected a lex error but got {} tokens\n--- source ---\n{source}",
            tokens.len()
        ),
        Err(err) => err,
    }
}

// ----- parser ----------------------------------------------------------------

/// Lex and parse a whole program, panicking on any error.
pub fn parse_program(source: &str) -> Program {
    parser::parse(lex(source))
        .unwrap_or_else(|err| panic!("unexpected parse error: {err}\n--- source ---\n{source}"))
}

/// Lex and parse a whole program that must fail **in the parser**.
pub fn parse_err(source: &str) -> CompileError {
    match parser::parse(lex(source)) {
        Ok(_) => panic!("expected a parse error but parsing succeeded\n--- source ---\n{source}"),
        Err(err) => err,
    }
}

/// Lex and parse a single standalone expression.
pub fn parse_expr(source: &str) -> Expr {
    parser::parse_expression(lex(source))
        .unwrap_or_else(|err| panic!("unexpected parse error: {err}\n--- source ---\n{source}"))
}

/// Lex and parse a standalone expression that must fail **in the parser**.
pub fn parse_expr_err(source: &str) -> CompileError {
    match parser::parse_expression(lex(source)) {
        Ok(expr) => panic!(
            "expected a parse error but got `{}`\n--- source ---\n{source}",
            sexpr(&expr)
        ),
        Err(err) => err,
    }
}

/// Parse an expression and render its tree as an S-expression, which makes
/// precedence and associativity visible at a glance:
///
/// ```text
/// 1 + 2 * 3      →  (+ 1 (* 2 3))
/// -a[0]          →  (neg (index a 0))
/// f(x, y + 1)    →  (call f x (+ y 1))
/// ```
pub fn tree(source: &str) -> String {
    sexpr(&parse_expr(source))
}

/// Render an expression as an S-expression (see [`tree`]).
pub fn sexpr(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::IntLit(value) => value.to_string(),
        ExprKind::FloatLit(value) => format!("{value:?}"),
        ExprKind::BoolLit(value) => value.to_string(),
        ExprKind::Var(name) => name.clone(),
        ExprKind::Unary { op, operand } => {
            let name = match op {
                UnaryOp::Neg => "neg",
                UnaryOp::Not => "not",
            };
            format!("({name} {})", sexpr(operand))
        }
        ExprKind::Binary { op, lhs, rhs } => {
            format!("({} {} {})", op.symbol(), sexpr(lhs), sexpr(rhs))
        }
        ExprKind::Call { callee, args } => {
            let mut out = format!("(call {callee}");
            for arg in args {
                out.push(' ');
                out.push_str(&sexpr(arg));
            }
            out.push(')');
            out
        }
        ExprKind::Index { array, index } => {
            format!("(index {} {})", sexpr(array), sexpr(index))
        }
    }
}

/// Parse `fn main() { <body> }` and return the statements of the body.
pub fn parse_body(body: &str) -> Vec<Stmt> {
    let source = format!("fn main() {{ {body} }}");
    let mut program = parse_program(&source);
    assert_eq!(program.functions.len(), 1, "expected exactly one function");
    program.functions.remove(0).body.stmts
}

// ----- type checker ----------------------------------------------------------

/// Lex, parse and type-check a program that must be accepted. Returns the
/// annotated AST.
pub fn check_ok(source: &str) -> Program {
    let mut program = parse_program(source);
    if let Err(err) = typeck::check(&mut program) {
        panic!("unexpected type error: {err}\n--- source ---\n{source}");
    }
    program
}

/// Lex, parse and type-check a program that must be rejected **by the type
/// checker** (lexing and parsing have to succeed).
pub fn check_err(source: &str) -> CompileError {
    let mut program = parse_program(source);
    match typeck::check(&mut program) {
        Ok(()) => panic!("expected a type error but the program was accepted\n--- source ---\n{source}"),
        Err(err) => err,
    }
}

/// Call `visit` on every expression in the program, parents before children.
pub fn for_each_expr(program: &Program, visit: &mut dyn FnMut(&Expr)) {
    for function in &program.functions {
        walk_block(&function.body, visit);
    }
}

fn walk_block(block: &Block, visit: &mut dyn FnMut(&Expr)) {
    for stmt in &block.stmts {
        match stmt {
            Stmt::Let { init, .. } => walk_expr(init, visit),
            Stmt::Assign { value, .. } => walk_expr(value, visit),
            Stmt::IndexAssign { array, index, value, .. } => {
                walk_expr(array, visit);
                walk_expr(index, visit);
                walk_expr(value, visit);
            }
            Stmt::If { cond, then_block, else_block, .. } => {
                walk_expr(cond, visit);
                walk_block(then_block, visit);
                if let Some(else_block) = else_block {
                    walk_block(else_block, visit);
                }
            }
            Stmt::While { cond, body, .. } => {
                walk_expr(cond, visit);
                walk_block(body, visit);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    walk_expr(value, visit);
                }
            }
            Stmt::Expr(expr) => walk_expr(expr, visit),
        }
    }
}

fn walk_expr(expr: &Expr, visit: &mut dyn FnMut(&Expr)) {
    visit(expr);
    match &expr.kind {
        ExprKind::IntLit(_) | ExprKind::FloatLit(_) | ExprKind::BoolLit(_) | ExprKind::Var(_) => {}
        ExprKind::Unary { operand, .. } => walk_expr(operand, visit),
        ExprKind::Binary { lhs, rhs, .. } => {
            walk_expr(lhs, visit);
            walk_expr(rhs, visit);
        }
        ExprKind::Call { args, .. } => {
            for arg in args {
                walk_expr(arg, visit);
            }
        }
        ExprKind::Index { array, index } => {
            walk_expr(array, visit);
            walk_expr(index, visit);
        }
    }
}

// ----- whole pipeline --------------------------------------------------------

/// Compile `source` to a wasm binary, panicking on any compile error.
pub fn compile_ok(source: &str) -> Vec<u8> {
    kuro::compile(source)
        .unwrap_or_else(|err| panic!("unexpected compile error: {err}\n--- source ---\n{source}"))
}
