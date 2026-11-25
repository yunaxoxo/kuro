//! # Abstract syntax tree
//!
//! The data structure the parser builds, the type checker annotates, and the
//! code generator walks. This file is complete: there is nothing to implement
//! here.
//!
//! ## Shape of a program
//!
//! ```text
//! Program
//! └── Function*            fn name(params) -> return_type { body }
//!     └── Block            { stmt* }
//!         └── Stmt*        let / assign / index-assign / if / while / return / expr;
//!             └── Expr     literals, variables, unary, binary, call, index
//! ```
//!
//! ## The `ty` annotation contract
//!
//! Every [`Expr`] has a `ty: Option<Type>` slot.
//!
//! * The **parser** always leaves it `None`.
//! * The **type checker** fills in `Some(..)` on *every* expression node it
//!   accepts (including sub-expressions).
//! * The **code generator** reads it.
//!
//! That is the whole interface between the type checker and codegen.
//!
//! JSON: every type here derives `Serialize` with serde's default (externally
//! tagged) representation; the playground renders it as a tree.

use crate::token::Span;
use serde::Serialize;
use std::fmt;

/// A kuro type.
///
/// | Type       | Source spelling | Wasm representation                         |
/// |------------|-----------------|---------------------------------------------|
/// | `Int`      | `int`           | `i32`                                       |
/// | `Float`    | `float`         | `f64`                                       |
/// | `Bool`     | `bool`          | `i32` holding exactly `0` or `1`            |
/// | `IntArray` | `int[]`         | `i32` address of the array header in memory |
/// | `Void`     | *(none)*        | no value                                    |
///
/// `Void` cannot be written in source. It is the return type of a function
/// declared without `-> T`, and therefore the type of a call to one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Type {
    Int,
    Float,
    Bool,
    IntArray,
    Void,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Type::Int => "int",
            Type::Float => "float",
            Type::Bool => "bool",
            Type::IntArray => "int[]",
            Type::Void => "void",
        })
    }
}

/// A whole source file: a list of functions in source order.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Program {
    pub functions: Vec<Function>,
}

/// `fn name(params) -> return_type { body }`
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    /// [`Type::Void`] when the `-> type` clause is omitted.
    pub return_type: Type,
    pub body: Block,
    /// From the `fn` keyword through the closing `}`.
    pub span: Span,
}

/// One `name: type` parameter.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

/// `{ stmt* }`. A block is also a lexical scope.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    /// From `{` through `}`.
    pub span: Span,
}

/// A statement.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Stmt {
    /// `let name = init;` or `let name: declared_ty = init;`
    Let {
        name: String,
        /// The annotation, if the programmer wrote one.
        declared_ty: Option<Type>,
        init: Expr,
        span: Span,
    },
    /// `name = value;`
    Assign {
        name: String,
        value: Expr,
        span: Span,
    },
    /// `array[index] = value;`
    IndexAssign {
        array: Expr,
        index: Expr,
        value: Expr,
        span: Span,
    },
    /// `if cond { .. }` with an optional `else { .. }`.
    ///
    /// `else if c { .. }` is sugar: the parser stores it as an `else_block`
    /// containing exactly one nested `Stmt::If`.
    If {
        cond: Expr,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    /// `while cond { .. }`
    While {
        cond: Expr,
        body: Block,
        span: Span,
    },
    /// `return;` or `return value;`
    Return {
        value: Option<Expr>,
        span: Span,
    },
    /// `expr;` — an expression evaluated for its side effects, typically a
    /// call such as `print_int(x);`.
    Expr(Expr),
}

/// An expression: a node kind, its source span, and (after type checking)
/// its type.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    /// `None` out of the parser, `Some` after the type checker. See the
    /// module docs.
    pub ty: Option<Type>,
}

impl Expr {
    /// Build an un-annotated expression (what the parser produces).
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Expr { kind, span, ty: None }
    }
}

/// The different shapes an expression can take.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ExprKind {
    /// `42`
    IntLit(i32),
    /// `3.14`
    FloatLit(f64),
    /// `true` / `false`
    BoolLit(bool),
    /// A variable or parameter reference: `x`
    Var(String),
    /// `-operand` or `!operand`
    Unary { op: UnaryOp, operand: Box<Expr> },
    /// `lhs op rhs`
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    /// `callee(args...)`. Functions are not first-class, so the callee is
    /// always a plain name.
    Call { callee: String, args: Vec<Expr> },
    /// `array[index]`
    Index { array: Box<Expr>, index: Box<Expr> },
}

/// Prefix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum UnaryOp {
    /// Arithmetic negation `-x` (`int` or `float`).
    Neg,
    /// Logical not `!x` (`bool`).
    Not,
}

impl UnaryOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
        }
    }
}

/// Infix operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum BinaryOp {
    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    // comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    // logical (short-circuiting)
    And,
    Or,
}

impl BinaryOp {
    /// The operator as written in source, e.g. `"<="`.
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
            BinaryOp::And => "&&",
            BinaryOp::Or => "||",
        }
    }
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}
