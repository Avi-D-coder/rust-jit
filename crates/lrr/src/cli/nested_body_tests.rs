//! Block-local functions whose bodies contain macro calls (E-R5-010).
//!
//! A function declared inside another function's body shares that body's
//! retained tree: the common source-syntax assembly attaches the calls its
//! nested bodies own in the same transaction as the enclosing body's own
//! (`SourceSyntaxAssembly::first_call_within`), and a call between block-local
//! functions lowers through the same typed call route as a call in the
//! declaring body (`EnteredCallGoal::retained_plan` is the call site's plan,
//! the admission's parent the declaring one). Each shape runs handwritten,
//! through a user macro, through a wrapper macro and, where it applies,
//! emitted by a macro, in all four editions. Expected output is the program's
//! output under real rustc (`rustc +1.99.0`).
use super::format_trait_facade_tests::prints;

const MACROS: &str = "macro_rules! show { ($e:expr) => { println!(\"{}\", $e) }; }
macro_rules! outer { ($e:expr) => { show!($e) }; }
";

/// `P!(expr)` in `program` prints `expr`: written as `println!`, as a user
/// macro call and as a wrapper macro call. The nested bodies must take the one
/// route and agree.
fn print_forms(label: &str, program: &str, expected: &str) {
    prints(
        &format!("{label}-handwritten"),
        &program.replace("P!(", "println!(\"{}\", "),
        expected,
    );
    prints(
        &format!("{label}-macro"),
        &format!("{MACROS}{}", program.replace("P!(", "show!(")),
        expected,
    );
    prints(
        &format!("{label}-wrapped"),
        &format!("{MACROS}{}", program.replace("P!(", "outer!(")),
        expected,
    );
}

/// The same block items written by hand, emitted by a user macro and emitted
/// through a wrapper macro (macro-emitted nested functions and their calls).
fn emitted_forms(label: &str, items: &str, rest: &str, expected: &str) {
    prints(
        &format!("{label}-handwritten"),
        &format!("fn main() {{\n{items}\n{rest}\n}}\n"),
        expected,
    );
    prints(
        &format!("{label}-macro"),
        &format!(
            "macro_rules! decl {{ () => {{\n{items}\n}} }}\nfn main() {{\n    decl!();\n{rest}\n}}\n"
        ),
        expected,
    );
    prints(
        &format!("{label}-wrapped"),
        &format!(
            "macro_rules! decl {{ () => {{\n{items}\n}} }}\nmacro_rules! wrap {{ () => {{ decl!(); }} }}\nfn main() {{\n    wrap!();\n{rest}\n}}\n"
        ),
        expected,
    );
}

#[test]
fn nested_function_with_a_printing_macro_call() {
    print_forms(
        "print",
        "fn main() { fn f(a: i32) { P!(a + 1); } f(1); f(5); }\n",
        "2\n6\n",
    );
    prints(
        "println-literal",
        "fn main() { fn f() { println!(\"x\"); } f(); }\n",
        "x\n",
    );
}

#[test]
fn nested_function_with_assertion_and_formatting_macro_calls() {
    prints(
        "assert-eq",
        "fn main() { fn f(a: i32) { assert_eq!(a, 3); } f(3); println!(\"ok\"); }\n",
        "ok\n",
    );
    prints(
        "format",
        "fn main() { fn f(a: i32) -> String { format!(\"<{}>\", a) } println!(\"{}\", f(2)); }\n",
        "<2>\n",
    );
    prints(
        "vec-matches",
        "fn main() {
    fn v() -> Vec<i32> { vec![1, 2, 3] }
    fn m(x: Option<i32>) -> bool { matches!(x, Some(1..=3)) }
    println!(\"{:?} {}\", v(), m(Some(2)));
}
",
        "[1, 2, 3] true\n",
    );
}

#[test]
fn nested_function_called_inside_an_assertion_macro() {
    prints(
        "assert-call",
        "fn main() {
    fn f(a: i32) -> i32 { println!(\"f {}\", a); a * 2 }
    assert_eq!(f(2), 4);
    println!(\"done\");
}
",
        "f 2\ndone\n",
    );
}

#[test]
fn macro_emitted_nested_functions_with_macro_bodies() {
    emitted_forms(
        "emit-fn",
        "    fn dbl(a: i32) -> i32 { println!(\"in {}\", a); a * 2 }",
        "    println!(\"{}\", dbl(4));",
        "in 4\n8\n",
    );
    emitted_forms(
        "emit-pair",
        "    fn helper() -> i32 { 7 }
    fn user() -> i32 { println!(\"user\"); helper() }",
        "    println!(\"{}\", user());",
        "user\n7\n",
    );
    prints(
        "emit-with-ident",
        "macro_rules! mk { ($n:ident) => { fn $n(a: i32) -> i32 { println!(\"in {}\", a); a * 2 } } }
fn main() { mk!(dbl); println!(\"{}\", dbl(4)); }
",
        "in 4\n8\n",
    );
    prints(
        "emit-chain",
        "macro_rules! helper {
    ($name:ident, $callee:ident) => {
        fn $name(x: i32) -> i32 { println!(\"{}({})\", stringify!($name), x); $callee(x) + 1 }
    };
}
fn main() {
    fn base(x: i32) -> i32 { x * 10 }
    helper!(mid, base);
    helper!(top, mid);
    println!(\"{}\", top(2));
}
",
        "top(2)\nmid(2)\n22\n",
    );
}

#[test]
fn macro_emitted_block_expression_declares_and_calls_a_nested_function() {
    prints(
        "emit-block",
        "macro_rules! m { () => {{ fn z() -> i32 { println!(\"z\"); 9 } z() }} }
fn main() { let v = m!(); println!(\"{}\", v); { fn q() { println!(\"q\"); } q(); } }
",
        "z\n9\nq\n",
    );
}

#[test]
fn block_local_functions_call_their_siblings_and_themselves() {
    print_forms(
        "sibling",
        "fn main() {
    fn b() -> i32 { 1 }
    fn a() -> i32 { b() + 1 }
    fn c() -> i32 { a() + b() }
    P!(format!(\"{} {} {}\", a(), b(), c()));
}
",
        "2 1 3\n",
    );
    prints(
        "sibling-minimal",
        "fn main() { fn b() -> i32 { 1 } fn a() -> i32 { b() + 1 } println!(\"{}\", a()); }\n",
        "2\n",
    );
    prints(
        "recursive",
        "fn main() {
    fn fact(n: u64) -> u64 { if n <= 1 { 1 } else { n * fact(n - 1) } }
    println!(\"{}\", fact(5));
}
",
        "120\n",
    );
    prints(
        "mutual",
        "fn main() {
    fn even(n: u32) -> bool { if n == 0 { true } else { odd(n - 1) } }
    fn odd(n: u32) -> bool { if n == 0 { false } else { even(n - 1) } }
    println!(\"{} {}\", even(4), odd(4));
}
",
        "true false\n",
    );
    prints(
        "sibling-effects-once",
        "fn main() {
    fn a(n: i32) -> i32 { println!(\"a{}\", n); n }
    fn b(n: i32) -> i32 { a(n) + a(n + 1) }
    println!(\"{}\", b(1));
    println!(\"{}\", a(7));
}
",
        "a1\na2\n3\na7\n7\n",
    );
    prints(
        "sibling-calls-module-fn",
        "fn outer_helper() -> i32 { 100 }
fn main() {
    fn inner() -> i32 { println!(\"inner\"); outer_helper() + 1 }
    fn inner2() -> i32 { inner() + inner() }
    println!(\"{}\", inner2());
}
",
        "inner\ninner\n202\n",
    );
    prints(
        "sibling-reference-arguments",
        "fn main() {
    fn sum(v: &[i32]) -> i32 { let mut t = 0; for x in v { t += *x; } t }
    fn show(v: &[i32]) { println!(\"{:?} -> {}\", v, sum(v)); }
    show(&[1, 2, 3]);
    let v = vec![4, 5];
    show(&v);
}
",
        "[1, 2, 3] -> 6\n[4, 5] -> 9\n",
    );
}

#[test]
fn doubly_nested_functions_expand_and_run() {
    print_forms(
        "levels",
        "fn main() {
    fn level1() { fn level2() { fn level3() { P!(\"l3\"); } P!(\"l2\"); level3(); } P!(\"l1\"); level2(); }
    level1();
}
",
        "l1\nl2\nl3\n",
    );
}

#[test]
fn closures_and_nested_functions_in_either_direction() {
    prints(
        "closure-calls-nested",
        "fn main() {
    fn a() -> i32 { 1 }
    fn b() -> i32 { 2 }
    let c = || a() + b();
    println!(\"{}\", c());
}
",
        "3\n",
    );
    prints(
        "nested-in-closure",
        "fn main() {
    let d = |x: i32| { fn sq(v: i32) -> i32 { println!(\"sq {}\", v); v * v } sq(x) };
    println!(\"{}\", d(3));
}
",
        "sq 3\n9\n",
    );
}

#[test]
fn nested_functions_inside_methods_and_trait_default_methods() {
    prints(
        "inherent-method",
        "struct S(i32);
impl S { fn m(&self) -> i32 { fn h(v: i32) -> i32 { println!(\"h {}\", v); v + 1 } h(self.0) } }
fn main() { println!(\"{}\", S(4).m()); }
",
        "h 4\n5\n",
    );
    prints(
        "trait-default",
        "trait T { fn go(&self) { fn helper(x: i32) -> String { format!(\"helper {}\", x) } println!(\"{}\", helper(3)); } }
struct A;
impl T for A {}
fn main() { A.go(); }
",
        "helper 3\n",
    );
    prints(
        "generic-outer",
        "fn generic<T: std::fmt::Debug>(t: T) { fn show<U: std::fmt::Debug>(u: U) { println!(\"{:?}\", u); } show(t); }
fn main() { generic(5); generic(\"x\"); }
",
        "5\n\"x\"\n",
    );
}

/// Type paths written inside a block-local function (annotations, casts,
/// turbofish, a record constructor of a module type) belong to the original
/// source of the outermost module-level definition whose retained body holds
/// the nested one (`retained_original_origin`, through the declaring-body
/// chain), and resolve like the same paths in that definition's own body.
#[test]
fn nested_function_type_occurrences_resolve_in_a_retained_body() {
    emitted_forms_module_struct(
        "types",
        "    fn mm(y: i32) -> M { M { y } }
    fn total(a: u8, b: u8) -> i32 { let wide: i32 = a as i32; wide + b as i32 }
    fn id<T>(t: T) -> T { t }
    fn pick() -> i32 { id::<i32>(4) + Vec::<i32>::new().len() as i32 }",
        "    println!(\"{} {} {}\", mm(5).y, total(1, 2), pick());",
        "5 3 4\n",
    );
    prints(
        "types-doubly-nested",
        "struct M { y: i32 }
fn main() {
    fn outer() -> i32 {
        fn inner(v: u8) -> M { let m: M = M { y: v as i32 }; m }
        inner(6).y
    }
    println!(\"{}\", outer());
}
",
        "6\n",
    );
}

fn emitted_forms_module_struct(label: &str, items: &str, rest: &str, expected: &str) {
    const STRUCT: &str = "struct M { y: i32 }\n";
    prints(
        &format!("{label}-handwritten"),
        &format!("{STRUCT}fn main() {{\n{items}\n{rest}\n}}\n"),
        expected,
    );
    prints(
        &format!("{label}-macro"),
        &format!(
            "{STRUCT}macro_rules! decl {{ () => {{\n{items}\n}} }}\nfn main() {{\n    decl!();\n{rest}\n}}\n"
        ),
        expected,
    );
    prints(
        &format!("{label}-wrapped"),
        &format!(
            "{STRUCT}macro_rules! decl {{ () => {{\n{items}\n}} }}\nmacro_rules! wrap {{ () => {{ decl!(); }} }}\nfn main() {{\n    wrap!();\n{rest}\n}}\n"
        ),
        expected,
    );
}

/// A nested function names the types its declaring body introduced: record
/// constructors, annotations and patterns resolve to the declaring body's
/// declaration (its row lives in the declaring plan, found through
/// `LocalDeclarationOwner`).
#[test]
fn nested_functions_name_block_local_types_of_the_declaring_body() {
    emitted_forms(
        "enclosing-types",
        "    struct P { x: i32, y: i32 }
    enum E { A, B(i32), C { v: i32 } }
    fn mk(x: i32) -> P { let p: P = P { x, y: x + 1 }; p }
    fn sum(p: &P) -> i32 { let P { x, y } = p; println!(\"sum\"); x + y }
    fn desc(e: E) -> String {
        match e { E::A => format!(\"a\"), E::B(n) => format!(\"b{}\", n), E::C { v } => format!(\"c{}\", v) }
    }
    fn make(n: i32) -> E { if n == 0 { E::A } else if n == 1 { E::B(n) } else { E::C { v: n } } }",
        "    println!(\"{} {} {} {}\", sum(&mk(2)), desc(make(0)), desc(make(1)), desc(make(5)));",
        "sum\n5 a b1 c5\n",
    );
}

/// Items of every enclosing body are visible to a deeper nested function: the
/// grandchild calls its grandparent's function and builds its type.
#[test]
fn doubly_nested_functions_name_items_of_every_enclosing_body() {
    emitted_forms(
        "grandparent-items",
        "    struct S { v: i32 }
    fn a() -> i32 { println!(\"a\"); 1 }
    fn b() -> i32 {
        fn c() -> i32 { let s: S = S { v: a() + 1 }; println!(\"c {}\", s.v); s.v }
        c() + 1
    }",
        "    println!(\"{}\", b());",
        "a\nc 2\n3\n",
    );
}

/// A nested function that is never called stays cold: its macro calls are
/// attached with the enclosing body (structural only) but never executed, and a
/// never-taken branch that calls it does not run it.
#[test]
fn uncalled_nested_functions_stay_cold() {
    prints(
        "cold",
        "fn main() {
    fn cold() { println!(\"never\"); assert_eq!(1, 2); }
    fn hot() { println!(\"hot\"); }
    fn branch(n: i32) { if n > 100 { cold(); } else { hot(); } }
    branch(1);
}
",
        "hot\n",
    );
}

/// A macro's own binding does not capture the nested function's parameter.
#[test]
fn nested_function_macro_hygiene_matches_the_enclosing_body() {
    prints(
        "hygiene",
        "macro_rules! m { ($x:ident) => { let a = 10; println!(\"{} {}\", a, $x); } }
fn main() { fn f(a: i32) { m!(a); } f(1); }
",
        "10 1\n",
    );
}

/// Macro calls inside the members of block-local impl blocks (the `Display`
/// and `Drop` shapes of the ignored `local_impl_tests` case, with the std paths
/// imported at module level).
#[test]
fn block_local_impl_members_with_macro_calls() {
    emitted_forms_with_imports(
        "impl-members",
        "    struct P(i32);
    impl Display for P {
        fn fmt(&self, f: &mut Formatter) -> Result { write!(f, \"P<{}>\", self.0) }
    }
    impl Default for P { fn default() -> P { P(77) } }
    struct D(i32);
    impl Drop for D { fn drop(&mut self) { println!(\"drop {}\", self.0); } }",
        "    let _d = D(5);
    println!(\"{} {}\", P(3), P::default());",
        "P<3> P<77>\ndrop 5\n",
    );
}

fn emitted_forms_with_imports(label: &str, items: &str, rest: &str, expected: &str) {
    const USE: &str = "use std::fmt::{Display, Formatter, Result};\n";
    prints(
        &format!("{label}-handwritten"),
        &format!("{USE}fn main() {{\n{items}\n{rest}\n}}\n"),
        expected,
    );
    prints(
        &format!("{label}-macro"),
        &format!(
            "{USE}macro_rules! decl {{ () => {{\n{items}\n}} }}\nfn main() {{\n    decl!();\n{rest}\n}}\n"
        ),
        expected,
    );
    prints(
        &format!("{label}-wrapped"),
        &format!(
            "{USE}macro_rules! decl {{ () => {{\n{items}\n}} }}\nmacro_rules! wrap {{ () => {{ decl!(); }} }}\nfn main() {{\n    wrap!();\n{rest}\n}}\n"
        ),
        expected,
    );
}

#[test]
fn block_local_function_is_called_instead_of_the_same_named_module_function() {
    // The entered lexical context selects the block item; the module
    // function of the same name (whose body would panic) is never entered.
    prints(
        "nested-shadows-module-fn",
        "fn id() -> i8 {
    panic!(\"wrong module entered\");
}
fn twice(n: i8) -> i8 {
    panic!(\"wrong module entered {}\", n);
}
fn main() {
    fn id() -> i8 {
        2
    }
    fn twice(n: i8) -> i8 {
        n * 2
    }
    println!(\"{} {}\", id(), twice(3));
}
",
        "2 6\n",
    );
}
