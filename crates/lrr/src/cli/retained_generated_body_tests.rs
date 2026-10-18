//! Macro calls inside the bodies of macro-generated functions join the same
//! retained source admission as original bodies: the owning source's plan
//! attaches the issued producer chain and the body's own calls, then ordinary
//! lowering/VM execution runs the expanded body. LRR route controls.
use super::super::*;

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
        "lrr-generated-body-{}-{}.rs",
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
const EDITIONS: [u16; 2] = [2015, 2021];

#[test]
fn generated_free_functions_and_methods_execute_their_body_macro_calls() {
    const SOURCE: &str = r#"
        struct A;
        macro_rules! inner { ($e:expr) => { $e + 1 } }
        macro_rules! free { () => { fn g(x: i32) -> i32 { let y = inner!(x); println!("g={}", y); y } } }
        macro_rules! method { () => { impl A { fn m(&self) -> i32 { println!("m"); 4 } } } }
        macro_rules! wrap { () => { free!(); } }
        free!();
        method!();
        mod nested { wrap!(); pub fn call() -> i32 { g(1) } }
        fn main() {
            assert_eq!(g(4), 5);
            let a = A;
            let v = a.m();
            assert!(v == 4);
            assert_eq!(nested::call(), 2);
        }
    "#;
    for edition in EDITIONS {
        let report = completed(SOURCE, edition);
        assert_eq!(report.output.stdout, "g=5\nm\ng=2\n", "edition {edition}");
    }
}

#[test]
fn generated_body_uses_the_same_operations_as_the_handwritten_body() {
    const HANDWRITTEN: &str = r#"
        macro_rules! inner { ($e:expr) => { $e + 1 } }
        fn g(x: i32) -> i32 { let y = inner!(x); println!("{} {}", x, y); if y > 3 { y * 2 } else { y } }
        fn main() { assert_eq!(g(4), 10); assert_eq!(g(1), 2); }
    "#;
    const GENERATED: &str = r#"
        macro_rules! inner { ($e:expr) => { $e + 1 } }
        macro_rules! mk { () => { fn g(x: i32) -> i32 { let y = inner!(x); println!("{} {}", x, y); if y > 3 { y * 2 } else { y } } } }
        mk!();
        fn main() { assert_eq!(g(4), 10); assert_eq!(g(1), 2); }
    "#;
    for edition in EDITIONS {
        let handwritten = completed(HANDWRITTEN, edition);
        let generated = completed(GENERATED, edition);
        assert_eq!(handwritten.output.stdout, "4 5\n1 2\n");
        assert_eq!(generated.output.stdout, handwritten.output.stdout);
        // One ordinary lowering/VM route: identical executed operations.
        assert_eq!(
            generated.opcode_counters.started_by_opcode,
            handwritten.opcode_counters.started_by_opcode,
            "edition {edition}"
        );
    }
}

#[test]
fn unreached_generated_body_with_macro_calls_is_never_lowered() {
    // `cold` is admitted structurally but never entered; its unsupported
    // runtime shape must not block the reached program.
    const SOURCE: &str = r#"
        macro_rules! mk { () => {
            fn cold() { let p: *const u8 = std::ptr::null(); println!("{:p}", p); }
            fn hot() -> u8 { println!("hot"); 7 }
        } }
        mk!();
        fn main() { assert_eq!(hot(), 7); }
    "#;
    for edition in EDITIONS {
        assert_eq!(completed(SOURCE, edition).output.stdout, "hot\n");
    }
}

/// Generated module functions enter the same source-local plan as ordinary
/// functions before their nested items are demanded. The assertion itself is
/// an actual standard macro expansion in each source form.
#[test]
fn generated_module_functions_publish_their_nested_item_environment() {
    const BODY: &str = "fn outer(x: i32) -> i32 { fn inner(v: i32) -> i32 { v + 1 } inner(x) }";
    for edition in [2015, 2018, 2021, 2024] {
        let main = "fn main() { assert_eq!(outer(1), 2); assert_eq!(outer(4), 5); }";
        let ordinary = completed(&format!("{BODY}\n{main}"), edition);
        for source in [
            format!("macro_rules! make {{ () => {{ {BODY} }} }} make!(); {main}"),
            format!(
                "macro_rules! make {{ () => {{ {BODY} }} }} macro_rules! wrap {{ () => {{ make!(); }} }} wrap!(); {main}"
            ),
        ] {
            let generated = completed(&source, edition);
            assert_eq!(generated.output.stdout, ordinary.output.stdout);
            assert_eq!(
                generated.opcode_counters.started_by_opcode,
                ordinary.opcode_counters.started_by_opcode,
                "edition {edition}"
            );
        }
        completed(
            "macro_rules! emit { () => { mod generated { pub fn f() -> u8 { fn child() -> u8 { 7 } child() } } } } emit!(); fn main() { assert_eq!(generated::f(), 7); }",
            edition,
        );
    }
}


#[test]
fn opaque_definition_module_shares_type_value_and_pattern_routes() {
    // LRR route/effect control, supplementary to unchanged upstream witnesses.
    // Shadow the producer's names at the call site: opaque item lookup must
    // retain the actual declaration module for Type and Value alike.
    const SOURCE: &str = r#"
        #![feature(decl_macro)]
        mod definitions {
            struct Target { value: i32 }
            fn number() -> i32 { println!("number"); 7 }
            pub macro emit($name:ident) {
                fn $name() -> i32 {
                    let item: Target = Target { value: number() };
                    let result = item.value;
                    result
                }
            }
        }
        struct Target { value: bool }
        fn number() -> bool { false }
        definitions::emit!(answer);
        fn main() { println!("{}", answer()); }
    "#;
    for edition in [2015, 2018, 2021, 2024] {
        let report = completed(SOURCE, edition);
        assert_eq!(report.output.stdout, "number\n7\n", "edition {edition}");
    }
}

#[test]
fn opaque_expression_module_shares_type_value_and_pattern_routes() {
    // Independent entered-body route control. The generated-function signature
    // control above remains required; this does not stand in for that gate.
    const SOURCE: &str = r#"
        #![feature(decl_macro)]
        mod definitions {
            struct Target { value: i32 }
            fn number() -> i32 { println!("number"); 7 }
            pub macro body() {{
                let item: Target = Target { value: number() };
                let result = item.value;
                println!("{}", result);
            }}
        }
        struct Target { value: bool }
        fn number() -> bool { false }
        fn main() { definitions::body!(); }
    "#;
    for edition in [2015, 2018, 2021, 2024] {
        let report = completed(SOURCE, edition);
        assert_eq!(report.output.stdout, "number\n7\n", "edition {edition}");
    }
}
