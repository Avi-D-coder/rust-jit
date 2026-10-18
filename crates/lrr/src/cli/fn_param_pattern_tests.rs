//! Function parameters with non-identifier patterns (E-R5-010):
//! `fn f(PAT: T) { body }` is `fn f(arg0: T) { let PAT = arg0; body }`. The
//! function's body handle spans its parameter list, the one `BodyIndex` indexes
//! the parameter patterns, and the shared pattern transaction binds them at
//! entry. Each shape runs in editions 2015-2024 and the expected output is the
//! program's output under real rustc (`rustc +1.99.0`).
//!
//! Generated forms: handwritten, emitted by a user macro, and emitted through a
//! wrapper macro must take the one route and agree.
use super::format_trait_facade_tests::prints;

/// Module-level `prelude` (kept outside the macros), then `items` followed by
/// `main`, written by hand, emitted by a user macro and emitted through a
/// wrapper macro.
fn all_forms(label: &str, prelude: &str, items: &str, main: &str, expected: &str) {
    prints(
        &format!("{label}-handwritten"),
        &format!("{prelude}\n{items}\n{main}\n"),
        expected,
    );
    prints(
        &format!("{label}-macro"),
        &format!("{prelude}\nmacro_rules! decl {{ () => {{\n{items}\n}} }}\ndecl!();\n{main}\n"),
        expected,
    );
    prints(
        &format!("{label}-wrapped"),
        &format!(
            "{prelude}\nmacro_rules! decl {{ () => {{\n{items}\n}} }}\nmacro_rules! wrap {{ () => {{ decl!(); }} }}\nwrap!();\n{main}\n"
        ),
        expected,
    );
}

/// Every irrefutable parameter-pattern family binds through the shared pattern
/// transaction: handwritten, emitted by a user macro and through a wrapper macro.
#[test]
fn tuple_reference_struct_tuple_struct_array_and_binding_mode_patterns() {
    all_forms(
        "families",
        r#"struct P { x: i32, y: i32 }
struct W(i32, i32);"#,
        r#"fn tup((a, b): (i32, i32)) -> i32 { a + b }
fn rf(&x: &i32) -> i32 { x + 1 }
fn st(P { x, y }: P) -> i32 { x * y }
fn ts(W(a, b): W) -> i32 { a - b }
fn arr([a, b]: [i32; 2]) -> i32 { a * 10 + b }
fn rfp(ref x: i32) -> i32 { *x + 100 }
fn mixed((ref mut a, mut b): (i32, i32)) -> i32 { *a += 1; b += 2; *a + b }
fn under((a, _): (i32, i32)) -> i32 { a }
fn rest(P { x, .. }: P) -> i32 { x }
fn renamed(P { x: px, y: py }: P) -> i32 { px - py }
fn nested(((a, b), [c, d]): ((i32, i32), [i32; 2])) -> i32 { a + b * 2 + c * 3 + d * 4 }"#,
        r#"fn main() {
    println!("{}", tup((1, 2)));
    println!("{}", rf(&41));
    println!("{}", st(P { x: 6, y: 7 }));
    println!("{}", ts(W(10, 4)));
    println!("{}", arr([4, 2]));
    println!("{}", rfp(5));
    println!("{}", mixed((1, 1)));
    println!("{}", under((8, 9)));
    println!("{}", rest(P { x: 10, y: 3 }));
    println!("{}", renamed(P { x: 10, y: 3 }));
    println!("{}", nested(((1, 2), [3, 4])));
}"#,
        r#"3
42
42
6
42
105
5
8
10
7
30
"#,
    );
}

/// Impl and trait methods (the receiver is argument 0), generic functions and a
/// block-local `fn`.
#[test]
fn methods_after_self_generics_and_nested_functions() {
    prints(
        "methods",
        r#"struct S { n: i32 }
struct W(i32, i32);
impl S {
    fn add(&self, (a, b): (i32, i32)) -> i32 { self.n + a + b }
    fn take(self, W(x, y): W) -> i32 { self.n * x + y }
    fn new((n, _): (i32, i32)) -> S { S { n } }
    fn refs(&mut self, &d: &i32) { self.n += d; }
}
trait T {
    fn go(&self, t: (i32, i32)) -> i32;
    fn dflt(&self, t: (i32, i32)) -> i32 { t.0 * t.1 + self.go(t) }
}
impl T for S {
    fn go(&self, (a, _): (i32, i32)) -> i32 { self.n + a }
}
fn generic<T: Copy + std::fmt::Debug>((a, b): (T, T)) -> (T, T) { (b, a) }
fn main() {
    let mut s = S { n: 5 };
    println!("{}", s.add((1, 2)));
    println!("{}", s.dflt((3, 4)));
    s.refs(&10);
    println!("{}", s.n);
    println!("{}", S::new((7, 8)).n);
    println!("{}", s.take(W(2, 3)));
    println!("{:?}", generic((1u8, 2u8)));
    println!("{:?}", generic(("a", "b")));
    fn nested((p, q): (i32, i32)) -> i32 { p - q }
    println!("{}", nested((10, 3)));
}
"#,
        r#"8
20
15
7
33
(2, 1)
("b", "a")
7
"#,
    );
}

/// The static constraint service sees the parameter pattern: an unsuffixed literal
/// combined with a binding takes the declared parameter's type.
#[test]
fn unsuffixed_literals_in_the_body_take_the_parameter_type() {
    prints(
        "literals",
        r#"fn a((x, y): (u8, u8)) -> u8 { let c = 200; let d = x + c; d.wrapping_add(y) }
fn b((x, _): (u64, u64)) -> u64 { let big = 1099511627776; x + big }
fn c(&x: &u8) -> u8 { let k = 100; k + x }
fn d((x, y): (f32, f32)) -> f32 { let h = 0.5; (x + y) * h }
fn e([a, b, c]: [i16; 3]) -> i16 { let m = 1000; a * m + b + c }
fn f(plain: i64, (x, y): (i64, i64)) -> i64 { let z = 3_000_000_000; plain + x + y + z }
fn lit((a, b): (u8, u8)) -> u8 { a + b + 250 }
fn main() {
    println!("{}", a((10, 3)));
    println!("{}", b((1, 2)));
    println!("{}", c(&7));
    println!("{}", d((1.0, 2.0)));
    println!("{}", e([1, 2, 3]));
    println!("{}", f(1, (2, 3)));
    println!("{}", lit((1, 2)));
}
"#,
        r#"213
1099511627777
107
1.5
1005
3000000006
253
"#,
    );
}

/// The bindings are visible to the whole body: closures capture them, inner `let`s
/// shadow them, `return` leaves through the parameter scope.
#[test]
fn bindings_reach_closures_shadowing_loops_early_returns_and_recursion() {
    prints(
        "body-visibility",
        r#"fn closure_cap((a, b): (i32, i32)) -> i32 {
    let f = |x: i32| x + a * b;
    f(1) + f(2)
}
fn shadow((a, b): (i32, i32)) -> i32 {
    let a = a * 100;
    let b = { let b = b + 1; b * 2 };
    a + b
}
fn early((a, b): (i32, i32)) -> i32 {
    if a > b { return a; }
    for i in 0..3 { if i == b { return i + 1000; } }
    b
}
fn plain_and_pat(x: i32, (a, b): (i32, i32), y: i32) -> i32 { x * 1000 + a * 100 + b * 10 + y }
fn adder((a, b): (i32, i32)) -> impl Fn(i32) -> i32 { move |x| x + a * b }
fn fact((n, acc): (u64, u64)) -> u64 { if n == 0 { acc } else { fact((n - 1, acc * n)) } }
fn strs((a, b): (String, String)) -> String { format!("{}{}", a, b) }
fn muts(&mut (ref mut a, b): &mut (i32, i32)) { *a += b; }
fn tick(c: &mut i32) -> (i32, i32) { *c += 1; println!("tick {}", *c); (*c, *c * 2) }
fn once((a, b): (i32, i32)) -> i32 { a + b }
fn main() {
    println!("{}", closure_cap((2, 3)));
    println!("{}", shadow((1, 2)));
    println!("{} {} {}", early((5, 1)), early((0, 2)), early((0, 9)));
    println!("{}", plain_and_pat(1, (2, 3), 4));
    println!("{}", adder((3, 4))(1));
    println!("{}", fact((10, 1)));
    println!("{}", strs((String::from("ab"), String::from("cd"))));
    let mut t = (1, 2);
    muts(&mut t);
    println!("{:?}", t);
    let mut c = 0;
    println!("{}", once(tick(&mut c)));
    println!("{}", c);
}
"#,
        r#"15
106
5 1002 9
1234
13
3628800
abcd
(3, 2)
tick 1
3
1
"#,
    );
}

/// A pattern function is an ordinary function item: callable through a value, a
/// fn pointer and a generic bound; an irrefutable enum pattern binds.
#[test]
fn pattern_functions_as_values_and_enum_single_variant() {
    prints(
        "as-values",
        r#"enum E { A(i32, i32) }
fn sw((a, b): (i32, i32)) -> (i32, i32) { (b, a) }
fn sw_sum((a, b): (i32, i32)) -> i32 { a * 100 + b }
fn call_with<F: Fn((i32, i32)) -> i32>(f: F) -> i32 { f((2, 5)) }
fn irref(E::A(a, b): E) -> i32 { a * b }
fn main() {
    println!("{}", call_with(sw_sum));
    let g = sw;
    println!("{:?}", g((1, 2)));
    let h: fn((i32, i32)) -> (i32, i32) = sw;
    println!("{:?}", h((3, 4)));
    println!("{}", irref(E::A(6, 7)));
}
"#,
        r#"205
(2, 1)
(4, 3)
42
"#,
    );
}

/// A user macro emits the parameter pattern (also through a wrapper macro and in an
/// impl block): hygiene is kept, a macro-introduced name is not the caller's, and
/// generated syntax takes the same route as handwritten.
#[test]
fn user_macro_emitting_pattern_parameters() {
    prints(
        "macro-params",
        r#"macro_rules! make_fn {
    ($name:ident) => {
        fn $name((a, b): (i32, i32)) -> i32 { a * 10 + b }
    };
}
macro_rules! make_fn_arg {
    ($name:ident, $x:ident, $y:ident, $body:expr) => {
        fn $name(($x, $y): (i32, i32)) -> i32 { $body }
    };
}
macro_rules! wrap { ($($t:tt)*) => { make_fn!(wrapped); $($t)* }; }
wrap!(fn after() -> i32 { wrapped((1, 2)) });
make_fn!(emitted);
make_fn_arg!(sum, p, q, p + q);
make_fn_arg!(hyg, a, b, { let k = 5; a * k - b });
macro_rules! shadow_caller {
    ($v:ident) => {
        fn shadowed((x, y): (i32, i32), $v: i32) -> i32 { x + y + $v }
    };
}
shadow_caller!(z);
struct S;
impl S {
    make_fn!(method);
}
fn main() {
    println!("{}", emitted((3, 4)));
    println!("{}", sum((3, 4)));
    println!("{}", hyg((3, 4)));
    println!("{}", after());
    println!("{}", shadowed((1, 2), 3));
    println!("{}", S::method((7, 8)));
}
"#,
        r#"34
7
11
12
6
78
"#,
    );
}

/// Macro calls in the body of a pattern function resolve the bindings like any body.
#[test]
fn body_macros_inside_pattern_functions() {
    prints(
        "body-macros",
        r#"struct P { x: i32, y: i32 }
fn shout((a, b): (i32, i32), s: &str) {
    println!("{} {} {}", a, b, s);
    assert_eq!(a + b, 7);
    assert!(a < b, "a={} b={}", a, b);
    let v = vec![a, b, a + b];
    println!("{:?} {}", v, format!("{}-{}", a, b));
}
fn pp(P { x, y }: P, &(m, n): &(i32, i32)) -> i32 {
    println!("pp {} {} {} {}", x, y, m, n);
    x * m + y * n
}
fn main() {
    shout((3, 4), "hi");
    println!("{}", pp(P { x: 1, y: 2 }, &(3, 4)));
}
"#,
        r#"3 4 hi
[3, 4, 7] 3-4
pp 1 2 3 4
11
"#,
    );
}

/// rustc binds each parameter as `let PAT = arg;` in order and drops them in
/// reverse, after the body's locals; a `_` inside a pattern leaves that part in
/// the argument.
#[test]
fn parameters_drop_after_the_body_in_reverse_declaration_order() {
    prints(
        "drop-order",
        r#"struct D(&'static str);
impl Drop for D {
    fn drop(&mut self) { println!("drop {}", self.0); }
}
struct W(D, D);
fn pair((a, b): (D, D)) {
    println!("body {} {}", a.0, b.0);
    let _l = D("local");
    println!("end");
}
fn under((a, _): (D, D)) { println!("body under {}", a.0); }
fn plain_then_pat(x: D, (a, b): (D, D)) { println!("body {} {} {}", x.0, a.0, b.0); }
fn pat_then_plain((a, b): (D, D), y: D) { println!("body {} {} {}", a.0, b.0, y.0); }
fn two_pats((a, b): (D, D), (c, d): (D, D)) { println!("body {} {} {} {}", a.0, b.0, c.0, d.0); }
fn by_ref(ref a: D, (ref b, c): (D, D)) { println!("body {} {} {}", a.0, b.0, c.0); }
fn moved_out((_a, b): (D, D)) -> D { println!("moving {}", b.0); b }
fn early((a, b): (D, D), go: bool) -> i32 {
    if go { return 1; }
    println!("not early {}", a.0);
    2
}
fn w(W(a, b): W) { println!("w {} {}", a.0, b.0); }
fn wildcard_first(_: D, (a, b): (D, D)) { println!("f2 {} {}", a.0, b.0); }
fn wildcard_last((a, b): (D, D), _: D) { println!("f1 {} {}", a.0, b.0); }
struct S(D);
impl S {
    fn m(self, (a, b): (D, D), _z: D) { println!("m {} {} {}", self.0.0, a.0, b.0); }
}
fn main() {
    pair((D("a"), D("b")));
    println!("--");
    under((D("a"), D("b")));
    println!("--");
    plain_then_pat(D("x"), (D("a"), D("b")));
    println!("--");
    pat_then_plain((D("a"), D("b")), D("y"));
    println!("--");
    two_pats((D("a"), D("b")), (D("c"), D("d")));
    println!("--");
    by_ref(D("r"), (D("b"), D("c")));
    println!("--");
    let r = moved_out((D("a"), D("b")));
    println!("got {}", r.0);
    drop(r);
    println!("--");
    early((D("a"), D("b")), true);
    println!("--");
    early((D("a"), D("b")), false);
    println!("--");
    w(W(D("a"), D("b")));
    println!("--");
    wildcard_first(D("u"), (D("a"), D("b")));
    println!("--");
    wildcard_last((D("a"), D("b")), D("u"));
    println!("--");
    S(D("self")).m((D("a"), D("b")), D("z"));
}
"#,
        r#"body a b
end
drop local
drop b
drop a
--
body under a
drop a
drop b
--
body x a b
drop b
drop a
drop x
--
body a b y
drop y
drop b
drop a
--
body a b c d
drop d
drop c
drop b
drop a
--
body r b c
drop c
drop b
drop r
--
moving b
drop a
got b
drop b
--
drop b
drop a
--
not early a
drop b
drop a
--
w a b
drop b
drop a
--
f2 a b
drop b
drop a
drop u
--
f1 a b
drop u
drop b
drop a
--
m self a b
drop z
drop b
drop a
drop self
"#,
    );
}

/// In a pattern function every parameter is bound in order: `&mut` reborrows,
/// trait objects, generics, `impl Fn` and `mut` parameters behave as elsewhere.
#[test]
fn plain_parameters_beside_pattern_parameters_keep_their_semantics() {
    prints(
        "plain-beside-pattern",
        r#"use std::fmt::Display;
trait Speak { fn say(&self) -> String; }
struct Dog;
impl Speak for Dog { fn say(&self) -> String { String::from("woof") } }
fn push_all(v: &mut Vec<i32>, (a, b): (i32, i32)) { v.push(a); v.push(b); v.push(v.len() as i32); }
fn show<T: Display>(t: T, (a, b): (i32, i32)) -> String { format!("{}:{}:{}", t, a, b) }
fn dynamic(s: &dyn Speak, (a, b): (i32, i32)) -> String { format!("{}{}", s.say(), a + b) }
fn boxed(s: Box<dyn Speak>, (a, _): (i32, i32)) -> String { format!("{}{}", s.say(), a) }
fn apply(f: impl Fn(i32) -> i32, (a, b): (i32, i32)) -> i32 { f(a) + f(b) }
fn counter(mut n: i32, (a, b): (i32, i32)) -> i32 { n += a; n += b; n }
fn reborrow(x: &mut i32, (a, b): (i32, i32)) { *x += a; inc(x); *x += b; }
fn inc(x: &mut i32) { *x += 100; }
fn main() {
    let mut v = vec![];
    push_all(&mut v, (7, 8));
    println!("{:?}", v);
    println!("{}", show("x", (1, 2)));
    println!("{}", dynamic(&Dog, (1, 2)));
    println!("{}", boxed(Box::new(Dog), (5, 6)));
    println!("{}", apply(|x| x * 2, (3, 4)));
    println!("{}", counter(10, (1, 2)));
    let mut z = 1;
    reborrow(&mut z, (2, 3));
    println!("{}", z);
}
"#,
        r#"[7, 8, 2]
x:1:2
woof3
woof5
14
13
106
"#,
    );
}

/// Controls: `x`, `mut x`, `_` and `self` are not pattern parameters; a function
/// with only those keeps its block-only body and frame-parameter reads.
#[test]
fn underscore_mut_and_receiver_parameters_keep_the_direct_route() {
    prints(
        "plain-controls",
        r#"struct S(i32);
impl S {
    fn a(&self, _: i32, mut k: i32) -> i32 { k += self.0; k }
    fn b(self, x: i32) -> i32 { self.0 + x }
}
fn c(mut x: i32, _: i32, y: i32) -> i32 { x += y; x }
fn main() {
    println!("{}", S(1).a(9, 2));
    println!("{}", S(5).b(6));
    println!("{}", c(1, 2, 3));
}
"#,
        r#"3
11
4
"#,
    );
}
