//! # Tokens
//!
//! The vocabulary shared by the lexer (which produces tokens) and the parser
//! (which consumes them). Everything in this file is a plain data definition
//! except [`TokenKind::keyword`].

use serde::Serialize;
use std::fmt;

/// A region of the source text.
///
/// * `start..end` is a **half-open byte range** into the UTF-8 source, so
///   `&source[span.start..span.end]` is exactly the text the span covers.
/// * `line` and `col` locate `start` for humans. Both are **1-based**. `col`
///   counts bytes from the start of the line, which equals the character
///   column for ASCII (the only thing kuro allows outside comments).
///
/// The playground underlines `start..end` and prints `line:col`, so all four
/// fields need to be right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: u32,
    pub col: u32,
}

impl Span {
    pub const fn new(start: usize, end: usize, line: u32, col: u32) -> Self {
        Span { start, end, line, col }
    }

    /// The smallest span covering `self` through `other`.
    ///
    /// Precondition: `self` is the piece that starts first.
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: self.end.max(other.end),
            line: self.line,
            col: self.col,
        }
    }
}

/// Every kind of token the lexer can produce.
///
/// Literal and identifier variants carry their decoded value; all other
/// variants are fully described by the variant itself.
///
/// JSON shape (for the playground): `{"type":"Int","value":42}` for the
/// payload-carrying variants and `{"type":"Plus"}` for the rest.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum TokenKind {
    // ----- literals & names -------------------------------------------------
    /// Decimal integer literal, already range-checked to `0..=i32::MAX`.
    /// (Negative numbers are the unary `-` operator applied to a literal.)
    Int(i32),
    /// Float literal `digits '.' digits`, e.g. `3.14`. No exponent form.
    Float(f64),
    /// Identifier: `[A-Za-z_][A-Za-z0-9_]*` that is not a keyword.
    Ident(String),

    // ----- keywords ---------------------------------------------------------
    Fn,
    Let,
    If,
    Else,
    While,
    Return,
    True,
    False,
    /// The type keyword `int`.
    IntType,
    /// The type keyword `float`.
    FloatType,
    /// The type keyword `bool`.
    BoolType,

    // ----- punctuation ------------------------------------------------------
    LParen,    // (
    RParen,    // )
    LBrace,    // {
    RBrace,    // }
    LBracket,  // [
    RBracket,  // ]
    Comma,     // ,
    Colon,     // :
    Semicolon, // ;
    Arrow,     // ->

    // ----- operators --------------------------------------------------------
    Plus,    // +
    Minus,   // -
    Star,    // *
    Slash,   // /
    Percent, // %
    Assign,  // =
    EqEq,    // ==
    NotEq,   // !=
    Lt,      // <
    LtEq,    // <=
    Gt,      // >
    GtEq,    // >=
    AndAnd,  // &&
    OrOr,    // ||
    Bang,    // !

    /// End of input. The lexer always appends exactly one `Eof` as the final
    /// token so the parser never has to bounds-check its cursor.
    Eof,
}

impl TokenKind {
    /// Map a reserved word to its keyword token, or `None` for any other
    /// string.
    pub fn keyword(text: &str) -> Option<TokenKind> {
        let _ = text;
        todo!("token: TokenKind::keyword")
    }

    /// Human-readable description for error messages, e.g. `` `->` `` or
    /// ``identifier `foo` ``. Already implemented.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Int(v) => format!("integer literal `{v}`"),
            TokenKind::Float(v) => format!("float literal `{v:?}`"),
            TokenKind::Ident(name) => format!("identifier `{name}`"),
            TokenKind::Eof => "end of input".to_string(),
            other => format!("`{}`", other.fixed_text()),
        }
    }

    /// Source text of tokens whose spelling never varies.
    fn fixed_text(&self) -> &'static str {
        match self {
            TokenKind::Fn => "fn",
            TokenKind::Let => "let",
            TokenKind::If => "if",
            TokenKind::Else => "else",
            TokenKind::While => "while",
            TokenKind::Return => "return",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::IntType => "int",
            TokenKind::FloatType => "float",
            TokenKind::BoolType => "bool",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBrace => "{",
            TokenKind::RBrace => "}",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Comma => ",",
            TokenKind::Colon => ":",
            TokenKind::Semicolon => ";",
            TokenKind::Arrow => "->",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::Assign => "=",
            TokenKind::EqEq => "==",
            TokenKind::NotEq => "!=",
            TokenKind::Lt => "<",
            TokenKind::LtEq => "<=",
            TokenKind::Gt => ">",
            TokenKind::GtEq => ">=",
            TokenKind::AndAnd => "&&",
            TokenKind::OrOr => "||",
            TokenKind::Bang => "!",
            TokenKind::Int(_) | TokenKind::Float(_) | TokenKind::Ident(_) | TokenKind::Eof => "",
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe())
    }
}

/// One lexical unit: what it is and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }
}
