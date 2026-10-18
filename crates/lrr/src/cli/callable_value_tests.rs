//! Whole programs reading constants, statics and function items as values
//! and creating, capturing and calling closures, through the ordinary
//! pipeline (value paths, the call boundary, closure frames over the
//! creating body). Unchanged rustc originals plus LRR route, laziness,
//! macro-origin and refusal controls.
use super::*;
use crate::ir::op::MAKE_CLOSURE_OPCODE;

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
        "lrr-callable-values-{}-{}.rs",
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
    match standalone_edition(&owned.0, None, edition) {
        Ok(StandaloneOutcome::Completed(report)) => report,
        Ok(StandaloneOutcome::Panicked(report)) => {
            panic!("edition {edition} panicked: {:?}\n{text}", report.panic)
        }
        Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
        Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
    }
}
fn panicked(text: &str, edition: u16) -> String {
    let owned = source(text);
    match standalone_edition(&owned.0, None, edition) {
        Ok(StandaloneOutcome::Panicked(report)) => report.panic.description().to_string(),
        _ => panic!("edition {edition} did not panic\n{text}"),
    }
}
fn closures_made(report: &crate::engine::run::RunReport) -> u64 {
    report.opcode_counters.started_by_opcode[usize::from(MAKE_CLOSURE_OPCODE)]
}
const EDITIONS: [u16; 2] = [2015, 2021];

#[test]
fn constants_statics_and_function_items_are_ordinary_value_items() {
    const SOURCE: &str = "
        const C: i32 = 3;
        static S: u8 = 2 + 3;
        fn double(x: i32) -> i32 { x * 2 }
        fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
        fn main() {
            let x = C;
            assert!(x == 3);
            let r = match 3_i32 { C => 1, _ => 2 };
            assert!(r == 1);
            assert!(S as u32 * 2 == 10);
            let f = double;
            assert!(f(4) == 8);
            let g = f;
            assert!(g(1) == 2 && f(2) == 4);
            assert!(apply(double, 5) == 10);
        }
    ";
    for edition in EDITIONS {
        completed(SOURCE, edition);
    }
}

#[test]
fn static_reads_and_places_share_one_value_path_route() {
    // A static read by value and a borrow of its place both select the same
    // Value item through the ordinary value path.
    for edition in EDITIONS {
        completed(
            "static S: i32 = 6;\nfn main() { let x = S; let r = &S; assert!(*r == x); }\n",
            edition,
        );
    }
}

#[test]
fn closures_capture_by_shared_unique_and_value_capture() {
    const SOURCE: &str = "
        fn main() {
            let y = 5;
            let add = |x: i32| x + y;
            assert!(add(1) == 6);
            let mut count = 0;
            let mut bump = || count += 1;
            bump();
            bump();
            assert!(count == 2);
            let k = 10;
            let times = move |x: i32| x * k;
            let copy = times;
            assert!(times(2) == 20 && copy(3) == 30);
            let a = 3;
            let outer = |x: i32| {
                let inner = |z: i32| z + a;
                inner(x) * 2
            };
            assert!(outer(1) == 8);
            assert!((|p: i32, q: i32| p * q)(3, 4) == 12);
        }
    ";
    for edition in EDITIONS {
        let report = completed(SOURCE, edition);
        assert!(closures_made(&report) >= 6);
    }
}

#[test]
fn closures_cross_the_ordinary_call_boundary_into_generic_callees() {
    const SOURCE: &str = "
        fn apply<F: Fn(i32) -> i32>(f: F, x: i32) -> i32 { f(x) }
        fn twice<F: FnMut()>(mut f: F) { f(); f(); }
        fn make_adder(k: i32) -> impl Fn(i32) -> i32 { move |x| x + k }
        fn compose<F: Fn(i32) -> i32, G: Fn(i32) -> i32>(f: F, g: G) -> impl Fn(i32) -> i32 {
            move |x| g(f(x))
        }
        struct Scale { n: i32 }
        impl Scale {
            fn by(&self, k: i32) -> i32 { let f = |x: i32| x * self.n + k; f(2) }
        }
        fn main() {
            let y = 2;
            assert!(apply(|x: i32| x * y, 3) == 6);
            let mut n = 0;
            twice(|| n += 1);
            assert!(n == 2);
            let add2 = make_adder(2);
            assert!(add2(3) == 5);
            let h = compose(|x| x + 1, |x| x * 10);
            assert!(h(2) == 30);
            assert!(Scale { n: 3 }.by(1) == 7);
        }
    ";
    for edition in EDITIONS {
        completed(SOURCE, edition);
    }
}

#[test]
fn macro_emitted_and_handwritten_closures_share_one_route() {
    const HANDWRITTEN: &str = "
        fn main() {
            let k = 4;
            let f = move |x: i32| x + k;
            assert_eq!(f(1), 5);
        }
    ";
    const EMITTED: &str = "
        macro_rules! adder { ($k:expr) => { move |x: i32| x + $k } }
        fn main() {
            let k = 4;
            let f = adder!(k);
            assert_eq!(f(1), 5);
        }
    ";
    for edition in EDITIONS {
        let handwritten = completed(HANDWRITTEN, edition);
        let emitted = completed(EMITTED, edition);
        assert_eq!(closures_made(&handwritten), 1);
        assert_eq!(closures_made(&emitted), 1);
    }
}

#[test]
fn closure_bodies_stay_cold_until_called_and_panics_unwind_their_frames() {
    // An uncalled function's closure and a created-but-uncalled closure keep
    // their unsupported bodies cold; only a reached closure call enters one.
    const SOURCE: &str = "
        fn never() { let _c = |x: i32| x.unsupported_method_xyz(); }
        fn main() {
            let _cold = |x: i32| x.another_unsupported_method();
            let f = |x: i32| { if x > 1 { panic!(\"boom {}\", x); } x };
            assert_eq!(f(1), 1);
            f(5);
        }
    ";
    for edition in EDITIONS {
        assert!(panicked(SOURCE, edition).contains("boom 5"));
    }
}

#[test]
fn moved_capture_drops_with_its_closure() {
    const SOURCE: &str = "
        struct Noisy(i32);
        impl Drop for Noisy { fn drop(&mut self) { println!(\"drop {}\", self.0); } }
        fn main() {
            let a = Noisy(1);
            let c = move || a.0 + 1;
            println!(\"call {}\", c());
            println!(\"end\");
        }
    ";
    for edition in EDITIONS {
        let report = completed(SOURCE, edition);
        assert_eq!(report.output.stdout, "call 2\nend\ndrop 1\n");
    }
}

#[test]
fn rustc_closure_originals_execute_unchanged() {
    for name in [
        "basic-closure-syntax",
        "simple-capture-and-call",
        "issue-868",
        "explicit-return-in-closure-and-outer-fn",
        "nested-closure-call",
        "pass-lifetime-fn-to-generic-fnonce",
        "semistatement-in-lambda",
        "module-path-not-confused-with-upvar",
    ] {
        let path = std::path::PathBuf::from(format!(
            "{}/../../fixtures/upstream/rustc-checkout/tests/ui/closures/{name}.rs",
            env!("CARGO_MANIFEST_DIR")
        ));
        match standalone_edition(&path, None, 2015) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(_) => panic!("{name}: unexpected outcome"),
            Err(error) => panic!("{name}: {error:?}"),
        }
    }
}

#[test]
fn annotated_destructuring_lets_take_the_selected_annotation_type() {
    // The shared pattern transaction's initializer takes the entered Type
    // service's selection of a composite annotation (`let_annotations`):
    // `255 + 1` overflows exactly when the element Type is `u8`.
    for (text, panics) in [
        (
            "fn main() { let [a, _b]: [u8; 2] = [255, 0]; let c = a + 1; assert!(c == 0); }\n",
            true,
        ),
        (
            "fn main() { let &x: &u8 = &255; let y = x + 1; assert!(y == 0); }\n",
            true,
        ),
        (
            "fn main() { let (p, q): (u16, i64) = (3, -4); assert!(p == 3 && q == -4); }\n",
            false,
        ),
    ] {
        for edition in EDITIONS {
            if panics {
                assert!(panicked(text, edition).contains("overflow"), "{text}");
            } else {
                completed(text, edition);
            }
        }
    }
}

#[test]
fn rustc_autoderef_vec_box_fn_original_executes_unchanged() {
    let path = std::path::PathBuf::from(format!(
        "{}/../../fixtures/upstream/rustc-checkout/tests/ui/autoref-autoderef/autoderef-vec-box-fn-36786.rs",
        env!("CARGO_MANIFEST_DIR")
    ));
    match super::crate_graph::format_trait_facade_tests::run(
        "rustc-callee-autoderef",
        &std::fs::read_to_string(&path).unwrap(),
        2015,
    ) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(_) => panic!("unchanged rustc callable autoderef original returned another outcome"),
        Err(error) => panic!("unchanged rustc callable autoderef original: {error:?}"),
    }
}

#[test]
fn rustc_autoderef_arc_boxed_closure_original_executes_unchanged() {
    let path = std::path::PathBuf::from(format!(
        "{}/../../fixtures/upstream/rustc-checkout/tests/ui/autoref-autoderef/autoderef-arc-boxed-closure-call.rs",
        env!("CARGO_MANIFEST_DIR")
    ));
    match super::crate_graph::format_trait_facade_tests::run(
        "rustc-callee-autoderef",
        &std::fs::read_to_string(&path).unwrap(),
        2015,
    ) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(_) => panic!("unchanged rustc callable autoderef original returned another outcome"),
        Err(error) => panic!("unchanged rustc callable autoderef original: {error:?}"),
    }
}

#[test]
fn callable_autoderef_uses_source_deref_once_before_arguments_across_macro_origins() {
    // An LRR route/effect control, outside imported originals. Each wrapper
    // calls its ordinary Deref body; no runtime wrapper layout is inspected.
    let prefix = r#"
        use std::ops::Deref;
        struct Wrap<T>(T);
        impl<T> Deref for Wrap<T> {
            type Target = T;
            fn deref(&self) -> &T { println!("deref"); &self.0 }
        }
        fn argument() -> i32 { println!("argument"); 9 }
        macro_rules! invoke { ($f:expr) => { $f(argument()) } }
        macro_rules! wrapper { ($f:expr) => { invoke!($f) } }
    "#;
    for expression in [
        "command(argument())",
        "invoke!(command)",
        "wrapper!(command)",
    ] {
        let text = format!(
            "{prefix} fn main() {{
                let command = Wrap(Wrap(|x: i32| {{ println!(\"call\"); x + 1 }}));
                assert_eq!({expression}, 10);
            }}"
        );
        let report = completed_with_standard_facade(&text, 2021);
        assert_eq!(report.output.stdout, "deref\nderef\nargument\ncall\n");
        assert!(
            report.opcode_counters.started_by_opcode
                [usize::from(crate::ir::op::CALLEE_DEREF_OPCODE)]
                > 0
        );
    }
    completed_with_standard_facade(
        "use std::rc::Rc; fn main() { let f = Rc::new(|x: i32| x + 1); assert_eq!(f(9), 10); }",
        2021,
    );
}

fn completed_with_standard_facade(text: &str, edition: u16) -> crate::engine::run::RunReport {
    match super::crate_graph::format_trait_facade_tests::run("callee-autoderef", text, edition) {
        Ok(StandaloneOutcome::Completed(report)) => report,
        Ok(StandaloneOutcome::Panicked(report)) => {
            panic!("callee autoderef panicked: {:?}", report.panic)
        }
        Ok(_) => panic!("callee autoderef returned another outcome"),
        Err(error) => panic!("callee autoderef faulted: {error:?}"),
    }
}
