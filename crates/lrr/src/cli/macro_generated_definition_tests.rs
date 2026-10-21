//! Macros that define macros, and macros defined through `#[macro_use]`
//! modules, reached from ordinary bodies and from the arguments of standard
//! macros. LRR-specific controls through the production CLI path: the same
//! operation is written as a handwritten call, as a call of a macro generated
//! by an item expansion, by a statement expansion and by a second-level
//! expansion, and as an argument of `assert!`/`assert_eq!`/`println!`/`format!`/
//! `vec!`. They are not a stock-suite count; the unchanged upstream programs
//! (`hygiene/issue-40847.rs`, `macros/meta-variable-misuse.rs`,
//! `cfg/cfg-macros-notfoo.rs`, `macros/macro-of-higher-order.rs`) are the
//! primary witnesses.
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
        "lrr-macro-generated-{}-{}.rs",
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
/// Every official edition runs the same program to completion.
fn completed(text: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        let owned = source(text);
        match standalone_edition(&owned.0, None, edition) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("edition {edition} panicked: {:?}\n{text}", report.panic)
            }
            Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
            Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
        }
    }
}

const MAKE: &str = "macro_rules! mk { ($n:ident) => { macro_rules! $n { () => { 5 } } } }\n";

#[test]
fn item_expansion_defines_a_macro_called_from_a_body() {
    completed(&format!(
        "{MAKE}mk!(five);\nfn main() {{ let x = five!(); assert_eq!(x, 5); }}\n"
    ));
}

#[test]
fn statement_expansion_defines_a_macro_called_later_in_the_body() {
    completed(&format!(
        "{MAKE}fn main() {{ mk!(five); let x = five!(); assert_eq!(x, 5); }}\n"
    ));
}

#[test]
fn definitions_two_levels_deep_are_visible_after_each_expansion() {
    completed(
        "macro_rules! mk { ($n:ident, $m:ident) => { macro_rules! $n { () => { macro_rules! $m { () => { 7 } } } } } }\n\
         mk!(outer, inner);\nouter!();\nfn main() { assert_eq!(inner!(), 7); }\n",
    );
    completed(
        "macro_rules! mk { ($n:ident, $m:ident) => { macro_rules! $n { () => { macro_rules! $m { () => { 7 } } } } } }\n\
         fn main() { mk!(outer, inner); outer!(); assert_eq!(inner!(), 7); }\n",
    );
}

#[test]
fn generated_macro_is_visible_inside_standard_macro_arguments() {
    completed(&format!(
        "{MAKE}mk!(five);\n\
         fn main() {{\n\
             assert!(five!() == 5);\n\
             assert_eq!(five!(), 5);\n\
             println!(\"{{}}\", five!());\n\
             assert_eq!(format!(\"{{}}\", five!()), \"5\");\n\
             let v = vec![five!(), 6];\n\
             assert_eq!(v[0], 5);\n\
         }}\n"
    ));
}

#[test]
fn macro_use_module_macros_are_visible_inside_standard_macro_arguments() {
    completed(
        "#[macro_use]\nmod foo { macro_rules! bar { () => { 3 } } }\n\
         fn main() {\n\
             let a = bar!();\n\
             assert!(bar!() == 3);\n\
             assert_eq!(bar!(), a);\n\
             assert_ne!(bar!(), 4);\n\
             println!(\"{}\", bar!());\n\
             assert_eq!(format!(\"{}-{}\", bar!(), 2), \"3-2\");\n\
         }\n",
    );
}

#[test]
fn generated_macro_defined_inside_a_macro_use_module_keeps_its_scope() {
    completed(
        "#[macro_use]\nmod m {\n\
             macro_rules! mk { ($n:ident) => { macro_rules! $n { () => { 5 } } } }\n\
             mk!(five);\n\
             pub fn local() -> i32 { five!() }\n\
         }\n\
         fn main() { assert_eq!(five!(), 5); assert_eq!(m::local(), 5); assert!(five!() == 5); }\n",
    );
}

#[test]
fn generated_macro_keeps_crate_and_local_hygiene() {
    completed(
        "mod util { pub fn helper() -> i32 { 7 } }\n\
         macro_rules! mk { ($n:ident) => { macro_rules! $n { () => { $crate::util::helper() } } } }\n\
         mk!(seven);\n\
         macro_rules! mkx { ($n:ident) => { macro_rules! $n { ($e:expr) => {{ let x = 10; $e + x }} } } }\n\
         mkx!(addx);\n\
         fn main() { let x = 1; assert_eq!(seven!(), 7); assert_eq!(addx!(x), 11); }\n",
    );
}

#[test]
fn macro_generated_by_a_macro_expands_to_items_called_from_a_body() {
    // The shape of `macros/meta-variable-misuse.rs`: the generated macros
    // `m`/`n` define functions whose names and values come from the outer call.
    completed(
        "macro_rules! foo {\n\
             ($($m:ident $($f:ident $v:tt)+),*) => {\n\
                 $($(macro_rules! $f { () => { $v } })+)*\n\
                 $(macro_rules! $m { () => { $(fn $f() -> i32 { $v })+ } })*\n\
             }\n\
         }\n\
         foo!(m a 1 b 2, n c 3);\nm!();\nn!();\n\
         fn main() {\n\
             assert_eq!(a!(), 1);\n\
             assert_eq!(c!(), 3);\n\
             assert_eq!(a(), 1);\n\
             assert_eq!(b(), 2);\n\
             assert_eq!(c(), 3);\n\
         }\n",
    );
}

#[test]
fn false_assertion_through_a_generated_macro_still_panics() {
    // Control: the generated call is evaluated, not skipped.
    let owned = source(&format!(
        "{MAKE}mk!(five);\nfn main() {{ assert!(five!() == 6); }}\n"
    ));
    match standalone_edition(&owned.0, None, 2021) {
        Ok(StandaloneOutcome::Panicked(report)) => {
            assert!(report.panic.description().contains("assertion failed"));
        }
        other => panic!("generated macro assertion did not panic: {:?}", other.is_ok()),
    }
}
