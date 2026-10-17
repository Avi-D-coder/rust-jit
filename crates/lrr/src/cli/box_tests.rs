//! `Box<T>` through the ordinary standalone pipeline with the LRR standard
//! facade. A box is the alloc facade's `Box(T)` object, constructed by its
//! ordinary `Box::new` body; its field cell is the heap allocation. `*b` is
//! the builtin box projection (read, written, borrowed, moved out of);
//! methods, fields and indexing autoderef through it; dropping a box drops
//! its contents through ordinary drop glue. Expected outputs are rustc
//! 1.99.0's for the same programs. Unchanged upstream witnesses run from
//! `fixtures/upstream/rustc-checkout` under their own directive edition.
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
            "lrr-boxes-{label}-{}-{}",
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
    for edition in [2015, 2021] {
        match run(label, source, edition) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.output.stdout, expected, "edition {edition}\n{source}")
            }
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("edition {edition} panicked: {:?}\n{source}", report.panic)
            }
            Ok(_) => panic!("edition {edition}: unexpected outcome\n{source}"),
            Err(error) => panic!("edition {edition}: {error}\n{source}"),
        }
    }
}

/// An unchanged upstream run-pass witness completes under its directive
/// edition (rustc's default 2015 when it names none).
fn upstream_completes(path: &str, text: &str) {
    let edition = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("//@ edition:"))
        .map_or(2015, |edition| edition.trim().parse().unwrap());
    match run("upstream", text, edition) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(StandaloneOutcome::Panicked(report)) => panic!("{path} panicked: {:?}", report.panic),
        Ok(_) => panic!("{path}: unexpected outcome"),
        Err(error) => panic!("{path}: {error}"),
    }
}

const D: &str = "struct D(i32);
impl Drop for D {
    fn drop(&mut self) {
        println!(\"drop {}\", self.0);
    }
}
impl D {
    fn get(&self) -> i32 { self.0 }
    fn bump(&mut self) { self.0 += 100; }
    fn consume(self) -> i32 { self.0 }
}
";

#[test]
fn box_owns_one_heap_cell_moved_with_the_box_and_dropped_with_it() {
    // Moving a box moves its allocation (no drop); moving out of `*d`
    // leaves only the allocation; assigning `*f` drops the old contents;
    // locals drop in reverse declaration order with their contents.
    prints(
        "ownership",
        &format!(
            "{D}fn take(b: Box<D>) -> i32 {{ b.0 }}
fn main() {{
    let a = Box::new(D(1));
    let b = a;
    println!(\"b {{}}\", b.get());
    let mut c = Box::new(D(2));
    c.bump();
    println!(\"c {{}}\", c.get());
    let d = Box::new(D(3));
    let inner = *d;
    println!(\"moved {{}}\", inner.0);
    println!(\"take {{}}\", take(Box::new(D(4))));
    let e = Box::new(D(5));
    println!(\"consume {{}}\", e.consume());
    let mut f = Box::new(D(6));
    *f = D(7);
    println!(\"f {{}}\", f.0);
    let g = Box::new(Box::new(D(8)));
    println!(\"g {{}}\", g.get());
    println!(\"end\");
}}
"
        ),
        "b 1\nc 102\nmoved 3\ndrop 4\ntake 4\ndrop 5\nconsume 5\ndrop 6\nf 7\ng 8\nend\n\
         drop 8\ndrop 7\ndrop 3\ndrop 102\ndrop 1\n",
    );
}

#[test]
fn box_deref_reads_writes_and_borrows_its_heap_cell() {
    // `*b` read (Copy), compound-assigned and assigned in place; `&*b` and
    // `&mut *b` borrow the heap cell; the facade's `Deref` impl is ordinary
    // Rust over the same builtin projection (`&**self`).
    prints(
        "deref",
        "use std::ops::Deref;
fn main() {
    let mut b = Box::new(1);
    *b += 5;
    let x = *b;
    {
        let r = &mut *b;
        *r = *r * 10;
    }
    let s: &i32 = &*b;
    println!(\"{} {} {}\", x, *s, b);
    *b = 3;
    println!(\"{}\", *b);
    let c = Box::new(Box::new(4));
    println!(\"{}\", **c + 1);
    println!(\"{}\", *c.deref().deref());
}
",
        "6 60 60\n3\n5\n4\n",
    );
}

#[test]
fn box_methods_fields_operators_and_formatting_autoderef_to_the_contents() {
    prints(
        "autoderef",
        "trait Speak { fn speak(&self) -> i32; }
struct Dog(i32);
impl Speak for Dog { fn speak(&self) -> i32 { self.0 } }
struct P { x: i32, y: (i32, i32) }
impl Clone for P { fn clone(&self) -> P { P { x: self.x, y: self.y } } }
impl PartialEq for P { fn eq(&self, o: &P) -> bool { self.x == o.x } }
fn main() {
    let b = Box::new(P { x: 4, y: (5, 6) });
    let c = b.clone();
    println!(\"{} {} {}\", c.x, c.y.1, b == c);
    let r = &b;
    println!(\"{}\", r.x + r.y.0);
    let d = Box::new(Dog(9)) as Box<dyn Speak>;
    println!(\"{}\", d.speak());
    let e = Box::new(7);
    let f = e.clone();
    println!(\"{} {:?} {}\", e, f, e == f);
    let h = Box::new([1i32, 2, 3]);
    println!(\"{} {} {:?}\", h[1], h.len(), h);
}
",
        "4 6 true\n9\n9\n7 7 true\n2 3 [1, 2, 3]\n",
    );
}

#[test]
fn box_unsizes_and_deref_coerces_at_coercion_sites_and_formats_by_borrow() {
    // `Box<[T; N]>` meets `Box<[T]>` at a cast and a typed `let` (its
    // dereference is the heap cell's element window); `&Box<Box<i32>>`
    // meets a typed `&i32` by deref coercion; `format_args!` borrows its
    // place arguments (rustc's `Argument::new_display(&b)`), so formatting,
    // and a closure that only formats, never move the box.
    prints(
        "coercions",
        "fn main() {
    let x = Box::new([1i32, 2, 3]);
    let y = x as Box<[i32]>;
    println!(\"{:?} {} {}\", y, y.len(), y[2]);
    let z: Box<[i32]> = Box::new([4i32, 5]);
    println!(\"{:?} {}\", z, z[0]);
    let e = Box::new(Box::new(7));
    let r: &i32 = &e;
    println!(\"{}\", r);
    let b = Box::new(8);
    println!(\"{}\", b);
    println!(\"{}\", b);
    let shown = || println!(\"{}\", b);
    shown();
    let moved = b;
    println!(\"{}\", moved);
}
",
        "[1, 2, 3] 3 3\n[4, 5] 4\n7\n8\n8\n8\n8\n",
    );
}

#[test]
fn box_operations_mean_the_same_in_macro_generated_source() {
    // The same construction, deref, method and field operations emitted by
    // user macros select the same box projection and drop glue.
    prints(
        "macro",
        &format!(
            "{D}macro_rules! boxed {{ ($e:expr) => {{ Box::new($e) }}; }}
macro_rules! open {{ ($b:expr) => {{ *$b }}; }}
macro_rules! call {{ ($b:ident) => {{ $b.get() + $b.0 }}; }}
fn main() {{
    let b = boxed!(D(1));
    println!(\"{{}}\", call!(b));
    let n = boxed!(5);
    println!(\"{{}}\", open!(n) + 1);
    let inner = open!(b);
    println!(\"{{}}\", inner.0);
    assert!(*boxed!(2i32) == 2i32);
}}
"
        ),
        "2\n6\n1\ndrop 1\n",
    );
}

#[test]
fn upstream_box_witnesses_complete() {
    macro_rules! witness {
        ($path:literal) => {
            upstream_completes(
                $path,
                include_str!(concat!(
                    "../../../../fixtures/upstream/rustc-checkout/",
                    $path
                )),
            )
        };
    }
    witness!("tests/ui/array-slice-vec/issue-18425.rs");
    witness!("tests/ui/regions/owned-implies-static.rs");
    witness!("tests/ui/binding/match-reassign.rs");
    witness!("tests/ui/closures/issue-10682.rs");
    witness!("tests/ui/drop/drop-struct-as-object.rs");
    witness!("tests/ui/cast/cast-to-box-arr.rs");
    witness!("tests/ui/closures/moved-upvar-mut-rebind-11958.rs");
    witness!("tests/ui/traits/coercion-generic.rs");
    witness!("tests/ui/impl-trait/auto-trait-leakage/auto-trait-leak-rpass.rs");
    witness!("tests/ui/moves/move-4.rs");
}
