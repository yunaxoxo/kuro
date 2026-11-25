//! Lexer tests: `cargo test --test lexer_tests`
//!
//! Implementation notes: `src/lexer.rs`.

mod common;

use common::{kinds, lex, lex_err};
use kuro::error::{ErrorKind, Stage};
use kuro::token::{Span, TokenKind as T};

fn ident(name: &str) -> T {
    T::Ident(name.to_string())
}

#[test]
fn empty_source_is_just_eof() {
    assert_eq!(kinds(""), vec![T::Eof]);
    assert_eq!(kinds("  \t \r\n \n"), vec![T::Eof]);
}

#[test]
fn keywords_are_not_identifiers() {
    assert_eq!(
        kinds("fn let if else while return true false int float bool"),
        vec![
            T::Fn,
            T::Let,
            T::If,
            T::Else,
            T::While,
            T::Return,
            T::True,
            T::False,
            T::IntType,
            T::FloatType,
            T::BoolType,
            T::Eof,
        ]
    );

    // The lookup table itself.
    assert_eq!(T::keyword("while"), Some(T::While));
    assert_eq!(T::keyword("int"), Some(T::IntType));
    assert_eq!(T::keyword("whil"), None);
    assert_eq!(T::keyword("While"), None, "keywords are case-sensitive");
    assert_eq!(T::keyword(""), None);
}

#[test]
fn identifiers_use_longest_match() {
    // None of these is a keyword, even though several start with one.
    assert_eq!(
        kinds("iffy _tmp x1 int_x Fn returned lets __ a_b_c"),
        vec![
            ident("iffy"),
            ident("_tmp"),
            ident("x1"),
            ident("int_x"),
            ident("Fn"),
            ident("returned"),
            ident("lets"),
            ident("__"),
            ident("a_b_c"),
            T::Eof,
        ]
    );
}

#[test]
fn integer_and_float_literals() {
    assert_eq!(
        kinds("0 42 2147483647 3.14 0.5 10.0 007"),
        vec![
            T::Int(0),
            T::Int(42),
            T::Int(2147483647),
            T::Float(3.14),
            T::Float(0.5),
            T::Float(10.0),
            T::Int(7),
            T::Eof,
        ]
    );
}

#[test]
fn single_character_tokens() {
    assert_eq!(
        kinds("( ) { } [ ] , : ; + - * / %"),
        vec![
            T::LParen,
            T::RParen,
            T::LBrace,
            T::RBrace,
            T::LBracket,
            T::RBracket,
            T::Comma,
            T::Colon,
            T::Semicolon,
            T::Plus,
            T::Minus,
            T::Star,
            T::Slash,
            T::Percent,
            T::Eof,
        ]
    );
}

#[test]
fn two_character_operators_use_maximal_munch() {
    assert_eq!(
        kinds("-> == != <= >= && ||"),
        vec![T::Arrow, T::EqEq, T::NotEq, T::LtEq, T::GtEq, T::AndAnd, T::OrOr, T::Eof]
    );
    // The one-character prefixes are tokens in their own right...
    assert_eq!(
        kinds("= < > ! -"),
        vec![T::Assign, T::Lt, T::Gt, T::Bang, T::Minus, T::Eof]
    );
    // ...and whitespace between the two characters keeps them separate.
    assert_eq!(kinds("- >"), vec![T::Minus, T::Gt, T::Eof]);
    assert_eq!(kinds("= ="), vec![T::Assign, T::Assign, T::Eof]);
    // Three in a row: the first two pair up.
    assert_eq!(kinds("==="), vec![T::EqEq, T::Assign, T::Eof]);
    assert_eq!(
        kinds("!a != b"),
        vec![T::Bang, ident("a"), T::NotEq, ident("b"), T::Eof]
    );
}

#[test]
fn tokens_need_no_whitespace_between_them() {
    assert_eq!(
        kinds("a+b*-3"),
        vec![ident("a"), T::Plus, ident("b"), T::Star, T::Minus, T::Int(3), T::Eof]
    );
    assert_eq!(
        kinds("arr[i]=x;"),
        vec![
            ident("arr"),
            T::LBracket,
            ident("i"),
            T::RBracket,
            T::Assign,
            ident("x"),
            T::Semicolon,
            T::Eof,
        ]
    );
    assert_eq!(kinds("int[]"), vec![T::IntType, T::LBracket, T::RBracket, T::Eof]);
    assert_eq!(
        kinds("f(x)->int"),
        vec![ident("f"), T::LParen, ident("x"), T::RParen, T::Arrow, T::IntType, T::Eof]
    );
}

#[test]
fn number_followed_by_letters_is_two_tokens() {
    // Not the lexer's problem: the parser rejects this sequence.
    assert_eq!(kinds("12abc"), vec![T::Int(12), ident("abc"), T::Eof]);
}

#[test]
fn comments_and_whitespace_are_skipped() {
    let source = "// leading comment\n\
                  let x = 1; // trailing comment\n\
                  \t// indented comment\n\
                  // last line, no newline at the end";
    assert_eq!(
        kinds(source),
        vec![T::Let, ident("x"), T::Assign, T::Int(1), T::Semicolon, T::Eof]
    );

    // A single slash is still division.
    assert_eq!(kinds("a / b"), vec![ident("a"), T::Slash, ident("b"), T::Eof]);
    assert_eq!(kinds("a /b// c"), vec![ident("a"), T::Slash, ident("b"), T::Eof]);

    // Comments may contain any text, including non-ASCII.
    assert_eq!(kinds("// naïve Σ sum → done\nx"), vec![ident("x"), T::Eof]);
}

#[test]
fn spans_track_bytes_lines_and_columns() {
    let source = "let x = 42;\n  foo";
    let tokens = lex(source);
    let spans: Vec<Span> = tokens.iter().map(|token| token.span).collect();
    assert_eq!(
        spans,
        vec![
            Span::new(0, 3, 1, 1),   // let
            Span::new(4, 5, 1, 5),   // x
            Span::new(6, 7, 1, 7),   // =
            Span::new(8, 10, 1, 9),  // 42
            Span::new(10, 11, 1, 11), // ;
            Span::new(14, 17, 2, 3), // foo
            Span::new(17, 17, 2, 6), // Eof: zero-width, at the end
        ]
    );

    // A span's byte range slices the lexeme back out of the source.
    let lexemes: Vec<&str> = tokens.iter().map(|t| &source[t.span.start..t.span.end]).collect();
    assert_eq!(lexemes, vec!["let", "x", "=", "42", ";", "foo", ""]);
}

#[test]
fn spans_of_multi_character_tokens() {
    let tokens = lex("a <= 3.25 -> name_1");
    let spans: Vec<(usize, usize)> = tokens.iter().map(|t| (t.span.start, t.span.end)).collect();
    assert_eq!(spans, vec![(0, 1), (2, 4), (5, 9), (10, 12), (13, 19), (19, 19)]);
}

#[test]
fn lexes_a_whole_function() {
    assert_eq!(
        kinds("fn add(a: int, b: int) -> int {\n    return a + b;\n}\n"),
        vec![
            T::Fn,
            ident("add"),
            T::LParen,
            ident("a"),
            T::Colon,
            T::IntType,
            T::Comma,
            ident("b"),
            T::Colon,
            T::IntType,
            T::RParen,
            T::Arrow,
            T::IntType,
            T::LBrace,
            T::Return,
            ident("a"),
            T::Plus,
            ident("b"),
            T::Semicolon,
            T::RBrace,
            T::Eof,
        ]
    );
}

#[test]
fn lexes_the_example_programs() {
    for (name, source) in common::EXAMPLES {
        let tokens = lex(source);
        assert!(tokens.len() > 50, "{name}: suspiciously few tokens");
        assert_eq!(tokens.last().unwrap().kind, T::Eof, "{name}: must end with Eof");
        let eof_count = tokens.iter().filter(|t| t.kind == T::Eof).count();
        assert_eq!(eof_count, 1, "{name}: exactly one Eof");
        for token in &tokens {
            assert!(token.span.start <= token.span.end && token.span.end <= source.len());
        }
    }
}

#[test]
fn unexpected_character_is_an_error() {
    let err = lex_err("let x = 5 @ 3;");
    assert_eq!(err.kind, ErrorKind::UnexpectedChar);
    assert_eq!(err.stage, Stage::Lex);
    assert_eq!((err.span.line, err.span.col), (1, 11));
    assert_eq!(err.span.start, 10);

    let err = lex_err("let a = 1;\nlet b = #;");
    assert_eq!(err.kind, ErrorKind::UnexpectedChar);
    assert_eq!((err.span.line, err.span.col), (2, 9));

    // Non-ASCII outside a comment.
    assert_eq!(lex_err("let π = 3;").kind, ErrorKind::UnexpectedChar);
}

#[test]
fn lone_ampersand_or_pipe_is_an_error() {
    assert_eq!(lex_err("a & b").kind, ErrorKind::UnexpectedChar);
    assert_eq!(lex_err("a | b").kind, ErrorKind::UnexpectedChar);
    // The doubled forms are fine.
    assert_eq!(
        kinds("a && b || c"),
        vec![ident("a"), T::AndAnd, ident("b"), T::OrOr, ident("c"), T::Eof]
    );
}

#[test]
fn integer_literal_overflow_is_an_error() {
    // i32::MAX is the largest literal; one more does not fit.
    assert_eq!(kinds("2147483647"), vec![T::Int(i32::MAX), T::Eof]);
    assert_eq!(lex_err("2147483648").kind, ErrorKind::InvalidNumber);
    assert_eq!(lex_err("99999999999999999999").kind, ErrorKind::InvalidNumber);

    let err = lex_err("let big = 4000000000;");
    assert_eq!(err.kind, ErrorKind::InvalidNumber);
    assert_eq!(err.span.line, 1);
}

#[test]
fn malformed_float_is_an_error() {
    // A `.` after digits must be followed by at least one digit.
    assert_eq!(lex_err("1.").kind, ErrorKind::InvalidNumber);
    assert_eq!(lex_err("1.x").kind, ErrorKind::InvalidNumber);
    assert_eq!(lex_err("let y = 2. + 1;").kind, ErrorKind::InvalidNumber);
}
