//! LRR-specific controls for deferred destructuring declarations
//! (`let (a, b);`), destructuring assignment (`(a, b) = ..`) and inline
//! `const { .. }` blocks. Each program runs through the production CLI path in
//! every official edition: the declaration binds through the shared pattern
//! owner, and an assignee is the pattern its shape spells, matched by the same
//! transaction as every `let`, `match` and macro-emitted pattern, then assigned
//! to its leaf places left to right. These are internal controls, not a stock
//! suite count.
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
        "lrr-destructuring-{}-{}.rs",
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

#[test]
fn deferred_destructuring_declarations() {
    completed(
        r#"struct S { f: i32, g: u8 }
struct T(i32, i32);
fn pair() -> (i32, u8) { (7, 9) }
fn main() {
    let x;
    x = 5;
    assert_eq!(x, 5);
    let (a, b);
    a = 1;
    b = 2;
    assert_eq!(a + b, 3);
    let (c, mut d): (u8, i64);
    c = 200;
    d = 5;
    d += 1;
    assert_eq!(c, 200u8);
    assert_eq!(d, 6i64);
    let (e, _);
    e = 3;
    assert_eq!(e, 3);
    let [p, q];
    p = 10; q = 20;
    assert_eq!(p + q, 30);
    let S { f, g };
    f = 4; g = 5;
    assert_eq!(f, 4); assert_eq!(g, 5u8);
    let T(h, k);
    h = 8; k = 9;
    assert_eq!(h * k, 72);
    let ((m, n), o);
    m = 1; n = 2; o = 3;
    assert_eq!(m + n + o, 6);
    let (u, v) = pair();
    assert_eq!(u, 7); assert_eq!(v, 9);
    let (w, z): (String, Vec<u8>);
    w = String::from("hi");
    z = vec![1, 2];
    assert_eq!(w, "hi"); assert_eq!(z.len(), 2);
}
"#,
    );
}

#[test]
fn deferred_declarations_drop_only_assigned_bindings() {
    completed(
        r#"static mut LOG: u64 = 0;
struct D(u64);
impl Drop for D { fn drop(&mut self) { unsafe { LOG = LOG * 100 + self.0; } } }
fn log() -> u64 { unsafe { LOG } }
fn reset() { unsafe { LOG = 0; } }
fn main() {
    {
        let (a, b, c);
        a = D(1);
        c = D(3);
        let _ = (&a, &c);
    }
    assert_eq!(log(), 301);
    reset();
    {
        let (x, y);
        if log() == 0 { x = D(10); y = D(20); } else { x = D(11); y = D(21); }
        let _ = (&x, &y);
    }
    assert_eq!(log(), 2010);
}
"#,
    );
}

#[test]
fn destructuring_assignment_forms() {
    completed(
        r#"struct S { f: i32, g: i32 }
struct U { f: i32, g: u8 }
struct T(i32, i32);
fn pair() -> (i32, u8) { (7, 9) }
fn main() {
    let (mut a, mut b) = (1, 2);
    (a, b) = (b, a);
    assert_eq!((a, b), (2, 1));
    let (mut c, mut d);
    (c, d) = pair();
    assert_eq!((c, d), (7, 9u8));
    (c, _) = (100, 200);
    assert_eq!(c, 100);
    let mut arr = [0; 3];
    [arr[0], arr[2]] = [5, 6];
    assert_eq!(arr, [5, 0, 6]);
    let (mut x, mut y);
    [x, y] = [10i32, 20];
    assert_eq!(x + y, 30);
    [x, .., y] = [1i32, 2, 3, 4];
    assert_eq!((x, y), (1, 4));
    [_, x, _] = [7i32, 8, 9];
    assert_eq!(x, 8);
    let mut s = S { f: 0, g: 0 };
    S { f: s.g, g: s.f } = S { f: 3, g: 4 };
    assert_eq!(s.g, 3);
    assert_eq!(s.f, 4);
    let (mut p, mut q);
    U { f: p, g: q } = U { f: -1, g: 255 };
    assert_eq!(p, -1);
    assert_eq!(q, 255u8);
    U { f: p, .. } = U { f: 11, g: 12 };
    assert_eq!(p, 11);
    let (mut p2, mut q2);
    T(p2, q2) = T(5, 6);
    assert_eq!((p2, q2), (5, 6));
    T(p2, ..) = T(9, 10);
    assert_eq!(p2, 9);
    let mut t = (0, 1);
    (t.1, t.0) = (t.0 + 10, t.1 + 20);
    assert_eq!(t, (21, 10));
    ((a, b), c) = ((1, 2), 3);
    assert_eq!((a, b, c), (1, 2, 3));
    (a, (b, c)) = (4, (5, 6));
    assert_eq!((a, b, c), (4, 5, 6));
    _ = 5;
    (_, _) = (1, 2);
    () = ();
    let mut v = vec![1, 2, 3];
    let r = &mut v;
    (r[0], r[1]) = (r[1], r[0]);
    assert_eq!(v, vec![2, 1, 3]);
    let mut m = 1;
    let pm = &mut m;
    (*pm, a) = (50, 60);
    assert_eq!((m, a), (50, 60));
}
"#,
    );
}

#[test]
fn destructuring_assignment_order_and_drops() {
    completed(
        r#"// evaluation order and drop order
static mut LOG: u64 = 0;
fn log(n: u64) { unsafe { LOG = LOG * 10 + n; } }
fn reset() { unsafe { LOG = 0; } }
fn cur() -> u64 { unsafe { LOG } }
struct D(u64);
impl Drop for D { fn drop(&mut self) { log(self.0); } }
fn idx(n: u64, i: usize) -> usize { log(n); i }
fn main() {
    // rhs first, then places left to right
    let mut v = [0u8; 4];
    (v[idx(2, 0)], v[idx(3, 1)]) = { log(1); (7, 8) };
    assert_eq!(cur(), 123);
    assert_eq!(v, [7, 8, 0, 0]);
    reset();
    // drop of old values on assignment, left to right
    let (mut a, mut b) = (D(1), D(2));
    (a, b) = (D(3), D(4));
    assert_eq!(cur(), 12);
    drop(a);
    drop(b);
    assert_eq!(cur(), 1234);
    reset();
    // underscore: the temporary dies at the end of the statement
    let x;
    (x, _) = (D(5), D(6));
    assert_eq!(cur(), 6);
    reset();
    _ = D(7);
    assert_eq!(cur(), 7);
    drop(x);
    assert_eq!(cur(), 75);
}
"#,
    );
}

#[test]
fn destructuring_assignment_through_macros() {
    completed(
        r#"macro_rules! swap { ($a:expr, $b:expr) => { ($a, $b) = ($b, $a) }; }
macro_rules! unpack { ($x:ident, $y:ident, $e:expr) => { ($x, $y) = $e }; }
macro_rules! pair { ($l:expr, $r:expr) => { ($l, $r) }; }
macro_rules! assign_all { ($($p:ident),*; $e:expr) => { ($($p),*) = $e; } }
fn main() {
    let (mut a, mut b) = (1, 2);
    swap!(a, b);
    assert_eq!((a, b), (2, 1));
    unpack!(a, b, (30, 40));
    assert_eq!((a, b), (30, 40));
    pair!(a, b) = (7, 8);
    assert_eq!((a, b), (7, 8));
    let mut c = 0;
    assign_all!(a, b, c; (1, 2, 3));
    assert_eq!((a, b, c), (1, 2, 3));
    let v = { let mut z = 0; let mut w = 0; (z, w) = (4, 5); z + w };
    assert_eq!(v, 9);
}
"#,
    );
}

#[test]
fn destructuring_assignment_in_methods_closures_and_loops() {
    completed(
        r#"struct P { x: i32, y: i32 }
impl P {
    fn swap(&mut self) { (self.x, self.y) = (self.y, self.x); }
}
fn fib(n: u32) -> u64 {
    let (mut a, mut b) = (0u64, 1u64);
    for _ in 0..n { (a, b) = (b, a + b); }
    a
}
fn swap_vals<T>(a: &mut T, b: &mut T) where T: Clone { (*a, *b) = (b.clone(), a.clone()); }
fn make() -> (String, String) { (String::from("l"), String::from("r")) }
fn main() {
    let mut p = P { x: 1, y: 2 };
    p.swap();
    assert_eq!((p.x, p.y), (2, 1));
    assert_eq!(fib(10), 55);
    let mut s = String::from("a");
    let mut t = String::from("b");
    swap_vals(&mut s, &mut t);
    assert_eq!((s.as_str(), t.as_str()), ("b", "a"));
    // closure capturing and assigning
    let (mut u, mut v) = (1, 2);
    let mut f = || { (u, v) = (v + 10, u + 20); };
    f();
    f();
    assert_eq!((u, v), (31, 32));
    // rhs is a place
    let pair = (7, 8);
    let (mut a, mut b);
    (a, b) = pair;
    assert_eq!((a, b), (7, 8));
    let np = (String::from("x"), String::from("y"));
    let (mut c, mut d);
    (c, d) = np;
    assert_eq!((c.as_str(), d.as_str()), ("x", "y"));
    (c, d) = make();
    assert_eq!((c.as_str(), d.as_str()), ("l", "r"));
    // loop with re-execution and untyped
    let (mut lo, mut hi) = (0, 100);
    let mut steps = 0;
    while lo < hi { (lo, hi) = (lo + 10, hi - 10); steps += 1; }
    assert_eq!((lo, hi, steps), (50, 50, 5));
    // result of the assignment is ()
    let r = { (a, b) = (b, a) };
    assert_eq!(r, ());
    // array of places
    let mut grid = [[0u8; 2]; 2];
    [grid[0][1], grid[1][0]] = [4, 5];
    assert_eq!(grid, [[0, 4], [5, 0]]);
    // enum variant
    enum E { A(i32, i32) }
    enum F { B { l: i32, r: i32 } }
    let (mut q, mut w) = (0, 0);
    E::A(q, w) = E::A(1, 2);
    assert_eq!((q, w), (1, 2));
    F::B { l: q, r: w } = F::B { l: 3, r: 4 };
    assert_eq!((q, w), (3, 4));
}
"#,
    );
}

#[test]
fn destructuring_assignment_rest_and_wildcards_infer_literals() {
    completed(
        r#"fn main() {
    let (mut a, mut b);
    (a, b) = (0, 1);
    (a, .., b) = (1, 2);
    (.., a) = (1, 2);
    (..) = (3, 4);
    (b, ..) = (5, 6, 7);
    assert_eq!(b, 5);
    let (mut x, mut y);
    [x, y] = [10, 20];
    [x, .., y] = [1, 2, 3, 4];
    assert_eq!((x, y), (1, 4));
    [_, x, _] = [7, 8, 9];
    assert_eq!(x, 8);
    let mut c;
    [c, ..] = [5, 6, 6];
    assert_eq!(c, 5);
    [..] = [1, 2, 3];
    let mut m = 1;
    [m, _] = [10, 20];
    assert_eq!(m, 10);
}
"#,
    );
}

#[test]
fn inline_const_blocks_are_ordinary_blocks() {
    completed(
        r#"const fn double(n: u32) -> u32 { n * 2 }
fn size<T>() -> usize { const { std::mem::size_of::<T>() } }
macro_rules! seven { () => { const { 3 + 4 } }; }
macro_rules! wrap { ($e:expr) => { $e + 1 }; }
fn main() {
    assert_eq!(const { 1 + 2 }, 3);
    let a = const { let x = 5; x * 2 };
    assert_eq!(a, 10);
    let b: u8 = const { 255 };
    assert_eq!(b, 255);
    let c = const { double(21) };
    assert_eq!(c, 42);
    let d = const {
        let mut s = 0;
        let mut i = 0;
        while i < 5 { s += i; i += 1; }
        s
    };
    assert_eq!(d, 10);
    assert_eq!(size::<u32>(), 4);
    assert_eq!(size::<u8>(), 1);
    assert_eq!(seven!(), 7);
    assert_eq!(wrap!((const { 9 })), 10);
    let v = [const { 5u8 }; 3];
    assert_eq!(v, [5, 5, 5]);
    let n = match 3 { 3 => const { 100 }, _ => 0 };
    assert_eq!(n, 100);
    let t = const { (1, "x", [1u8, 2]) };
    assert_eq!(t.0, 1);
    assert_eq!(t.1, "x");
    let u = unsafe { const { 8 } };
    assert_eq!(u, 8);
    const { assert!(1 + 1 == 2) };
    let nested = const { (const { 4 }) + 1 };
    assert_eq!(nested, 5);
}
"#,
    );
}

#[test]
fn destructuring_assignment_statics_and_parameters() {
    completed(
        r#"static mut S: i32 = 0;
fn param(mut a: i32, mut b: i32) -> (i32, i32) { (a, b) = (b + 1, a + 1); (a, b) }
fn main() {
    let mut x = 0;
    unsafe { (S, x) = (5, 6); }
    assert_eq!(unsafe { S }, 5);
    assert_eq!(x, 6);
    assert_eq!(param(1, 2), (3, 2));
}
"#,
    );
}

#[test]
fn destructuring_assignment_nested_places() {
    completed(
        r#"// shapes with calls, method receivers and nested places on the left
struct W { v: Vec<u32>, n: (u8, u8) }
fn pick(w: &mut W) -> &mut u32 { &mut w.v[1] }
fn main() {
    let mut w = W { v: vec![0, 0, 0], n: (0, 0) };
    (*pick(&mut w), w.n.1) = (7, 8);
    assert_eq!(w.v, vec![0, 7, 0]);
    assert_eq!(w.n, (0, 8));
    (w.v[0], (w.n.0, _)) = (1, (2, 3));
    assert_eq!(w.v[0], 1);
    assert_eq!(w.n.0, 2);
    let mut arr = [[0u8; 2]; 2];
    ([arr[0][0], arr[1][1]], _) = ([9, 8], 0);
    assert_eq!(arr, [[9, 0], [0, 8]]);
}
"#,
    );
}

#[test]
fn destructuring_assignment_drops_replaced_place_values() {
    completed(
        r#"static mut LOG: u64 = 0;
fn log(n: u64) { unsafe { LOG = LOG * 10 + n; } }
struct D(u64);
impl Drop for D { fn drop(&mut self) { log(self.0); } }
struct S { a: D, b: D }
fn main() {
    let mut s = S { a: D(1), b: D(2) };
    (s.a, s.b) = (D(3), D(4));
    assert_eq!(unsafe { LOG }, 12);
    let mut v = vec![D(5), D(6)];
    [v[1], v[0]] = [D(7), D(8)];
    assert_eq!(unsafe { LOG }, 1265);
    drop(s);
    drop(v);
    assert_eq!(unsafe { LOG }, 12653487);
}
"#,
    );
}

#[test]
fn destructuring_assignment_in_control_flow() {
    completed(
        r#"fn pairs(ok: bool) -> Option<(i32, i32)> { if ok { Some((3, 4)) } else { None } }
fn run(ok: bool) -> Option<i32> {
    let (a, b);
    (a, b) = match pairs(ok) { Some(p) => p, None => return None };
    Some(a + b)
}
fn early(n: i32) -> i32 {
    let (mut x, mut y) = (0, 0);
    (x, y) = if n > 0 { (n, n * 2) } else { return -1 };
    x + y
}
fn main() {
    assert_eq!(run(true), Some(7));
    assert_eq!(run(false), None);
    assert_eq!(early(2), 6);
    assert_eq!(early(-5), -1);
    let (mut a, mut b) = (0, 0);
    match 3 { 3 => (a, b) = (1, 2), _ => (a, b) = (9, 9) }
    assert_eq!((a, b), (1, 2));
    let r = if a == 1 { (a, b) = (b, a) } else { () };
    assert_eq!(r, ());
    assert_eq!((a, b), (2, 1));
    for i in 0..3 { (a, b) = (b + i, a); }
    assert_eq!((a, b), (3, 3));
    let mut o = (1u8, 2u8);
    (o.1, o.0) = (o.0 + 5, o.1 + 6);
    assert_eq!(o, (8, 6));
}
"#,
    );
}

#[test]
fn destructuring_declarations_and_assignment_nest_in_standard_macros_and_const_blocks() {
    completed(
        r#"macro_rules! decl_assign { ($a:ident, $b:ident, $v:expr) => { let ($a, $b); ($a, $b) = $v; }; }
macro_rules! nested { ($($x:tt)*) => { $($x)* }; }
fn main() {
    assert!({ let (a, b); (a, b) = (1, 2); a + b == 3 });
    assert_eq!({ let [x, y]; [x, y] = [3, 4]; x * y }, 12);
    decl_assign!(p, q, (5, 6));
    assert_eq!(p + q, 11);
    nested! { let (m, n); (m, n) = (7, 8); assert_eq!(m + n, 15); }
    let r = vec![{ let (u, v); (u, v) = (1, 1); u + v }];
    assert_eq!(r, vec![2]);
    println!("{}", { let (s, t); (s, t) = ("a", "b"); format!("{}{}", s, t) });
    let k = const { let (i, j); (i, j) = (2, 3); i * j };
    assert_eq!(k, 6);
}
"#,
    );
}
