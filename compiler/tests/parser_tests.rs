//! Parser tests: `cargo test --test parser_tests`
//!
//! Implementation notes: `src/parser.rs`.
//!
//! The expression tests compare S-expressions (see `common::tree`), so a
//! failure shows you the tree your parser built next to the expected one:
//!
//! ```text
//! left:  "(* (+ 1 2) 3)"      ← what you produced
//! right: "(+ 1 (* 2 3))"      ← what precedence requires
//! ```
//!
//! The expression tests only need `parse_expression`, so you can get them
//! green before writing any statement parsing.

mod common;

use common::{parse_body, parse_err, parse_expr, parse_expr_err, parse_program, sexpr, tree};
use kuro::ast::{ExprKind, Stmt, Type};
use kuro::error::{ErrorKind, Stage};

// ============================================================================
// Expressions: precedence and associativity
// ============================================================================

#[test]
fn literals_and_variables() {
    assert_eq!(tree("42"), "42");
    assert_eq!(tree("3.5"), "3.5");
    assert_eq!(tree("2.0"), "2.0");
    assert_eq!(tree("true"), "true");
    assert_eq!(tree("false"), "false");
    assert_eq!(tree("total"), "total");

    assert_eq!(parse_expr("42").kind, ExprKind::IntLit(42));
    assert_eq!(parse_expr("2.5").kind, ExprKind::FloatLit(2.5));
    assert_eq!(parse_expr("true").kind, ExprKind::BoolLit(true));
    assert_eq!(parse_expr("x").kind, ExprKind::Var("x".to_string()));
}

#[test]
fn parser_leaves_type_annotations_empty() {
    // `ty` is the type checker's job.
    let expr = parse_expr("1 + 2");
    assert_eq!(expr.ty, None);
    if let ExprKind::Binary { lhs, rhs, .. } = &expr.kind {
        assert_eq!(lhs.ty, None);
        assert_eq!(rhs.ty, None);
    } else {
        panic!("expected a binary expression, got {:?}", expr.kind);
    }
}

#[test]
fn multiplication_binds_tighter_than_addition() {
    assert_eq!(tree("1 + 2 * 3"), "(+ 1 (* 2 3))");
    assert_eq!(tree("1 * 2 + 3"), "(+ (* 1 2) 3)");
    assert_eq!(tree("1 + 2 % 3"), "(+ 1 (% 2 3))");
    assert_eq!(tree("a / b - c * d"), "(- (/ a b) (* c d))");
    assert_eq!(tree("1 + 2 * 3 + 4"), "(+ (+ 1 (* 2 3)) 4)");
}

#[test]
fn binary_operators_are_left_associative() {
    assert_eq!(tree("1 - 2 - 3"), "(- (- 1 2) 3)");
    assert_eq!(tree("8 / 4 / 2"), "(/ (/ 8 4) 2)");
    assert_eq!(tree("1 - 2 + 3"), "(+ (- 1 2) 3)");
    assert_eq!(tree("10 / 2 % 3"), "(% (/ 10 2) 3)");
    assert_eq!(tree("a && b && c"), "(&& (&& a b) c)");
    assert_eq!(tree("a || b || c"), "(|| (|| a b) c)");
}

#[test]
fn parentheses_override_precedence() {
    assert_eq!(tree("(1 + 2) * 3"), "(* (+ 1 2) 3)");
    assert_eq!(tree("2 * (3 + 4) * 5"), "(* (* 2 (+ 3 4)) 5)");
    assert_eq!(tree("1 - (2 - 3)"), "(- 1 (- 2 3))");
    // Parentheses add no node of their own.
    assert_eq!(tree("((1))"), "1");
    assert_eq!(parse_expr("(7)").kind, ExprKind::IntLit(7));
}

#[test]
fn unary_operators_bind_tighter_than_binary() {
    assert_eq!(tree("-a * b"), "(* (neg a) b)");
    assert_eq!(tree("-a + b"), "(+ (neg a) b)");
    assert_eq!(tree("a - -b"), "(- a (neg b))");
    assert_eq!(tree("- -a"), "(neg (neg a))");
    assert_eq!(tree("!a && b"), "(&& (not a) b)");
    assert_eq!(tree("!!ok"), "(not (not ok))");
    assert_eq!(tree("-(a + b)"), "(neg (+ a b))");
    assert_eq!(tree("!(a < b)"), "(not (< a b))");
    assert_eq!(tree("-1"), "(neg 1)");
}

#[test]
fn comparison_is_looser_than_arithmetic() {
    assert_eq!(tree("a + 1 < b * 2"), "(< (+ a 1) (* b 2))");
    assert_eq!(tree("x % 2 == 0"), "(== (% x 2) 0)");
    assert_eq!(tree("n - 1 >= 0"), "(>= (- n 1) 0)");
    // Equality is looser than ordering.
    assert_eq!(tree("a < b == c > d"), "(== (< a b) (> c d))");
    assert_eq!(tree("a <= b != c >= d"), "(!= (<= a b) (>= c d))");
}

#[test]
fn logical_operators_have_the_lowest_precedence() {
    assert_eq!(tree("a || b && c"), "(|| a (&& b c))");
    assert_eq!(tree("a && b || c"), "(|| (&& a b) c)");
    assert_eq!(tree("a == b && c != d"), "(&& (== a b) (!= c d))");
    assert_eq!(tree("a < 1 || b > 2 && c"), "(|| (< a 1) (&& (> b 2) c))");
    assert_eq!(tree("!a || -b < c + 1"), "(|| (not a) (< (neg b) (+ c 1)))");
}

#[test]
fn postfix_index_binds_tightest() {
    assert_eq!(tree("a[0]"), "(index a 0)");
    assert_eq!(tree("-a[0]"), "(neg (index a 0))");
    assert_eq!(tree("a[i + 1] * 2"), "(* (index a (+ i 1)) 2)");
    assert_eq!(tree("a[j] > a[j + 1]"), "(> (index a j) (index a (+ j 1)))");
    assert_eq!(tree("a[b[0]]"), "(index a (index b 0))");
    // Syntactically fine; the type checker rejects indexing an int.
    assert_eq!(tree("a[i][j]"), "(index (index a i) j)");
}

#[test]
fn calls_take_full_expressions_as_arguments() {
    assert_eq!(tree("f()"), "(call f)");
    assert_eq!(tree("f(x)"), "(call f x)");
    assert_eq!(tree("f(1, 2 + 3)"), "(call f 1 (+ 2 3))");
    assert_eq!(tree("f(a || b, -c)"), "(call f (|| a b) (neg c))");
    assert_eq!(
        tree("fib(n - 1) + fib(n - 2)"),
        "(+ (call fib (- n 1)) (call fib (- n 2)))"
    );
    assert_eq!(tree("f(g(x))[0]"), "(index (call f (call g x)) 0)");
    assert_eq!(tree("-f(x) * 2"), "(* (neg (call f x)) 2)");
}

#[test]
fn expression_spans_cover_their_operands() {
    //            0123456789
    let source = "1 + 2 * 3";
    let expr = parse_expr(source);
    assert_eq!((expr.span.start, expr.span.end), (0, 9));
    assert_eq!((expr.span.line, expr.span.col), (1, 1));
    match &expr.kind {
        ExprKind::Binary { lhs, rhs, .. } => {
            assert_eq!((lhs.span.start, lhs.span.end), (0, 1));
            assert_eq!((rhs.span.start, rhs.span.end), (4, 9));
            assert_eq!(&source[rhs.span.start..rhs.span.end], "2 * 3");
        }
        other => panic!("expected a binary expression, got {other:?}"),
    }

    let call = parse_expr("foo(1, 2)");
    assert_eq!((call.span.start, call.span.end), (0, 9), "a call spans name through `)`");

    let index = parse_expr("arr[10]");
    assert_eq!((index.span.start, index.span.end), (0, 7), "an index spans array through `]`");

    let neg = parse_expr("-value");
    assert_eq!((neg.span.start, neg.span.end), (0, 6), "a unary spans operator through operand");
}

// ============================================================================
// Declarations and statements
// ============================================================================

#[test]
fn empty_program_has_no_functions() {
    assert_eq!(parse_program("").functions.len(), 0);
    assert_eq!(parse_program("// only a comment\n").functions.len(), 0);
}

#[test]
fn parses_function_signatures() {
    let program = parse_program(
        "fn add(a: int, b: float) -> bool { return true; }\n\
         fn nothing() { }\n\
         fn one(flag: bool) -> float { return 1.0; }",
    );
    assert_eq!(program.functions.len(), 3);

    let add = &program.functions[0];
    assert_eq!(add.name, "add");
    let params: Vec<(&str, Type)> = add.params.iter().map(|p| (p.name.as_str(), p.ty)).collect();
    assert_eq!(params, vec![("a", Type::Int), ("b", Type::Float)]);
    assert_eq!(add.return_type, Type::Bool);
    assert_eq!(add.body.stmts.len(), 1);

    let nothing = &program.functions[1];
    assert_eq!(nothing.name, "nothing");
    assert!(nothing.params.is_empty());
    assert_eq!(nothing.return_type, Type::Void, "no `->` clause means Void");
    assert!(nothing.body.stmts.is_empty());

    let one = &program.functions[2];
    assert_eq!(one.params[0].ty, Type::Bool);
    assert_eq!(one.return_type, Type::Float);
}

#[test]
fn parses_array_types() {
    let program = parse_program("fn first(a: int[], n: int) -> int[] { return a; }");
    let f = &program.functions[0];
    assert_eq!(f.params[0].ty, Type::IntArray);
    assert_eq!(f.params[1].ty, Type::Int);
    assert_eq!(f.return_type, Type::IntArray);
}

#[test]
fn function_span_runs_from_fn_to_closing_brace() {
    let source = "  fn f() { }\nfn g() {\n}\n";
    let program = parse_program(source);
    let f = &program.functions[0];
    assert_eq!(&source[f.span.start..f.span.end], "fn f() { }");
    let g = &program.functions[1];
    assert_eq!(&source[g.span.start..g.span.end], "fn g() {\n}");
    assert_eq!((g.span.line, g.span.col), (2, 1));
}

#[test]
fn parses_let_with_and_without_annotation() {
    let stmts = parse_body("let x = 1; let y: float = 2.5; let a: int[] = alloc(3); let ok: bool = x < 2;");
    assert_eq!(stmts.len(), 4);

    let expect_let = |stmt: &Stmt, want_name: &str, want_ty: Option<Type>, want_init: &str| match stmt {
        Stmt::Let { name, declared_ty, init, .. } => {
            assert_eq!(name, want_name);
            assert_eq!(*declared_ty, want_ty);
            assert_eq!(sexpr(init), want_init);
        }
        other => panic!("expected a let statement, got {other:?}"),
    };
    expect_let(&stmts[0], "x", None, "1");
    expect_let(&stmts[1], "y", Some(Type::Float), "2.5");
    expect_let(&stmts[2], "a", Some(Type::IntArray), "(call alloc 3)");
    expect_let(&stmts[3], "ok", Some(Type::Bool), "(< x 2)");
}

#[test]
fn parses_assignment_index_assignment_and_expression_statements() {
    let stmts = parse_body("x = x + 1; a[i + 1] = x * 2; print_int(x); a[0];");
    assert_eq!(stmts.len(), 4);

    match &stmts[0] {
        Stmt::Assign { name, value, .. } => {
            assert_eq!(name, "x");
            assert_eq!(sexpr(value), "(+ x 1)");
        }
        other => panic!("expected an assignment, got {other:?}"),
    }
    match &stmts[1] {
        Stmt::IndexAssign { array, index, value, .. } => {
            assert_eq!(sexpr(array), "a");
            assert_eq!(sexpr(index), "(+ i 1)");
            assert_eq!(sexpr(value), "(* x 2)");
        }
        other => panic!("expected an index assignment, got {other:?}"),
    }
    match &stmts[2] {
        Stmt::Expr(expr) => assert_eq!(sexpr(expr), "(call print_int x)"),
        other => panic!("expected an expression statement, got {other:?}"),
    }
    match &stmts[3] {
        // An index expression NOT followed by `=` is just an expression.
        Stmt::Expr(expr) => assert_eq!(sexpr(expr), "(index a 0)"),
        other => panic!("expected an expression statement, got {other:?}"),
    }
}

#[test]
fn parses_if_without_else() {
    let stmts = parse_body("if x < 3 { y = 1; }");
    match &stmts[0] {
        Stmt::If { cond, then_block, else_block, .. } => {
            assert_eq!(sexpr(cond), "(< x 3)");
            assert_eq!(then_block.stmts.len(), 1);
            assert!(else_block.is_none());
        }
        other => panic!("expected an if statement, got {other:?}"),
    }
}

#[test]
fn parses_if_else_chains() {
    let stmts = parse_body("if a { x = 1; } else if b { x = 2; x = 3; } else { }");
    assert_eq!(stmts.len(), 1);

    let Stmt::If { cond, then_block, else_block, .. } = &stmts[0] else {
        panic!("expected an if statement, got {:?}", stmts[0]);
    };
    assert_eq!(sexpr(cond), "a");
    assert_eq!(then_block.stmts.len(), 1);

    // `else if` is stored as an else block holding exactly one nested `if`.
    let else_block = else_block.as_ref().expect("the first `if` has an else");
    assert_eq!(else_block.stmts.len(), 1);
    let Stmt::If { cond, then_block, else_block, .. } = &else_block.stmts[0] else {
        panic!("expected a nested if, got {:?}", else_block.stmts[0]);
    };
    assert_eq!(sexpr(cond), "b");
    assert_eq!(then_block.stmts.len(), 2);
    let final_else = else_block.as_ref().expect("the nested `if` has the final else");
    assert!(final_else.stmts.is_empty());
}

#[test]
fn parses_while_and_return() {
    let stmts = parse_body("while i < 10 { i = i + 1; } return; return i * 2;");
    assert_eq!(stmts.len(), 3);

    match &stmts[0] {
        Stmt::While { cond, body, .. } => {
            assert_eq!(sexpr(cond), "(< i 10)");
            assert_eq!(body.stmts.len(), 1);
        }
        other => panic!("expected a while statement, got {other:?}"),
    }
    match &stmts[1] {
        Stmt::Return { value, .. } => assert!(value.is_none()),
        other => panic!("expected a return statement, got {other:?}"),
    }
    match &stmts[2] {
        Stmt::Return { value, .. } => assert_eq!(sexpr(value.as_ref().unwrap()), "(* i 2)"),
        other => panic!("expected a return statement, got {other:?}"),
    }
}

#[test]
fn conditions_may_be_parenthesised() {
    let stmts = parse_body("if (x == 1) { } while (true) { }");
    match &stmts[0] {
        Stmt::If { cond, .. } => assert_eq!(sexpr(cond), "(== x 1)"),
        other => panic!("expected an if statement, got {other:?}"),
    }
    assert!(matches!(&stmts[1], Stmt::While { .. }));
}

#[test]
fn parses_the_example_programs() {
    let names = |source: &str| -> Vec<String> {
        parse_program(source).functions.into_iter().map(|f| f.name).collect()
    };
    assert_eq!(names(common::FIBONACCI), ["fib", "golden_ratio", "main"]);
    assert_eq!(names(common::ARRAY_SUM), ["sum", "main"]);
    assert_eq!(names(common::BUBBLE_SORT), ["bubble_sort", "main"]);
}

// ============================================================================
// Syntax errors
// ============================================================================

#[test]
fn missing_semicolon_is_an_error() {
    //                       1111111111222
    //             01234567890123456789012
    let err = parse_err("fn main() { let x = 1 }");
    assert_eq!(err.kind, ErrorKind::UnexpectedToken);
    assert_eq!(err.stage, Stage::Parse);
    // The error points at the token found where `;` was expected: the `}`.
    assert_eq!(err.span.start, 22);
    assert_eq!((err.span.line, err.span.col), (1, 23));

    assert_eq!(parse_err("fn main() { f(x) }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn main() { return 1 }").kind, ErrorKind::UnexpectedToken);
}

#[test]
fn unclosed_delimiters_are_errors() {
    assert_eq!(parse_expr_err("(1 + 2").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("f(1, 2").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("a[1").kind, ErrorKind::UnexpectedToken);
    // An unterminated block must produce an error, not loop forever.
    assert_eq!(parse_err("fn main() { let x = 1;").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn main() { if x { ").kind, ErrorKind::UnexpectedToken);
}

#[test]
fn missing_expression_is_an_error() {
    assert_eq!(parse_err("fn main() { let x = ; }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("1 +").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("* 2").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("f(1,)").kind, ErrorKind::UnexpectedToken);
}

#[test]
fn trailing_tokens_after_an_expression_are_an_error() {
    assert_eq!(parse_expr_err("1 2").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("a b").kind, ErrorKind::UnexpectedToken);
    // Only a bare name can be called.
    assert_eq!(parse_expr_err("(f)(1)").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_expr_err("f(1)(2)").kind, ErrorKind::UnexpectedToken);
}

#[test]
fn invalid_assignment_target_is_an_error() {
    let err = parse_err("fn main() { 1 + 2 = 3; }");
    assert_eq!(err.kind, ErrorKind::InvalidAssignTarget);
    assert_eq!(err.stage, Stage::Parse);

    assert_eq!(parse_err("fn main() { f(x) = 3; }").kind, ErrorKind::InvalidAssignTarget);
    assert_eq!(parse_err("fn main() { -x = 3; }").kind, ErrorKind::InvalidAssignTarget);
    assert_eq!(parse_err("fn main() { 5 = x; }").kind, ErrorKind::InvalidAssignTarget);
}

#[test]
fn malformed_declarations_are_errors() {
    // Only functions are allowed at the top level.
    assert_eq!(parse_err("let x = 1;").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn main() { } x = 1;").kind, ErrorKind::UnexpectedToken);
    // Missing pieces of a function header.
    assert_eq!(parse_err("fn (x: int) { }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn f(x) { }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn f(x: int,) { }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn f() -> { }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn f() return 1;").kind, ErrorKind::UnexpectedToken);
    // `let` needs an initialiser.
    assert_eq!(parse_err("fn main() { let x; }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn main() { let x: int; }").kind, ErrorKind::UnexpectedToken);
}

#[test]
fn only_int_arrays_exist() {
    assert_eq!(parse_err("fn f(a: float[]) { }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn f(a: bool[]) { }").kind, ErrorKind::UnexpectedToken);
    assert_eq!(parse_err("fn f(a: int[) { }").kind, ErrorKind::UnexpectedToken);
}

#[test]
fn errors_report_the_line_of_the_offending_token() {
    let source = "fn main() {\n    let x = 1;\n    let y = * 2;\n}\n";
    let err = parse_err(source);
    assert_eq!(err.kind, ErrorKind::UnexpectedToken);
    assert_eq!((err.span.line, err.span.col), (3, 13));
    assert_eq!(&source[err.span.start..err.span.end], "*");
}
