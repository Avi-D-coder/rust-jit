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

// --- slice 2: user Deref, smart pointers, library breadth ---

/// `prints` for programs whose expected output is rustc's under 2021 only
/// (array `into_iter` by value, or heavy enough that one edition suffices).
fn prints21(label: &str, source: &str, expected: &str) {
    match run(label, source, 2021) {
        Ok(StandaloneOutcome::Completed(report)) => {
            assert_eq!(report.output.stdout, expected, "edition 2021\n{source}")
        }
        Ok(StandaloneOutcome::Panicked(report)) => {
            panic!("edition 2021 panicked: {:?}\n{source}", report.panic)
        }
        Ok(_) => panic!("edition 2021: unexpected outcome\n{source}"),
        Err(error) => panic!("edition 2021: {error}\n{source}"),
    }
}

/// Method receivers autoderef through user `Deref`/`DerefMut` impls (single, double, mutable, through a reference, to a `String` and to a `Vec`'s slice).
#[test]
fn user_deref_method_autoderef_chains() {
    prints(
        "deref",
        r####"use std::ops::{Deref, DerefMut};
struct In { n: i32 }
impl In {
    fn get(&self) -> i32 { self.n }
    fn bump(&mut self) { self.n += 1; }
}
struct W2 { i: In }
impl Deref for W2 { type Target = In; fn deref(&self) -> &In { &self.i } }
impl DerefMut for W2 { fn deref_mut(&mut self) -> &mut In { &mut self.i } }
struct W3 { w: W2 }
impl Deref for W3 { type Target = W2; fn deref(&self) -> &W2 { &self.w } }
impl DerefMut for W3 { fn deref_mut(&mut self) -> &mut W2 { &mut self.w } }
struct Text(String);
impl Deref for Text { type Target = String; fn deref(&self) -> &String { &self.0 } }
fn main() {
    let mut w2 = W2 { i: In { n: 4 } };
    println!("{}", w2.get());
    w2.bump();
    println!("{}", w2.get());
    let mut w3 = W3 { w: W2 { i: In { n: 10 } } };
    println!("{}", w3.get());
    w3.bump();
    println!("{}", w3.get());
    let r = &w3;
    println!("{}", r.get());
    let t = Text(String::from("deref chain"));
    println!("{} {}", t.len(), t.chars().count());
    let v: Vec<i32> = vec![3, 1, 2];
    println!("{} {:?} {:?}", v.iter().count(), v.first(), v.last());
    println!("{}", v.contains(&2));
}
"####,
        r####"4
5
10
11
11
11 11
3 Some(3) Some(2)
true
"####,
    );
}

/// `*x`, `x.field` and `x.f = v` through overloaded `Deref`/`DerefMut` take the trait method of the place's use (read, assignment, compound assignment, `&mut`).
#[test]
fn explicit_overloaded_deref_of_places_fields_and_assignment() {
    prints(
        "derefs",
        r####"use std::ops::{Deref, DerefMut};
struct W(i32);
impl Deref for W {
    type Target = i32;
    fn deref(&self) -> &i32 {
        &self.0
    }
}
impl DerefMut for W {
    fn deref_mut(&mut self) -> &mut i32 {
        &mut self.0
    }
}
struct Name(String);
impl Deref for Name {
    type Target = String;
    fn deref(&self) -> &String {
        &self.0
    }
}
struct Pair {
    a: i32,
    b: i32,
}
struct Holder(Pair);
impl Deref for Holder {
    type Target = Pair;
    fn deref(&self) -> &Pair {
        &self.0
    }
}
impl DerefMut for Holder {
    fn deref_mut(&mut self) -> &mut Pair {
        &mut self.0
    }
}
fn main() {
    let mut w = W(5);
    println!("{}", *w);
    println!("{}", *w + 1);
    *w += 2;
    println!("{}", w.0);
    *w = 10;
    println!("{}", w.0);
    let r: &i32 = &*w;
    println!("{}", r);
    let m: &mut i32 = &mut *w;
    *m += 1;
    println!("{}", w.0);
    let n = Name(String::from("deref"));
    println!("{} {}", n.len(), n.to_uppercase());
    let mut h = Holder(Pair { a: 1, b: 2 });
    println!("{} {}", h.a, h.b);
    h.a = 7;
    h.b += 5;
    println!("{} {}", h.a, h.b);
    let t = Holder(Pair { a: 3, b: 4 });
    println!("{}", t.a * t.b);
}
"####,
        r####"5
6
7
10
10
11
5 DEREF
1 2
7 7
12
"####,
    );
}

/// `Rc`, `Weak`, `RefCell` guards and a user smart pointer: `*b`, `rc.field`, `guard[i]`, `guard.0 += 1`, `*cell.borrow_mut() = v`.
#[test]
fn rc_weak_refcell_and_user_boxes_share_one_deref_route() {
    prints21(
        "rc2",
        r####"use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::{Rc, Weak};
struct Node {
    name: String,
    parent: RefCell<Weak<Node>>,
    children: RefCell<Vec<Rc<Node>>>,
}
struct MyBox<T>(T);
impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T> DerefMut for MyBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}
struct Point {
    x: i32,
    y: i32,
}
fn main() {
    let leaf = Rc::new(Node {
        name: String::from("leaf"),
        parent: RefCell::new(Weak::<Node>::new()),
        children: RefCell::new(Vec::new()),
    });
    println!("{}", leaf.parent.borrow().upgrade().is_none());
    let branch = Rc::new(Node {
        name: String::from("branch"),
        parent: RefCell::new(Weak::<Node>::new()),
        children: RefCell::new(vec![Rc::clone(&leaf)]),
    });
    *leaf.parent.borrow_mut() = Rc::downgrade(&branch);
    match leaf.parent.borrow().upgrade() {
        Some(p) => println!("{}", p.name.len()),
        None => println!("none"),
    }
    println!("{} {}", Rc::strong_count(&branch), Rc::weak_count(&branch));
    println!("{} {}", Rc::strong_count(&leaf), branch.children.borrow().len());
    println!("{}", branch.children.borrow()[0].name);
    let b = MyBox(5);
    println!("{} {}", *b, *b + 1);
    let mut m = MyBox(Point { x: 1, y: 2 });
    println!("{} {}", m.x, m.y);
    m.x = 10;
    m.y += 5;
    println!("{} {}", m.x, m.y);
    let mut n = MyBox(3);
    *n += 4;
    *n = *n * 2;
    println!("{}", *n);
    let shared = Rc::new(Point { x: 7, y: 8 });
    let other = Rc::clone(&shared);
    println!("{} {}", other.x + shared.y, *Rc::new(9i32));
    let cell = RefCell::new(vec![1i32, 2, 3]);
    *cell.borrow_mut() = vec![4i32, 5];
    cell.borrow_mut()[0] = 9;
    println!("{:?} {}", cell.borrow(), cell.borrow()[1]);
    let grid = RefCell::new((1i32, 2i32));
    grid.borrow_mut().0 += 10;
    println!("{:?}", grid.borrow());
}
"####,
        r####"true
6
1 1
2 1
leaf
5 6
1 2
10 7
14
15 9
[9, 5] 5
(11, 2)
"####,
    );
}

/// `Rc` reference counts, `Weak::upgrade`, `try_unwrap`/`get_mut`, and the last owner dropping its contents.
#[test]
fn rc_counts_weak_upgrade_try_unwrap_and_drop_order() {
    prints(
        "rc",
        r####"use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
struct D(i32);
impl D {
    fn val(&self) -> i32 {
        self.0
    }
}
impl Drop for D {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}
struct Counter {
    hits: Cell<i32>,
    log: RefCell<Vec<String>>,
}
impl Counter {
    fn hit(&self, what: &str) {
        self.hits.set(self.hits.get() + 1);
        self.log.borrow_mut().push(what.to_string());
    }
}
fn main() {
    let a = Rc::new(D(1));
    println!("{}", Rc::strong_count(&a));
    let b = Rc::clone(&a);
    println!("{} {}", Rc::strong_count(&a), a.val());
    drop(b);
    println!("{}", Rc::strong_count(&a));
    let c = a.clone();
    println!("{}", Rc::ptr_eq(&a, &c));
    drop(a);
    println!("before last");
    drop(c);
    println!("after last");
    let shared = Rc::new(RefCell::new(vec![1i32, 2]));
    let other = Rc::clone(&shared);
    other.borrow_mut().push(3);
    shared.borrow_mut().push(4);
    println!("{:?} {}", shared.borrow(), Rc::strong_count(&shared));
    println!("{}", shared.borrow().len());
    let counter = Rc::new(Counter { hits: Cell::new(0), log: RefCell::new(Vec::new()) });
    let again = Rc::clone(&counter);
    counter.hit("a");
    again.hit("b");
    println!("{} {:?}", counter.hits.get(), counter.log.borrow());
    let weak: Weak<Counter> = Rc::downgrade(&counter);
    println!("{} {}", Rc::strong_count(&counter), Rc::weak_count(&counter));
    println!("{}", weak.upgrade().is_some());
    drop(counter);
    drop(again);
    println!("{}", weak.upgrade().is_none());
    let unique = Rc::new(String::from("solo"));
    println!("{:?}", Rc::try_unwrap(unique));
    let mut owned = Rc::new(5i32);
    println!("{}", Rc::get_mut(&mut owned).is_some());
    let copy = Rc::clone(&owned);
    println!("{}", Rc::get_mut(&mut owned).is_none());
    println!("{}", copy);
    let cell = RefCell::new(5);
    println!("{:?}", cell);
    let held = cell.borrow_mut();
    println!("{} {}", cell.try_borrow().is_err(), cell.try_borrow_mut().is_err());
    drop(held);
    println!("{}", cell.try_borrow().is_ok());
    println!("{}", cell.replace(9));
    println!("{}", cell.into_inner());
}
"####,
        r####"1
2 1
1
true
before last
drop 1
after last
[1, 2, 3, 4] 2
4
2 ["a", "b"]
2 1
true
true
Ok("solo")
true
true
5
RefCell { value: 5 }
true true
true
5
9
"####,
    );
}

/// `Cell`, `RefCell::borrow`/`borrow_mut`/`try_borrow*`/`replace`/`into_inner` and guard drops.
#[test]
fn cell_and_refcell_borrow_flags_and_guards() {
    prints(
        "cells",
        r####"use std::cell::{Cell, RefCell};
fn main() {
    let c = Cell::new(5);
    c.set(c.get() + 1);
    println!("{}", c.get());
    let old = c.replace(10);
    println!("{} {}", old, c.get());
    let r = RefCell::new(vec![1i32, 2, 3]);
    r.borrow_mut().push(4);
    println!("{:?}", r.borrow());
    println!("{}", r.borrow().len());
    {
        let mut g = r.borrow_mut();
        g.push(5);
        println!("{}", g.len());
    }
    println!("{:?}", r);
    println!("{}", r.try_borrow_mut().is_ok());
    let held = r.borrow_mut();
    println!("{} {}", r.try_borrow().is_err(), r.try_borrow_mut().is_err());
    drop(held);
    println!("{}", r.try_borrow().is_ok());
    let cell = RefCell::new(5);
    println!("{}", cell.replace(9));
    println!("{}", cell.into_inner());
    let tuple = RefCell::new((1i32, 2i32));
    tuple.borrow_mut().0 += 10;
    println!("{:?}", tuple.borrow());
}
"####,
        r####"6
6 10
[1, 2, 3, 4]
4
5
RefCell { value: [1, 2, 3, 4, 5] }
true
true true
true
5
9
(11, 2)
"####,
    );
}

/// `sort`, `sort_by`, `sort_by_key`, `windows`, `chunks`, `retain`, `drain(range)` and `iter().rev()` (closures receiving references).
#[test]
fn slice_methods_sort_windows_chunks_retain_drain_and_closures() {
    prints(
        "slices2",
        r####"fn main() {
    let mut v: Vec<i32> = vec![5, 3, 9, 1, 7, 3];
    v.sort();
    println!("{:?}", v);
    v.sort_by(|a, b| b.cmp(a));
    println!("{:?}", v);
    let mut w: Vec<(i32, u8)> = vec![(2, 1), (1, 2), (2, 0), (1, 1)];
    w.sort_by_key(|p| p.0);
    println!("{:?}", w);
    let n: Vec<i32> = vec![1, 2, 3, 4, 5];
    for win in n.windows(2) {
        println!("{} {}", win[0], win[1]);
    }
    for c in n.chunks(2) {
        println!("{}", c.len());
    }
    let mut r: Vec<i32> = vec![1, 2, 3, 4, 5, 6];
    r.retain(|x| x % 2 == 0);
    println!("{:?}", r);
    let mut d: Vec<i32> = vec![1, 2, 3, 4, 5];
    let taken: Vec<i32> = d.drain(1..3).collect();
    println!("{:?} {:?}", taken, d);
    let all: Vec<i32> = d.drain(..).collect();
    println!("{:?} {}", all, d.len());
    let mut total: i32 = 0;
    for x in n.iter().rev() {
        total = total * 10 + *x;
    }
    println!("{}", total);
    println!("{} {}", n.starts_with(&[1i32, 2]), n.ends_with(&[4i32, 5]));
    let copy: Vec<i32> = n[1..4].to_vec();
    println!("{:?}", copy);
}
"####,
        r####"[1, 3, 3, 5, 7, 9]
[9, 7, 5, 3, 3, 1]
[(1, 2), (1, 1), (2, 1), (2, 0)]
1 2
2 3
3 4
4 5
2
2
1
[2, 4, 6]
[2, 3] [1, 4, 5]
[1, 4, 5] 0
54321
true true
[2, 3, 4]
"####,
    );
}

/// `trim*`, `split`, `split_whitespace`, `lines`, `find`, `contains`, `starts_with`, `parse::<T>`, case mapping, `repeat`, `replace`, `from_utf8` and `str` ordering.
#[test]
fn str_methods_parse_split_find_case_repeat_and_ordering() {
    prints21(
        "strs3",
        r####"fn main() {
    let s = "  hello world  ";
    println!("[{}] [{}] [{}]", s.trim(), s.trim_start(), s.trim_end());
    let parts: Vec<&str> = "a,b,,c".split(',').collect();
    println!("{:?} {}", parts, parts.len());
    let words: Vec<&str> = "  the quick  brown fox ".split_whitespace().collect();
    println!("{:?}", words);
    let text = "one\ntwo\r\nthree\n";
    for line in text.lines() {
        println!("<{}>", line);
    }
    println!("{:?} {:?}", "hello".find('l'), "hello".find("lo"));
    println!("{} {} {}", "hello".contains("ell"), "hello".starts_with("he"), "hello".ends_with('o'));
    let n = "42".parse::<i32>().unwrap();
    println!("{}", n + 1);
    let bad = "4x2".parse::<i32>();
    println!("{:?}", bad);
    println!("{:?} {:?}", "255".parse::<u8>(), "256".parse::<u8>());
    println!("{:?}", "-129".parse::<i8>());
    println!("{}", "MiXed".to_uppercase());
    println!("{}", "MiXed".to_lowercase());
    println!("{}", "ab".repeat(3));
    println!("{}", "a-b-c".replace("-", "+"));
    let rev: String = "héy".chars().rev().collect();
    println!("{}", rev);
    for (i, c) in "aé".char_indices() {
        println!("{} {}", i, c);
    }
    println!("{}", "xxabcxx".trim_matches('x'));
    let owned = String::from("  pad ");
    println!("[{}]", owned.trim());
    println!("{}", owned.len());
    println!("{:?}", String::from_utf8(vec![104u8, 105u8]));
    println!("{}", String::from_utf8(vec![255u8]).is_err());
    println!("{}", "apple" < "banana");
    println!("{}", String::from("b") > String::from("a"));
    println!("{:?}", "abc".cmp("abd"));
}
"####,
        r####"[hello world] [hello world  ] [  hello world]
["a", "b", "", "c"] 4
["the", "quick", "brown", "fox"]
<one>
<two>
<three>
Some(2) Some(3)
true true true
43
Err(ParseIntError { kind: InvalidDigit })
Ok(255) Err(ParseIntError { kind: PosOverflow })
Err(ParseIntError { kind: NegOverflow })
MIXED
mixed
ababab
a+b+c
yéh
0 a
1 é
abc
[pad]
6
Ok("hi")
true
true
true
Less
"####,
    );
}

/// `join`/`concat` for strings and vectors, `Vec == [T; N]`/`&[T]`/`Vec`, `Vec` ordering, `max`/`min`, tuple `clone`, `zip` and `sum`/`product`.
#[test]
fn join_concat_vec_comparisons_tuples_of_references_and_sums() {
    prints(
        "joins",
        r####"fn main() {
    let words: Vec<String> = vec![String::from("a"), String::from("bc"), String::from("d")];
    println!("{}", words.join(", "));
    println!("{}", words.concat());
    let parts = vec!["x", "y", "z"];
    println!("{}", parts.join("-"));
    println!("{}", parts.concat());
    let vv: Vec<Vec<i32>> = vec![vec![1, 2], vec![3], vec![4, 5]];
    println!("{:?}", vv.concat());
    println!("{:?}", vv.join(&0i32));
    let v: Vec<i32> = vec![1, 2, 3];
    let w: Vec<i32> = vec![1, 2, 3];
    println!("{} {}", v == [1i32, 2, 3], v == w);
    let a = [1i32, 2, 3];
    let s: &[i32] = &a;
    println!("{} {}", v == s, v[..] == a[..]);
    let lo: Vec<i32> = vec![1, 2];
    let hi: Vec<i32> = vec![1, 3];
    println!("{} {:?}", lo < hi, lo.cmp(&hi));
    let names: Vec<&str> = vec!["b", "a"];
    println!("{:?} {:?}", names.iter().max(), names.iter().min());
    let t: (i32, &str) = (1, "a");
    let u = t.clone();
    println!("{:?}", u);
    let pairs: Vec<(i32, &str)> = vec![(1, "a"), (2, "b")];
    let copy = pairs.clone();
    println!("{:?} {}", copy, copy == pairs);
    let xs: Vec<i32> = vec![4, 5, 6];
    let ys: Vec<i32> = vec![7, 8, 9];
    let zipped: Vec<(&i32, &i32)> = xs.iter().zip(ys.iter()).collect();
    println!("{:?}", zipped);
    let total: i32 = xs.iter().sum();
    let product: i32 = xs.iter().product();
    println!("{} {}", total, product);
}
"####,
        r####"a, bc, d
abcd
x-y-z
xyz
[1, 2, 3, 4, 5]
[1, 2, 0, 3, 0, 4, 5]
true true
true true
true Less
Some("b") Some("a")
(1, "a")
[(1, "a"), (2, "b")] true
[(4, 7), (5, 8), (6, 9)]
15 120
"####,
    );
}

/// `BTreeMap` (entry API, ordered iteration, `first_key_value`), `BTreeSet` set algebra, `VecDeque` and `BinaryHeap`.
#[test]
fn ordered_maps_sets_deques_and_heaps() {
    prints21(
        "btree",
        r####"use std::collections::{BTreeMap, BTreeSet, BinaryHeap, VecDeque};
fn main() {
    let mut words: BTreeMap<String, usize> = BTreeMap::new();
    for w in vec!["pear", "apple", "fig", "apple", "pear", "apple"] {
        let count = words.entry(String::from(w)).or_insert(0);
        *count += 1;
    }
    println!("{:?}", words);
    println!("{:?} {:?}", words.get("apple"), words.get("kiwi"));
    println!("{}", words.contains_key("fig"));
    for (k, v) in words.iter_mut() {
        *v += k.len();
    }
    for (k, v) in &words {
        println!("{}={}", k, v);
    }
    println!("{:?}", words.first_key_value());
    println!("{:?}", words.last_key_value());
    let keys: Vec<&String> = words.keys().collect();
    println!("{:?}", keys);
    let total: usize = words.values().sum();
    println!("{}", total);
    println!("{:?}", words.remove("fig"));
    println!("{}", words.len());
    let mut inv: BTreeMap<i32, &str> = BTreeMap::new();
    inv.insert(5, "five");
    inv.insert(1, "one");
    inv.insert(3, "three");
    inv.entry(3).and_modify(|v| *v = "THREE").or_insert("x");
    inv.entry(4).or_default();
    println!("{:?}", inv);
    let cloned = inv.clone();
    println!("{}", cloned == inv);
    let mut set: BTreeSet<i32> = BTreeSet::new();
    for x in [5i32, 1, 3, 1, 5] {
        set.insert(x);
    }
    println!("{:?} {}", set, set.len());
    println!("{} {}", set.contains(&3), set.contains(&4));
    let other: BTreeSet<i32> = [3, 4, 5].into_iter().collect();
    let union: Vec<&i32> = set.union(&other).collect();
    let inter: Vec<&i32> = set.intersection(&other).collect();
    let diff: Vec<&i32> = set.difference(&other).collect();
    println!("{:?} {:?} {:?}", union, inter, diff);
    let mut dq: VecDeque<i32> = VecDeque::new();
    dq.push_back(1);
    dq.push_back(2);
    dq.push_front(0);
    println!("{:?} {:?} {:?}", dq, dq.front(), dq.back());
    println!("{:?} {:?} {}", dq.pop_front(), dq.pop_back(), dq.len());
    let mut heap: BinaryHeap<i32> = BinaryHeap::new();
    heap.push(3);
    heap.push(9);
    heap.push(1);
    println!("{:?}", heap.peek());
    println!("{:?}", heap.pop());
    println!("{:?}", heap.into_sorted_vec());
}
"####,
        r####"{"apple": 3, "fig": 1, "pear": 2}
Some(3) None
true
apple=8
fig=4
pear=6
Some(("apple", 8))
Some(("pear", 6))
["apple", "fig", "pear"]
18
Some(4)
2
{1: "one", 3: "THREE", 4: "", 5: "five"}
true
{1, 3, 5} 3
true false
[1, 3, 4, 5] [3, 5] [1]
[0, 1, 2] Some(0) Some(2)
Some(0) Some(2) 1
Some(9)
Some(9)
[1, 3]
"####,
    );
}

/// `HashMap` (`insert`/`get`/`entry`/`remove`/`into_iter`) and `HashSet` with string and `char` keys.
#[test]
fn hash_maps_and_sets() {
    prints(
        "hashm",
        r####"use std::collections::{HashMap, HashSet};
fn main() {
    let mut scores: HashMap<String, i32> = HashMap::new();
    scores.insert(String::from("ann"), 3);
    scores.insert(String::from("bob"), 5);
    scores.insert(String::from("ann"), 4);
    println!("{} {:?} {:?}", scores.len(), scores.get("ann"), scores.get("zed"));
    *scores.entry(String::from("cy")).or_insert(0) += 7;
    let mut keys: Vec<&String> = scores.keys().collect();
    keys.sort();
    println!("{:?}", keys);
    let mut total = 0;
    for (_, v) in &scores {
        total += *v;
    }
    println!("{}", total);
    if let Some(v) = scores.get_mut("bob") {
        *v *= 2;
    }
    println!("{:?}", scores.get("bob"));
    println!("{:?}", scores.remove("ann"));
    println!("{}", scores.contains_key("ann"));
    let mut counts: HashMap<char, usize> = HashMap::new();
    for c in "hello world".chars() {
        *counts.entry(c).or_insert(0) += 1;
    }
    let mut pairs: Vec<(char, usize)> = counts.into_iter().collect();
    pairs.sort();
    println!("{:?}", pairs);
    let mut seen: HashSet<i32> = HashSet::new();
    for x in vec![3, 1, 3, 2, 1] {
        if !seen.insert(x) {
            println!("dup {}", x);
        }
    }
    let mut items: Vec<&i32> = seen.iter().collect();
    items.sort();
    println!("{:?} {}", items, seen.contains(&2));
    let single: HashMap<&str, i32> = HashMap::from([("k", 1)]);
    println!("{:?}", single);
}
"####,
        r####"2 Some(4) None
["ann", "bob", "cy"]
16
Some(10)
Some(4)
false
[(' ', 1), ('d', 1), ('e', 1), ('h', 1), ('l', 3), ('o', 2), ('r', 1), ('w', 1)]
dup 3
dup 1
[1, 2, 3] true
{"k": 1}
"####,
    );
}

/// `for x in [D(1), D(2), D(3)]` with a `break`: the yielded element drops with its iteration, the rest drop with the iterator.
#[test]
fn array_by_value_iteration_drops_unyielded_elements_in_order() {
    prints(
        "arrays",
        r####"struct D(i32);
impl Drop for D {
    fn drop(&mut self) {
        println!("drop {}", self.0);
    }
}
fn main() {
    let mut total: i32 = 0;
    for x in [5i32, 1, 3] {
        total += x;
    }
    println!("{}", total);
    for d in [D(1), D(2), D(3)] {
        println!("got {}", d.0);
        if d.0 == 2 {
            break;
        }
    }
    println!("end");
}
"####,
        r####"9
got 1
drop 1
got 2
drop 2
drop 3
end
"####,
    );
}

/// A generic type with a `Drop` impl, closures forwarding reference arguments, an iterator yielding `(&K, &V)` and by-value array loops.
#[test]
fn generic_drop_reference_forwarding_closures_and_tuple_of_reference_items() {
    prints(
        "misc2",
        r####"struct W<T>(T);
impl<T> Drop for W<T> {
    fn drop(&mut self) {
        println!("drop W");
    }
}
fn call<T, F: FnMut(&T, &T) -> bool>(a: &T, b: &T, mut f: F) -> bool {
    f(a, b)
}
struct Pairs<'a, K, V> {
    s: &'a [(K, V)],
    i: usize,
}
impl<'a, K, V> Iterator for Pairs<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        if self.i < self.s.len() {
            let e = &self.s[self.i];
            self.i += 1;
            Some((&e.0, &e.1))
        } else {
            None
        }
    }
}
fn main() {
    let w = W(5);
    println!("made {}", w.0);
    let x = 3i32;
    let y = 4i32;
    println!("{}", call(&x, &y, |a, b| a < b));
    println!("{}", call(&x, &y, |a, b| b.cmp(a).is_le()));
    let v: Vec<(i32, i32)> = vec![(1, 2), (3, 4)];
    let it = Pairs { s: &v[..], i: 0 };
    for (k, val) in it {
        println!("{} {}", k, val);
    }
    let t: (i32, &str) = (1, "a");
    println!("{:?}", t.clone());
    for x in [5i32, 1, 3] {
        println!("{}", x);
    }
    println!("end");
}
"####,
        r####"made 5
true
false
1 2
3 4
(1, "a")
5
1
3
end
drop W
"####,
    );
}

/// Unchanged upstream rustc run-pass fixtures that complete with the second
/// slice (`Rc`/`RefCell`, `HashMap`, `mem::swap`, `str::parse`/`split_whitespace`,
/// overloaded `Deref` of places and receivers).
#[test]
fn upstream_smart_pointer_collection_and_deref_witnesses_complete() {
    for path in [
        "tests/ui/binding/borrowed-ptr-pattern-2.rs",
        "tests/ui/binding/match-with-ret-arm.rs",
        "tests/ui/borrowck/borrow-box-in-map-3026.rs",
        "tests/ui/borrowck/borrowck-mut-uniq.rs",
        "tests/ui/borrowck/issue-23338-params-outlive-temps-of-body.rs",
        "tests/ui/borrowck/ref-mut-rebind-does-not-affect-outer.rs",
        "tests/ui/deref/deref-newtype-method-call.rs",
        "tests/ui/drop/drop-with-type-ascription-1.rs",
        "tests/ui/drop/rc-trait-object-double-drop.rs",
        "tests/ui/mir/mir_early_return_scope.rs",
        "tests/ui/nll/rc-loop.rs",
        "tests/ui/rfcs/rfc-2005-default-binding-mode/for-ok.rs",
        "tests/ui/traits/issue-6128.rs",
    ] {
        upstream_completes(path);
    }
}

/// Deref coercion at every borrow coercion site (call arguments, typed `let`s,
/// struct fields, returns, reference-valued reads): `&Vec<T>` to `&[T]`,
/// `&String` to `&str`, `&Rc<T>`/`&RefCell` guard/`&Box<Vec<T>>` to their
/// targets and a user `Deref` type in two steps (the Rust-book `hello(&m)`).
/// Operands that already have the expected type need no step.
#[test]
fn deref_coercion_at_call_arguments_lets_fields_and_returns() {
    prints(
        "coerce",
        r###"use std::cell::RefCell;
use std::ops::Deref;
use std::rc::Rc;

struct MyBox<T>(T);
impl<T> MyBox<T> {
    fn new(x: T) -> MyBox<T> {
        MyBox(x)
    }
}
impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
struct Holder<'a> {
    name: &'a str,
    items: &'a [u32],
}

fn first(v: &[u64]) -> u64 {
    v[0]
}
fn len(s: &str) -> usize {
    s.len()
}
fn get(x: &u64) -> u64 {
    *x
}
fn show(x: &i32) -> i32 {
    *x
}
fn hello(name: &str) {
    println!("Hello, {}!", name);
}
fn total(xs: &[i32]) -> i32 {
    let mut t = 0;
    for x in xs {
        t += *x;
    }
    t
}
fn bump(v: &mut [i32]) {
    v[0] += 1;
}
fn pick(s: &String) -> &str {
    s.as_str()
}
fn tail(v: &Vec<u32>) -> &[u32] {
    &v[1..]
}
fn outer(s: &String, v: &Vec<u64>) -> usize {
    len(s) + first(v) as usize
}
fn make<'a>(s: &'a String, v: &'a Vec<u32>) -> Holder<'a> {
    Holder { name: s, items: v }
}

fn main() {
    let v = vec![3_000_000_000, 5];
    println!("{}", first(&v));
    let s = String::from("héllo");
    println!("{}", len(&s));
    let r = Rc::new(7u64);
    println!("{}", get(&r));

    let m = MyBox::new(String::from("Rust"));
    hello(&m);
    hello(&MyBox::new(String::from("Rust")));
    let w = MyBox::new(5i32);
    let b: &i32 = &w;
    println!("{}", b);

    println!("{}", first(&vec![7u64, 8]));
    println!("{}", len(&String::from("abc")));
    println!("{}", len(&format!("{}{}", 1, 2)));

    let rr = Rc::new(Rc::new(6i32));
    println!("{}", show(&rr));
    let cell = RefCell::new(vec![1i32, 2, 3]);
    println!("{}", total(&cell.borrow()));
    let g = cell.borrow();
    println!("{}", total(&g));
    let boxed = Box::new(vec![4i32, 5]);
    println!("{}", total(&boxed));

    let mut mv = vec![1i32, 2];
    bump(&mut mv);
    println!("{:?}", mv);

    let word = String::from("abc");
    println!("{}", pick(&word));
    let nums = vec![1u32, 2, 3];
    println!("{:?}", tail(&nums));
    let wide = vec![3u64];
    println!("{}", outer(&word, &wide));
    let rs = &word;
    println!("{}", len(rs));
    let rv = &wide;
    println!("{}", first(rv));
    let t: &str = rs;
    println!("{}", t);
    let u: &[u64] = rv;
    println!("{}", u.len());
    let h = Holder { name: &word, items: &nums };
    println!("{} {}", h.name, h.items.len());
    let h2 = make(&word, &nums);
    println!("{} {}", h2.name, h2.items.len());
}
"###,
        r###"3000000000
6
7
Hello, Rust!
Hello, Rust!
5
7
3
2
6
6
6
9
[2, 2]
abc
[2, 3]
6
3
3
abc
1
abc 3
abc 3
"###,
    );
}

/// The same coercing calls emitted by user macros (an `$e:expr` fragment is a
/// grouped expression), through a wrapper macro and nested in `assert_eq!`
/// run the one coercion route.
#[test]
fn deref_coercion_runs_through_user_macros_and_assertions() {
    prints(
        "coerce-macro",
        r###"use std::ops::Deref;
use std::rc::Rc;

struct MyBox<T>(T);
impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

fn first(v: &[u64]) -> u64 {
    v[0]
}
fn len(s: &str) -> usize {
    s.len()
}

macro_rules! call_first {
    ($v:expr) => {
        first(&$v)
    };
}
macro_rules! wrapped_len {
    ($s:expr) => {
        call_len!($s)
    };
}
macro_rules! call_len {
    ($s:expr) => {
        len(&$s)
    };
}
macro_rules! bind_ref {
    ($name:ident : $ty:ty = $e:expr) => {
        let $name: $ty = &$e;
    };
}

fn main() {
    let v = vec![3_000_000_000u64, 5];
    println!("{}", call_first!(v));
    println!("{}", call_first!(vec![9u64]));
    let s = String::from("héllo");
    println!("{}", wrapped_len!(s));
    println!("{}", wrapped_len!(MyBox(String::from("abcd"))));
    bind_ref!(sl: &[u64] = v);
    bind_ref!(st: &str = s);
    println!("{} {}", sl.len(), st);
    assert_eq!(first(&v), 3_000_000_000);
    assert_eq!(len(&s), 6);
    assert_eq!(len(&Rc::new(String::from("rc"))), 2);
    assert_eq!(len(&MyBox(MyBox(String::from("two")))), 3);
    println!("done");
}
"###,
        r###"3000000000
9
6
4
2 héllo
done
"###,
    );
}

/// rustc's method probe past the first step of an overloaded autoderef: a
/// by-value `self` of an impl for `&[T]`/`&mut [T]` is reached by autoref of
/// the `Vec`'s slice target, and an explicit `self: Box<Self>`,
/// `&Rc<Self>` or `Rc<Self>` receiver is the type of the step it names.
#[test]
fn method_probe_autorefs_deref_targets_and_honors_explicit_receiver_types() {
    prints(
        "probe-receivers",
        r###"use std::rc::Rc;

trait Sum {
    fn sum_(self) -> usize;
}
impl<'a> Sum for &'a [isize] {
    fn sum_(self) -> usize {
        self.len()
    }
}
trait Twice {
    fn twice(self) -> Vec<u8>;
}
impl<'a> Twice for &'a mut [u8] {
    fn twice(self) -> Vec<u8> {
        let mut out = Vec::new();
        for b in self.iter_mut() {
            *b *= 2;
            out.push(*b);
        }
        out
    }
}
struct Foo {
    x: i32,
}
impl Foo {
    fn boxed(self: Box<Self>) -> i32 {
        self.x
    }
    fn via_ref(self: &Rc<Self>) -> i32 {
        self.x + 1
    }
    fn owned(self: Rc<Self>) -> i32 {
        self.x + 2
    }
    fn plain(&self) -> i32 {
        self.x + 3
    }
}

fn main() {
    let items: Vec<isize> = vec![3, 5, 1, 2, 4];
    println!("{}", items.sum_());
    let mut bytes = vec![1u8, 2, 3];
    println!("{:?}", bytes.twice());
    println!("{:?}", bytes);
    let b = Box::new(Foo { x: 10 });
    println!("{}", b.boxed());
    let r = Rc::new(Foo { x: 20 });
    println!("{}", r.via_ref());
    println!("{}", r.plain());
    println!("{}", Rc::strong_count(&r));
    let r2 = r.clone();
    println!("{}", r2.owned());
    println!("{}", Rc::strong_count(&r));
    let rr = &r;
    println!("{}", rr.via_ref());
}
"###,
        r###"5
[2, 4, 6]
[2, 4, 6]
10
21
23
1
22
1
21
"###,
    );
}

/// Slice, array and `Vec` comparisons across their reference forms.
#[test]
fn slice_array_and_vec_equality_across_reference_forms() {
    prints(
        "slice-array-eq",
        r###"fn main() {
    let v = vec![1, 2, 3, 4, 5];
    let s: &[i32] = &v[..4];
    let a = [1, 2, 3, 4];
    println!("{}", s == &[1, 2, 3, 4]);
    println!("{}", s == &a[..]);
    println!("{}", *s == a);
    println!("{}", s == &a);
    println!("{}", a == *s);
    println!("{}", a == s);
    println!("{}", a[..] == *s);
    println!("{}", &v[..3] == &[1, 2, 3]);
    println!("{}", s != &[1, 2, 3, 5]);
}
"###,
        r###"true
true
true
true
true
true
true
true
true
"###,
    );
}

/// Unchanged upstream run-pass fixtures for deref coercion, slice receivers
/// and arbitrary self types.
#[test]
fn upstream_coercion_receiver_and_slice_pattern_witnesses_complete() {
    for path in [
        "tests/ui/array-slice-vec/rcvr-borrowed-to-slice.rs",
        "tests/ui/autoref-autoderef/autoderef-vec-to-slice-by-value.rs",
        "tests/ui/coercion/coerce-reborrow-imm-ptr-arg.rs",
        "tests/ui/coercion/coerce-reborrow-imm-ptr-rcvr.rs",
        "tests/ui/coercion/coerce-reborrow-imm-vec-arg.rs",
        "tests/ui/coercion/coerce-reborrow-imm-vec-rcvr.rs",
        "tests/ui/coercion/coerce-reborrow-mut-ptr-arg.rs",
        "tests/ui/coercion/coerce-reborrow-mut-ptr-rcvr.rs",
        "tests/ui/coercion/coerce-reborrow-mut-vec-arg.rs",
        "tests/ui/coercion/coerce-reborrow-mut-vec-rcvr.rs",
        "tests/ui/or-patterns/slice-patterns.rs",
        "tests/ui/pattern/bindings-after-at/slice-patterns.rs",
        "tests/ui/regions/regions-borrow-evec-uniq.rs",
        "tests/ui/regions/regions-dependent-autoslice.rs",
        "tests/ui/self/arbitrary_self_types_struct.rs",
        "tests/ui/self/arbitrary_self_types_struct_receiver_trait.rs",
    ] {
        upstream_completes(path);
    }
}

/// Closure parameters are patterns matched against the call arguments by the
/// shared pattern transaction with the closure as its owner: reference,
/// tuple, array, record and nested patterns, `ref` and `mut` bindings,
/// non-primitive annotations, patterns whose bindings type an open
/// accumulator through the `Fn*` bound, and parameters that mix plain
/// bindings with patterns.
#[test]
fn closure_parameter_patterns_run_the_shared_pattern_transaction() {
    prints(
        "closure-patterns",
        r###"fn main() {
    let v = vec![1isize, 2, 3];
    let s: isize = v.iter().fold(0, |a, &b| a + b);
    println!("{}", s);
    let pairs = vec![(1, "a"), (2, "b")];
    let mut out: Vec<(i32, &str)> = Vec::new();
    pairs.iter().for_each(|&(n, t)| out.push((n * 2, t)));
    println!("{:?}", out);
    let f = |(a, b): (i32, i32), c: i32| a + b + c;
    println!("{}", f((1, 2), 3));
    let a = [1, 2, 3, 4];
    let evens: Vec<&i32> = a.iter().filter(|&&x| x % 2 == 0).collect();
    println!("{:?}", evens);
    let g = |mut acc: Vec<i32>, &x: &i32| { acc.push(x); acc };
    println!("{:?}", g(vec![0], &5));
    let owned = vec![(String::from("k"), 1)];
    for (i, (k, n)) in owned.into_iter().enumerate() {
        println!("{} {} {}", i, k, n);
    }
    let h = |&x: &i32, ref y: i32, z: i32| x + *y + z;
    println!("{}", h(&1, 2, 3));
    let arr = |[a, b]: [i32; 2]| a - b;
    println!("{}", arr([5, 3]));
    struct P { x: i32, y: i32 }
    let sp = |P { x, y }: P| x * y;
    println!("{}", sp(P { x: 3, y: 4 }));
    let mut total = 0;
    vec![(1, 2), (3, 4)].iter().for_each(|&(x, y)| total += x * y);
    println!("{}", total);
    let nested = |((a, b), [c, d]): ((i32, i32), [i32; 2])| a + b + c + d;
    println!("{}", nested(((1, 2), [3, 4])));
    let strs = |(x, _): (&str, i32), (_, y): (i32, &str)| format!("{}{}", x, y);
    println!("{}", strs(("p", 1), (2, "q")));
}
"###,
        r###"6
[(2, "a"), (4, "b")]
6
[2, 4]
[0, 5]
0 k 1
6
2
12
14
10
pq
"###,
    );
}

/// The same closure-pattern parameters emitted by user macros (an `$f:expr`
/// closure applied through a macro, a macro producing a closure with a pattern
/// parameter, a pattern closure nested in a macro-expanded `fold`) and nested
/// in `assert_eq!`.
#[test]
fn closure_parameter_patterns_through_user_macros_and_assertions() {
    prints(
        "closure-patterns-macro",
        r###"macro_rules! sum_pairs {
    ($v:expr) => {
        $v.iter().fold(0i32, |acc, &(a, b)| acc + a * b)
    };
}
macro_rules! apply {
    ($f:expr, $x:expr) => {
        $f($x)
    };
}
macro_rules! make_adder {
    () => {
        |(a, b): (i32, i32)| move |c: i32| a + b + c
    };
}

fn main() {
    let v: Vec<(i32, i32)> = vec![(1, 2), (3, 4)];
    println!("{}", sum_pairs!(v));
    println!("{}", apply!(|&(x, y): &(i32, i32)| x * 10 + y, &(4, 5)));
    let adder = make_adder!();
    let inner = adder((1, 2));
    println!("{}", inner(3));
    assert_eq!(sum_pairs!(vec![(2, 2)]), 4);
    let shadow = |x: i32, (x2, _): (i32, i32)| x + x2;
    println!("{}", shadow(1, (2, 3)));
}
"###,
        r###"14
45
6
3
"###,
    );
}

/// `base[idx]` of a user type with its own `Index<Idx>`/`IndexMut<Idx>` impls
/// (tuple, `usize`, `&str` and `u8` indices, several impls selected by the
/// operand, nested and delegating indexing, `&mut base[i]`, compound
/// assignment through `index_mut`), `VecDeque` and `HashMap` indexing through
/// the facade's impls, and a method returning a reference with an elided
/// lifetime beside a `&str` parameter.
#[test]
fn user_index_impls_select_by_operand_and_index_mut_by_use() {
    prints(
        "user-index",
        r###"use std::collections::{HashMap, VecDeque};
use std::ops::{Index, IndexMut};

struct Grid {
    w: usize,
    cells: Vec<i32>,
}
impl Index<(usize, usize)> for Grid {
    type Output = i32;
    fn index(&self, p: (usize, usize)) -> &i32 {
        &self.cells[p.0 * self.w + p.1]
    }
}
impl IndexMut<(usize, usize)> for Grid {
    fn index_mut(&mut self, p: (usize, usize)) -> &mut i32 {
        &mut self.cells[p.0 * self.w + p.1]
    }
}
struct Table {
    names: Vec<String>,
}
impl Index<usize> for Table {
    type Output = String;
    fn index(&self, i: usize) -> &String {
        &self.names[i]
    }
}
impl Index<&str> for Table {
    type Output = String;
    fn index(&self, key: &str) -> &String {
        for n in &self.names {
            if n == key {
                return n;
            }
        }
        panic!("missing {}", key)
    }
}
struct Matrix {
    rows: usize,
    data: Vec<Vec<i64>>,
}
impl Index<usize> for Matrix {
    type Output = Vec<i64>;
    fn index(&self, r: usize) -> &Vec<i64> {
        &self.data[r]
    }
}
impl IndexMut<usize> for Matrix {
    fn index_mut(&mut self, r: usize) -> &mut Vec<i64> {
        &mut self.data[r]
    }
}
struct Bits(u32);
impl Index<u8> for Bits {
    type Output = bool;
    fn index(&self, i: u8) -> &bool {
        if self.0 & (1 << i) != 0 { &true } else { &false }
    }
}
struct Wrapper {
    inner: Matrix,
}
impl Index<usize> for Wrapper {
    type Output = Vec<i64>;
    fn index(&self, r: usize) -> &Vec<i64> {
        &self.inner[r]
    }
}
impl Table {
    fn lookup(&self, key: &str) -> &String {
        &self[key]
    }
}

fn main() {
    let mut g = Grid { w: 2, cells: vec![0; 4] };
    g[(1, 1)] = 5;
    g[(0, 1)] += 2;
    println!("{} {} {}", g[(1, 1)], g[(0, 1)], g[(0, 0)]);
    let r = &g;
    println!("{}", r[(1, 1)]);
    let t = Table { names: vec![String::from("a"), String::from("bb")] };
    println!("{} {}", t[1], t["a"]);
    println!("{} {}", t[1].len(), t.lookup("bb"));
    let mut m = Matrix { rows: 2, data: vec![vec![1, 2], vec![3, 4]] };
    println!("{} {}", m[0][1], m[1][0]);
    m[1][0] = 30;
    m[0][1] += 5;
    m[1].push(9);
    println!("{:?} {}", m.data, m.rows);
    let row = &mut m[0];
    row.push(100);
    println!("{:?} {}", m[0], m[0].len());
    let b = Bits(0b101);
    println!("{} {} {}", b[0], b[1], b[2]);
    let w = Wrapper { inner: m };
    println!("{:?}", w[1]);
    let mut dq: VecDeque<i32> = VecDeque::new();
    dq.push_back(4);
    dq.push_back(5);
    dq[0] += 10;
    println!("{} {}", dq[0], dq[1]);
    let mut hm: HashMap<String, i32> = HashMap::new();
    hm.insert(String::from("a"), 1);
    println!("{}", hm["a"]);
}
"###,
        r###"5 2 0
5
bb a
2 bb
2 3
[[1, 7], [30, 4, 9]] 2
[1, 7, 100] 3
true false true
[30, 4, 9]
14 5
1
"###,
    );
}

/// The same indexing emitted by user macros (read, assignment, compound
/// assignment, nested macro) and nested in `assert_eq!`/`assert_ne!`.
#[test]
fn user_index_through_user_macros_and_assertions() {
    prints(
        "user-index-macro",
        r###"use std::ops::{Index, IndexMut};
struct Grid {
    w: usize,
    cells: Vec<i32>,
}
impl Index<(usize, usize)> for Grid {
    type Output = i32;
    fn index(&self, p: (usize, usize)) -> &i32 {
        &self.cells[p.0 * self.w + p.1]
    }
}
impl IndexMut<(usize, usize)> for Grid {
    fn index_mut(&mut self, p: (usize, usize)) -> &mut i32 {
        &mut self.cells[p.0 * self.w + p.1]
    }
}
macro_rules! at {
    ($c:expr, $i:expr) => {
        $c[$i]
    };
}
macro_rules! set {
    ($c:expr, $i:expr, $v:expr) => {
        $c[$i] = $v
    };
}
macro_rules! bump {
    ($c:expr, $i:expr) => {
        $c[$i] += 1
    };
}
macro_rules! twice {
    ($c:expr, $i:expr) => {
        at!($c, $i) * 2
    };
}
fn main() {
    let mut g = Grid { w: 3, cells: vec![0; 9] };
    set!(g, (1, 2), 7);
    bump!(g, (1, 2));
    bump!(g, (0, 0));
    println!("{} {}", at!(g, (1, 2)), at!(g, (0, 0)));
    println!("{}", twice!(g, (1, 2)));
    assert_eq!(g[(1, 2)], 8);
    assert_eq!(at!(g, (0, 0)), 1);
    assert_ne!(twice!(g, (1, 2)), 0);
    let mut total = 0;
    for &p in [(0, 0), (1, 2)].iter() {
        total += g[p];
    }
    println!("{}", total);
}
"###,
        r###"8 1
16
9
"###,
    );
}

/// A `&mut` local passed on to a callee whose parameter is `&mut H` of a
/// generic `H` is reborrowed, not moved (rustc's coercion site for any
/// `&mut _` parameter), so a derived `Hash` can feed one hasher to every
/// field and a generic helper can call another twice.
#[test]
fn mutable_reference_arguments_of_generic_parameters_are_reborrowed() {
    prints(
        "generic-reborrow",
        r###"trait Sink {
    fn put(&mut self, v: u32);
}
struct S(u32);
impl Sink for S {
    fn put(&mut self, v: u32) {
        self.0 += v;
    }
}
trait Feed {
    fn feed<H: Sink>(&self, state: &mut H);
}
impl Feed for u32 {
    fn feed<H: Sink>(&self, state: &mut H) {
        state.put(*self);
    }
}
struct Two {
    a: u32,
    b: u32,
}
impl Feed for Two {
    fn feed<H: Sink>(&self, state: &mut H) {
        Feed::feed(&self.a, state);
        Feed::feed(&self.b, state);
    }
}
struct Three {
    a: u32,
    b: u32,
    c: u32,
}
impl Feed for Three {
    fn feed<H: Sink>(&self, state: &mut H) {
        self.a.feed(state);
        self.b.feed(state);
        self.c.feed(state)
    }
}
fn gen<T: Sink>(t: &mut T) {
    t.put(1);
    t.put(2);
}
fn gen2<T: Sink>(t: &mut T) {
    gen(t);
    gen(t);
}
fn shared<T: Sink>(t: &mut T, n: &mut u32) {
    bump(n);
    bump(n);
    gen(t);
}
fn bump(n: &mut u32) {
    *n += 1;
}
fn main() {
    let mut s = S(0);
    gen2(&mut s);
    println!("{}", s.0);
    Two { a: 1, b: 2 }.feed(&mut s);
    println!("{}", s.0);
    Three { a: 10, b: 20, c: 30 }.feed(&mut s);
    println!("{}", s.0);
    let mut n = 0;
    shared(&mut s, &mut n);
    println!("{} {}", s.0, n);
}
"###,
        r###"6
9
69
72 2
"###,
    );
}

/// The real `Hash`/`Hasher` protocol: `DefaultHasher` is SipHash-1-3 with zero
/// keys (std's `h(&42u64)` value), derived `Hash` feeds fields in order and
/// enums an `isize` discriminant, `str` its bytes then `0xff`, sequences a
/// length prefix; a user `Hasher` sees the same byte stream real std gives it.
#[test]
fn hash_protocol_streams_match_real_std_and_default_hasher_is_siphash13() {
    prints(
        "hash-protocol",
        r###"use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[derive(Hash, PartialEq, Eq, Debug, Clone)]
struct Key {
    id: u32,
    name: String,
}
#[derive(Hash)]
struct Pair(i64, char);
#[derive(Hash)]
enum Shape {
    Unit,
    Tuple(u8, bool),
    Named { w: u16, label: &'static str },
}
struct Manual(u32);
impl Hash for Manual {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.0 % 10).hash(state);
    }
}
// A user hasher that records the byte stream it is fed.
struct Recorder {
    bytes: Vec<u8>,
}
impl Hasher for Recorder {
    fn finish(&self) -> u64 {
        self.bytes.len() as u64
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.bytes.push(*b);
        }
    }
}
fn record<T: Hash>(t: &T) -> Vec<u8> {
    let mut r = Recorder { bytes: Vec::new() };
    t.hash(&mut r);
    r.bytes
}
fn h<T: Hash + ?Sized>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}
fn main() {
    println!("{:x}", h(&42u64));
    println!("{:?}", record(&(1u8, 2u16, "ab")));
    println!("{:?}", record(&Shape::Tuple(4, false)));
    println!("{:?}", record(&Shape::Named { w: 9, label: "L" }));
    println!("{:?} {:?}", record(&Manual(13)), record(&Manual(23)));
    println!("{:?}", record(&Key { id: 7, name: String::from("k") }));
    let k = Key { id: 7, name: String::from("seven") };
    let mut m: HashMap<Key, i32> = HashMap::new();
    m.insert(k.clone(), 1);
    println!("{:?}", m.get(&k));
}
"###,
        r###"7b3e724b36ebdf51
[1, 2, 0, 97, 98, 255]
[1, 0, 0, 0, 0, 0, 0, 0, 4, 0]
[2, 0, 0, 0, 0, 0, 0, 0, 9, 0, 76, 255]
[3, 0, 0, 0] [3, 0, 0, 0]
[7, 0, 0, 0, 107, 255]
Some(1)
"###,
    );
}

/// Keep the unchanged slice-pattern witnesses independently runnable while
/// their shared generated receiver/parameter source join is repaired.
#[test]
fn upstream_slice_pattern_receivers_complete() {
    for path in [
        "tests/ui/or-patterns/slice-patterns.rs",
        "tests/ui/pattern/bindings-after-at/slice-patterns.rs",
    ] {
        upstream_completes(path);
    }
}
