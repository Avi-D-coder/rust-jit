//! Builtin `#[derive(..)]` through the ordinary standalone pipeline: the
//! library's `#[rustc_builtin_macro]` derive definitions expand in process and
//! their impls join the generated-item route procedural derive output uses
//! (member index, generated bodies, ordinary calls, patterns and
//! constructors). The derived, handwritten and user-macro impls below run the
//! same operations.
use super::capture_with;
use crate::{
    cli::{StandaloneOutcome, runtime_capture::Request, standalone_captured, standalone_edition},
    core::profile::ProfileIdentity,
};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lrr-derive-{label}-{}-{}",
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

/// Runs `source` with the LRR standard facade crates (the `Clone`/`PartialEq`
/// impls of primitives the derived bodies call live there).
fn prints(label: &str, source: &str, editions: &[u16], expected: &str) {
    for &edition in editions {
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
        match standalone_captured(
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
        ) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.output.stdout, expected, "{edition}\n{source}");
                eprintln!(
                    "actual standalone constructor/derive control {label},edition{edition},operations{},completed{}",
                    report.opcode_counters.operations_started,
                    report.opcode_counters.operations_completed
                );
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
fn rustc_unchanged_copy_derive_fixtures_complete_without_the_facade() {
    // `#[derive(Copy, Clone)]` makes the record Copy: the moved-from local is
    // still readable after `let mut y = x;` and after by-value calls.
    for fixture in ["expr/copy.rs", "structs-enums/rec.rs"] {
        let path = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/"
        ))
        .join(fixture);
        match standalone_edition(&path, None, 2015) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(_) => panic!("{fixture} did not complete"),
            Err(error) => panic!("{fixture}: {error}"),
        }
    }
}

#[test]
fn derived_impls_run_like_handwritten_and_user_macro_impls() {
    prints(
        "matrix",
        "struct H { a: i32 }
impl Clone for H { fn clone(&self) -> H { H { a: Clone::clone(&self.a) } } }
impl PartialEq for H { fn eq(&self, other: &H) -> bool { self.a == other.a } }
#[derive(Clone, PartialEq)]
struct D { a: i32 }
struct M { a: i32 }
macro_rules! derive_like {
    ($name:ident) => {
        impl Clone for $name { fn clone(&self) -> $name { $name { a: Clone::clone(&self.a) } } }
        impl PartialEq for $name { fn eq(&self, other: &$name) -> bool { self.a == other.a } }
    };
}
derive_like!(M);
fn main() {
    let h = H { a: 1 };
    let d = D { a: 1 };
    let m = M { a: 1 };
    assert!(h.clone() == h);
    assert!(d.clone() == d);
    assert!(m.clone() == m);
    assert!(D { a: 2 } != d);
    println!(\"{} {} {}\", h.clone().a, d.clone().a, m.clone().a);
}
",
        &[2015, 2021],
        "1 1 1\n",
    );
}

#[test]
fn derived_clone_partial_eq_and_default_cover_struct_enum_and_generic_shapes() {
    prints(
        "shapes",
        "#[derive(Clone, PartialEq)]
struct Inner { a: i32 }
#[derive(Clone, PartialEq)]
struct P { x: i32, s: Inner }
#[derive(Clone, PartialEq)]
enum E { A, B(i32, Inner), C { y: u8 } }
#[derive(Clone)]
struct G<T> { v: T }
#[derive(Clone)]
struct W<T>(T);
#[derive(Clone, Copy)]
struct U;
#[derive(Default)]
struct Z { n: i32, b: bool }
#[derive(Default, PartialEq)]
enum F { #[default] First, Second }
fn main() {
    let p = P { x: 1, s: Inner { a: 5 } };
    let q = p.clone();
    assert!(p == q);
    assert!(q.s.a == 5);
    let e = E::B(2, Inner { a: 6 });
    assert!(e.clone() == e);
    assert!(E::C { y: 1 }.clone() == E::C { y: 1 });
    assert!(E::A.clone() != E::C { y: 1 });
    assert!(E::B(2, Inner { a: 6 }) != E::B(2, Inner { a: 7 }));
    assert!(G { v: 3u8 }.clone().v == 3);
    assert!(W(4i64).clone().0 == 4);
    let _u = U.clone();
    let z: Z = Default::default();
    assert!(z.n == 0 && !z.b);
    let f: F = Default::default();
    assert!(f == F::First && f != F::Second);
    println!(\"ok\");
}
",
        &[2021],
        "ok\n",
    );
}

#[test]
fn unchanged_copy_contract_survives_generated_derives_all_origins_and_editions() {
    const ORIGINAL: &str =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/expr/copy.rs");
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nemit! {{ {ORIGINAL} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nmacro_rules! wrapper {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }}\nwrapper! {{ {ORIGINAL} }}"
    );
    for text in [ORIGINAL, &emitted, &wrapped] {
        prints("stock-copy-origins", text, &[2015, 2018, 2021, 2024], "");
    }
}

// LRR route control for the constructor prerequisite exposed by unchanged
// derive-clone-basic.rs. This does not replace that original or prove derives.
#[test]
fn explicit_constructor_arguments_share_context_and_activation_across_origins() {
    prints(
        "constructor-type-arguments",
        r#"
enum G<T, U> { A(T), B(U), C }
struct S<T>(T);
trait Project { type Value; }
impl Project for u8 { type Value = u32; }
impl Project for u16 { type Value = u64; }
struct Projected<T: Project>(T::Value);
fn construct<T: Project>(value: T::Value) { Projected::<T>(value); }
macro_rules! emit { ($value:expr) => { $value }; }
macro_rules! wrapper { ($value:expr) => { emit!($value) }; }
fn touch(n: &mut i32) -> u8 { *n += 1; 3 }
fn make<T>(value: T) -> G<T, bool> { G::A::<T, bool>(value) }
fn take_u8(_value: u8) {}
fn take_u16(_value: u16) {}
fn main() {
    let mut n = 0i32;
    let ordinary = G::A::<u8, bool>(touch(&mut n));
    match ordinary { G::A(value) => take_u8(value), _ => panic!() }
    let emitted = emit!(G::A::<u8, bool>(touch(&mut n)));
    match emitted { G::A(value) => take_u8(value), _ => panic!() }
    let wrapped = wrapper!(G::<u8, bool>::A(touch(&mut n)));
    match wrapped { G::A(value) => take_u8(value), _ => panic!() }
    assert!(match wrapper!(G::A::<u8, bool>(touch(&mut n))) {
        G::A(value) => value == 3u8, _ => false
    });
    assert!(n == 4i32);
    let unit = G::<u16, bool>::C;
    match unit { G::C => (), _ => panic!() }
    let S(value) = S::<u16>(9);
    take_u16(value);
    let first = make::<u8>(1);
    match first { G::A(value) => take_u8(value), _ => panic!() }
    let second = make::<u16>(2);
    match second { G::A(value) => take_u16(value), _ => panic!() }
    // The same discarded constructor site has no inferred result context;
    // each activation must bind fields using its own genuine T application.
    construct::<u8>(3_u32);
    construct::<u16>(4_u64);
    // Empty turbofish preserves contextual inference, as in the unchanged
    // rustc empty-generic-brackets-equiv.rs witness. This compound field
    // additionally needs the retained context before it can be bound.
    let _: Projected<u8> = Projected::<>(5_u32);
}

"#,
        &[2015, 2018, 2021, 2024],
        "",
    );
}

#[test]
fn unchanged_associated_constructor_uses_reached_source_arguments_all_origins_and_editions() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/associated-types/assoc-type-in-struct-constructor.rs"
    );
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} emit! {{ {ORIGINAL} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} macro_rules! wrapper {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }} wrapper! {{ {ORIGINAL} }}"
    );
    for (origin, text) in [ORIGINAL, &emitted, &wrapped].into_iter().enumerate() {
        prints(
            &format!("stock-associated-constructor-origin{origin}"),
            text,
            &[2015, 2018, 2021, 2024],
            "",
        );
    }
}
