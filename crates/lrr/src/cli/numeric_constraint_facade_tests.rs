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
