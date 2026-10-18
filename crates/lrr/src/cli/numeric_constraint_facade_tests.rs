//! Source numeric constraints that depend on the LRR standard facade, through
//! the ordinary standalone pipeline: `for` bindings take the head's
//! `IntoIterator::Item` normalized through the member index; constants,
//! statics and trait-bound method results give their declared Types; a
//! single applicable impl selects a literal's Type where rustc's selection
//! does; a selected impl's self type and a callee's declared generic
//! structure relate literals. Each program prints what it proved (a literal
//! beyond `i32` only runs under the Type the relation states).
use super::capture_with;
use crate::{
    cli::{StandaloneOutcome, runtime_capture::Request, standalone_captured},
    core::profile::ProfileIdentity,
};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lrr-numeric-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(
    label: &str,
    source: &str,
    edition: u16,
) -> Result<StandaloneOutcome, crate::core::diagnostic::Diagnostic> {
    let temp = Temp::new(label);
    let root = temp.0.join("main.rs");
    std::fs::write(&root, source).unwrap();
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let (prepared, view, sources, mut snapshot) = capture_with(
        &root,
        false,
        edition,
        &super::ExternCrates::default(),
        crate::source::embedded_crates::standard_facade_crates(&profile),
        None,
    )
    .unwrap()
    .0;
    standalone_captured(
        &root,
        prepared,
        view,
        sources,
        &mut snapshot,
        None,
        Request {
            enabled: false,
            context: None,
        },
        None,
    )
}

fn prints(label: &str, source: &str, expected: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        match run(label, source, edition) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.output.stdout, expected, "{edition}\n{source}")
            }
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("{edition} panicked: {:?}\n{source}", report.panic)
            }
            Ok(_) => panic!("{edition} unexpected outcome\n{source}"),
            Err(error) => panic!("{edition} {error}\n{source}"),
        }
    }
}

#[test]
fn for_binding_takes_the_selected_iterator_item() {
    // `C: Iterator<Item = u32>` through the blanket `IntoIterator` impl:
    // `s += x` makes the unannotated `s` a `u32`.
    prints(
        "for-item",
        "struct C { n: u32 }
impl Iterator for C {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.n < 3 { self.n += 1; Some(self.n) } else { None }
    }
}
fn main() {
    let mut s = 0;
    for x in (C { n: 0 }) { s += x; }
    // Only a `u32` holds this sum (an `i32` literal could not).
    let big = s + 4000000000;
    println!(\"{} {}\", s, big);
}
",
        "6 4000000006\n",
    );
}

#[test]
fn for_binding_takes_a_range_element() {
    // `Range<A>: Iterator<Item = A>` through the blanket `IntoIterator`
    // impl: the binding is the range's own argument, unified with its
    // operands.
    prints(
        "for-range",
        "fn main() {
    let mut v = 0;
    for i in 0..10u64 { v += i; }
    let big = v + 5000000000;
    let mut w = 0;
    for i in 0..4 { w += i; }
    println!(\"{} {} {}\", v, big, w);
}
",
        "45 5000000045 6\n",
    );
}

#[test]
fn constants_and_statics_give_their_declared_types() {
    prints(
        "declared-items",
        "const BIG: u64 = 5;
static S: u64 = 7;
struct W;
impl W {
    const K: u64 = 9;
}
trait Foo {
    const ID: u64;
}
impl Foo for i32 {
    const ID: u64 = 3;
}
fn main() {
    let a = 4000000000 + BIG;
    assert_eq!(1, BIG - 4);
    let b = S + 4000000000;
    let c = W::K + 4000000000;
    let d = <i32 as Foo>::ID + 4000000000;
    println!(\"{} {} {} {}\", a, b, c, d);
}
",
        "4000000005 4000000007 4000000009 4000000003\n",
    );
}

#[test]
fn type_parameter_receiver_takes_its_bound_method_result() {
    prints(
        "bound-method",
        "trait Get {
    fn get(&self) -> u64;
}
struct A;
impl Get for A {
    fn get(&self) -> u64 {
        5
    }
}
fn total<T: Get>(t: T) -> u64 {
    let x = 4000000000;
    x + t.get()
}
fn main() {
    println!(\"{}\", total(A));
}
",
        "4000000005\n",
    );
}

#[test]
fn a_single_applicable_impl_selects_the_literal_type() {
    // rustc's impl candidates for `{integer}: Trait`: one impl unifies the
    // variable; several leave it to the `i32` fallback. A `Display` bound
    // (every primitive) constrains nothing.
    prints(
        "single-impl",
        "trait One {
    fn one(self);
}
impl One for u8 {
    fn one(self) {
        println!(\"one u8 {}\", self)
    }
}
trait Two {
    fn two(self);
}
impl Two for u8 {
    fn two(self) {
        println!(\"two u8 {}\", self)
    }
}
impl Two for i32 {
    fn two(self) {
        println!(\"two i32 {}\", self)
    }
}
fn by_one<T: One>(t: T) {
    t.one()
}
fn by_two<T>(t: T) where T: Two {
    t.two()
}
fn show<T: std::fmt::Display>(t: T) {
    println!(\"{}\", t);
}
fn main() {
    by_one(200);
    by_two(7);
    show(5);
    201.one();
    8.two();
}
",
        "one u8 200\ntwo i32 7\n5\none u8 201\ntwo i32 8\n",
    );
}

#[test]
fn the_selected_impl_self_type_and_expected_arguments_relate_literals() {
    // tests/ui/lint/dead-code/with-impl.rs: `impl GenericFoo<u32>` (through
    // an alias) fixes the receiver's argument; `Box::new(1234)` passed as a
    // `Box<isize>` relates its argument through the declared `Box<T>`.
    prints(
        "impl-self",
        "pub struct GenericFoo<T>(T);
type Foo = GenericFoo<u32>;
impl Foo {
    fn bar(self) -> u8 {
        0
    }
}
fn f(x: Box<isize>) -> isize {
    *x
}
fn main() {
    println!(\"{}\", GenericFoo(4000000000).bar());
    assert!(f(Box::new(1234)) == 1234);
}
",
        "0\n",
    );
}

#[test]
fn bound_methods_include_supertrait_declarations() {
    // tests/ui/traits/inheritance/auto.rs: `T: Quux` reaches `Foo::f`
    // through `trait Quux: Foo + Bar`.
    prints(
        "supertraits",
        "trait Foo {
    fn f(&self) -> u64;
}
trait Bar {
    fn g(&self) -> u64;
}
trait Quux: Foo + Bar {}
impl<T: Foo + Bar> Quux for T {}
struct A;
impl Foo for A {
    fn f(&self) -> u64 {
        10
    }
}
impl Bar for A {
    fn g(&self) -> u64 {
        20
    }
}
fn f<T: Quux>(a: &T) {
    assert_eq!(a.f(), 10);
    let big = a.g() + 4000000000;
    println!(\"{}\", big);
}
fn main() {
    f(&A);
}
",
        "4000000020\n",
    );
}

#[test]
fn annotation_leaves_the_type_service_refuses_stay_open() {
    // A qualified generic base and a trait object: the literal keeps the
    // reached annotation hint instead of a static answer.
    prints(
        "open-leaves",
        "mod m {
    pub struct W<T> {
        pub x: T,
    }
}
trait T {
    fn f(&self) -> i32;
}
struct S;
impl T for S {
    fn f(&self) -> i32 {
        4
    }
}
fn main() {
    let w: m::W<u8> = m::W { x: 1 };
    let s = S;
    let o: &dyn T = &s;
    assert!(o.f() == 4);
    println!(\"{}\", w.x);
}
",
        "1\n",
    );
}

const GENERIC_WRAPPER: &str = "struct W<T> { a: Option<T>, n: u32 }
impl<T> W<T> {
    fn new() -> W<T> { W { a: None, n: 0 } }
    fn set(&mut self, x: T) { self.a = Some(x); self.n += 1; }
    fn count(&self) -> u32 { self.n }
}
fn make<T>() -> W<T> { W { a: None, n: 0 } }
fn takes(w: &W<i64>) -> u32 { w.count() }
fn make2<T>(n: u32, m: u32) -> W<T> { W { a: None, n: n + m } }
struct M;
impl M { fn build<T>(&self, n: u32) -> W<T> { W { a: None, n } } }
enum Either<T> { Left(T), Right(u8) }
struct List<T> { value: T, next: Option<Box<List<T>>> }
";

/// A call whose result names type parameters no argument fixes (`W::new()`,
/// `Vec::new()`, `None`) takes them from the code that uses the value: the
/// declared signature of a method called on it, an assignment, a typed
/// parameter, a comparison. One shared constraint service answers all of
/// them, so the same program text runs whether the value is bound by a
/// `let`, passed on directly or built inside `assert_ne!`.
#[test]
fn open_type_arguments_are_fixed_by_later_uses() {
    for (label, body, expected) in [
        // A method whose parameter relates the receiver's argument.
        (
            "method",
            "let mut w = W::new(); w.set(7u8); println!(\"{}\", w.count());",
            "1\n",
        ),
        // The declared parameter of a free function.
        (
            "parameter",
            "let w = W::new(); println!(\"{}\", takes(&w));",
            "0\n",
        ),
        // A free generic function whose result names its parameter.
        (
            "free-fn",
            "let mut w = make(); w.set(7u8); println!(\"{}\", w.count());",
            "1\n",
        ),
        // Only unsuffixed literals reach the variable: rustc's fallback.
        (
            "fallback",
            "let mut w = W::new(); w.set(1); w.set(2); println!(\"{}\", w.count());",
            "2\n",
        ),
        // A fieldless generic variant assigned a value later.
        (
            "variant",
            "let mut x = None; if w_flag() { x = Some(1u8); } println!(\"{}\", x.is_some());",
            "true\n",
        ),
        (
            "typed-binding",
            "let x = None; let y: Option<u8> = x; println!(\"{}\", y.is_none());",
            "true\n",
        ),
        // A variant built inside the standard macro's own expansion.
        (
            "assert-ne",
            "assert_ne!(Some(1), None); println!(\"ok\");",
            "ok\n",
        ),
        // The standard library's own `Vec<T>` is ordinary source.
        (
            "vec-strings",
            "let mut v = Vec::new(); v.push(\"a\"); v.push(\"b\"); println!(\"{}\", v.len());",
            "2\n",
        ),
        (
            "vec-range",
            "let mut v = Vec::new(); for i in 0..3u8 { v.push(i); } println!(\"{}\", v.len());",
            "3\n",
        ),
        (
            "vec-parameter",
            "let mut a = Vec::new(); a.push(1); println!(\"{}\", sum(a));",
            "1\n",
        ),
        // A `for` loop over a reference reads its elements' Type.
        (
            "vec-for-ref",
            "let v = vec![1u8, 2, 3]; let mut out = Vec::new(); for x in &v { out.push(*x); } \
             let mut total = 0; for x in &out { total += *x; } println!(\"{} {}\", out.len(), total);",
            "3 6\n",
        ),
        // Patterns match the variable through the constructor's fields.
        (
            "match-pattern",
            "let mut x = None; match x { None => { x = Some(0u8); } Some(_) => {} } \
             println!(\"{}\", x.is_some());",
            "true\n",
        ),
        (
            "if-let",
            "let y = None; if let Some(v) = y { let z: u16 = v; println!(\"{}\", z); } \
             println!(\"{}\", y.is_none());",
            "true\n",
        ),
        // A constructor call whose fields do not name the type parameter.
        (
            "ctor-open",
            "let e = Either::Right(3); match e { Either::Left(x) => { let k: u32 = x; \
             println!(\"{}\", k); } Either::Right(n) => println!(\"{}\", n), }",
            "3\n",
        ),
        // A field Type built around the constructed value's own parameter.
        (
            "dependent",
            "let list = List { value: 1, next: None }; println!(\"{}\", list.value);",
            "1\n",
        ),
        (
            "tuple",
            "let mut v = Vec::new(); v.push((1u8, true)); println!(\"{}\", v.len());",
            "1\n",
        ),
        // The Type of one open value decided through another's elements.
        (
            "chained",
            "let mut names = Vec::new(); names.push(String::from(\"a\")); \
             let mut lens = Vec::new(); for n in &names { lens.push(n.len()); } \
             println!(\"{}\", lens.len());",
            "1\n",
        ),
        (
            "enumerate",
            "let mut s = Vec::new(); for (i, c) in \"ab\".chars().enumerate() { s.push((i, c)); } \
             println!(\"{}\", s.len());",
            "2\n",
        ),
        (
            "builder-chain",
            "let mut w = W::new(); w.set(5u8); w.set(6); println!(\"{}\", w.count());",
            "2\n",
        ),
        // A call with arguments, and a method call, whose results name
        // parameters their arguments do not fix.
        (
            "arguments",
            "let mut w = make2(3, 4); w.set(5u8); println!(\"{}\", w.count());",
            "8\n",
        ),
        (
            "method-result",
            "let m = M; let mut x = m.build(9); x.set(\"s\"); println!(\"{}\", x.count());",
            "10\n",
        ),
    ] {
        let source = format!(
            "{GENERIC_WRAPPER}fn w_flag() -> bool {{ true }}
fn sum(v: Vec<u8>) -> usize {{ v.len() }}
fn main() {{ {body} }}
"
        );
        prints(label, &source, expected);
    }
}

/// A literal inside a `Box` reaches its uses through the box: a method
/// receiver is stripped of references only (never of a box, which a box
/// method such as `deref` takes itself), so a chain of box methods relates
/// each level once and ends; a field access dereferences the box to its
/// content before selecting the field.
#[test]
fn numeric_demand_through_boxes_terminates_and_selects_content_fields() {
    prints(
        "box-deref-chain",
        "use std::ops::Deref;
fn main() {
    let c = Box::new(Box::new(4));
    println!(\"{}\", *c.deref().deref());
}
",
        "4\n",
    );
    prints(
        "box-fields",
        "struct P { x: i32 }
struct Q(i32);
fn main() {
    let b = Box::new(P { x: 4 });
    let q = Box::new(Q(5));
    println!(\"{} {}\", b.x, q.0);
}
",
        "4 5\n",
    );
}

#[test]
fn associated_type_projections_give_literals_the_impl_type() {
    // The literals beyond `i32` run only under `isize`/`u64`, the Types the
    // selected impls' associated types state: `<T as Get>::Value` of the
    // argument's Type, `Self::Value` of a provided method's receiver, the
    // same call emitted by a user macro, and `<I as Foo>::A` of an integer
    // variable that its one applicable impl decides.
    prints(
        "projection-literals",
        "trait Get {
    type Value;
    fn get(&self) -> &<Self as Get>::Value;
    fn grab(&self) -> &<Self as Get>::Value { self.get() }
}
struct S { x: isize }
impl Get for S {
    type Value = isize;
    fn get(&self) -> &isize { &self.x }
}
fn grab<T: Get>(x: &T) -> &<T as Get>::Value { x.get() }
macro_rules! value { ($s:expr) => { *grab(&$s) } }
trait Foo { type A; fn boo(&self) -> <Self as Foo>::A; }
struct Bar;
impl Foo for u8 { type A = u64; fn boo(&self) -> u64 { 5_000_000_000 } }
impl Foo for Bar { type A = u32; fn boo(&self) -> u32 { 7 } }
fn foo2<I: Foo>(x: I) -> <I as Foo>::A { x.boo() }
fn main() {
    let s = S { x: 5_000_000_000 };
    assert_eq!(*grab(&s), 5_000_000_000);
    assert_eq!(*s.grab(), 5_000_000_000);
    assert_eq!(value!(s), 5_000_000_000);
    let a = 42;
    assert_eq!(foo2(a), 5_000_000_000);
    assert_eq!(foo2(Bar), 7);
    println!(\"ok\");
}
",
        "ok\n",
    );
}

#[test]
fn aggregate_comparisons_type_their_literals_from_the_other_operand() {
    // `u8` values beyond `i8`'s range: the right-hand literals inside an
    // `Option`, a tuple and an array take the left operand's `u8`.
    prints(
        "aggregate-compare",
        "fn main() {
    let x: Option<u8> = Some(243);
    let y: (u8, bool) = (217, true);
    let a: [u8; 4] = [1, 2, 3, 4];
    assert_eq!(x, Some(243));
    assert_eq!(y, (217, true));
    assert_eq!(a, [1, 2, 3, 4]);
    assert!(x == Some(243));
    println!(\"{} {} {} {}\", x == Some(243), y == (217, true), a == [1, 2, 3, 4], x == Some(244));
}
",
        "true true true false\n",
    );
}

#[test]
fn box_dereference_trait_objects_and_function_items_relate_literals() {
    // Each right-hand literal beyond `i32` runs only under the Type the
    // relation states: `u64` through a `Box` dereference, through a trait
    // object's method and its one applicable impl, through a literal
    // receiver's trait method, and through a local holding a function item.
    prints(
        "box-dyn-fn-item",
        "trait Foo { fn f(&self) -> u64; fn b(&self) -> u64 { self.f() + 2 } }
impl Foo for u64 { fn f(&self) -> u64 { *self } }
fn ten() -> u64 { 10 }
fn main() {
    let b = Box::new(5);
    let sum: u64 = *b + 5_000_000_000;
    let seven = 7;
    let o: &dyn Foo = &seven;
    let bo: Box<dyn Foo> = Box::new(8);
    let rs = ten;
    assert_eq!(rs(), 10);
    assert_eq!(3.b(), 5);
    assert!(o.f() == 7);
    println!(\"{} {} {} {}\", sum, o.f() + 5_000_000_000, bo.f() + 5_000_000_000, rs() + 5_000_000_000);
}
",
        "5000000005 5000000007 5000000008 5000000010\n",
    );
}

/// A program in its handwritten form, emitted by a user macro, and emitted
/// through a wrapper macro that forwards its tokens to a second macro. The
/// same operation must take the same route in all three.
fn forms(prelude: &str, body: &str) -> [String; 3] {
    [
        format!("{prelude}\nfn main() {{ {body} }}\n"),
        format!("{prelude}\nmacro_rules! emit {{ () => {{ {body} }}; }}\nfn main() {{ emit!(); }}\n"),
        format!(
            "{prelude}\nmacro_rules! inner {{ ($($t:tt)*) => {{ $($t)* }}; }}\n\
             macro_rules! wrap {{ ($($t:tt)*) => {{ inner!($($t)*) }}; }}\n\
             fn main() {{ wrap!({body}); }}\n"
        ),
    ]
}

/// `prints` for each form of a program (all four editions each). The expected
/// output of every program is real rustc's.
fn prints_in_all_forms(label: &str, cases: &[(&str, &str, &str, &str)]) {
    for (name, prelude, body, expected) in cases {
        for (index, source) in forms(prelude, body).iter().enumerate() {
            prints(&format!("{label}-{name}-{index}"), source, expected);
        }
    }
}

/// A by-value or borrowed array iterated by `for` takes the element Type its uses state, else rustc's fallback: the array's `IntoIterator::Item` is normalized through the impls over its const-generic length.
#[test]
fn array_iteration_closes_its_literals() {
    prints_in_all_forms(
        "array_iteration_clos",
        &[
            (
                "witness-array-for",
                r##""##,
                r##"let mut total = 0; for x in [5, 1, 3] { total += x; } println!("{}", total);"##,
                "9\n",
            ),
            (
                "array-for-typed-sum",
                r##""##,
                r##"let mut total = 0u64; for x in [3_000_000_000, 1] { total += x; } println!("{}", total);"##,
                "3000000001\n",
            ),
            (
                "array-ref-for",
                r##""##,
                r##"let a = [4_000_000_000, 5]; let mut t = 0u64; for x in &a { t += *x; } println!("{}", t);"##,
                "4000000005\n",
            ),
            (
                "array-mut-for",
                r##""##,
                r##"let mut a = [0; 4]; for x in &mut a { *x += 22; } let mut t = 0u64; for x in &a { t += *x; } println!("{} {}", a[0], t);"##,
                "22 88\n",
            ),
        ],
    );
}

/// A numeric variable under the built-in operator bounds (`T: Add<Output = T>`) is left to its other relations and the fallback; a `Vec`, an array and a slice compare element to element through the references between them.
#[test]
fn operator_bounds_and_sequence_comparisons_close_literals() {
    prints_in_all_forms(
        "operator_bounds_and_",
        &[
            (
                "operator-bounds",
                r##"fn first<T: std::ops::Add<Output = T> + Copy>(a: T, _b: T) -> T { a }"##,
                r##"println!("{} {}", first(1, 2), first(3_000_000_000u64, 1));"##,
                "1 3000000000\n",
            ),
            (
                "vec-array-compare",
                r##""##,
                r##"let w = vec![5_000_000_000u64, 2]; assert_eq!(w, [5_000_000_000, 2]); assert_eq!(vec![1, 2, 3], [1, 2, 3]); println!("{}", w == vec![5_000_000_000, 2]);"##,
                "true\n",
            ),
        ],
    );
}

/// A parameter typed by `Fn*(..) -> R` sugar (a generic bound, `impl Fn`) is called with the written Types, and a closure written for it holds them in its parameters and body; a closure in a block is a closure.
#[test]
fn callable_bounds_relate_closures_and_calls() {
    prints_in_all_forms(
        "callable_bounds_rela",
        &[
            (
                "fn-bound-call",
                r##"fn call<F: Fn(u64) -> u64>(f: F) -> u64 { f(3_000_000_000) }"##,
                r##"println!("{}", call(|x| x + 1));"##,
                "3000000001\n",
            ),
            (
                "fn-bound-reference",
                r##"fn with<T, F: FnOnce(&u32) -> T>(f: F) -> T { f(&22) }"##,
                r##"println!("{}", with(|x| *x * 2));"##,
                "44\n",
            ),
            (
                "impl-fn-param",
                r##"fn hrtb(f: impl Fn(&u32) -> u32) -> u32 { f(&22) + f(&44) }"##,
                r##"println!("{}", hrtb(|x| x * 2));"##,
                "132\n",
            ),
            (
                "closure-param-from-bound",
                r##"fn each<T, F>(v: Vec<T>, mut f: F) where F: FnMut(&T) { for x in &v { f(x); } }"##,
                r##"let v = vec![1, 2, 3]; let mut odds = 0; each(v, |i| { if *i % 2 == 1 { odds += 1; } }); println!("{}", odds);"##,
                "2\n",
            ),
            (
                "closure-result-from-bound",
                r##"fn apply<T, F>(s: String, mut f: F) -> T where F: FnMut(String) -> T { f(s) }"##,
                r##"let result = apply("test".to_string(), |s| s.len()); assert_eq!(result, 4); println!("{}", result);"##,
                "4\n",
            ),
            (
                "block-closure",
                r##""##,
                r##"let f = {|i| i}; println!("{} {}", f(5), f(3_000_000_000u64));"##,
                "5 3000000000\n",
            ),
        ],
    );
}

/// `*rc` and the deref coercion of `&Box<T>` take the `Deref::Target`; indexing and slicing a vector reads its element type.
#[test]
fn dereference_relates_literals_through_smart_pointers_and_indexing() {
    prints_in_all_forms(
        "dereference_relates_",
        &[
            (
                "box-coercion",
                r##"fn get(x: &u64) -> u64 { *x }"##,
                r##"let b = Box::new(3_000_000_000); println!("{}", get(&b));"##,
                "3000000000\n",
            ),
            (
                "rc-deref",
                r##""##,
                r##"let x = std::rc::Rc::new(3_000_000_000u64); let r = std::rc::Rc::new(7); let z: u8 = *r; println!("{} {}", *x + 1, z);"##,
                "3000000001 7\n",
            ),
            (
                "vec-index",
                r##""##,
                r##"let v = vec![1, 2, 3]; let i = 1; let big: u64 = v[i] + 4_000_000_000; let w = vec![7, 8, 9]; let s = &w[1..]; let t: u64 = s[0] + 4_000_000_000; println!("{} {}", big, t);"##,
                "4000000002 4000000008\n",
            ),
        ],
    );
}

/// A tuple receiver selects its impl; a parameter's associated-type binding (`Iterator<Item = u64>`) relates the variable inside the instantiated parameter, and a plain bound on it states nothing.
#[test]
fn receivers_and_parameter_bounds_relate_literals() {
    prints_in_all_forms(
        "receivers_and_parame",
        &[
            (
                "tuple-receiver",
                r##"trait Car { fn car(&self) -> u64; } impl Car for (u64, u8) { fn car(&self) -> u64 { self.0 } }"##,
                r##"let p = (3_000_000_000, 7); println!("{}", p.car());"##,
                "3000000000\n",
            ),
            (
                "binding-bound",
                r##"fn pairwise<T: Iterator<Item = u64>>(t: T) -> u64 { let mut s = 0; for x in t { s += x; } s }"##,
                r##"let v = vec![3_000_000_000, 1]; println!("{}", pairwise(v.into_iter()));"##,
                "3000000001\n",
            ),
            (
                "binding-bound-ref",
                r##"fn total<'a, I>(it: I) -> u64 where I: Iterator<Item = &'a u64> { let mut s = 0; for x in it { s += *x; } s }"##,
                r##"println!("{}", total([3_000_000_000, 5].iter()));"##,
                "3000000005\n",
            ),
            (
                "plain-bound-inside",
                r##"trait Mark {} impl Mark for &i32 {} fn need<T: Mark>(_: T) {}"##,
                r##"let x = 22; need(&x); println!("{}", x);"##,
                "22\n",
            ),
        ],
    );
}

/// The same loop nested in a standard macro's argument (an expression block
/// inside `println!`) takes the route the statement form takes.
#[test]
fn array_iteration_nested_in_a_standard_macro() {
    prints(
        "array-for-println",
        "fn main() { println!(\"{}\", { let mut total = 0; for x in [5, 1, 3] { total += x; } total }); }\n",
        "9\n",
    );
}

/// A result-only generic call whose declared result names its parameter
/// through a projection (`TryInto::try_into -> Result<U, <U as
/// TryFrom<T>>::Error>`, `str::parse -> Result<F, <F as FromStr>::Err>`)
/// takes the parameter later code decides as an explicit argument, exactly
/// as if written as a turbofish; the existing projection normalization
/// (which selects the impl by the trait's arguments) then closes the result.
/// The hand-written, user-macro, wrapper-macro and standard-macro-nested
/// forms are one route. Expected output verified with rustc 1.99.0.
#[test]
fn decided_generic_arguments_close_projected_results() {
    for (label, source, expected) in [
        (
            "try-into",
            "fn main() { let n = 200u16; let x: u8 = n.try_into().unwrap(); println!(\"{}\", x); }",
            "200\n",
        ),
        (
            "try-into-annotated",
            "use std::num::TryFromIntError;
fn main() {
    let r: Result<u8, TryFromIntError> = 300u16.try_into();
    println!(\"{}\", r.is_err());
}",
            "true\n",
        ),
        (
            "try-into-macros",
            "macro_rules! narrow { ($e:expr) => { $e.try_into().unwrap() } }
macro_rules! outer { ($e:expr) => { narrow!($e) } }
fn main() {
    let n = 200u16;
    let a: u8 = narrow!(n);
    let b: u8 = outer!(n);
    println!(\"{} {}\", a, b);
}",
            "200 200\n",
        ),
        (
            "try-into-std-macro",
            "fn main() {
    let n = 200u16;
    assert_eq!(n.try_into().ok(), Some(200u8));
    println!(\"{}\", { let x: u8 = n.try_into().unwrap(); x });
}",
            "200\n",
        ),
        (
            "parse",
            "fn main() { let x: i32 = \"42\".parse().unwrap(); let y: u64 = \"9\".parse().unwrap(); \
             println!(\"{} {}\", x, y); }",
            "42 9\n",
        ),
        (
            "parse-macros",
            "macro_rules! num { ($s:expr) => { $s.parse().unwrap() } }
fn main() {
    let a: i32 = num!(\"5\");
    assert_eq!(\"7\".parse().ok(), Some(7i32));
    println!(\"{}\", a);
}",
            "5\n",
        ),
        // A parameter the call's own arguments fix is not decided here.
        (
            "argument-fixed",
            "fn takes(x: u8) -> u8 { x + 1 }
fn main() { println!(\"{}\", takes(\"5\".parse().unwrap())); }",
            "6\n",
        ),
        (
            "default-and-collect",
            "fn main() {
    let m: u64 = Default::default();
    let v: Vec<u8> = (1..4u8).collect();
    let t: u32 = [1u32, 2].iter().sum();
    println!(\"{} {} {}\", m, v.len(), t);
}",
            "0 3 3\n",
        ),
    ] {
        prints(label, source, expected);
    }
}
