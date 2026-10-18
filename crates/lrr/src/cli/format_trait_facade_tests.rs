//! Trait-method formatting through the standard facade: a `format_args!`
//! placeholder whose value is not a native scalar/str calls its trait's
//! `fmt` impl (`vm/frame_format_calls.rs`, `engine/run_format_calls.rs`),
//! handwritten or emitted by a user macro, nested inside another impl's
//! `write!`, through the `Formatter` options and the `Debug` builders; plus
//! the field, cast and coercion shapes those impls rely on.
use super::capture_with;
use crate::{
    cli::{StandaloneOutcome, runtime_capture::Request, standalone_captured},
    core::profile::ProfileIdentity,
};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lrr-format-{label}-{}-{}",
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

pub(in crate::cli) fn run(
    label: &str,
    source: &str,
    edition: u16,
) -> Result<StandaloneOutcome, crate::core::diagnostic::Diagnostic> {
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
    standalone_captured(
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
    )
}

pub(super) fn prints(label: &str, source: &str, expected: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        match run(label, source, edition) {
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
fn user_display_and_debug_impls_format_through_their_trait() {
    prints(
        "display",
        "use std::fmt;
struct P { x: i32 }
impl fmt::Display for P {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, \"P({})\", self.x)
    }
}
impl fmt::Debug for P {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(\"P\").field(\"x\", &self.x).finish()
    }
}
fn main() {
    let p = P { x: 3 };
    println!(\"{} {:?} [{}]\", p, p, &p);
    println!(\"{:#?}\", p);
}
",
        "P(3) P { x: 3 } [P(3)]\nP {\n    x: 3,\n}\n",
    );
}

#[test]
fn nested_impls_macro_emitted_impls_and_formatter_options() {
    prints(
        "nested",
        "use std::fmt;
struct Inner(u8);
struct Outer { a: Inner, b: Inner }
macro_rules! show {
    ($t:ty, $text:expr) => {
        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, $text, self.0)
            }
        }
    };
}
show!(Inner, \"<{}>\");
impl fmt::Display for Outer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            write!(f, \"{}+{}\", self.a, self.b)
        } else {
            f.write_str(\"outer\")
        }
    }
}
struct Pad(&'static str);
impl fmt::Display for Pad {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.pad(self.0)
    }
}
fn main() {
    let o = Outer { a: Inner(1), b: Inner(2) };
    println!(\"{} {:#}\", o, o);
    println!(\"[{:>5}] [{:<4}]\", Pad(\"ab\"), Pad(\"c\"));
}
",
        "outer <1>+<2>\n[   ab] [c   ]\n",
    );
}

#[test]
fn builtin_derived_debug_formats_through_the_generated_impl() {
    // The builtin derive's impl names `$crate::fmt::Formatter`; its
    // signature resolves that root in core like any other written Type.
    prints(
        "derive-debug",
        "#[derive(Debug)]
struct P { x: i32, y: bool }
#[derive(Debug)]
struct T(u8, char);
#[derive(Debug)]
enum E { A, B(i32), C { z: u8 } }
#[derive(Debug)]
struct Outer { p: P, e: E }
fn main() {
    println!(\"{:?}\", P { x: 3, y: true });
    println!(\"{:?}\", T(1, 'c'));
    println!(\"{:?} {:?} {:?}\", E::A, E::B(2), E::C { z: 9 });
    println!(\"{:#?}\", Outer { p: P { x: -1, y: false }, e: E::B(7) });
}
",
        "P { x: 3, y: true }\nT(1, 'c')\nA B(2) C { z: 9 }\nOuter {\n    p: P {\n        x: -1,\n        y: false,\n    },\n    e: B(\n        7,\n    ),\n}\n",
    );
}

#[test]
fn alias_typed_and_reference_typed_fields_construct_and_read() {
    // `result: Result` (an alias of a generic application) is its target's
    // structure; `d.f.n` reads through a reference stored in a field.
    prints(
        "field-kinds",
        "#[derive(Clone, Copy)]
struct E;
type R = Result<(), E>;
struct G { r: R, h: bool }
struct F<'a> { n: &'a str }
struct D<'a, 'b: 'a> { f: &'a mut F<'b>, h: bool }
fn mk<'a, 'b>(f: &'a mut F<'b>) -> D<'a, 'b> { D { f: f, h: false } }
fn main() {
    let g = G { r: Ok(()), h: true };
    let mut f = F { n: \"x\" };
    let d = mk(&mut f);
    println!(\"{} {} {} {}\", g.h, g.r.is_ok(), d.h, d.f.n);
}
",
        "true true false x\n",
    );
}

#[test]
fn reference_unsizing_casts_coerce_like_annotations() {
    // `&arr as &[i32]` is rustc's unsizing coercion cast, the same coercion
    // a `let x: &[i32] = &arr;` annotation performs.
    prints(
        "unsizing-cast",
        "fn total(xs: &[i32]) -> i32 {
    let mut t = 0;
    let mut i = 0;
    while i < xs.len() { t += xs[i]; i += 1; }
    t
}
fn main() {
    let arr = [1, 2, 3];
    let a = &arr as &[i32];
    let m = &[4u8, 5] as &[u8];
    println!(\"{} {} {}\", a.len(), total(a), m[1]);
}
",
        "3 6 5\n",
    );
}
