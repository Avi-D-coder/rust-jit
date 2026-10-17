//! Generic calls between generic functions and moves out of nominal
//! owners: a caller's own type parameters cross every call boundary as its
//! runtime instantiation (`Frame::bound_view`), and a field moves out of a
//! struct rustc admits it from (no `Drop` impl, E0509), leaving the
//! remaining fields to ordinary drop glue in declaration order.
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
            "lrr-generic-{label}-{}-{}",
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

fn run(
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

fn prints(label: &str, source: &str, expected: &str) {
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
fn generic_callers_pass_their_instantiation_to_generic_callees() {
    // The shape of `core::cell::Cell::new` over `UnsafeCell::new`.
    prints(
        "nested-generic",
        "struct U<T> { value: T }
impl<T> U<T> {
    fn new(value: T) -> U<T> { U { value } }
    fn into_inner(self) -> T { self.value }
}
struct C<T> { value: U<T> }
impl<T> C<T> {
    fn new(value: T) -> C<T> { C { value: U::new(value) } }
    fn into_inner(self) -> T { self.value.into_inner() }
}
fn wrap<T>(v: T) -> U<T> { let u = U::new(v); u }
macro_rules! wrapped { ($e:expr) => { wrap($e) }; }
fn main() {
    let c = C::new(5u8);
    let w = wrapped!(\"w\");
    println!(\"{} {} {}\", c.into_inner(), wrap(7i64).into_inner(), w.into_inner());
}
",
        "5 7 w\n",
    );
}

#[test]
fn partial_moves_out_of_structs_drop_the_rest_in_declaration_order() {
    prints(
        "partial-move",
        "struct D(&'static str);
impl Drop for D {
    fn drop(&mut self) { println!(\"drop {}\", self.0); }
}
struct P { a: D, b: D, c: D }
fn take(d: D) { println!(\"took {}\", d.0); }
fn main() {
    let p = P { a: D(\"a\"), b: D(\"b\"), c: D(\"c\") };
    take(p.b);
    println!(\"end\");
}
",
        "took b\ndrop b\nend\ndrop a\ndrop c\n",
    );
}
