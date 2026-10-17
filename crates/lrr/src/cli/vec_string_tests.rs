//! `Vec<T>` and `String` through the ordinary standalone pipeline with the
//! LRR standard facade. A `Vec<T>` is an interpreter object whose fields are
//! its element cells (`runtime/cell_vecs.rs`); the facade `alloc::vec`
//! writes every other operation as ordinary Rust over a handful of closed
//! `lrr_vec_*` intrinsics and the slice place projections. A `String` is a
//! `Vec<u8>` with a `&str` view. The same operations run handwritten, emitted
//! by a user `macro_rules!` macro and nested in `assert_eq!`; dropping a
//! vector drops its elements in index order. Expected outputs are rustc
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
            "lrr-vec-string-{label}-{}-{}",
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

/// The program panics after printing `expected`; the guest panic message is
/// rustc's.
fn panics(label: &str, source: &str, expected: &str, message: &str) {
    for edition in [2015, 2021] {
        match run(label, source, edition) {
            Ok(StandaloneOutcome::Panicked(report)) => {
                assert_eq!(report.output.stdout, expected, "edition {edition}\n{source}");
                let found = format!("{:?}", report.panic);
                assert!(
                    found.contains(message),
                    "edition {edition}: panic {found} lacks {message:?}\n{source}"
                );
            }
            Ok(StandaloneOutcome::Completed(_)) => {
                panic!("edition {edition}: expected a guest panic\n{source}")
            }
            Ok(_) => panic!("edition {edition}: unexpected outcome\n{source}"),
            Err(error) => panic!("edition {edition}: {error}\n{source}"),
        }
    }
}

/// An unchanged upstream run-pass witness completes under its directive
/// edition (rustc's default 2015 when it names none).
fn upstream_completes(path: &str) {
    let full = format!(
        "{}/../../fixtures/upstream/rustc-checkout/{path}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&full).unwrap();
    let edition = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("//@ edition:"))
        .map_or(2015, |edition| edition.trim().parse().unwrap());
    match run("upstream", &text, edition) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(StandaloneOutcome::Panicked(report)) => panic!("{path} panicked: {:?}", report.panic),
        Ok(_) => panic!("{path}: unexpected outcome"),
        Err(error) => panic!("{path}: {error}"),
    }
}

/// Handwritten element operations on an annotated vector: push, pop, index read/write/compound assignment, insert/remove/clear and a `for` over `&v`.
#[test]
fn vec_push_pop_index_and_iteration_through_references() {
    prints(
        "basic",
        r###"fn main() {
    let mut v: Vec<i32> = Vec::new();
    println!("{} {}", v.len(), v.is_empty());
    v.push(10);
    v.push(20);
    v.push(30);
    println!("{} {}", v.len(), v.is_empty());
    println!("{}", v[1]);
    v[1] = 25;
    v[2] += 5;
    println!("{} {} {}", v[0], v[1], v[2]);
    let p = v.pop();
    println!("{:?}", p);
    println!("{:?} {}", v.pop(), v.len());
    let mut sum = 0;
    for x in &v {
        sum += *x;
    }
    println!("sum {}", sum);
    v.insert(0, 7);
    let r = v.remove(1);
    println!("{} {}", r, v[0]);
    v.clear();
    println!("{:?} {}", v.pop(), v.len());
}
"###,
        r###"0 true
3 false
20
10 25 35
Some(35)
Some(25) 1
sum 10
10 7
None 0
"###,
    );
}

/// `iter`/`iter_mut`/by-value iteration, `first`/`last`/`get`, `contains`, `&Vec<T>` coercing to `&[T]` and a range window of the vector.
#[test]
fn vec_iterators_slices_and_deref_coercion() {
    prints(
        "viter",
        r###"fn main() {
    let mut v: Vec<u32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    let mut total: u32 = 0;
    for x in v.iter() {
        total += *x;
    }
    println!("total {}", total);
    for x in v.iter_mut() {
        *x *= 2;
    }
    println!("{:?}", v);
    let mut out: u32 = 0;
    for x in v {
        out = out * 10 + x;
    }
    println!("{}", out);
    let w: Vec<u32> = vec![4u32, 5, 6];
    println!("{:?} {:?} {:?}", w.first(), w.last(), w.get(1));
    println!("{}", w.contains(&5));
    let s: &[u32] = &w;
    println!("{} {}", s.len(), s[2]);
    let t = &w[1..3];
    println!("{} {}", t.len(), t[0]);
}
"###,
        r###"total 6
[2, 4, 6]
246
Some(4) Some(6) Some(5)
true
3 6
2 5
"###,
    );
}

/// The same operations emitted by user `macro_rules!` macros and nested in `assert_eq!`/`assert_ne!`/`assert!`.
#[test]
fn vec_operations_emitted_by_a_user_macro_and_nested_in_assertions() {
    prints(
        "macro_user",
        r###"macro_rules! fill {
    ($v:ident, $($x:expr),*) => {
        $( $v.push($x); )*
    };
}
macro_rules! total {
    ($v:expr) => {{
        let mut t: i64 = 0;
        let mut i: usize = 0;
        while i < $v.len() {
            t += $v[i];
            i += 1;
        }
        t
    }};
}
fn main() {
    let mut v: Vec<i64> = Vec::new();
    fill!(v, 5, 6, 7);
    println!("{} {}", v.len(), total!(v));
    assert_eq!(v.len(), 3);
    assert_eq!(v[0], 5);
    assert_eq!(v.pop(), Some(7i64));
    assert!(v.contains(&6));
    assert_eq!(total!(v), 11);
    println!("done");
}
"###,
        r###"3 18
done
"###,
    );
}

/// Dropping a vector, truncating, clearing and moving it drops its elements in index order; a vector field is dropped with its struct.
#[test]
fn vec_drops_elements_in_index_order_through_ordinary_cleanup() {
    prints(
        "drop_order",
        r###"struct D(i32);
impl Drop for D {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}
struct Holder {
    items: Vec<D>,
    tag: i32,
}
fn take(v: Vec<D>) -> usize {
    v.len()
}
fn main() {
    let mut v: Vec<D> = Vec::new();
    v.push(D(1));
    v.push(D(2));
    v.push(D(3));
    let x = v.remove(1);
    println!("removed {}", x.0);
    v.insert(0, D(9));
    println!("first {}", v[0].0);
    v.truncate(1);
    println!("after truncate {}", v.len());
    let mut h = Holder { items: Vec::new(), tag: 7 };
    h.items.push(D(20));
    h.items.push(D(21));
    println!("h {} {}", h.tag, h.items.len());
    let n = take(v);
    println!("took {}", n);
    let mut w: Vec<D> = Vec::new();
    w.push(D(30));
    w.push(D(31));
    w.clear();
    println!("cleared");
    println!("end");
}
"###,
        r###"removed 2
first 9
drop 1
drop 3
after truncate 1
h 7 2
drop 9
took 1
drop 30
drop 31
cleared
end
drop 20
drop 21
drop 2
"###,
    );
}

/// `String::from`, `format!`, `to_string`, `push_str`/`push`, `+`/`+=`, comparison with `&str` and `String`, `chars()` and `Debug`.
#[test]
fn string_construction_formatting_and_comparison() {
    prints(
        "strings",
        r###"fn main() {
    let s = String::from("abc");
    println!("{}", s);
    let t = format!("{}-{}", 1, 2);
    println!("{}", t);
    let u = 5i32.to_string();
    println!("{}", u);
    let n: u8 = 7;
    println!("{}", n.to_string());
    println!("{}", "str".to_string());
    println!("{}", 'c'.to_string());
    let mut w = String::new();
    w.push_str("héllo");
    w.push('!');
    println!("{} {}", w, w.len());
    let z = s + "def";
    println!("{}", z);
    println!("{}", z == "abcdef");
    println!("{}", z == String::from("abcdef"));
    let mut a = String::from("x");
    a += "yz";
    println!("{} {}", a, a.is_empty());
    a.clear();
    println!("[{}] {}", a, a.is_empty());
    let mut count = 0;
    for c in "héy".chars() {
        count += 1;
        println!("{}", c);
    }
    println!("{}", count);
    let c = String::from("clone me");
    let d = c.clone();
    println!("{} {}", c, d);
    println!("{:?}", d);
}
"###,
        r###"abc
1-2
5
7
str
c
héllo! 7
abcdef
true
true
xyz false
[] true
h
é
y
3
clone me clone me
"clone me"
"###,
    );
}

/// The pinned `vec!` macro's three forms (`vec![]`, `vec![a, b]`, `vec![x; n]`) through `Box::new_uninit`, `write_box_via_move` and `box_assume_init_into_vec_unsafe`.
#[test]
fn vec_macro_forms_expand_through_the_library_leaves() {
    prints(
        "vecmacro",
        r###"fn main() {
    let v = vec![1u8, 2, 3];
    println!("{} {}", v.len(), v[2]);
    let z = vec![String::from("x"), String::from("y")];
    println!("{} {}", z.len(), z[0]);
    let e: Vec<i32> = vec![];
    println!("{}", e.len());
    let r = vec![7u16; 3];
    println!("{} {}", r.len(), r[2]);
    let n = vec![vec![1u8], vec![2u8, 3]];
    println!("{} {}", n.len(), n[1][1]);
    let q = vec![1, 2, 3];
    println!("{}", q.len());
}
"###,
        r###"3 3
2 x
0
3 7
2 3
3
"###,
    );
}

/// Nested vectors, clone and equality, `collect` into a vector, `extend` from an iterator and pretty `Debug` output.
#[test]
fn nested_vectors_clone_equality_collect_extend_and_debug() {
    prints(
        "more",
        r###"fn main() {
    let mut grid: Vec<Vec<i32>> = Vec::new();
    let mut row: Vec<i32> = Vec::new();
    row.push(1);
    row.push(2);
    grid.push(row);
    let mut row2: Vec<i32> = Vec::new();
    row2.push(3);
    grid.push(row2);
    println!("{} {} {}", grid.len(), grid[0].len(), grid[1][0]);
    grid[0][1] = 20;
    println!("{:?}", grid);
    let copy = grid.clone();
    println!("{}", copy == grid);
    grid[1].push(4);
    println!("{} {}", copy == grid, grid[1].len());
    let v: Vec<u32> = (1u32..4).collect();
    println!("{:?}", v);
    let mut e: Vec<u32> = Vec::new();
    e.extend(v);
    e.extend(7u32..9);
    println!("{:?}", e);
    assert_eq!(e, vec![1u32, 2, 3, 7, 8]);
    println!("{:#?}", e);
    let names: Vec<String> = vec![String::from("a"), String::from("b")];
    println!("{:?} {}", names, names.len());
}
"###,
        r###"2 2 3
[[1, 20], [3]]
true
false 2
[1, 2, 3]
[1, 2, 3, 7, 8]
[
    1,
    2,
    3,
    7,
    8,
]
["a", "b"] 2
"###,
    );
}

/// `Vec`, `Option` and `String` values compared inside `assert_eq!` (`Vec == Vec`, `String == &str`, `format!`).
#[test]
fn vec_and_string_values_inside_assert_macros() {
    prints(
        "assert_in",
        r###"fn main() {
    let mut v: Vec<u8> = Vec::new();
    v.push(3);
    v.push(4);
    assert_eq!(v.len(), 2);
    assert_eq!(v[0], 3);
    assert_eq!(v.pop(), Some(4u8));
    assert_ne!(v.len(), 5);
    assert!(!v.is_empty());
    assert_eq!(v, vec![3u8]);
    let s = String::from("hi");
    assert_eq!(s, "hi");
    assert_eq!(s.len(), 2);
    assert_eq!(format!("{}{}", s, 1), "hi1");
    println!("ok");
}
"###,
        r###"ok
"###,
    );
}

/// Indexing a vector out of bounds is rustc's slice bounds panic, raised by
/// the element place projection after the output so far.
#[test]
fn vec_index_out_of_bounds_panics_with_rustcs_message() {
    panics(
        "oob",
        r###"fn main() {
    let v: Vec<i32> = vec![1i32, 2, 3];
    println!("start");
    println!("{}", v[5]);
}
"###,
        "start\n",
        "index out of bounds: the len is 3 but the index is 5",
    );
}

/// A partially consumed `into_iter` drops its remaining elements in index
/// order when the iterator is dropped; vectors of tuples, boxes and vectors
/// drop their contents in order at scope end.
#[test]
fn vec_by_value_iteration_and_nested_owners_drop_in_order() {
    prints(
        "edge",
        r###"struct D(i32);
impl Drop for D {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}
fn make(n: i32) -> Vec<D> {
    let mut v: Vec<D> = vec![D(n), D(n + 1)];
    v.push(D(n + 2));
    v
}
fn main() {
    let v = make(1);
    let mut it = v.into_iter();
    let first = it.next();
    let got = match &first {
        Some(d) => d.0,
        None => 0,
    };
    println!("got {}", got);
    drop(it);
    println!("after iter drop");
    let pairs: Vec<(i32, D)> = vec![(1, D(10)), (2, D(11))];
    println!("{}", pairs[1].0);
    let boxed: Vec<Box<D>> = vec![Box::new(D(20)), Box::new(D(21))];
    println!("{}", boxed.len());
    let mut nested: Vec<Vec<D>> = Vec::new();
    nested.push(make(30));
    nested.push(Vec::new());
    println!("{}", nested[0].len());
    println!("end");
}
"###,
        r###"got 1
drop 2
drop 3
after iter drop
2
2
3
end
drop 30
drop 31
drop 32
drop 20
drop 21
drop 10
drop 11
drop 1
"###,
    );
}

/// Unchanged upstream rustc run-pass fixtures that need `Vec`, `String`,
/// `format!` or `to_string` complete (before the library facade they stopped
/// at `lrr_unsupported` stubs).
#[test]
fn upstream_vec_and_string_witnesses_complete() {
    for path in [
        "tests/ui/array-slice-vec/empty-mutable-vec.rs",
        "tests/ui/array-slice-vec/mutable-alias-vec.rs",
        "tests/ui/array-slice-vec/nested-vec-1.rs",
        "tests/ui/array-slice-vec/nested-vec-2.rs",
        "tests/ui/array-slice-vec/vec-late-init.rs",
        "tests/ui/array-slice-vec/ivec-pass-by-value.rs",
        "tests/ui/array-slice-vec/vector-no-ann-2.rs",
        "tests/ui/binding/match-join.rs",
        "tests/ui/binding/match-borrowed_str.rs",
        "tests/ui/binding/match-pipe-binding.rs",
        "tests/ui/macros/assert-eq-macro-success.rs",
        "tests/ui/for-loop-while/while-prelude-drop.rs",
        "tests/ui/generics/generic-ivec-leak.rs",
        "tests/ui/structs/trie-node-structure-usage-3389.rs",
        "tests/ui/fmt/println-debug-different-types.rs",
    ] {
        upstream_completes(path);
    }
}
