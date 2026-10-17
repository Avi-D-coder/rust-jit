//! Standalone `--extern proc-macro:NAME=PATH` through explicit host-tool
//! preparation, the matched external macro server and the ordinary discovery,
//! namespace and execution pipeline. Requires the pinned 1.99.0 toolchain.
use crate::cli::{
    StandaloneOutcome,
    crate_graph::{ExternArg, ExternCrates},
    standalone_edition_with_capture,
};
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lrr-proc-macro-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(std::fs::canonicalize(path).unwrap())
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Every procedural form, a derive pair on one item, a declarative wrapper
/// around a procedural call, a printing macro and an invocation counter.
const MACROS: &str = r#"
extern crate proc_macro;
use proc_macro::TokenStream;
use std::io::Write;

#[proc_macro_derive(Answer)]
pub fn answer(_: TokenStream) -> TokenStream {
    "fn answer() -> u32 { 42 }".parse().unwrap()
}
#[proc_macro_derive(Other)]
pub fn other(_: TokenStream) -> TokenStream {
    "fn other() -> u32 { 7 }".parse().unwrap()
}
#[proc_macro]
pub fn make_fn(input: TokenStream) -> TokenStream {
    format!("fn made() -> u32 {{ {} }}", input).parse().unwrap()
}
#[proc_macro]
pub fn make_wrapped(input: TokenStream) -> TokenStream {
    format!("fn wrapped_value() -> u32 {{ {} }}", input).parse().unwrap()
}
#[proc_macro_attribute]
pub fn double(_: TokenStream, item: TokenStream) -> TokenStream {
    item.to_string().replace("1", "2").parse().unwrap()
}
#[proc_macro]
pub fn shout(_: TokenStream) -> TokenStream {
    println!("native stdout from a macro must not reach the protocol stream");
    TokenStream::new()
}
#[proc_macro]
pub fn count(input: TokenStream) -> TokenStream {
    let path = input.to_string();
    let path = path.trim_matches('"');
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap();
    writeln!(file, "invoked").unwrap();
    TokenStream::new()
}
"#;

fn run(root: &Path, macros: &Path, prelude: bool) -> StandaloneOutcome {
    let externs = ExternCrates::new(
        vec![ExternArg {
            name: "fixture_macros".into(),
            path: macros.to_owned(),
            prelude,
            proc_macro: true,
        }],
        vec![("fixture_macros".into(), 2018)],
    )
    .unwrap();
    match standalone_edition_with_capture(root, None, 2018, &externs, false, None) {
        Ok(outcome) => outcome,
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn all_procedural_forms_expand_once_through_the_ordinary_pipeline() {
    let temp = Temp::new("forms");
    let macros = temp.write("fixture_macros.rs", MACROS);
    let counter = temp.0.join("count.log");
    let root = temp.write(
        "main.rs",
        &format!(
            r#"
extern crate fixture_macros;
use fixture_macros::{{Answer, Other, make_fn, make_wrapped, double, shout, count}};

#[derive(Answer)]
#[derive(Other)]
struct S;

make_fn!(5);
shout!();
count!({counter:?});

macro_rules! wrapped {{
    ($value:literal) => {{ make_wrapped!($value); }};
}}
wrapped!(9);

#[double]
fn one() -> u32 {{ 1 }}

fn main() {{
    assert_eq!(answer() + other() + made() + wrapped_value() + one(), 42 + 7 + 5 + 9 + 2);
}}
"#
        ),
    );
    match run(&root, &macros, true) {
        StandaloneOutcome::Completed(_) => {}
        StandaloneOutcome::Panicked(report) => panic!("guest panic: {:?}", report.panic),
        _ => panic!("unexpected outcome"),
    }
    // The Type and Value seals of the request share one adopted record.
    assert_eq!(
        std::fs::read_to_string(&counter).unwrap().lines().count(),
        1,
        "procedural invocation must execute exactly once per request epoch"
    );
}

#[test]
fn noprelude_procedural_crate_is_reached_through_extern_crate() {
    let temp = Temp::new("noprelude");
    let macros = temp.write("fixture_macros.rs", MACROS);
    let root = temp.write(
        "main.rs",
        r#"
#[macro_use]
extern crate fixture_macros;
#[derive(Answer)]
struct S;
fn main() {
    assert_eq!(answer(), 42);
}
"#,
    );
    assert!(matches!(
        run(&root, &macros, false),
        StandaloneOutcome::Completed(_)
    ));
}

#[test]
fn procedural_macro_panic_is_a_macro_fault_not_a_guest_panic() {
    let temp = Temp::new("panic");
    let macros = temp.write(
        "fixture_macros.rs",
        "extern crate proc_macro;\n#[proc_macro]\npub fn boom(_: proc_macro::TokenStream) -> proc_macro::TokenStream { panic!(\"boom\") }\n",
    );
    let root = temp.write(
        "main.rs",
        "extern crate fixture_macros;\nfixture_macros::boom!();\nfn main() {}\n",
    );
    let externs = ExternCrates::new(
        vec![ExternArg {
            name: "fixture_macros".into(),
            path: macros,
            prelude: true,
            proc_macro: true,
        }],
        vec![],
    )
    .unwrap();
    let error = match standalone_edition_with_capture(&root, None, 2018, &externs, false, None) {
        Err(error) => error,
        Ok(_) => panic!("a panicking procedural macro must fail expansion"),
    };
    assert_eq!(error.code, "LRR-E-MACRO-PANIC", "{error}");
}

/// Witness from unchanged `tests/ui/proc-macro/attributes-included.rs`: an
/// attached syntax tree spells doc comments as `#[doc = ...]`; pairing the
/// original declarations with it must treat both spellings as one header.
#[test]
fn doc_comment_items_beside_item_macros_keep_original_anchors() {
    let temp = Temp::new("doc-anchors");
    let root = temp.write(
        "main.rs",
        "macro_rules! m { () => { fn g() -> u32 { 3 } } }\nm!();\n/// doc one\n/** block */\n#[doc = \"explicit\"]\nfn foo() -> u32 { 4 }\nfn main() { if foo() + g() != 7 { panic!(\"bad\") } }\n",
    );
    match standalone_edition_with_capture(&root, None, 2015, &ExternCrates::default(), false, None)
    {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(_) => panic!("unexpected outcome"),
        Err(error) => panic!("{error}"),
    }
}
