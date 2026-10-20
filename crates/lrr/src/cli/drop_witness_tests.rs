//! Whole programs whose values have user `Drop` impls: drop glue calls the
//! selected `<T as Drop>::drop` as an ordinary call, in rustc's drop order,
//! for every cleanup owner (scope exit, return, assignment, temporaries,
//! moved arguments, panic unwinding) and for nested values. Expected outputs
//! are rustc's (`rustc 1.99.0` semantics; drop order is stable since RFC 1857).
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
        "lrr-drop-witness-{}-{}.rs",
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
const D: &str = "struct D(i32);
impl Drop for D {
    fn drop(&mut self) {
        println!(\"drop {}\", self.0);
    }
}
";
const EDITIONS: [u16; 2] = [2015, 2021];

#[test]
fn upstream_union_replacement_uses_common_cleanup_all_editions_and_origins() {
    let original = include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/union/union-drop-assign.rs");
    // Crate attributes remain outside item-producing macros. Original bodies
    // and assertions are retained; imported originals themselves are unchanged.
    let attrs = "#![allow(unused_assignments, static_mut_refs)]\n";
    let payload = original.replace("#![allow(unused_assignments)]", "")
        .replace("#![allow(static_mut_refs)]", "");
    let emitted = format!("{attrs}macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} emit!{{{payload}}}");
    let wrapped = format!("{attrs}macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} macro_rules! wrap {{($($t:tt)*)=>{{emit!{{$($t)*}}}}}} wrap!{{{payload}}}");
    let nested = original.replace("*u.a = S;", "assert!({ *u.a = S; true });");
    assert_ne!(nested, original);
    for edition in [2015, 2018, 2021, 2024] {
        for (origin, text) in [original, &emitted, &wrapped, &nested].into_iter().enumerate() {
            let report = match super::crate_graph::format_trait_facade_tests::run(
                "upstream-union-replacement", text, edition,
            ) {
                Ok(StandaloneOutcome::Completed(report)) => report,
                Err(error) => panic!("edition={edition} origin={origin}: {error:?}"),
                Ok(_) => panic!("edition={edition} origin={origin}: not completed"),
            };
            assert!(report.output.is_empty());
            assert!(report.opcode_counters.started_by_opcode[16] >= 1);
            assert!(report.opcode_counters.started_by_opcode[34] >= 1);
            eprintln!("union cleanup edition={edition} origin={origin}: ordinary local/place assignments");
        }
    }
}

fn completed(body: &str, expected: &str) {
    let text = format!("{D}{body}");
    for edition in EDITIONS {
        let owned = source(&text);
        match standalone_edition(&owned.0, None, edition) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.output.stdout, expected, "edition {edition}\n{text}")
            }
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("edition {edition} panicked: {:?}\n{text}", report.panic)
            }
            Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
            Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
        }
    }
}
fn panicked(body: &str, message: &str, expected: &str) {
    let text = format!("{D}{body}");
    for edition in EDITIONS {
        let owned = source(&text);
        match standalone_edition(&owned.0, None, edition) {
            Ok(StandaloneOutcome::Panicked(report)) => {
                assert_eq!(report.panic.description(), message, "edition {edition}\n{text}");
                assert_eq!(report.output.stdout, expected, "edition {edition}\n{text}");
            }
            Ok(StandaloneOutcome::Completed(_)) => {
                panic!("edition {edition} completed\n{text}")
            }
            Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
            Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
        }
    }
}

#[test]
fn scope_exit_drops_locals_in_reverse_declaration_order() {
    completed(
        "fn main() { let _a = D(1); let _b = D(2); { let _c = D(3); } }",
        "drop 3\ndrop 2\ndrop 1\n",
    );
}

#[test]
fn struct_fields_drop_in_declaration_order_and_tuple_elements_in_order() {
    completed(
        "struct Pair { a: D, b: D }
fn main() {
    let _p = Pair { a: D(1), b: D(2) };
    let _t = (D(3), D(4));
    println!(\"end\");
}",
        "end\ndrop 3\ndrop 4\ndrop 1\ndrop 2\n",
    );
}

#[test]
fn a_drop_type_runs_its_own_drop_before_its_fields() {
    completed(
        "struct Holder(D, D);
impl Drop for Holder {
    fn drop(&mut self) {
        println!(\"holder {} {}\", (self.0).0, (self.1).0);
    }
}
struct Plain { h: Holder, d: D }
fn main() {
    let _x = Plain { h: Holder(D(1), D(2)), d: D(3) };
}",
        "holder 1 2\ndrop 1\ndrop 2\ndrop 3\n",
    );
}

#[test]
fn shadowed_bindings_drop_at_scope_exit_newest_first() {
    completed(
        "fn main() { let x = D(1); let x = D(2); let _ = &x; println!(\"end\"); }",
        "end\ndrop 2\ndrop 1\n",
    );
}

#[test]
fn assignment_drops_the_old_value_at_the_assignment() {
    completed(
        "fn main() {
    let mut d = D(1);
    println!(\"before\");
    d = D(2);
    println!(\"after {}\", d.0);
}",
        "before\ndrop 1\nafter 2\ndrop 2\n",
    );
}

#[test]
fn field_and_dereferenced_assignment_drop_the_old_value_at_the_assignment() {
    completed(
        "struct S { d: D, n: i32 }
fn set(r: &mut D) {
    *r = D(9);
    println!(\"set\");
}
fn main() {
    let mut s = S { d: D(1), n: 0 };
    s.d = D(2);
    println!(\"field {}\", s.n);
    let mut d = D(3);
    set(&mut d);
    let mut t = (D(4), D(5));
    t = (D(6), D(7));
    println!(\"end {}\", d.0);
}",
        "drop 1\nfield 0\ndrop 3\nset\ndrop 4\ndrop 5\nend 9\ndrop 6\ndrop 7\ndrop 9\ndrop 2\n",
    );
}

#[test]
fn break_and_continue_leave_their_scopes_through_the_same_glue() {
    completed(
        "fn main() {
    let mut i: i32 = 0;
    while i < 3 {
        let _d = D(i);
        i = i + 1;
        if i == 2 {
            continue;
        }
        println!(\"body {}\", i);
    }
    'outer: loop {
        let _a = D(10);
        loop {
            let _b = D(11);
            break 'outer;
        }
    }
    let got = loop {
        let _x = D(12);
        break D(13);
    };
    println!(\"got {}\", got.0);
}",
        "body 1\ndrop 0\ndrop 1\nbody 3\ndrop 2\ndrop 11\ndrop 10\ndrop 12\ngot 13\ndrop 13\n",
    );
}

#[test]
fn statement_temporaries_drop_at_the_end_of_their_statement() {
    completed(
        "fn main() { let _a = D(1); D(4); println!(\"next\"); }",
        "drop 4\nnext\ndrop 1\n",
    );
}

#[test]
fn early_return_drops_the_entered_locals() {
    completed(
        "fn f(early: bool) {
    let _a = D(10);
    if early {
        return;
    }
    let _b = D(11);
}
fn main() {
    f(true);
    println!(\"--\");
    f(false);
}",
        "drop 10\n--\ndrop 11\ndrop 10\n",
    );
}

#[test]
fn moved_argument_drops_at_callee_exit_and_returned_value_in_the_caller() {
    completed(
        "fn take(_d: D) {
    println!(\"in take\");
}
fn make(n: i32) -> D {
    let d = D(n);
    d
}
fn main() {
    take(D(5));
    println!(\"after take\");
    let m = make(7);
    let moved = m;
    println!(\"made {}\", moved.0);
}",
        "in take\ndrop 5\nafter take\nmade 7\ndrop 7\n",
    );
}

#[test]
fn drop_body_uses_ordinary_methods_on_its_receiver() {
    completed(
        "struct C(i32);
impl C {
    fn twice(&self) -> i32 {
        self.0 * 2
    }
}
impl Drop for C {
    fn drop(&mut self) {
        self.0 += 1;
        println!(\"c {}\", self.twice());
    }
}
fn main() {
    let _c = C(20);
}",
        "c 42\n",
    );
}

#[test]
fn values_emitted_by_a_user_macro_use_the_same_witness() {
    // The same `let` and block-tail temporary, handwritten and emitted by a
    // user macro, reach one drop glue and one selected `Drop::drop`.
    completed(
        "macro_rules! make {
    ($e:expr) => {
        let _v = $e;
    };
}
fn main() {
    {
        make!(D(9));
    }
    {
        let _v = D(6);
    }
    let _d = D(8);
    println!(\"{}\", { let _t = D(7); 1 });
}",
        "drop 9\ndrop 6\ndrop 7\n1\ndrop 8\n",
    );
}

#[test]
fn match_scrutinee_bindings_and_arrays_drop_through_the_same_glue() {
    completed(
        "enum E { A(D), B(i32) }
fn main() {
    let d = D(1);
    match d {
        D(n) => println!(\"n {}\", n),
    }
    let _a = [D(2), D(3)];
    let _x = E::A(D(4));
    let _y = E::B(5);
    let mut i: i32 = 0;
    while i < 2 {
        let _w = D(10 + i);
        i = i + 1;
    }
    println!(\"end\");
}",
        "n 1\ndrop 10\ndrop 11\nend\ndrop 4\ndrop 2\ndrop 3\ndrop 1\n",
    );
}

#[test]
fn a_drop_body_may_call_functions_that_drop_their_own_locals() {
    completed(
        "fn helper(n: i32) {
    let _h = D(n * 10);
}
struct W(i32);
impl Drop for W {
    fn drop(&mut self) {
        helper(self.0);
        println!(\"w {}\", self.0);
    }
}
fn main() {
    let _w = W(4);
}",
        "drop 40\nw 4\n",
    );
}

#[test]
fn panic_unwinding_drops_every_live_local() {
    panicked(
        "fn inner() {
    let _c = D(3);
    panic!(\"boom\");
}
fn main() {
    let _a = D(1);
    let _b = D(2);
    inner();
}",
        "boom",
        "drop 3\ndrop 2\ndrop 1\n",
    );
}

#[test]
fn a_panicking_drop_unwinds_through_the_remaining_owners() {
    panicked(
        "struct P(D);
impl Drop for P {
    fn drop(&mut self) {
        panic!(\"in drop\");
    }
}
fn main() {
    let _a = D(1);
    {
        let _p = P(D(2));
        println!(\"inner\");
    }
    println!(\"unreached\");
}",
        "in drop",
        "inner\ndrop 2\ndrop 1\n",
    );
}

#[test]
fn a_panicking_drop_in_return_field_and_break_cleanup_continues_unwinding() {
    let p = "struct P(i32);
impl Drop for P {
    fn drop(&mut self) {
        panic!(\"p {}\", self.0);
    }
}
";
    // Return cleanup: the scalar result is abandoned; `_a` and the caller's
    // `_m` still drop. Drop glue: the fields after the panicking one drop.
    // Break cleanup: `_d` (declared later) drops before `_p` panics.
    for (body, message, expected) in [
        (
            "fn f() -> i32 { let _a = D(1); let _p = P(2); 5 }
fn main() { let _m = D(0); let x = f(); println!(\"{}\", x); }",
            "p 2",
            "drop 1\ndrop 0\n",
        ),
        (
            "struct H(D, P, D);
fn main() { let _h = H(D(1), P(8), D(2)); }",
            "p 8",
            "drop 1\ndrop 2\n",
        ),
        (
            "fn main() { let _m = D(0); loop { let _p = P(6); let _d = D(7); break; } }",
            "p 6",
            "drop 7\ndrop 0\n",
        ),
        (
            "fn main() { let _m = D(0); P(3); println!(\"unreached\"); }",
            "p 3",
            "drop 0\n",
        ),
    ] {
        panicked(&format!("{p}{body}"), message, expected);
    }
}

#[test]
fn a_drop_panicking_during_unwinding_is_a_double_panic() {
    let text = format!(
        "{D}struct P;
impl Drop for P {{
    fn drop(&mut self) {{
        panic!(\"second\");
    }}
}}
fn main() {{
    let _p = P;
    panic!(\"first\");
}}"
    );
    for edition in EDITIONS {
        let owned = source(&text);
        match standalone_edition(&owned.0, None, edition) {
            Err(error) => assert_eq!(error.code, "LRR-E-DOUBLE-PANIC", "{error:?}"),
            Ok(_) => panic!("edition {edition}: a panic during unwinding did not abort"),
        }
    }
}
