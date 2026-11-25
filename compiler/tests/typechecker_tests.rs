//! Type checker tests: `cargo test --test typechecker_tests`
//!
//! Implementation notes: `src/typeck.rs`.
//!
//! Tests assert on `ErrorKind`, never on message wording.

mod common;

use common::{check_err, check_ok, for_each_expr};
use kuro::ast::{ExprKind, Stmt, Type};
use kuro::error::{ErrorKind, Stage};

/// Wrap statements in `fn main() { .. }` and return the type error they cause.
fn body_err(body: &str) -> ErrorKind {
    check_err(&format!("fn main() {{ {body} }}")).kind
}

/// Wrap statements in `fn main() { .. }` and require them to type-check.
fn body_ok(body: &str) {
    check_ok(&format!("fn main() {{ {body} }}"));
}

// ============================================================================
// Programs that must be accepted
// ============================================================================

#[test]
fn accepts_the_example_programs() {
    for (name, source) in common::EXAMPLES {
        let program = check_ok(source);
        let mut count = 0;
        for_each_expr(&program, &mut |expr| {
            count += 1;
            assert!(expr.ty.is_some(), "{name}: expression at {}:{} has no type", expr.span.line, expr.span.col);
        });
        assert!(count > 20, "{name}: suspiciously few expressions");
    }
}

#[test]
fn annotates_every_expression_with_its_type() {
    let program = check_ok(
        "fn main() -> int {\n\
             let x = 2.5;\n\
             let ok = x < 3.0 && true;\n\
             let a: int[] = alloc(4);\n\
             a[0] = -7;\n\
             return a[0] + len(a) * 3;\n\
         }",
    );
    for_each_expr(&program, &mut |expr| {
        assert!(expr.ty.is_some(), "missing type on {:?}", expr.kind);
    });

    let stmts = &program.functions[0].body.stmts;

    // let x = 2.5;
    let Stmt::Let { init, .. } = &stmts[0] else { panic!("expected let") };
    assert_eq!(init.ty, Some(Type::Float));

    // let ok = x < 3.0 && true;
    let Stmt::Let { init, .. } = &stmts[1] else { panic!("expected let") };
    assert_eq!(init.ty, Some(Type::Bool));
    let ExprKind::Binary { lhs, rhs, .. } = &init.kind else { panic!("expected &&") };
    assert_eq!(lhs.ty, Some(Type::Bool), "`x < 3.0` is a bool");
    assert_eq!(rhs.ty, Some(Type::Bool));
    let ExprKind::Binary { lhs: x, rhs: three, .. } = &lhs.kind else { panic!("expected <") };
    assert_eq!(x.ty, Some(Type::Float), "the operands of `<` keep their own type");
    assert_eq!(three.ty, Some(Type::Float));

    // let a: int[] = alloc(4);
    let Stmt::Let { init, .. } = &stmts[2] else { panic!("expected let") };
    assert_eq!(init.ty, Some(Type::IntArray));

    // a[0] = -7;
    let Stmt::IndexAssign { array, index, value, .. } = &stmts[3] else { panic!("expected index assign") };
    assert_eq!(array.ty, Some(Type::IntArray));
    assert_eq!(index.ty, Some(Type::Int));
    assert_eq!(value.ty, Some(Type::Int));

    // return a[0] + len(a) * 3;
    let Stmt::Return { value: Some(value), .. } = &stmts[4] else { panic!("expected return") };
    assert_eq!(value.ty, Some(Type::Int));
}

#[test]
fn void_calls_are_annotated_as_void() {
    let program = check_ok("fn main() { print_int(1); }");
    let Stmt::Expr(call) = &program.functions[0].body.stmts[0] else { panic!("expected expr stmt") };
    assert_eq!(call.ty, Some(Type::Void));
}

#[test]
fn accepts_all_operators_on_their_operand_types() {
    body_ok("let a = 1 + 2 - 3 * 4 / 5 % 6;");
    body_ok("let x = 1.5 + 2.5 - 3.5 * 4.5 / 5.5;");
    body_ok("let n = -5; let y = -2.5; let b = !true;");
    body_ok("let b = 1 < 2 && 1 <= 2 && 2 > 1 && 2 >= 1 && 1 == 1 && 1 != 2;");
    body_ok("let b = 1.0 < 2.0 || 1.0 <= 2.0 || 2.0 > 1.0 || 2.0 >= 1.0 || 1.0 == 1.0 || 1.0 != 2.0;");
    body_ok("let b = true == false || true != false;");
}

#[test]
fn accepts_builtins() {
    body_ok("print_int(1 + 2); print_float(1.5); let a: int[] = alloc(10); let n: int = len(a);");
    body_ok("let a = alloc(3); a[0] = len(a); print_int(a[a[0] - 3]);");
}

#[test]
fn functions_may_be_used_before_their_definition() {
    check_ok(
        "fn main() -> int { return twice(21); }\n\
         fn twice(x: int) -> int { return x + x; }",
    );
    // Mutual recursion needs both signatures before either body.
    check_ok(
        "fn is_even(n: int) -> bool { if n == 0 { return true; } return is_odd(n - 1); }\n\
         fn is_odd(n: int) -> bool { if n == 0 { return false; } return is_even(n - 1); }\n\
         fn main() { }",
    );
}

#[test]
fn main_may_have_any_return_type() {
    check_ok("fn main() { }");
    check_ok("fn main() -> int { return 0; }");
    check_ok("fn main() -> float { return 0.5; }");
    check_ok("fn main() -> bool { return true; }");
}

#[test]
fn inner_blocks_may_shadow_outer_variables() {
    body_ok("let x = 1; if true { let x = 2.5; print_float(x); } print_int(x);");
    body_ok("let x = 1; while x < 3 { let x = true; } print_int(x);");
    // The initialiser of the inner `x` still sees the outer `x`.
    body_ok("let x = 1; if true { let x = x + 1; print_int(x); }");
    // Sibling blocks may reuse a name, even with different types.
    body_ok("if true { let v = 1; } if true { let v = 1.5; }");
    // A local may shadow a parameter (the body is a block inside the parameter scope).
    check_ok("fn f(n: int) -> float { let n = 1.5; return n; } fn main() { }");
}

#[test]
fn parameters_are_ordinary_mutable_variables() {
    check_ok("fn countdown(n: int) { while n > 0 { n = n - 1; } } fn main() { countdown(3); }");
}

#[test]
fn value_returning_function_needs_no_trailing_return_when_all_paths_return() {
    check_ok("fn pick(c: bool) -> int { if c { return 1; } else { return 2; } } fn main() { }");
    check_ok(
        "fn sign(x: int) -> int { if x < 0 { return -1; } else if x == 0 { return 0; } else { return 1; } }\n\
         fn main() { }",
    );
    // Code after a return is allowed (and simply unreachable).
    check_ok("fn f() -> int { return 1; print_int(2); } fn main() { }");
}

// ============================================================================
// Names
// ============================================================================

#[test]
fn undefined_variable_is_an_error() {
    let err = check_err("fn main() { print_int(missing); }");
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
    assert_eq!(err.stage, Stage::TypeCheck);

    assert_eq!(body_err("x = 1;"), ErrorKind::UndefinedVariable);
    assert_eq!(body_err("let y = y + 1;"), ErrorKind::UndefinedVariable);
    assert_eq!(body_err("a[0] = 1;"), ErrorKind::UndefinedVariable);
}

#[test]
fn variables_go_out_of_scope_at_the_end_of_their_block() {
    assert_eq!(body_err("if true { let inner = 1; } print_int(inner);"), ErrorKind::UndefinedVariable);
    assert_eq!(body_err("while false { let inner = 1; } inner = 2;"), ErrorKind::UndefinedVariable);
    // One function cannot see another function's locals or parameters.
    assert_eq!(
        check_err("fn f(secret: int) { } fn main() { print_int(secret); }").kind,
        ErrorKind::UndefinedVariable
    );
}

#[test]
fn undefined_function_is_an_error() {
    assert_eq!(body_err("missing(1);"), ErrorKind::UndefinedFunction);
    assert_eq!(body_err("let x = 1 + nope();"), ErrorKind::UndefinedFunction);
    // A variable is not callable.
    assert_eq!(body_err("let f = 1; f(2);"), ErrorKind::UndefinedFunction);
}

#[test]
fn duplicate_definitions_are_errors() {
    // Two functions with the same name.
    assert_eq!(
        check_err("fn f() { } fn f() { } fn main() { }").kind,
        ErrorKind::DuplicateDefinition
    );
    // A function named like a builtin.
    assert_eq!(
        check_err("fn print_int(x: int) { } fn main() { }").kind,
        ErrorKind::DuplicateDefinition
    );
    assert_eq!(
        check_err("fn len(a: int[]) -> int { return 0; } fn main() { }").kind,
        ErrorKind::DuplicateDefinition
    );
    // Two parameters with the same name.
    assert_eq!(
        check_err("fn f(a: int, a: int) { } fn main() { }").kind,
        ErrorKind::DuplicateDefinition
    );
    // Two `let`s of the same name in the same block.
    assert_eq!(body_err("let x = 1; let x = 2;"), ErrorKind::DuplicateDefinition);
    assert_eq!(body_err("if true { let x = 1; let x = 2.5; }"), ErrorKind::DuplicateDefinition);
}

#[test]
fn missing_or_invalid_main_is_an_error() {
    assert_eq!(check_err("").kind, ErrorKind::MissingMain);
    assert_eq!(check_err("fn helper() { }").kind, ErrorKind::MissingMain);
    assert_eq!(check_err("fn Main() { }").kind, ErrorKind::MissingMain);

    assert_eq!(check_err("fn main(argc: int) { }").kind, ErrorKind::InvalidMain);
    assert_eq!(check_err("fn main(a: int, b: int) -> int { return a; }").kind, ErrorKind::InvalidMain);
}

// ============================================================================
// Type mismatches
// ============================================================================

#[test]
fn let_annotation_must_match_the_initialiser() {
    assert_eq!(body_err("let x: int = 1.5;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x: float = 1;"), ErrorKind::TypeMismatch, "no implicit int → float");
    assert_eq!(body_err("let x: bool = 0;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x: int = true;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a: int[] = 5;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let n: int = alloc(5);"), ErrorKind::TypeMismatch);
}

#[test]
fn assignment_must_keep_the_variable_type() {
    assert_eq!(body_err("let x = 1; x = 2.5;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x = 1.5; x = 2;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let ok = true; ok = 1;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2); a = 0;"), ErrorKind::TypeMismatch);
}

#[test]
fn arithmetic_operands_must_have_the_same_numeric_type() {
    assert_eq!(body_err("let x = 1 + 2.0;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x = 2.0 * 3;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x = true + 1;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x = true - false;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(1); let x = a + 1;"), ErrorKind::TypeMismatch);
    // `%` exists for ints only.
    assert_eq!(body_err("let x = 5.0 % 2.0;"), ErrorKind::TypeMismatch);
}

#[test]
fn comparison_operands_must_match() {
    assert_eq!(body_err("let b = 1 < 2.0;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let b = 1 == true;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let b = 1.0 != 1;"), ErrorKind::TypeMismatch);
    // Bools can be compared for equality but not ordered.
    assert_eq!(body_err("let b = true < false;"), ErrorKind::TypeMismatch);
    // `a < b < c` parses as `(a < b) < c`: bool < int.
    assert_eq!(body_err("let b = 1 < 2 < 3;"), ErrorKind::TypeMismatch);
    // Arrays are not comparable.
    assert_eq!(body_err("let a = alloc(1); let b = a == a;"), ErrorKind::TypeMismatch);
}

#[test]
fn logical_operators_require_bools() {
    assert_eq!(body_err("let b = 1 && 2;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let b = true || 0;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let b = 1.5 && true;"), ErrorKind::TypeMismatch);
}

#[test]
fn unary_operators_check_their_operand() {
    assert_eq!(body_err("let x = !5;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x = !1.5;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x = -true;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(1); let x = -a;"), ErrorKind::TypeMismatch);
}

#[test]
fn conditions_must_be_bool() {
    assert_eq!(body_err("if 1 { }"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("if 0.0 { }"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let n = 3; if n { }"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("while 1 { }"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let n = 3; while n - 1 { }"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("if true { } else if 2 { }"), ErrorKind::TypeMismatch);
}

#[test]
fn void_calls_have_no_value() {
    assert_eq!(body_err("let x = print_int(1);"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let x = 1 + print_int(1);"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("print_int(print_int(1));"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("if print_int(1) { }"), ErrorKind::TypeMismatch);
}

// ============================================================================
// Arrays
// ============================================================================

#[test]
fn indexing_a_non_array_is_an_error() {
    let err = check_err("fn main() { let x = 5; print_int(x[0]); }");
    assert_eq!(err.kind, ErrorKind::NotAnArray);
    assert_eq!(err.stage, Stage::TypeCheck);

    assert_eq!(body_err("let x = 1.5; let y = x[0];"), ErrorKind::NotAnArray);
    assert_eq!(body_err("let flag = true; let y = flag[1];"), ErrorKind::NotAnArray);
    // Writing through a non-array.
    assert_eq!(body_err("let x = 5; x[0] = 1;"), ErrorKind::NotAnArray);
    // An element of an int[] is an int, so it cannot be indexed again.
    assert_eq!(body_err("let a = alloc(2); let y = a[0][1];"), ErrorKind::NotAnArray);
    // The result of a non-array call.
    assert_eq!(body_err("let a = alloc(2); let y = len(a)[0];"), ErrorKind::NotAnArray);
}

#[test]
fn array_index_must_be_an_int() {
    assert_eq!(body_err("let a = alloc(2); let y = a[true];"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2); let y = a[1.0];"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2); a[0.5] = 1;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2); a[a] = 1;"), ErrorKind::TypeMismatch);
}

#[test]
fn array_elements_are_ints() {
    assert_eq!(body_err("let a = alloc(2); a[0] = 1.5;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2); a[0] = true;"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2); let x: float = a[0];"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2); if a[0] { }"), ErrorKind::TypeMismatch);
}

// ============================================================================
// Calls
// ============================================================================

#[test]
fn wrong_number_of_arguments_is_an_error() {
    let source = |call: &str| format!("fn add(a: int, b: int) -> int {{ return a + b; }} fn main() {{ {call} }}");
    assert_eq!(check_err(&source("add(1);")).kind, ErrorKind::ArgCountMismatch);
    assert_eq!(check_err(&source("add(1, 2, 3);")).kind, ErrorKind::ArgCountMismatch);
    assert_eq!(check_err(&source("add();")).kind, ErrorKind::ArgCountMismatch);
    check_ok(&source("add(1, 2);"));

    // Builtins are checked the same way.
    assert_eq!(body_err("print_int();"), ErrorKind::ArgCountMismatch);
    assert_eq!(body_err("print_int(1, 2);"), ErrorKind::ArgCountMismatch);
    assert_eq!(body_err("let a = alloc();"), ErrorKind::ArgCountMismatch);
}

#[test]
fn argument_types_must_match_parameters() {
    let source = |call: &str| format!("fn scale(x: float, by: int) -> float {{ return x; }} fn main() {{ {call} }}");
    assert_eq!(check_err(&source("scale(1, 2);")).kind, ErrorKind::TypeMismatch);
    assert_eq!(check_err(&source("scale(1.0, 2.0);")).kind, ErrorKind::TypeMismatch);
    assert_eq!(check_err(&source("scale(1.0, true);")).kind, ErrorKind::TypeMismatch);
    check_ok(&source("scale(1.0, 2);"));

    assert_eq!(body_err("print_int(1.5);"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("print_float(1);"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("print_int(true);"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let a = alloc(2.0);"), ErrorKind::TypeMismatch);
    assert_eq!(body_err("let n = len(5);"), ErrorKind::TypeMismatch);
}

#[test]
fn call_result_type_is_checked_at_the_use_site() {
    let source = |body: &str| format!("fn half(x: float) -> float {{ return x / 2.0; }} fn main() {{ {body} }}");
    assert_eq!(check_err(&source("let n: int = half(3.0);")).kind, ErrorKind::TypeMismatch);
    assert_eq!(check_err(&source("let n = 1 + half(3.0);")).kind, ErrorKind::TypeMismatch);
    check_ok(&source("let x = 1.0 + half(3.0);"));
}

// ============================================================================
// Returns
// ============================================================================

#[test]
fn return_value_must_match_the_declared_type() {
    let err = check_err("fn f() -> int { return true; } fn main() { }");
    assert_eq!(err.kind, ErrorKind::ReturnTypeMismatch);
    assert_eq!(err.stage, Stage::TypeCheck);

    assert_eq!(
        check_err("fn f() -> int { return 1.5; } fn main() { }").kind,
        ErrorKind::ReturnTypeMismatch
    );
    assert_eq!(
        check_err("fn f() -> float { return 1; } fn main() { }").kind,
        ErrorKind::ReturnTypeMismatch
    );
    assert_eq!(
        check_err("fn f() -> bool { return 0; } fn main() { }").kind,
        ErrorKind::ReturnTypeMismatch
    );
    assert_eq!(
        check_err("fn f(a: int[]) -> int { return a; } fn main() { }").kind,
        ErrorKind::ReturnTypeMismatch
    );
    // A mismatch nested inside control flow is still found.
    assert_eq!(
        check_err("fn f(c: bool) -> int { while c { if c { return 2.5; } } return 1; } fn main() { }").kind,
        ErrorKind::ReturnTypeMismatch
    );
}

#[test]
fn return_with_and_without_value_must_match_the_signature() {
    // A value from a function that returns nothing.
    assert_eq!(body_err("return 1;"), ErrorKind::ReturnTypeMismatch);
    // No value from a function that returns something.
    assert_eq!(
        check_err("fn f() -> int { return; } fn main() { }").kind,
        ErrorKind::ReturnTypeMismatch
    );
    // Bare `return;` in a void function is fine.
    body_ok("return;");
    body_ok("if true { return; } print_int(1);");
}

#[test]
fn value_returning_function_must_return_on_every_path() {
    let missing = |source: &str| assert_eq!(check_err(source).kind, ErrorKind::MissingReturn, "{source}");

    missing("fn f() -> int { } fn main() { }");
    missing("fn f() -> int { let x = 1; } fn main() { }");
    // `if` without `else` may fall through.
    missing("fn f(c: bool) -> int { if c { return 1; } } fn main() { }");
    // Only one branch returns.
    missing("fn f(c: bool) -> int { if c { return 1; } else { print_int(0); } } fn main() { }");
    missing("fn f(c: bool) -> int { if c { print_int(0); } else { return 1; } } fn main() { }");
    // A loop body may run zero times; the analysis is purely syntactic.
    missing("fn f(c: bool) -> int { while c { return 1; } } fn main() { }");
    missing("fn f() -> int { while true { return 1; } } fn main() { }");
    // `main` is not special.
    missing("fn main() -> int { }");
}

// ============================================================================
// Error locations
// ============================================================================

#[test]
fn errors_point_at_the_offending_code() {
    let source = "fn main() {\n    let count = 3;\n    let ratio = count + 0.5;\n}\n";
    let err = check_err(source);
    assert_eq!(err.kind, ErrorKind::TypeMismatch);
    assert_eq!(err.span.line, 3);
    // The span must lie on line 3 and include the operator.
    let text = &source[err.span.start..err.span.end];
    assert!(text.contains('+'), "span should cover the bad expression, got {text:?}");
    assert!(!text.contains('\n'), "span should not spill over lines, got {text:?}");

    let source = "fn main() {\n    let x = 5;\n\n    x[0] = 1;\n}\n";
    let err = check_err(source);
    assert_eq!(err.kind, ErrorKind::NotAnArray);
    assert_eq!(err.span.line, 4);

    let source = "fn main() {\n    print_int(1);\n    print_int(oops);\n}\n";
    let err = check_err(source);
    assert_eq!(err.kind, ErrorKind::UndefinedVariable);
    assert_eq!((err.span.line, err.span.col), (3, 15));
    assert_eq!(&source[err.span.start..err.span.end], "oops");
}
