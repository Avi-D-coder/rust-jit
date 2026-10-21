//! Array `rest @ ..` patterns (rustc MIR `Subslice` on an array): the sized
//! sub-array binds as a fresh `[T; K]` by value and as `&[T; K]` /
//! `&mut [T; K]` under reference binding modes, through the one pattern
//! transaction every owner shares (`let`, `match`, `if let`, parameters,
//! closure parameters and macro-emitted patterns). LRR-specific control
//! programs through the production CLI path; they are not a stock-suite count.
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
        "lrr-subarray-{}-{}.rs",
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
fn array_subslice_by_value_copy_elements() {
    completed(
        r#"fn sum3(a: [u8; 3]) -> u8 {
    a[0] + a[1] + a[2]
}
fn main() {
    // let with by-value Copy arrays
    let [x, rest @ ..] = [1u8, 2, 3, 4];
    assert_eq!(x, 1);
    assert_eq!(rest, [2u8, 3, 4]);
    assert_eq!(rest.len(), 3);
    assert_eq!(sum3(rest), 9);
    let [a, mid @ .., z] = [10i32, 20, 30, 40, 50];
    assert_eq!(a, 10);
    assert_eq!(mid, [20, 30, 40]);
    assert_eq!(z, 50);
    let [.., last] = [7u16, 8, 9];
    assert_eq!(last, 9);
    let [first, ..] = [7u16, 8, 9];
    assert_eq!(first, 7);
    let [pre @ .., t] = [1u64, 2, 3];
    assert_eq!(pre, [1, 2]);
    assert_eq!(t, 3);
    // empty rest
    let [p, q, empty @ ..] = [1i8, 2];
    assert_eq!((p, q), (1, 2));
    assert_eq!(empty.len(), 0);
    // whole rest
    let [all @ ..] = [5u8, 6];
    assert_eq!(all, [5, 6]);

    // mutation of the copy does not affect the source
    let src = [1u8, 2, 3, 4];
    let [_, mut r @ ..] = src;
    r[0] = 99;
    assert_eq!(r, [99, 3, 4]);
    assert_eq!(src, [1, 2, 3, 4]);

    // match
    let arr = [1u8, 2, 3, 4, 5];
    let v = match arr {
        [1, 3, ..] => 100,
        [1, 2, rest @ ..] => {
            assert_eq!(rest, [3, 4, 5]);
            rest[0] as i32 + rest[2] as i32
        }
        _ => -1,
    };
    assert_eq!(v, 8);
    // if let
    if let [a, b, rest @ ..] = arr {
        assert_eq!((a, b), (1, 2));
        assert_eq!(rest.len(), 3);
        assert_eq!(rest[1], 4);
    } else {
        panic!();
    }
    // refutable: literal in a middle position
    let w = match [1u8, 2, 3] {
        [_, 9, rest @ ..] => rest.len() as i32,
        [_, _, rest @ ..] => 100 + rest.len() as i32,
    };
    assert_eq!(w, 101);
    // while let on arrays of arrays
    let nested = [[1u8, 2, 3], [4, 5, 6]];
    let [[a, rest @ ..], ..] = nested;
    assert_eq!(a, 1);
    assert_eq!(rest, [2, 3]);
    let [_, [.., c]] = nested;
    assert_eq!(c, 6);
    let [[_, inner @ ..], [d, ..]] = nested;
    assert_eq!(inner, [2, 3]);
    assert_eq!(d, 4);
    println!("ok");
}
"#,
    );
}

#[test]
fn array_subslice_by_reference_and_default_binding_modes() {
    completed(
        r#"fn main() {
    let mut a = [1u8, 2, 3, 4];
    {
        let [x, ref rest @ ..] = a;
        assert_eq!(x, 1);
        assert_eq!(*rest, [2, 3, 4]);
        assert_eq!(rest.len(), 3);
        assert_eq!(rest[1], 3);
    }
    {
        let [ref a0, ref mut a1, ..] = a;
        assert_eq!(*a0, 1);
        *a1 = 20;
    }
    assert_eq!(a, [1, 20, 3, 4]);
    {
        let [_, ref mut rest @ ..] = a;
        rest[0] = 30;
        rest[2] += 1;
        assert_eq!(rest.len(), 3);
    }
    assert_eq!(a, [1, 30, 3, 5]);
    // default binding modes through & and &mut
    {
        let r = &a;
        let [f, rest @ ..] = r;
        assert_eq!(*f, 1);
        assert_eq!(*rest, [30, 3, 5]);
        assert_eq!(rest.len(), 3);
        let [mid @ .., l] = r;
        assert_eq!(*mid, [1, 30, 3]);
        assert_eq!(*l, 5);
    }
    {
        let r = &mut a;
        let [f, rest @ ..] = r;
        *f = 100;
        rest[0] = 200;
        for i in 0..rest.len() {
            rest[i] += 1;
        }
    }
    assert_eq!(a, [100, 201, 4, 6]);
    // slices still work
    let s: &[u8] = &a;
    if let [first, rest @ ..] = s {
        assert_eq!(*first, 100);
        assert_eq!(rest, &[201u8, 4, 6]);
        assert_eq!(rest.len(), 3);
    } else {
        panic!();
    }
    match s {
        [.., prev, last] => {
            assert_eq!((*prev, *last), (4, 6));
        }
        _ => panic!(),
    }
    match s {
        [a, mid @ .., z] => {
            assert_eq!(mid.len(), 2);
            assert_eq!((*a, *z), (100, 6));
        }
        _ => panic!(),
    }
    // unsizing a sub-array reference
    let [_, ref sub @ ..] = a;
    let as_slice: &[u8] = sub;
    assert_eq!(as_slice.len(), 3);
    assert_eq!(as_slice[0], 201);
    let mut total = 0u32;
    for b in sub {
        total += *b as u32;
    }
    assert_eq!(total, 211);
    println!("{:?}", sub);
    assert_eq!(format!("{:?}", sub), "[201, 4, 6]");
    println!("ok");
}
"#,
    );
}

#[test]
fn array_subslice_by_value_drop_elements_follow_ownership() {
    completed(
        r#"use std::cell::Cell;
struct D<'a>(&'a Cell<u32>, u32);
impl<'a> D<'a> { fn id(&self) -> u32 { self.1 } }
impl<'a> Drop for D<'a> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        self.0.set(self.0.get() * 10 + self.1);
    }
}
fn main() {
    let c = Cell::new(0);
    {
        let [a, rest @ ..] = [D(&c, 1), D(&c, 2), D(&c, 3)];
        assert_eq!(a.id(), 1);
        assert_eq!(rest.len(), 3 - 1);
        assert_eq!(rest[0].id(), 2);
        assert_eq!(rest[1].id(), 3);
        assert_eq!(c.get(), 0);
    }
    // a dropped first (declared first, dropped last? locals drop in reverse):
    // rest dropped first (2 then 3), then a.
    // c: after 2: (0+1)*10+2 = 12; after 3: (12+1)*10+3 = 133; after 1: (133+1)*10+1 = 1341
    assert_eq!(c.get(), 1341);

    let c = Cell::new(0);
    {
        let arr = [D(&c, 1), D(&c, 2), D(&c, 3), D(&c, 4)];
        let [first, .., last] = arr;
        assert_eq!((first.id(), last.id()), (1, 4));
        assert_eq!(c.get(), 0);
    }
    // locals drop in reverse: last (4), first (1), then arr's remaining 2, 3.
    // 4: 14; 1: (14+1)*10+1 = 151; 2: (151+1)*10+2 = 1522; 3: (1522+1)*10+3 = 15233
    assert_eq!(c.get(), 15233);

    // by-reference: nothing moves
    let c = Cell::new(0);
    {
        let arr = [D(&c, 1), D(&c, 2), D(&c, 3)];
        let [ref a, ref rest @ ..] = arr;
        assert_eq!(a.id(), 1);
        assert_eq!(rest.len(), 2);
        assert_eq!(rest[1].id(), 3);
        assert_eq!(c.get(), 0);
    }
    // arr drops 1, 2, 3 in order: 11; (11+1)*10+2 = 122; (122+1)*10+3 = 1233
    assert_eq!(c.get(), 1233);

    // a non-matching arm moves nothing
    let c = Cell::new(0);
    {
        let arr = [D(&c, 1), D(&c, 2), D(&c, 3)];
        match arr {
            [D(_, 9), ..] => panic!(),
            [first, rest @ ..] => {
                assert_eq!(first.id(), 1);
                assert_eq!(rest.len(), 2);
                assert_eq!(c.get(), 0);
            }
        }
    }
    assert_eq!(c.get(), 1341);

    // rest moved into a function, dropped there
    let c = Cell::new(0);
    fn eat(x: [D<'_>; 2]) {
        let _ = &x;
    }
    {
        let [a, rest @ ..] = [D(&c, 1), D(&c, 2), D(&c, 3)];
        eat(rest);
        assert_eq!(c.get(), 133);
        drop(a);
        assert_eq!(c.get(), 1341);
    }
    assert_eq!(c.get(), 1341);
    println!("ok");
}
"#,
    );
}

#[test]
fn array_subslice_in_parameters_and_closure_parameters() {
    completed(
        r#"fn f([a, rest @ ..]: [u8; 4]) -> u32 {
    assert_eq!(rest.len(), 3);
    a as u32 + rest[2] as u32
}
fn g(&[a, ref rest @ ..]: &[u8; 3]) -> u32 {
    a as u32 + rest.len() as u32
}
fn h([first, mid @ .., last]: [i16; 5]) -> i16 {
    first + last + mid[1]
}
fn k((x, [y, rest @ ..]): (u8, [u8; 3])) -> u8 {
    x + y + rest[0] + rest[1]
}
fn main() {
    assert_eq!(f([1, 2, 3, 4]), 5);
    assert_eq!(g(&[5, 6, 7]), 7);
    assert_eq!(h([1, 2, 3, 4, 5]), 9);
    assert_eq!(k((1, [2, 3, 4])), 10);
    let c = |[a, rest @ ..]: [u8; 3]| a as usize + rest.len();
    assert_eq!(c([7, 8, 9]), 9);
    let d = |&[a, ref rest @ ..]: &[u8; 3]| a as usize + rest.len() + rest[1] as usize;
    assert_eq!(d(&[7, 8, 9]), 7 + 2 + 9);
    let e = |[_, rest @ ..]: &[i32; 3]| rest[0] + rest[1];
    assert_eq!(e(&[1, 2, 3]), 5);
    for [a, rest @ ..] in [[1u8, 2, 3], [4, 5, 6]] {
        assert_eq!(rest.len(), 2);
        assert!(a == 1 || a == 4);
        assert_eq!(rest[0], a + 1);
    }
    println!("ok");
}
"#,
    );
}

#[test]
fn array_subslice_emitted_by_macros_and_matches() {
    completed(
        r#"macro_rules! first_and_rest {
    ($arr:expr) => {{
        let [f, rest @ ..] = $arr;
        (f, rest)
    }};
}
macro_rules! pat {
    ($a:ident, $r:ident) => {
        [$a, $r @ ..]
    };
}
macro_rules! check {
    ($arr:expr, $p:pat => $e:expr) => {
        match $arr {
            $p => $e,
        }
    };
}
fn main() {
    let (f, rest) = first_and_rest!([1u8, 2, 3]);
    assert_eq!(f, 1);
    assert_eq!(rest, [2, 3]);
    let pat!(a, r) = [4u8, 5, 6];
    assert_eq!(a, 4);
    assert_eq!(r, [5, 6]);
    let n = check!([1u8, 2, 3, 4], [_, rest @ ..] => rest.len() + rest[2] as usize);
    assert_eq!(n, 3 + 4);
    assert!(matches!([1u8, 2, 3], [1, rest @ ..]));
    assert!(!matches!([1u8, 2, 3], [2, ..]));
    let v = [10u8, 20, 30];
    assert!(matches!(&v, [10, rest @ ..] if rest.len() == 2 && rest[1] == 30));
    assert!(matches!(v, [x, ..] if x == 10));
    println!("ok");
}
"#,
    );
}

#[test]
fn array_subslice_transaction_guards_retry_nesting_and_consts() {
    completed(
        r#"use std::cell::Cell;
struct D<'a>(&'a Cell<u32>, u32);
impl<'a> D<'a> {
    fn id(&self) -> u32 {
        self.1
    }
}
impl<'a> Drop for D<'a> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
        self.0.set(self.0.get() * 10 + self.1);
    }
}
fn classify(arr: &[u8; 4]) -> u32 {
    match arr {
        [a, rest @ ..] if *a == 9 => 1000 + rest.len() as u32,
        [a, rest @ ..] if rest[0] == 2 && *a == 1 => 2000 + rest[2] as u32,
        [_, rest @ ..] => 3000 + rest.len() as u32,
    }
}
fn main() {
    // guards with default-binding-mode (reference) sub-arrays, arm retry
    assert_eq!(classify(&[9, 0, 0, 0]), 1003);
    assert_eq!(classify(&[1, 2, 3, 4]), 2004);
    assert_eq!(classify(&[5, 6, 7, 8]), 3003);
    // or patterns
    let f = |a: [u8; 3]| match a {
        [1, rest @ ..] | [2, rest @ ..] => rest[0] as i32,
        [.., last] => -(last as i32),
    };
    assert_eq!(f([1, 10, 20]), 10);
    assert_eq!(f([2, 11, 21]), 11);
    assert_eq!(f([3, 11, 21]), -21);
    // inside an enum and a tuple
    let o = Some([1u8, 2, 3]);
    if let Some([a, rest @ ..]) = o {
        assert_eq!(a, 1);
        assert_eq!(rest, [2u8, 3]);
    } else {
        panic!();
    }
    let t = (7u8, [1u8, 2, 3]);
    let (x, [a, rest @ ..]) = t;
    assert_eq!((x, a), (7, 1));
    assert_eq!(rest, [2u8, 3]);
    // sub-array of a sub-array, by value and by reference
    let arr = [1u8, 2, 3, 4, 5];
    let [a, rest @ ..] = arr;
    let [b, rest2 @ ..] = rest;
    let [.., c] = rest2;
    assert_eq!((a, b, c), (1, 2, 5));
    assert_eq!(rest2, [3u8, 4, 5]);
    let [a, rest @ ..] = &arr;
    let [b, rest2 @ ..] = rest;
    let [mid @ .., c] = rest2;
    assert_eq!((*a, *b, *c), (1, 2, 5));
    assert_eq!(*rest2, [3u8, 4, 5]);
    assert_eq!(*mid, [3u8, 4]);
    assert_eq!(mid.len(), 2);
    // by value out of a dereference
    let r = &arr;
    let [a, rest @ ..] = *r;
    assert_eq!(a, 1);
    assert_eq!(rest, [2u8, 3, 4, 5]);
    // arrays of Copy aggregates and of Strings
    let nested = [[1u8, 2], [3, 4], [5, 6]];
    let [first, rest @ ..] = nested;
    assert_eq!(first, [1u8, 2]);
    assert_eq!(rest, [[3u8, 4], [5, 6]]);
    assert_eq!(rest[1][0], 5);
    let strings = [String::from("a"), String::from("b"), String::from("c")];
    let [s0, rest @ ..] = strings;
    assert_eq!(s0, "a");
    assert_eq!(rest.len(), 2);
    assert_eq!(rest[0], "b");
    assert_eq!(rest[1], "c");
    // ref a, and the rest moves (disjoint places)
    let c = Cell::new(0);
    {
        let arr = [D(&c, 1), D(&c, 2), D(&c, 3)];
        let [ref a, rest @ ..] = arr;
        assert_eq!(a.id(), 1);
        assert_eq!(rest.len(), 2);
        assert_eq!(c.get(), 0);
    }
    // locals: arr (a's element 1 still inside), rest. Drop rest (2,3) first, then arr's 1.
    assert_eq!(c.get(), 1341);
    // mutation through ref mut on arrays of arrays
    let mut grid = [[0u8; 2]; 3];
    {
        let [_, ref mut tail @ ..] = grid;
        tail[0][1] = 7;
        tail[1][0] = 9;
        assert_eq!(tail.len(), 2);
    }
    assert_eq!(grid, [[0u8, 0], [0, 7], [9, 0]]);
    // while let over a slice
    let data: &[u8] = &[1, 2, 3, 4];
    let mut s = data;
    let mut sum = 0u32;
    while let [first, rest @ ..] = s {
        sum += *first as u32;
        s = rest;
    }
    assert_eq!(sum, 10);
    // slice with fixed prefix and suffix
    if let [a, mid @ .., z] = data {
        assert_eq!((*a, *z), (1, 4));
        assert_eq!(mid.len(), 2);
        assert_eq!(mid[0], 2);
    }
    // const context
    const R: [u8; 2] = {
        let [_, r @ ..] = [1u8, 2, 3];
        r
    };
    assert_eq!(R, [2u8, 3]);
    // empty sub-array of non-Copy
    let one = [String::from("only")];
    let [only, empty @ ..] = one;
    assert_eq!(only, "only");
    assert_eq!(empty.len(), 0);
    println!("ok");
}
"#,
    );
}

#[test]
fn array_subslice_guards_on_by_value_copy_arrays_release_their_exposed_bindings() {
    completed(
        r#"fn pick(arr: [u8; 4]) -> u32 {
    match arr {
        [a, rest @ ..] if a == 9 => 1000 + rest.len() as u32,
        [a, rest @ ..] if rest[0] == 2 && a == 1 => 2000 + rest[2] as u32,
        [_, rest @ ..] if rest == [7, 7, 7] => 4000,
        [_, rest @ ..] => 3000 + rest[0] as u32,
    }
}
fn main() {
    assert_eq!(pick([9, 0, 0, 0]), 1003);
    assert_eq!(pick([1, 2, 3, 4]), 2004);
    assert_eq!(pick([5, 7, 7, 7]), 4000);
    assert_eq!(pick([5, 6, 7, 8]), 3006);
    assert!(matches!([1u8, 2, 3], [1, rest @ ..] if rest.len() == 2));
    assert!(matches!([1u8, 2, 3], [1, rest @ ..] if rest == [2, 3]));
    assert!(!matches!([1u8, 2, 3], [1, rest @ ..] if rest == [3, 3]));
    assert!(matches!([[1u8, 2], [3, 4]], [a, ..] if a[0] == 1));
    let t = ((1u8, 2u8), 3u8);
    assert!(matches!(t, (a, _) if a.0 == 1));
}
"#,
    );
}

#[test]
fn array_subslice_slice_methods_and_mutation_through_sized_references() {
    completed(
        r#"fn main() {
    let a = [1u8, 2, 3, 4, 5];
    let [_, ref rest @ ..] = a;
    assert_eq!(&rest[1..], &[3u8, 4, 5]);
    assert_eq!(&rest[..2], &[2u8, 3]);
    assert_eq!(rest.first(), Some(&2));
    assert_eq!(rest.last(), Some(&5));
    assert!(rest.contains(&4));
    assert!(!rest.is_empty());
    let v = rest.to_vec();
    assert_eq!(v.len(), 4);
    assert_eq!(v[3], 5);
    let mut it = rest.iter();
    assert_eq!(it.next(), Some(&2));
    let mut n = 0;
    for x in rest.iter().rev() {
        n = n * 10 + *x as u32;
    }
    assert_eq!(n, 5432);
    let s: &[u8] = rest;
    assert_eq!(s.len(), 4);
    let [x, y, ..] = *rest;
    assert_eq!((x, y), (2, 3));
    let [_, ref rest2 @ ..] = *rest;
    assert_eq!(rest2.len(), 3);
    assert_eq!(rest2[2], 5);
    let t: (usize, &[u8; 4]) = (0, rest);
    assert_eq!(t.1[0], 2);
}
"#,
    );
}

#[test]
fn array_subslice_mutating_methods_through_mutable_sized_references() {
    completed(
        r#"fn bump(x: &mut [u8; 3]) {
    x[0] += 10;
    x[2] = 99;
}
fn main() {
    let mut a = [5u8, 3, 1, 4];
    {
        let [_, rest @ ..] = &mut a;
        rest.swap(0, 2);
        assert_eq!(rest[0], 4);
    }
    assert_eq!(a, [5u8, 4, 1, 3]);
    {
        let [_, ref mut rest @ ..] = a;
        rest.sort();
        bump(rest);
    }
    assert_eq!(a, [5u8, 11, 3, 99]);
    {
        let [_, rest @ ..] = &mut a;
        rest.fill(7);
    }
    assert_eq!(a, [5u8, 7, 7, 7]);
    {
        let [_, rest @ ..] = &mut a;
        rest.reverse();
        let [x, y, z] = rest;
        *x = 1;
        *y = 2;
        *z = 3;
    }
    assert_eq!(a, [5u8, 1, 2, 3]);
}
"#,
    );
}

#[test]
fn array_subslice_at_bindings_nested_fields_returns_and_trait_objects() {
    completed(
        r#"fn main() {
    let whole @ [first, rest @ ..] = [1u8, 2, 3];
    assert_eq!(whole, [1u8, 2, 3]);
    assert_eq!(first, 1);
    assert_eq!(rest, [2u8, 3]);
    let arr = [4u8, 5, 6];
    let r @ [_, tail @ ..] = &arr;
    assert_eq!(*r, [4u8, 5, 6]);
    assert_eq!(*tail, [5u8, 6]);
    let whole2 @ [.., last] = [7u8, 8, 9];
    assert_eq!(whole2[0], 7);
    assert_eq!(last, 9);
    match arr {
        all @ [4, rest @ ..] => {
            assert_eq!(all, [4u8, 5, 6]);
            assert_eq!(rest, [5u8, 6]);
        }
        _ => panic!(),
    }
}
"#,
    );
}

#[test]
fn array_subslice_in_struct_enum_closure_and_return_positions() {
    completed(
        r#"struct S { arr: [u8; 4], tag: u8 }
enum E { A([i32; 3]), B { items: [i32; 2] } }
fn main() {
    let [a, rest @ ..]: [u8; 3] = [1, 2, 3];
    assert_eq!(a, 1);
    assert_eq!(rest, [2u8, 3]);
    let s = S { arr: [1, 2, 3, 4], tag: 9 };
    let S { arr: [first, mid @ .., last], tag } = s;
    assert_eq!((first, last, tag), (1, 4, 9));
    assert_eq!(mid, [2u8, 3]);
    let e = E::A([5, 6, 7]);
    match e {
        E::A([x, tail @ ..]) => {
            assert_eq!(x, 5);
            assert_eq!(tail, [6, 7]);
        }
        E::B { items: [y, rest2 @ ..] } => {
            assert_eq!(y, 0);
            assert_eq!(rest2.len(), 1);
        }
    }
    let e2 = E::B { items: [8, 9] };
    if let E::B { items: [y, rest2 @ ..] } = e2 {
        assert_eq!(y, 8);
        assert_eq!(rest2, [9]);
    }
    // closure capture of a sub-array binding
    let arr = [1usize, 2, 3, 4];
    let [h, rest @ ..] = arr;
    let c = || rest.len() + h;
    assert_eq!(c(), 4);
    let m = move || rest[2] + h;
    assert_eq!(m(), 5);
    // shadowing and re-destructuring in a loop
    let mut cur = [1u32, 2, 3];
    let mut acc = 0;
    loop {
        let [head, tail @ ..] = cur;
        acc += head;
        if tail[0] == 0 {
            break;
        }
        cur = [tail[0], tail[1], 0];
    }
    assert_eq!(acc, 6);
    // references to a sub-array flow through functions returning refs
    fn tail_of(a: &[i32; 4]) -> &[i32; 3] {
        let [_, t @ ..] = a;
        t
    }
    let base = [1, 2, 3, 4];
    let t = tail_of(&base);
    assert_eq!(t[0] + t[2], 6);
    assert_eq!(t.len(), 3);
    fn tail_mut(a: &mut [i32; 4]) -> &mut [i32; 3] {
        let [_, t @ ..] = a;
        t
    }
    let mut base2 = [1, 2, 3, 4];
    {
        let t = tail_mut(&mut base2);
        t[1] = 30;
    }
    assert_eq!(base2, [1, 2, 30, 4]);
    // returned by value
    fn by_value(a: [u8; 4]) -> [u8; 3] {
        let [_, t @ ..] = a;
        t
    }
    assert_eq!(by_value([9, 8, 7, 6]), [8u8, 7, 6]);
    fn strings(a: [String; 3]) -> [String; 2] {
        let [_, t @ ..] = a;
        t
    }
    let st = strings([String::from("x"), String::from("y"), String::from("z")]);
    assert_eq!(st[0], "y");
    assert_eq!(st[1], "z");
}
"#,
    );
}

#[test]
fn array_subslice_closures_capture_arrays_and_own_drop_counts() {
    completed(
        r#"struct D<'a>(&'a std::cell::Cell<u32>);
impl<'a> Drop for D<'a> { fn drop(&mut self) { self.0.set(self.0.get() + 1); } }
fn main() {
    let arr = [1u8, 2, 3];
    let c = || {
        let [a, rest @ ..] = arr;
        a as usize + rest.len()
    };
    assert_eq!(c(), 3);
    assert_eq!(arr, [1u8, 2, 3]);
    let mut m = [1u8, 2, 3];
    let mut inc = || {
        let [_, rest @ ..] = &mut m;
        rest[0] += 5;
    };
    inc();
    inc();
    assert_eq!(m, [1u8, 12, 3]);
    let mv = move || {
        let [x, ref r @ ..] = arr;
        x as usize + r.len()
    };
    assert_eq!(mv(), 3);
    let cnt = std::cell::Cell::new(0);
    {
        let ds = [D(&cnt), D(&cnt), D(&cnt)];
        let take = move || {
            let [_first, rest @ ..] = ds;
            rest.len()
        };
        assert_eq!(take(), 2);
        assert_eq!(cnt.get(), 3);
    }
    assert_eq!(cnt.get(), 3);
}
"#,
    );
}

#[test]
fn array_subslice_of_trait_objects() {
    completed(
        r#"trait T { fn v(&self) -> u32; }
struct A;
struct B;
impl T for A { fn v(&self) -> u32 { 1 } }
impl T for B { fn v(&self) -> u32 { 2 } }
fn main() {
    let arr: [Box<dyn T>; 3] = [Box::new(A), Box::new(B), Box::new(A)];
    let [first, rest @ ..] = arr;
    assert_eq!(first.v(), 1);
    assert_eq!(rest.len(), 2);
    assert_eq!(rest[0].v(), 2);
    assert_eq!(rest[1].v(), 1);
    let arr2: [Box<dyn T>; 3] = [Box::new(B), Box::new(B), Box::new(A)];
    let [_, ref tail @ ..] = arr2;
    assert_eq!(tail[1].v(), 1);
    let [x, y] = tail else { panic!() };
    assert_eq!(x.v() + y.v(), 3);
}
"#,
    );
}
