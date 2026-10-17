//! Whole programs reading constants and statics through the ordinary
//! `ValuePath` route: items of nested modules (selected through their own
//! module Value scope, a facet of the one Value fixed point), body-local items
//! (selected through the entered lexical scope), and aggregate constants.
//! The legacy `SourceConstant` dispatch is never reached.
use super::*;
use crate::ir::op::{SOURCE_CONSTANT_OPCODE, VALUE_PATH_OPCODE};

struct OwnedSource(std::path::PathBuf);
impl Drop for OwnedSource {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn source(text: &str) -> OwnedSource {
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "lrr-value-scopes-{}-{}.rs",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
    OwnedSource(path)
}
fn completed(text: &str, edition: u16) -> crate::engine::run::RunReport {
    let owned = source(text);
    let report = match standalone_edition(&owned.0, None, edition) {
        Ok(StandaloneOutcome::Completed(report)) => report,
        Ok(StandaloneOutcome::Panicked(report)) => {
            panic!("edition {edition} panicked: {:?}\n{text}", report.panic)
        }
        Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
        Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
    };
    assert_eq!(
        report.opcode_counters.started_by_opcode[usize::from(SOURCE_CONSTANT_OPCODE)],
        0,
        "constants are ordinary value paths\n{text}"
    );
    report
}
fn panicked(text: &str, edition: u16) {
    let owned = source(text);
    assert!(
        matches!(
            standalone_edition(&owned.0, None, edition),
            Ok(StandaloneOutcome::Panicked(_))
        ),
        "edition {edition} did not panic\n{text}"
    );
}
fn faulted(text: &str, edition: u16) -> Diagnostic {
    let owned = source(text);
    match standalone_edition(&owned.0, None, edition) {
        Err(error) => error,
        Ok(_) => panic!("edition {edition} did not fault\n{text}"),
    }
}
fn value_paths(report: &crate::engine::run::RunReport) -> u64 {
    report.opcode_counters.started_by_opcode[usize::from(VALUE_PATH_OPCODE)]
}
const EDITIONS: [u16; 2] = [2015, 2021];

#[test]
fn nested_module_constants_and_statics_select_through_their_module_scope() {
    for edition in EDITIONS {
        let report = completed(
            "mod m { pub const C: i32 = 1; pub static S: i32 = 3; pub mod n { pub const D: i32 = E + 10; const E: i32 = 1; } }\n\
             fn main() { let x: i32 = m::C; assert!(x == 1i32); assert!(m::S == 3i32); assert!(m::n::D == 11i32); }",
            edition,
        );
        assert_eq!(value_paths(&report), 3);
        // A constant read inside the nested module's own function.
        completed(
            "mod m { pub const C: i32 = 1; pub fn f() -> i32 { C + 1 } }\n\
             fn main() { assert!(m::f() == 2i32); }",
            edition,
        );
        // `pub use` re-export chains select the same declaration.
        completed(
            "mod a { pub mod b { pub const K: u8 = 9; } pub use self::b::K; }\n\
             mod c { pub use super::a::K; }\n\
             use c::K;\n\
             fn main() { assert!(K == 9u8); assert!(a::K == 9u8); assert!(c::K == 9u8); }",
            edition,
        );
        panicked(
            "mod m { pub const C: i32 = 1; } fn main() { assert!(m::C == 2i32); }",
            edition,
        );
    }
}

#[test]
fn aggregate_constants_and_statics_are_ordinary_value_paths() {
    for edition in EDITIONS {
        completed(
            "const A: [i32; 3] = [1, 2, 3]; fn main() { assert!(A[1] == 2i32); let b = A; assert!(b[2] == 3i32); }",
            edition,
        );
        completed(
            "static A: [isize; 3] = [1, 2, 3]; fn main() { assert!(A[1] == 2isize); let r = &A; assert!(r[2] == 3isize); }",
            edition,
        );
        completed(
            "const N: usize = 2; const A: [u8; N] = [7, 8]; fn main() { let x = A[N - 1]; assert!(x == 8u8); }",
            edition,
        );
        // Outside `main` and in a nested module (formerly a SourceConstant
        // "prepared source/compiler facts changed" refusal).
        completed(
            "mod m { pub const A: [u8; 2] = [7, 8]; } fn f() -> u8 { m::A[0] } fn main() { assert!(f() == 7u8); }",
            edition,
        );
        panicked(
            "const A: [i32; 2] = [1, 2]; fn main() { assert!(A[0] == 2i32); }",
            edition,
        );
    }
}

#[test]
fn body_local_constants_and_statics_follow_block_scope() {
    for edition in EDITIONS {
        // A block item is visible throughout its block, before its declaration.
        completed(
            "fn main() { let y: isize = QUUX + 1; const QUUX: isize = 5; assert!(y == 6isize); assert!(QUUX == 5isize); }",
            edition,
        );
        // An inner block item shadows the module item only inside its block.
        completed(
            "const C: i32 = 1; fn main() { { const C: i32 = 2; assert!(C == 2i32); } assert!(C == 1i32); }",
            edition,
        );
        panicked(
            "const C: i32 = 1; fn main() { { const C: i32 = 2; assert!(C == 1i32); } }",
            edition,
        );
        // Local initializers name module items and other block items.
        completed(
            "const BASE: u32 = 10;\n\
             fn main() { const A: u32 = BASE + 1; const B: u32 = A * 2; assert!(B == 22u32);\n\
             const ARR: [u8; 3] = [1, 2, 3]; assert!(ARR[2] == 3u8); }",
            edition,
        );
        completed(
            "fn f() -> i64 { const K: i64 = -4; K * 2 } fn main() { assert!(f() == -8i64); }",
            edition,
        );
        // Statics: one run-global cell, readable, borrowable and writable.
        completed(
            "fn main() { static S: i32 = 7; let x: i32 = S; assert!(x == 7i32); let r = &S; assert!(*r == 7i32); }",
            edition,
        );
        completed(
            "fn main() { static mut COUNT: u32 = 0; unsafe { COUNT += 1; } unsafe { COUNT += 2; } assert!(unsafe { COUNT } == 3u32); }",
            edition,
        );
        // The same reads nested in a standard macro and passed through a
        // user macro's `ident` fragment.
        completed(
            "fn main() { const Q: i32 = 4; assert_eq!(Q, 4i32); let a = [Q, Q + 1]; assert!(a[1] == 5i32); }",
            edition,
        );
        completed(
            "macro_rules! sq { ($x:ident) => { $x * $x } } fn main() { static S: u8 = 3; let v: u8 = sq!(S); assert!(v == 9u8); }",
            edition,
        );
    }
}

#[test]
fn body_local_items_are_not_visible_outside_their_block() {
    for edition in EDITIONS {
        // No module item of that name exists: the out-of-scope read refuses.
        let error = faulted(
            "fn main() { { const D: i32 = 2; } let x: i32 = D; }",
            edition,
        );
        assert_ne!(error.code, "LRR-E-INTERNAL-IR", "{error:?}");
        // A same-named module item is selected outside the block.
        completed(
            "const D: i32 = 1; fn main() { { const D: i32 = 2; assert!(D == 2i32); } let x: i32 = D; assert!(x == 1i32); }",
            edition,
        );
    }
}

#[test]
fn rustc_value_scope_witnesses_complete_unchanged() {
    for path in [
        "tests/ui/where-clauses/where-clause-early-bound-lifetimes.rs",
        "tests/ui/imports/reexport-from-multiple-namespaces.rs",
    ] {
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/"
        ))
        .join(path);
        let outcome = standalone_edition(&path, None, 2015);
        assert!(
            matches!(outcome, Ok(StandaloneOutcome::Completed(_))),
            "{} did not complete: {:?}",
            path.display(),
            outcome.err()
        );
    }
}

#[test]
fn associated_constants_select_through_the_member_index() {
    for edition in EDITIONS {
        // Inherent: `Type::NAME` and `Self::NAME` in the impl.
        completed(
            "struct Foo; impl Foo { const BAR: i32 = 5; fn get() -> i32 { Self::BAR } }\n\
             fn main() { let x: i32 = Foo::BAR; assert!(x == 5i32); assert!(Foo::get() == 5i32); }",
            edition,
        );
        // A trait impl's own constant.
        completed(
            "struct A; trait T { const C: u8; } impl T for A { const C: u8 = 9; }\n\
             fn main() { let x: u8 = A::C; assert!(x == 9u8); }",
            edition,
        );
        // The trait's provided default through a qualified path.
        completed(
            "struct A; trait T { const C: u8 = 3; } impl T for A {}\n\
             fn main() { let x: u8 = <A as T>::C; assert!(x == 3u8); let y: u8 = A::C; assert!(y == 3u8); }",
            edition,
        );
        // An associated initializer names its enclosing module's items.
        completed(
            "const BASE: u32 = 40; struct S; impl S { const N: u32 = BASE + 2; }\n\
             fn main() { assert!(S::N == 42u32); }",
            edition,
        );
        panicked(
            "struct Foo; impl Foo { const BAR: i32 = 5; } fn main() { assert!(Foo::BAR == 6i32); }",
            edition,
        );
    }
}
