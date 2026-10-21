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

/// The imported program is unchanged; origin/standard-macro adaptations stay
/// here. Source guards for its actual alias/call are covered in preentry tests.
#[test]
fn upstream_bound_function_pointer_arguments_share_the_ordinary_call_route() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/functions-closures/fn-item-type-coerce.rs"
    );
    let (prefix, items) = original.split_at(original.find("\nfn foo").unwrap());
    let emitted = format!(
        "{prefix}\nmacro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} emit!{{{items}}}"
    );
    let wrapped = format!(
        "{prefix}\nmacro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} \
         macro_rules! wrap {{($($t:tt)*)=>{{emit!{{$($t)*}}}}}} wrap!{{{items}}}"
    );
    let nested = original.replace(
        "eq::<IntMap>(foo, bar);",
        "assert!({ eq::<IntMap>(foo, bar); true });",
    );
    for edition in [2015, 2018, 2021, 2024] {
        for (origin, text) in [original, &emitted, &wrapped, &nested].into_iter().enumerate() {
            let report = completed(text, edition);
            assert!(report.output.is_empty());
            // Stable ordinary opcodes: CallPrepare, CallSeal, CheckParameters.
            // An alternate macro evaluator cannot stand in for the one call.
            for opcode in [22, 19, 20] {
                assert_eq!(report.opcode_counters.started_by_opcode[opcode], 1,
                    "edition {edition}, origin {origin}, ordinary opcode {opcode}");
            }
            eprintln!("fn-item coercion edition={edition} origin={origin} ordinary prepare/seal/parameters=1/1/1");
        }
    }
}

#[test]
fn upstream_generic_annotation_and_explicit_application_share_all_origins() {
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/generics/generic-derived-type.rs"
    );
    let (prefix, items) = original.split_at(original.find("\nfn g").unwrap());
    let emitted = format!("{prefix}\nmacro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} emit!{{{items}}}");
    let wrapped = format!("{prefix}\nmacro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} macro_rules! wrap {{($($t:tt)*)=>{{emit!{{$($t)*}}}}}} wrap!{{{items}}}");
    // An LRR origin adapter outside the unchanged original: the same open
    // annotation and explicit application occur inside a standard macro.
    // Clone the inputs so this additional probe preserves the original work.
    let nested = original.replacen(
        "    let x: Pair<T> =",
        "    assert!({ let probe: Pair<T> = Pair { a: t.clone(), b: t.clone() }; let _ = g::<Pair<T>>(probe); true });\n    let x: Pair<T> =",
        1,
    );
    assert_ne!(nested, original);
    for edition in [2015, 2018, 2021, 2024] {
        for (origin, text) in [original, &emitted, &wrapped, &nested].into_iter().enumerate() {
            let report = match crate::cli::crate_graph::format_trait_facade_tests::run("generic-annotation", text, edition) {
                Ok(StandaloneOutcome::Completed(report)) => report,
                Err(error) => panic!("edition {edition}, origin {origin}: {error:?}"),
                Ok(_) => panic!("edition {edition}, origin {origin}: unexpected outcome"),
            };
            assert_eq!(report.output.stdout, "10\n10\n");
            assert!(report.output.stderr.is_empty());
            let calls = if origin == 3 { 8 } else { 5 };
            let entries = if origin == 3 { 6 } else { 3 };
            assert_eq!(report.opcode_counters.started_by_opcode[22], calls);
            assert_eq!(report.opcode_counters.started_by_opcode[19], calls);
            assert_eq!(report.opcode_counters.started_by_opcode[20], entries);
            eprintln!("generic annotation edition={edition} origin={origin} ordinary prepare/seal={calls}/{calls} parameter entries={entries}");
        }
    }
}

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

#[test]
fn overloaded_call_selects_the_types_own_fn_impl_in_rustc_order_across_macro_origins() {
    // An LRR route/effect control, outside imported originals. A callee of a
    // user Type implementing the `Fn*` traits is called through rustc's
    // lookup order (`Fn`, then `FnMut`, then `FnOnce`) by the ordinary call
    // boundary: its argument evaluates exactly once, before the impl's body,
    // and a consumed callee drops inside the call, identically handwritten,
    // emitted by a macro or wrapped.
    let prefix = r#"
        #![feature(unboxed_closures, fn_traits)]
        struct All;
        struct Mutable;
        struct Once(i32);
        impl Fn<(i32,)> for All {
            extern "rust-call" fn call(&self, (x,): (i32,)) -> i32 { println!("Fn"); x + 1 }
        }
        impl FnMut<(i32,)> for All {
            extern "rust-call" fn call_mut(&mut self, (x,): (i32,)) -> i32 { println!("FnMut"); x + 2 }
        }
        impl FnOnce<(i32,)> for All {
            type Output = i32;
            extern "rust-call" fn call_once(self, (x,): (i32,)) -> i32 { println!("FnOnce"); x + 3 }
        }
        impl FnMut<(i32,)> for Mutable {
            extern "rust-call" fn call_mut(&mut self, (x,): (i32,)) -> i32 { println!("FnMut"); x + 2 }
        }
        impl FnOnce<(i32,)> for Mutable {
            type Output = i32;
            extern "rust-call" fn call_once(self, (x,): (i32,)) -> i32 { println!("FnOnce"); x + 3 }
        }
        impl Drop for Once { fn drop(&mut self) { println!("drop"); } }
        impl FnOnce<(i32, i32)> for Once {
            type Output = i32;
            extern "rust-call" fn call_once(self, (a, b): (i32, i32)) -> i32 {
                println!("call_once");
                self.0 + a + b
            }
        }
        fn argument() -> i32 { println!("argument"); 9 }
        macro_rules! invoke { ($f:expr, $($a:expr),*) => { $f($($a),*) } }
        macro_rules! wrapper { ($f:expr, $($a:expr),*) => { invoke!($f, $($a),*) } }
    "#;
    let scenarios = [
        ("let all = All;", "all", "argument()", 10, "argument\nFn\n"),
        ("let mut m = Mutable;", "m", "argument()", 11, "argument\nFnMut\n"),
        ("let once = Once(1);", "once", "argument(), 5", 15, "argument\ncall_once\ndrop\n"),
    ];
    for (declaration, callee, arguments, expected, output) in scenarios {
        for expression in [
            format!("{callee}({arguments})"),
            format!("invoke!({callee}, {arguments})"),
            format!("wrapper!({callee}, {arguments})"),
        ] {
            for edition in [2015, 2024] {
                let text = format!(
                    "{prefix} fn main() {{
                        {declaration}
                        assert_eq!({expression}, {expected});
                    }}"
                );
                let report = completed_with_standard_facade(&text, edition);
                assert_eq!(report.output.stdout, output, "{expression} in {edition}");
            }
        }
    }
}

#[test]
fn overloaded_call_result_and_arguments_type_numeric_literals_from_the_impl() {
    // An LRR control, outside imported originals. The impl's declared
    // argument tuple and result state the Types the source numeric constraint
    // service relates a call's literals to, for a local callee and for a unit
    // struct named as the callee: the later `30` is an `i64`, which no
    // reached value of the call's operands could state before the comparison.
    let prefix = r#"
        #![feature(unboxed_closures, fn_traits)]
        struct Two;
        impl FnOnce<(u8, i64)> for Two {
            type Output = i64;
            extern "rust-call" fn call_once(self, (a, b): (u8, i64)) -> i64 { a as i64 * b }
        }
    "#;
    for body in [
        "let t = Two; let r = t(5, 6); let wide = &30; assert!(r == *wide);",
        "let wide = &30; assert!(Two(5, 6) == *wide);",
    ] {
        for edition in [2015, 2024] {
            let text = format!("{prefix} fn main() {{ {body} }}");
            completed_with_standard_facade(&text, edition);
        }
    }
}

fn completed_with_standard_facade(text: &str, edition: u16) -> crate::engine::run::RunReport {
    match super::crate_graph::format_trait_facade_tests::run("callee-autoderef", text, edition) {
        Ok(StandaloneOutcome::Completed(report)) => report,
        Ok(StandaloneOutcome::Panicked(report)) => {
            panic!("callee autoderef panicked: {:?}", report.panic)
        }
        Ok(_) => panic!("callee autoderef returned another outcome"),
        Err(error) => panic!("standard facade edition {edition} faulted: {error:?}\n{text}"),
    }
}

#[test]
fn upstream_generic_closure_factory_keeps_distinct_applications_after_return_all_origins() {
    // Disclosed LRR identity/lifetime adaptation: keep the actual upstream
    // factory unchanged, replace only the entry, and leave its original main cold.
    // The unchanged whole original is executed separately by the stock runner.
    let original = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/functions-closures/closure-to-fn-coercion.rs"
    );
    assert_eq!(original.matches("fn main() {").count(), 1);
    let mut adapted = original.replace("fn main() {", "fn original_main() {");
    adapted.push_str(
        r#"
fn main() {
    let a = generic(0_i8);
    let b = generic(0_u64);
    let again = generic(0_i8);
    assert_eq!(b(), 8);
    assert_eq!(a(), 1);
    assert_eq!(again(), 1);
    assert_eq!(generic(0_u16)(), 2);
    assert_eq!(b(), 8);
    assert!(a == again);
    assert!(a != b);
}
"#,
    );
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nemit! {{\n{adapted}\n}}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\n\
         macro_rules! wrapper {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }}\n\
         wrapper! {{\n{adapted}\n}}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for source in [&adapted, &emitted, &wrapped] {
            let report = completed_with_standard_facade(source, edition);
            assert_eq!(
                closures_made(&report),
                4,
                "all factory values use ordinary MakeClosure in edition {edition}"
            );
        }
    }
}

#[test]
fn closure_captures_use_expanded_hygienic_local_identities_all_origins() {
    // LRR route/hygiene control, outside unchanged upstream originals. Empty
    // arguments cannot disclose `x`; its definition-site identity must select
    // the parameter despite the later same-spelling local. The closure's own
    // argument occupies another ordinal in its call frame.
    const SOURCE: &str = r#"
fn factory(x: i32) -> impl Fn(i32) -> i32 {
    macro_rules! read { () => { x } }
    let x = 99_i32;
    move |delta: i32| {
        assert_eq!(read!(), 7);
        if false { cold() } else { read!() + delta }
    }
}
fn cold() -> i32 { let _f = |x: i32| x.unsupported_method_xyz(); 123 }
fn main() {
    let f = factory(7);
    assert_eq!(f(2), 9);
    assert_eq!(f(3), 10);
    let mut ignored = 1_i32;
    macro_rules! ignore { ($v:ident) => { 0_i32 } }
    let empty = || ignore!(ignored);
    ignored = 2;
    assert_eq!(empty(), 0);
    assert_eq!(ignored, 2);
    let mut count = 0_i32;
    macro_rules! increment { ($v:ident) => { $v += 1 } }
    let mut bump = || increment!(count);
    bump(); bump();
    assert_eq!(count, 2);
    let item = 1_i32;
    let local_item = || {
        const C: i32 = value();
        static S: i32 = value();
        const fn value() -> i32 { 2 }
        fn item() -> i32 { 8 }
        item()
    };
    assert_eq!(local_item(), 8);
    assert_eq!(item, 1);
}
"#;
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nemit! {{ {SOURCE} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nmacro_rules! wrapper {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }}\nwrapper! {{ {SOURCE} }}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for source in [SOURCE, &emitted, &wrapped] {
            let report = completed(source, edition);
            assert_eq!(
                closures_made(&report),
                4,
                "ordinary MakeClosure count in edition {edition}"
            );
        }
    }
}

#[test]
fn upstream_nested_builtin_positions_use_actual_expansion_causes_all_editions() {
    // Entire unchanged upstream original, including its exact byte positions.
    let original =
        include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/issue-26322.rs");
    for edition in [2015, 2018, 2021, 2024] {
        let report = completed_with_standard_facade(original, edition);
        assert_eq!(closures_made(&report), 2);
    }
}

#[test]
fn forwarded_builtin_positions_keep_raw_input_context_under_standard_macros() {
    // LRR location/source join control outside imported originals. `column!`
    // written as an argument retains its location through a wrapper; tokens
    // emitted by that wrapper use the wrapper's invocation. A standard assert
    // expansion also forwards original input tokens rather than relocating them.
    let source = concat!(
        "macro_rules! forward { ($e:expr) => { $e } }\n",
        "macro_rules! wrap { ($e:expr) => { forward!($e) } }\n",
        "macro_rules! inner { () => { column!() } }\n",
        "macro_rules! outer { () => { inner!() } }\n",
        "fn main() {\n",
        "    let direct = column!();\n",
        "    let passed = wrap!(column!());\n",
        "    let emitted = outer!();\n",
        "    assert_eq!(direct, 18);\n",
        "    assert_eq!(passed, 24);\n",
        "    assert_eq!(emitted, 19);\n",
        "    assert_eq!(column!(), 16);\n",
        "}\n",
    );
    for edition in [2015, 2018, 2021, 2024] {
        completed(source, edition);
    }
}

#[test]
fn included_source_uses_one_pipeline_and_preserves_boundaries_and_exact_once_effects() {
    // LRR source/origin control outside unchanged originals. The same physical
    // included bytes keep line 3 through a marked include caller, wrappers and
    // standard assertion expansion. Only reached VM execution prints.
    let fragment = source("{\n    println!(\"included\");\n    line!()\n}\n");
    let literal = format!("{:?}", fragment.0.to_str().unwrap());
    let program = format!(
        r#"
macro_rules! emit {{ () => {{ include!({literal}) }} }}
macro_rules! wrapper {{ () => {{ emit!() }} }}
mod nested {{ pub fn value() -> u32 {{ wrapper!() }} }}
fn cold() -> u32 {{ wrapper!() }}
fn main() {{
    let direct = include!({literal});
    let emitted = emit!();
    assert_eq!(direct, 3_u32);
    assert_eq!(emitted, 3_u32);
    assert_eq!(wrapper!(), 3_u32);
    assert_eq!(nested::value(), 3_u32);
}}
"#
    );
    for edition in [2015, 2018, 2021, 2024] {
        let report = completed(&program, edition);
        assert_eq!(
            report.output.stdout,
            "included\nincluded\nincluded\nincluded\n"
        );
    }
}

#[test]
fn compiler_literals_join_ordinary_patterns_across_macro_origins() {
    // LRR route/exact-once control, outside imported upstream originals.
    const SOURCE: &str = r#"
macro_rules! emitted { () => { concat!("a", "b") } }
macro_rules! wrapper { () => { emitted!() } }
fn value() -> &'static str { println!("scrutinee"); "ab" }
fn cold() -> bool { ().unsupported_method_xyz() }
fn main() {
    assert!(match value() { concat!("a", "b") => true, _ => false });
    assert!(if let emitted!() = value() { true } else { false });
    let mut selected = false;
    while let wrapper!() = value() { selected = true; break; }
    assert!(selected);
    let wrapper!() = value() else { panic!("unmatched") };
    assert!(if false { cold() } else { true });
}
"#;
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nemit! {{ {SOURCE} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nmacro_rules! outer {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }}\nouter! {{ {SOURCE} }}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for text in [SOURCE, &emitted, &wrapped] {
            let report = completed(text, edition);
            assert_eq!(
                report.output.stdout,
                "scrutinee\nscrutinee\nscrutinee\nscrutinee\n"
            );
        }
    }
}

#[test]
fn compiler_byte_literals_use_shared_values_and_patterns_across_origins() {
    // LRR route/exact-once control; upstream concat-bytes.rs is kept unchanged.
    const SOURCE: &str = r##"
macro_rules! bytes { () => { concat_bytes!(b'A', br"\", [0x0_u8, 0b1000_0000, 255u8], [b'X'; 0]) } }
macro_rules! wrapper { () => { bytes!() } }
fn value() -> &'static [u8] { println!("bytes"); wrapper!() }
fn cold() -> bool { ().unsupported_method_xyz() }
fn main() {
    assert_eq!(bytes!(), b"A\\\0\x80\xff");
    assert!(match value() { bytes!() => true, _ => false });
    assert!(if let concat_bytes!(bytes!()) = value() { true } else { false });
    let concat_bytes!(bytes!()) = value() else { panic!("unmatched") };
    let mut selected = false;
    while let wrapper!() = value() { selected = true; break; }
    assert!(selected);
    assert!(if false { cold() } else { true });
}
"##;
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nemit! {{ {SOURCE} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nmacro_rules! outer {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }}\nouter! {{ {SOURCE} }}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for text in [SOURCE, &emitted, &wrapped] {
            let report = completed_with_standard_facade(text, edition);
            assert_eq!(report.output.stdout, "bytes\nbytes\nbytes\nbytes\n");
        }
    }
}

#[test]
fn nominal_constant_patterns_keep_source_context_and_exact_once_effects_all_origins() {
    // LRR route/effect control outside the unchanged primary rustc fixture.
    const SOURCE: &str = r#"
#[derive(PartialEq, Debug)]
struct B(i32);
const OUTER: [B; 0] = [];
fn scrutinee() -> [B; 0] { println!("scrutinee"); [] }
fn cold() -> bool { ().unsupported_method_xyz() }
fn main() {
    const LOCAL: [B; 0] = [];
    assert!(match [] { LOCAL => { println!("matched"); true } });
    assert!(match scrutinee() { OUTER => { println!("matched"); true } });
    assert!(if let LOCAL = [] { true } else { false });
    let OUTER = [] else { panic!("unmatched") };
    while let LOCAL = [] { break; }
    assert!(if false { cold() } else { true });
}
"#;
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nemit! {{ {SOURCE} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nmacro_rules! wrapper {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }}\nwrapper! {{ {SOURCE} }}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for text in [SOURCE, &emitted, &wrapped] {
            let report = completed_with_standard_facade(text, edition);
            assert_eq!(report.output.stdout, "matched\nscrutinee\nmatched\n");
        }
    }
}

/// The upstream program remains unchanged in its licensed fixture directory.
/// Wrapping its actual main body is an LRR origin/edition route control, not
/// another upstream pass or a substitute for the directive-aware harness.
#[test]
fn upstream_pattern_captures_share_the_source_route_across_origins_and_editions() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/let-else/let-else-bindings.rs"
    );
    let (items, tail) = ORIGINAL.split_once("pub fn main() {").unwrap();
    let body = tail.trim_end().strip_suffix('}').unwrap();
    for edition in [2015, 2018, 2021, 2024] {
        for (origin, source) in [
            ("original", ORIGINAL.to_owned()),
            (
                "user",
                format!("{items} macro_rules! emit {{()=>{{{{{body}}}}}}} fn main(){{emit!();}}"),
            ),
            (
                "wrapper",
                format!(
                    "{items} macro_rules! emit {{()=>{{{{{body}}}}}}} macro_rules! wrapper {{()=>{{emit!()}}}} fn main(){{wrapper!();}}"
                ),
            ),
            (
                "standard",
                format!("{items} fn main(){{assert!({{{body};true}});}}"),
            ),
        ] {
            let report = completed(&source, edition);
            assert_eq!(closures_made(&report), 4, "{origin}, edition {edition}");
            assert!(report.opcode_counters.started_by_opcode[45] > 0);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        }
    }
}

/// LRR edition/origin adaptation of the unchanged upstream aggregate binding
/// witness. Standard nesting wraps the actual helper bodies where patterns run,
/// rather than only wrapping main's calls to still-handwritten functions.
#[test]
fn upstream_aggregate_pattern_publication_shares_all_editions_and_origins() {
    use ra_ap_syntax::{AstNode, ast, ast::HasName};
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/let-else/let-else-non-copy.rs"
    );
    let parsed = ast::SourceFile::parse(ORIGINAL, ra_ap_syntax::Edition::Edition2024);
    assert!(parsed.errors().is_empty());
    let mut ranges: Vec<_> = parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter(|function| {
            function
                .name()
                .is_some_and(|name| matches!(name.text(), "let_else" | "if_let"))
        })
        .map(|function| function.body().unwrap().syntax().text_range())
        .collect();
    assert_eq!(ranges.len(), 2);
    ranges.sort_by_key(|range| std::cmp::Reverse(range.start()));
    let mut standard = ORIGINAL.to_owned();
    for range in ranges {
        let (start, end) = (
            u32::from(range.start()) as usize,
            u32::from(range.end()) as usize,
        );
        standard.replace_range(
            start..end,
            &format!("{{assert!({{{};true}});}}", &ORIGINAL[start + 1..end - 1]),
        );
    }
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nemit! {{ {ORIGINAL} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }}\nmacro_rules! wrapper {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }}\nwrapper! {{ {ORIGINAL} }}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        let mut pattern_steps = None;
        for text in [ORIGINAL, &emitted, &wrapped, &standard] {
            let report = completed_with_standard_facade(text, edition);
            let actual = report.opcode_counters.started_by_opcode[45];
            assert!(actual > 0);
            assert_eq!(*pattern_steps.get_or_insert(actual), actual);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
            assert!(report.output.stdout.is_empty() && report.output.stderr.is_empty());
        }
    }
}

#[test]
fn upstream_reference_conversion_uses_shared_receiver_probe_all_editions_and_origins() {
    use ra_ap_syntax::{AstNode, ast, ast::HasName};
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/let-else/let-else-source-expr-nomove-pass.rs"
    );
    let tree = ast::SourceFile::parse(ORIGINAL, ra_ap_syntax::Edition::Edition2024);
    assert!(tree.errors().is_empty());
    let main = tree
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .find(|function| function.name().is_some_and(|name| name.text() == "main"))
        .unwrap();
    let range = main.body().unwrap().syntax().text_range();
    let (start, end) = (
        u32::from(range.start()) as usize,
        u32::from(range.end()) as usize,
    );
    let mut standard = ORIGINAL.to_owned();
    standard.replace_range(
        start..end,
        &format!("{{assert!({{{};true}});}}", &ORIGINAL[start + 1..end - 1]),
    );
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} emit! {{ {ORIGINAL} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} macro_rules! wrap {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }} wrap! {{ {ORIGINAL} }}"
    );
    for edition in [2015, 2018, 2021, 2024] {
        for source in [ORIGINAL, &emitted, &wrapped, &standard] {
            let report = completed_with_standard_facade(source, edition);
            assert_eq!(report.output.stdout, "inner: foo\nother: None\n");
            assert!(report.output.stderr.is_empty());
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        }
    }
}

#[test]
fn upstream_cloned_box_conversion_keeps_generic_context_all_editions_and_origins() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/closures/closure-capture-after-clone.rs"
    );
    let (items, tail) = ORIGINAL.split_once("pub fn main() {").unwrap();
    let body = tail.trim_end().strip_suffix('}').unwrap();
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} emit! {{ {ORIGINAL} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} macro_rules! wrap {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }} wrap! {{ {ORIGINAL} }}"
    );
    let standard = format!("{items} fn main(){{assert!({{{body};true}});}}");
    for edition in [2015, 2018, 2021, 2024] {
        for text in [ORIGINAL, &emitted, &wrapped, &standard] {
            let report = completed_with_standard_facade(text, edition);
            assert_eq!(report.output.stdout, "22\n");
            assert!(report.output.stderr.is_empty());
            assert_eq!(closures_made(&report), 1);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        }
    }
}

#[test]
fn upstream_trait_self_arguments_share_declared_types_all_editions_and_origins() {
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/methods/method-self-arg-trait.rs"
    );
    let (items, tail) = ORIGINAL.split_once("fn main() {").unwrap();
    let body = tail.trim_end().strip_suffix('}').unwrap();
    let emitted =
        format!("macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} emit! {{ {ORIGINAL} }}");
    let wrapped = format!(
        "macro_rules! emit {{ ($($t:tt)*) => {{ $($t)* }} }} macro_rules! wrap {{ ($($t:tt)*) => {{ emit! {{ $($t)* }} }} }} wrap! {{ {ORIGINAL} }}"
    );
    let standard = format!("{items} fn main(){{assert!({{{body};true}});}}");
    for edition in [2015, 2018, 2021, 2024] {
        for text in [ORIGINAL, &emitted, &wrapped, &standard] {
            let report = completed_with_standard_facade(text, edition);
            assert!(report.output.stdout.is_empty() && report.output.stderr.is_empty());
            assert!(report.opcode_counters.operations_started > 0);
            assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
            assert_eq!(
                report.opcode_counters.operations_started,
                report.opcode_counters.operations_completed
            );
            assert_eq!(report.opcode_counters.panic_scopes_cleaned, 0);
        }
    }
}
