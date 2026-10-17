//! Implicit `&mut` reborrows at coercion sites, through the ordinary local
//! and parameter reads, handwritten and emitted by a user macro.
use super::*;

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
        "lrr-reborrow-{}-{}.rs",
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
fn outcome(text: &str, edition: u16) -> Result<StandaloneOutcome, Diagnostic> {
    let owned = source(text);
    standalone_edition(&owned.0, None, edition)
}
fn completed(text: &str) {
    for edition in [2015, 2021] {
        match outcome(text, edition) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("edition {edition} panicked: {:?}\n{text}", report.panic)
            }
            Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
            Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
        }
    }
}

#[test]
fn mutable_reference_locals_reborrow_at_reference_parameters() {
    completed(
        "fn inc(x: &mut i32) { *x += 1; }\n\
         fn get(x: &i32) -> i32 { *x }\n\
         fn main() { let mut n = 0; let r = &mut n; inc(r); inc(r); assert!(get(r) == 2); *r += 1; assert!(n == 3); }",
    );
    // Emitted by a user macro: the same ordinary local read reborrows.
    completed(
        "fn inc(x: &mut i32) { *x += 1; }\n\
         macro_rules! twice { ($f:ident, $r:ident) => { $f($r); $f($r); } }\n\
         fn main() { let mut n = 0; let r = &mut n; twice!(inc, r); assert!(*r == 2); }",
    );
}

#[test]
fn mutable_reference_parameters_reborrow_when_forwarded() {
    completed(
        "fn inc(x: &mut i32) { *x += 1; }\n\
         fn twice(x: &mut i32) { inc(x); inc(x); *x += 10; }\n\
         fn main() { let mut n = 0; twice(&mut n); assert!(n == 12); }",
    );
}

#[test]
fn mutable_slice_references_reborrow_with_their_element_window() {
    completed(
        "fn first(s: &[i32]) -> i32 { s[0] }\n\
         fn bump(s: &mut [i32]) { s[0] += 1; }\n\
         fn twice(s: &mut [i32]) { bump(s); bump(s); }\n\
         fn main() { let mut a = [1i32, 2]; let r = &mut a; bump(r); twice(r); assert!(first(r) == 4); r[1] = 5; assert!(a[1] == 5); }",
    );
    completed(
        "fn bump(s: &mut [i32]) { s[0] += 1; }\n\
         fn main() { let mut a = [1i32, 2]; let r: &mut [i32] = &mut a; bump(r); bump(r); assert!(r[0] == 3); }",
    );
}

#[test]
fn mutable_reference_typed_lets_and_fields_reborrow_but_untyped_lets_move() {
    completed(
        "struct W<'a> { r: &'a mut i32 }\n\
         fn main() { let mut n = 0; let r = &mut n; { let s: &mut i32 = r; *s += 1; } { let w = W { r: r }; *w.r += 1; } *r += 1; assert!(n == 3); }",
    );
    // An unannotated `let` (no reference expectation) moves the reference:
    // the moved local's lease no longer reaches its referent (rustc rejects
    // the later use statically; LRR faults at the reached use).
    let moved = outcome(
        "fn main() { let mut n = 0; let r = &mut n; let s = r; *s += 1; *r += 1; }",
        2021,
    );
    assert!(
        matches!(&moved, Err(error)
            if matches!(error.code, "LRR-E-USE-AFTER-MOVE" | "LRR-E-STALE-HANDLE")),
        "{:?}",
        moved.as_ref().err()
    );
}
