//! End-to-end tests: compile kuro with your compiler, execute the
//! resulting `.wasm` with `wasmtime`, and check what it returned, printed and
//! wrote to linear memory.
//!
//! `cargo test --test e2e_tests`
//!
//! These need all four stages. They are grouped by feature: constants, arithmetic, locals, control flow,
//! calls, floats, memory.
//!
//! Every user function is exported under its own name, so many tests call a
//! function directly with arguments (`vm.call_i32("fib", &[10])`) instead of
//! going through `main`.

mod common;
#[path = "common/vm.rs"]
mod vm;

use vm::{Printed, Vm, run, run_int};
use wasmtime::Trap;

// ============================================================================
// Integers
// ============================================================================

#[test]
fn main_returns_a_constant() {
    assert_eq!(run_int("fn main() -> int { return 42; }"), 42);
    assert_eq!(run_int("fn main() -> int { return 0; }"), 0);
}

#[test]
fn constants_survive_signed_leb128_encoding() {
    // 64 and 8192 are the classic traps: their top payload bit is the sign
    // bit of the final LEB128 byte, so they need one more byte than you
    // would guess. A wrong encoder prints -64 and -8192 here.
    let vm = run(
        "fn main() {\n\
             print_int(0); print_int(1); print_int(63); print_int(64); print_int(127);\n\
             print_int(128); print_int(8191); print_int(8192); print_int(16384);\n\
             print_int(1000000); print_int(2147483647);\n\
         }",
    );
    assert_eq!(
        vm.printed_ints(),
        [0, 1, 63, 64, 127, 128, 8191, 8192, 16384, 1_000_000, i32::MAX]
    );
}

#[test]
fn integer_arithmetic_follows_precedence() {
    assert_eq!(run_int("fn main() -> int { return 2 + 3 * 4 - 10 / 2 % 3; }"), 12);
    assert_eq!(run_int("fn main() -> int { return (2 + 3) * 4; }"), 20);
    assert_eq!(run_int("fn main() -> int { return 100 - 30 - 20; }"), 50);
    assert_eq!(run_int("fn main() -> int { return 2 * 3 + 4 * 5; }"), 26);
}

#[test]
fn division_and_remainder_truncate_toward_zero() {
    assert_eq!(run_int("fn main() -> int { return 7 / 2; }"), 3);
    assert_eq!(run_int("fn main() -> int { return -7 / 2; }"), -3);
    assert_eq!(run_int("fn main() -> int { return 7 % 3; }"), 1);
    assert_eq!(run_int("fn main() -> int { return -7 % 3; }"), -1);
    assert_eq!(run_int("fn main() -> int { return 7 % -3; }"), 1);
}

#[test]
fn unary_minus_negates() {
    assert_eq!(run_int("fn main() -> int { return -5; }"), -5);
    assert_eq!(run_int("fn main() -> int { return - -5; }"), 5);
    assert_eq!(run_int("fn main() -> int { let x = 9; return -x + 1; }"), -8);
    assert_eq!(run_int("fn main() -> int { return 3 - -4; }"), 7);
    // The only way to write i32::MIN.
    assert_eq!(run_int("fn main() -> int { return -2147483647 - 1; }"), i32::MIN);
}

#[test]
fn integer_overflow_wraps() {
    assert_eq!(run_int("fn main() -> int { return 2147483647 + 1; }"), i32::MIN);
    assert_eq!(run_int("fn main() -> int { return 65536 * 65536; }"), 0);
    assert_eq!(run_int("fn main() -> int { return 0 - 2147483647 - 2; }"), i32::MAX);
}

#[test]
fn division_by_zero_traps() {
    let mut vm = Vm::from_source(
        "fn div(a: int, b: int) -> int { return a / b; }\n\
         fn rem(a: int, b: int) -> int { return a % b; }\n\
         fn main() { }",
    );
    assert_eq!(vm.call_i32("div", &[10, 2]), 5);
    assert_eq!(vm.call_expecting_trap("div", &[10, 0]), Trap::IntegerDivisionByZero);
    assert_eq!(vm.call_expecting_trap("rem", &[10, 0]), Trap::IntegerDivisionByZero);
    // The one overflowing division.
    assert_eq!(vm.call_expecting_trap("div", &[i32::MIN, -1]), Trap::IntegerOverflow);
}

// ============================================================================
// Variables and scopes
// ============================================================================

#[test]
fn let_bindings_and_assignment() {
    assert_eq!(
        run_int(
            "fn main() -> int {\n\
                 let a = 5;\n\
                 let b = a * 2;\n\
                 a = a + b;\n\
                 b = b - 1;\n\
                 return a * 100 + b;\n\
             }"
        ),
        1509
    );
}

#[test]
fn parameters_are_locals_zero_to_n() {
    let mut vm = Vm::from_source(
        "fn sub(a: int, b: int) -> int { return a - b; }\n\
         fn third(a: int, b: int, c: int) -> int { let extra = 1000; return c + extra; }\n\
         fn bump(n: int) -> int { n = n + 1; n = n * 2; return n; }\n\
         fn main() { }",
    );
    assert_eq!(vm.call_i32("sub", &[10, 3]), 7, "argument order matters");
    assert_eq!(vm.call_i32("third", &[1, 2, 3]), 1003, "the first `let` is local index 3");
    assert_eq!(vm.call_i32("bump", &[4]), 10, "parameters are assignable");
}

#[test]
fn block_scoping_and_shadowing() {
    let mut vm = Vm::from_source(
        "fn main() -> int {\n\
             let x = 1;\n\
             if true {\n\
                 let x = 10;\n\
                 x = x + 5;\n\
                 print_int(x);\n\
             }\n\
             let i = 0;\n\
             while i < 2 {\n\
                 let x = x + 100;\n\
                 print_int(x);\n\
                 i = i + 1;\n\
             }\n\
             return x;\n\
         }",
    );
    // The outer `x` is untouched by either inner `x`, and the initialiser of
    // the loop's `x` reads the OUTER one (1 + 100) on every iteration.
    assert_eq!(vm.call_i32("main", &[]), 1);
    assert_eq!(vm.printed_ints(), [15, 101, 101]);
}

#[test]
fn sibling_scopes_may_reuse_a_name_with_another_type() {
    // Each `v` needs its own wasm local: one is an f64, the other an i32.
    let vm = run(
        "fn main() {\n\
             if true { let v = 1.5; print_float(v); }\n\
             if true { let v = 7; print_int(v); }\n\
         }",
    );
    assert_eq!(vm.printed(), [Printed::Float(1.5), Printed::Int(7)]);
}

#[test]
fn expression_statements_discard_their_value() {
    // Each of the first three statements leaves a value that must be
    // dropped, or the function body fails validation.
    assert_eq!(
        run_int(
            "fn three() -> int { return 3; }\n\
             fn main() -> int {\n\
                 three();\n\
                 1 + 2;\n\
                 2.5 * 2.0;\n\
                 return 7;\n\
             }"
        ),
        7
    );
}

// ============================================================================
// Control flow
// ============================================================================

#[test]
fn if_else_selects_a_branch() {
    let mut vm = Vm::from_source(
        "fn max(a: int, b: int) -> int { if a > b { return a; } else { return b; } }\n\
         fn abs(x: int) -> int { if x < 0 { return -x; } return x; }\n\
         fn sign(x: int) -> int {\n\
             if x < 0 { return -1; } else if x == 0 { return 0; } else { return 1; }\n\
         }\n\
         fn main() -> int { return max(3, 7) * 100 + abs(-4) * 10 + sign(-9) + 1; }",
    );
    assert_eq!(vm.call_i32("main", &[]), 740);
    assert_eq!(vm.call_i32("max", &[10, 2]), 10);
    assert_eq!(vm.call_i32("max", &[2, 10]), 10);
    assert_eq!(vm.call_i32("abs", &[5]), 5);
    assert_eq!(vm.call_i32("abs", &[-5]), 5);
    assert_eq!(vm.call_i32("sign", &[-3]), -1);
    assert_eq!(vm.call_i32("sign", &[0]), 0);
    assert_eq!(vm.call_i32("sign", &[8]), 1);
}

#[test]
fn function_whose_last_statement_is_an_if_else_validates() {
    // Both branches return, so control never reaches the end of the body.
    // The wasm validator does not know that: without an `unreachable`
    // before the final `end` it reports a missing i32 result.
    let mut vm = Vm::from_source(
        "fn pick(c: bool) -> int { if c { return 1; } else { return 2; } }\n\
         fn main() { }",
    );
    assert_eq!(vm.call_i32("pick", &[1]), 1);
    assert_eq!(vm.call_i32("pick", &[0]), 2);
}

#[test]
fn if_without_else_and_empty_blocks() {
    assert_eq!(
        run_int(
            "fn main() -> int {\n\
                 let x = 5;\n\
                 if x > 3 { x = x * 2; }\n\
                 if x > 100 { x = 0; }\n\
                 if x == 10 { } else { x = -1; }\n\
                 return x;\n\
             }"
        ),
        10
    );
}

#[test]
fn while_loop_sums_a_range() {
    assert_eq!(
        run_int(
            "fn main() -> int {\n\
                 let total = 0;\n\
                 let i = 1;\n\
                 while i <= 100 {\n\
                     total = total + i;\n\
                     i = i + 1;\n\
                 }\n\
                 return total;\n\
             }"
        ),
        5050
    );
    // A loop whose condition is false from the start runs zero times.
    assert_eq!(run_int("fn main() -> int { let n = 0; while false { n = 1; } return n; }"), 0);
}

#[test]
fn nested_loops() {
    assert_eq!(
        run_int(
            "fn main() -> int {\n\
                 let total = 0;\n\
                 let i = 1;\n\
                 while i <= 5 {\n\
                     let j = 1;\n\
                     while j <= 5 {\n\
                         total = total + i * j;\n\
                         j = j + 1;\n\
                     }\n\
                     i = i + 1;\n\
                 }\n\
                 return total;\n\
             }"
        ),
        225
    );
}

#[test]
fn if_inside_while_inside_function() {
    let mut vm = Vm::from_source(
        "fn collatz(n: int) -> int {\n\
             let steps = 0;\n\
             while n != 1 {\n\
                 if n % 2 == 0 { n = n / 2; } else { n = 3 * n + 1; }\n\
                 steps = steps + 1;\n\
             }\n\
             return steps;\n\
         }\n\
         fn gcd(a: int, b: int) -> int {\n\
             while b != 0 {\n\
                 let t = a % b;\n\
                 a = b;\n\
                 b = t;\n\
             }\n\
             return a;\n\
         }\n\
         fn main() { }",
    );
    assert_eq!(vm.call_i32("collatz", &[1]), 0);
    assert_eq!(vm.call_i32("collatz", &[6]), 8);
    assert_eq!(vm.call_i32("collatz", &[27]), 111);
    assert_eq!(vm.call_i32("gcd", &[1071, 462]), 21);
    assert_eq!(vm.call_i32("gcd", &[17, 5]), 1);
}

#[test]
fn void_function_with_early_return() {
    let vm = run(
        "fn countdown(n: int) {\n\
             while true {\n\
                 if n == 0 { return; }\n\
                 print_int(n);\n\
                 n = n - 1;\n\
             }\n\
         }\n\
         fn main() { countdown(3); }",
    );
    assert_eq!(vm.printed_ints(), [3, 2, 1]);
}

// ============================================================================
// Functions and recursion
// ============================================================================

#[test]
fn recursive_fibonacci() {
    let mut vm = Vm::from_source(
        "fn fib(n: int) -> int {\n\
             if n < 2 { return n; }\n\
             return fib(n - 1) + fib(n - 2);\n\
         }\n\
         fn main() -> int { return fib(10); }",
    );
    assert_eq!(vm.call_i32("main", &[]), 55);
    let expected = [0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610];
    for (n, want) in expected.into_iter().enumerate() {
        assert_eq!(vm.call_i32("fib", &[n as i32]), want, "fib({n})");
    }
    assert_eq!(vm.call_i32("fib", &[20]), 6765);
}

#[test]
fn recursive_factorial() {
    let mut vm = Vm::from_source(
        "fn fact(n: int) -> int {\n\
             if n <= 1 { return 1; }\n\
             return n * fact(n - 1);\n\
         }\n\
         fn main() -> int { return fact(10); }",
    );
    assert_eq!(vm.call_i32("main", &[]), 3_628_800);
    assert_eq!(vm.call_i32("fact", &[0]), 1);
    assert_eq!(vm.call_i32("fact", &[5]), 120);
    assert_eq!(vm.call_i32("fact", &[12]), 479_001_600);
}

#[test]
fn mutual_recursion_and_forward_references() {
    // `main` calls functions defined after it, and they call each other.
    let mut vm = Vm::from_source(
        "fn main() -> int { if is_even(10) && is_odd(7) { return 1; } return 0; }\n\
         fn is_even(n: int) -> bool { if n == 0 { return true; } return is_odd(n - 1); }\n\
         fn is_odd(n: int) -> bool { if n == 0 { return false; } return is_even(n - 1); }",
    );
    assert_eq!(vm.call_i32("main", &[]), 1);
    assert_eq!(vm.call_i32("is_even", &[4]), 1, "true is the i32 1");
    assert_eq!(vm.call_i32("is_even", &[5]), 0, "false is the i32 0");
    assert_eq!(vm.call_i32("is_odd", &[5]), 1);
}

#[test]
fn print_int_calls_the_host_in_order() {
    let vm = run("fn main() { print_int(1); print_int(2); print_int(3); print_int(-4); }");
    assert_eq!(vm.printed_ints(), [1, 2, 3, -4]);
}

#[test]
fn arguments_are_evaluated_left_to_right() {
    let vm = run(
        "fn noisy(tag: int) -> int { print_int(tag); return tag; }\n\
         fn add3(a: int, b: int, c: int) -> int { return a + b + c; }\n\
         fn main() { print_int(add3(noisy(1), noisy(2), noisy(3)) + noisy(4)); }",
    );
    assert_eq!(vm.printed_ints(), [1, 2, 3, 4, 10]);
}

// ============================================================================
// Booleans
// ============================================================================

#[test]
fn boolean_logic() {
    assert_eq!(
        run_int(
            "fn main() -> int {\n\
                 let t = true;\n\
                 let f = false;\n\
                 let score = 0;\n\
                 if t && !f { score = score + 1; }\n\
                 if f || t { score = score + 10; }\n\
                 if !(t && f) { score = score + 100; }\n\
                 if t == true && f != true { score = score + 1000; }\n\
                 if f { score = score + 10000; }\n\
                 return score;\n\
             }"
        ),
        1111
    );
}

#[test]
fn comparisons_produce_zero_or_one() {
    let mut vm = Vm::from_source(
        "fn lt(a: int, b: int) -> bool { return a < b; }\n\
         fn le(a: int, b: int) -> bool { return a <= b; }\n\
         fn gt(a: int, b: int) -> bool { return a > b; }\n\
         fn ge(a: int, b: int) -> bool { return a >= b; }\n\
         fn eq(a: int, b: int) -> bool { return a == b; }\n\
         fn ne(a: int, b: int) -> bool { return a != b; }\n\
         fn not(a: bool) -> bool { return !a; }\n\
         fn main() { }",
    );
    for (a, b) in [(1, 2), (2, 1), (3, 3), (-5, 2), (2, -5)] {
        assert_eq!(vm.call_i32("lt", &[a, b]), (a < b) as i32, "{a} < {b}");
        assert_eq!(vm.call_i32("le", &[a, b]), (a <= b) as i32, "{a} <= {b}");
        assert_eq!(vm.call_i32("gt", &[a, b]), (a > b) as i32, "{a} > {b}");
        assert_eq!(vm.call_i32("ge", &[a, b]), (a >= b) as i32, "{a} >= {b}");
        assert_eq!(vm.call_i32("eq", &[a, b]), (a == b) as i32, "{a} == {b}");
        assert_eq!(vm.call_i32("ne", &[a, b]), (a != b) as i32, "{a} != {b}");
    }
    assert_eq!(vm.call_i32("not", &[1]), 0);
    assert_eq!(vm.call_i32("not", &[0]), 1);
}

#[test]
fn logical_operators_short_circuit() {
    let vm = run(
        "fn noisy(value: bool, tag: int) -> bool { print_int(tag); return value; }\n\
         fn main() {\n\
             if noisy(false, 1) && noisy(true, 2) { print_int(100); }\n\
             if noisy(true, 3) || noisy(true, 4) { print_int(200); }\n\
             if noisy(true, 5) && noisy(false, 6) { print_int(300); }\n\
             if noisy(false, 7) || noisy(false, 8) { print_int(400); }\n\
         }",
    );
    // 2 and 4 never print: the left operand already decided the result.
    assert_eq!(vm.printed_ints(), [1, 3, 200, 5, 6, 7, 8]);
}

// ============================================================================
// Floats
// ============================================================================

#[test]
fn float_arithmetic_and_print_float() {
    let vm = run(
        "fn main() {\n\
             print_float(1.5 + 2.25);\n\
             print_float(10.0 / 4.0);\n\
             print_float(2.5 * 4.0 - 0.5);\n\
             print_float(-(1.5));\n\
             print_float(0.1 + 0.2);\n\
             print_float(3.141592653589793);\n\
         }",
    );
    assert_eq!(
        vm.printed_floats(),
        [3.75, 2.5, 9.5, -1.5, 0.1 + 0.2, std::f64::consts::PI]
    );
}

#[test]
fn float_functions_locals_and_comparisons() {
    let mut vm = Vm::from_source(
        "fn average(a: float, b: float) -> float { return (a + b) / 2.0; }\n\
         fn is_close(a: float, b: float) -> bool {\n\
             let d = a - b;\n\
             if d < 0.0 { d = -d; }\n\
             return d < 0.001;\n\
         }\n\
         fn main() -> int {\n\
             if is_close(average(1.0, 2.0), 1.5) && 2.5 >= 2.5 && 1.0 != 2.0 && 0.5 <= 0.75 {\n\
                 return 1;\n\
             }\n\
             return 0;\n\
         }",
    );
    assert_eq!(vm.call_i32("main", &[]), 1);
    assert_eq!(vm.call_f64("average", &[3.0, 5.0]), 4.0);
    assert_eq!(vm.call_f64("average", &[-1.0, 0.5]), -0.25);
}

#[test]
fn mixed_int_and_float_parameters_and_locals() {
    // Parameters: i32, f64, i32, f64, i32. Locals after them: f64 then i32.
    // Getting any local index or declared local type wrong fails validation.
    let mut vm = Vm::from_source(
        "fn mix(a: int, x: float, b: int, y: float, flag: bool) -> float {\n\
             let t = x * y;\n\
             if flag { t = t + 1.0; }\n\
             let k = a + b;\n\
             if k > 5 { t = t * 2.0; }\n\
             return t;\n\
         }\n\
         fn main() { }",
    );
    use wasmtime::Val;
    let call = |vm: &mut Vm, a: i32, x: f64, b: i32, y: f64, flag: i32| -> f64 {
        let args = [Val::I32(a), Val::F64(x.to_bits()), Val::I32(b), Val::F64(y.to_bits()), Val::I32(flag)];
        match vm.call("mix", &args).expect("mix should not trap").as_slice() {
            [Val::F64(bits)] => f64::from_bits(*bits),
            other => panic!("expected one f64, got {other:?}"),
        }
    };
    assert_eq!(call(&mut vm, 2, 1.5, 4, 2.0, 1), 8.0);
    assert_eq!(call(&mut vm, 2, 1.5, 4, 2.0, 0), 6.0);
    assert_eq!(call(&mut vm, 1, 1.5, 1, 2.0, 1), 4.0);
    assert_eq!(call(&mut vm, 1, 1.5, 1, 2.0, 0), 3.0);
}

// ============================================================================
// Arrays and linear memory
// ============================================================================

#[test]
fn array_layout_in_linear_memory() {
    let mut vm = Vm::from_source(
        "fn main() -> int {\n\
             let a: int[] = alloc(3);\n\
             a[0] = 7;\n\
             a[1] = 8;\n\
             a[2] = 9;\n\
             return a[0] + a[1] + a[2] + len(a);\n\
         }",
    );
    assert_eq!(vm.memory_size(), 65536, "one 64 KiB page");
    assert_eq!(vm.read_i32(0), 0, "the heap pointer cell starts out zero");

    assert_eq!(vm.call_i32("main", &[]), 27);

    // The first allocation starts at HEAP_BASE = 8.
    assert_eq!(vm.read_i32(8), 3, "length header at 0x08");
    assert_eq!(vm.read_i32(12), 7, "a[0] at 0x0C");
    assert_eq!(vm.read_i32(16), 8, "a[1] at 0x10");
    assert_eq!(vm.read_i32(20), 9, "a[2] at 0x14");
    assert_eq!(vm.read_i32(0), 24, "heap pointer = 8 + 4 + 3 * 4");
    assert_eq!(vm.read_i32(24), 0, "memory past the array is untouched");
    assert_eq!(vm.read_array(8), [7, 8, 9]);
}

#[test]
fn allocations_do_not_overlap() {
    let mut vm = Vm::from_source(
        "fn main() -> int {\n\
             let a: int[] = alloc(2);\n\
             let b: int[] = alloc(3);\n\
             a[0] = 1; a[1] = 2;\n\
             b[0] = 10; b[1] = 20; b[2] = 30;\n\
             return a[0] + a[1] + b[0] + b[1] + b[2];\n\
         }",
    );
    assert_eq!(vm.call_i32("main", &[]), 63);
    assert_eq!(vm.read_array(8), [1, 2], "a: header at 8, elements at 12 and 16");
    assert_eq!(vm.read_array(20), [10, 20, 30], "b starts right after a: 8 + 4 + 2 * 4 = 20");
    assert_eq!(vm.read_i32(0), 36, "heap pointer = 20 + 4 + 3 * 4");
}

#[test]
fn elements_use_little_endian_i32_and_handle_negatives() {
    let mut vm = Vm::from_source(
        "fn main() -> int {\n\
             let a: int[] = alloc(2);\n\
             a[0] = 305419896;\n\
             a[1] = -2;\n\
             return a[1];\n\
         }",
    );
    assert_eq!(vm.call_i32("main", &[]), -2);
    // 305419896 == 0x12345678
    assert_eq!(vm.read_i32(12), 0x1234_5678);
    assert_eq!(vm.read_i32(16), -2);
}

#[test]
fn arrays_are_passed_by_reference() {
    let mut vm = Vm::from_source(
        "fn fill(a: int[], value: int) {\n\
             let i = 0;\n\
             while i < len(a) { a[i] = value; i = i + 1; }\n\
         }\n\
         fn make(n: int) -> int[] {\n\
             let a: int[] = alloc(n);\n\
             fill(a, 9);\n\
             return a;\n\
         }\n\
         fn main() -> int {\n\
             let a = make(4);\n\
             a[1] = a[1] + 1;\n\
             return a[0] + a[1] + a[3];\n\
         }",
    );
    assert_eq!(vm.call_i32("main", &[]), 28);
    assert_eq!(vm.read_array(8), [9, 10, 9, 9]);
}

#[test]
fn index_expressions_can_be_arbitrary_ints() {
    assert_eq!(
        run_int(
            "fn main() -> int {\n\
                 let a: int[] = alloc(5);\n\
                 let i = 0;\n\
                 while i < len(a) { a[i] = i * i; i = i + 1; }\n\
                 a[a[2]] = 100;\n\
                 a[len(a) - 1] = a[1 + 2] + a[4 - 4];\n\
                 return a[4] * 1000 + a[a[2]];\n\
             }"
        ),
        // a = [0, 1, 4, 9, 16] → a[4] = 100 → a[4] = a[3] + a[0] = 9 → 9 * 1000 + a[4] = 9009
        9009
    );
}

#[test]
fn host_can_allocate_through_the_exported_alloc() {
    let mut vm = Vm::from_source(
        "fn sum(a: int[]) -> int {\n\
             let total = 0;\n\
             let i = 0;\n\
             while i < len(a) { total = total + a[i]; i = i + 1; }\n\
             return total;\n\
         }\n\
         fn main() { }",
    );
    let first = vm.new_array(&[1, 2, 3, 4]);
    let second = vm.new_array(&[100, -50]);
    let empty = vm.new_array(&[]);
    assert_eq!(first, 8);
    assert_eq!(second, 8 + 4 + 4 * 4);
    assert_eq!(empty, second + 4 + 2 * 4);
    assert_eq!(vm.call_i32("sum", &[first]), 10);
    assert_eq!(vm.call_i32("sum", &[second]), 50);
    assert_eq!(vm.call_i32("sum", &[empty]), 0);
    assert_eq!(vm.read_array(first), [1, 2, 3, 4]);
}

// ============================================================================
// Algorithms
// ============================================================================

#[test]
fn bubble_sort_sorts_in_place() {
    let mut vm = Vm::from_source(
        "fn bubble_sort(a: int[]) {\n\
             let n = len(a);\n\
             let i = 0;\n\
             while i < n - 1 {\n\
                 let j = 0;\n\
                 while j < n - 1 - i {\n\
                     if a[j] > a[j + 1] {\n\
                         let tmp = a[j];\n\
                         a[j] = a[j + 1];\n\
                         a[j + 1] = tmp;\n\
                     }\n\
                     j = j + 1;\n\
                 }\n\
                 i = i + 1;\n\
             }\n\
         }\n\
         fn main() { }",
    );
    let cases: [&[i32]; 6] = [
        &[64, 34, 25, 12, 22, 11, 90, 5],
        &[1, 2, 3, 4, 5],
        &[5, 4, 3, 2, 1],
        &[3, -1, 3, 0, -7, 3],
        &[42],
        &[],
    ];
    for values in cases {
        let ptr = vm.new_array(values);
        vm.call_void("bubble_sort", &[ptr]);
        let mut expected = values.to_vec();
        expected.sort();
        assert_eq!(vm.read_array(ptr), expected, "sorting {values:?}");
    }
}

#[test]
fn recursive_quicksort_sorts_in_place() {
    let mut vm = Vm::from_source(
        "fn swap(a: int[], i: int, j: int) {\n\
             let t = a[i];\n\
             a[i] = a[j];\n\
             a[j] = t;\n\
         }\n\
         fn partition(a: int[], lo: int, hi: int) -> int {\n\
             let pivot = a[hi];\n\
             let i = lo - 1;\n\
             let j = lo;\n\
             while j < hi {\n\
                 if a[j] <= pivot {\n\
                     i = i + 1;\n\
                     swap(a, i, j);\n\
                 }\n\
                 j = j + 1;\n\
             }\n\
             swap(a, i + 1, hi);\n\
             return i + 1;\n\
         }\n\
         fn quicksort(a: int[], lo: int, hi: int) {\n\
             if lo < hi {\n\
                 let p = partition(a, lo, hi);\n\
                 quicksort(a, lo, p - 1);\n\
                 quicksort(a, p + 1, hi);\n\
             }\n\
         }\n\
         fn main() { }",
    );

    // A deterministic pseudo-random input with duplicates and negatives.
    let mut state: u32 = 0x2545_F491;
    let values: Vec<i32> = (0..200)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((state >> 16) % 201) as i32 - 100
        })
        .collect();

    let ptr = vm.new_array(&values);
    vm.call_void("quicksort", &[ptr, 0, values.len() as i32 - 1]);

    let mut expected = values.clone();
    expected.sort();
    assert_eq!(vm.read_array(ptr), expected);
}

#[test]
fn recursive_binary_search() {
    let mut vm = Vm::from_source(
        "fn search(a: int[], target: int, lo: int, hi: int) -> int {\n\
             if lo > hi { return -1; }\n\
             let mid = lo + (hi - lo) / 2;\n\
             if a[mid] == target { return mid; }\n\
             if a[mid] < target { return search(a, target, mid + 1, hi); }\n\
             return search(a, target, lo, mid - 1);\n\
         }\n\
         fn find(a: int[], target: int) -> int { return search(a, target, 0, len(a) - 1); }\n\
         fn main() { }",
    );
    let sorted: Vec<i32> = (0..10).map(|i| 2 * i + 1).collect(); // 1, 3, 5, ..., 19
    let ptr = vm.new_array(&sorted);
    for (index, value) in sorted.iter().enumerate() {
        assert_eq!(vm.call_i32("find", &[ptr, *value]), index as i32, "searching for {value}");
    }
    for missing in [0, 4, 20, -3, 100] {
        assert_eq!(vm.call_i32("find", &[ptr, missing]), -1, "searching for {missing}");
    }
    let empty = vm.new_array(&[]);
    assert_eq!(vm.call_i32("find", &[empty, 1]), -1);
}

// ============================================================================
// The playground's sample programs
// ============================================================================

#[test]
fn example_fibonacci() {
    let mut vm = Vm::from_source(common::FIBONACCI);
    assert_eq!(vm.call_i32("main", &[]), 6765);

    let mut golden = 1.0_f64;
    for _ in 0..30 {
        golden = 1.0 + 1.0 / golden;
    }
    let mut expected: Vec<Printed> =
        [0, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55].into_iter().map(Printed::Int).collect();
    expected.push(Printed::Float(golden));
    assert_eq!(vm.printed(), expected);
    assert!((golden - 1.618_033_988_749_895).abs() < 1e-9);
}

#[test]
fn example_array_sum() {
    let mut vm = Vm::from_source(common::ARRAY_SUM);
    assert_eq!(vm.call_i32("main", &[]), 91);
    assert_eq!(vm.printed_ints(), [1, 4, 9, 16, 25, 36, 91]);
    assert_eq!(vm.read_array(8), [1, 4, 9, 16, 25, 36]);
    assert_eq!(vm.read_i32(0), 8 + 4 + 6 * 4);
}

#[test]
fn example_bubble_sort() {
    let mut vm = Vm::from_source(common::BUBBLE_SORT);
    assert_eq!(vm.call_i32("main", &[]), 21, "total number of swaps");
    assert_eq!(
        vm.printed_ints(),
        [
            6, 5, 4, 2, 2, 1, 1, // swaps per pass
            5, 11, 12, 22, 25, 34, 64, 90, // the sorted array
        ]
    );
    assert_eq!(vm.read_array(8), [5, 11, 12, 22, 25, 34, 64, 90]);
}
