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
