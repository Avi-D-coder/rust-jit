//! `core::mem` layout intrinsics (E-R5-012). Expected output is real rustc 1.99.0's on a 64-bit
//! macOS host (`rustc +1.99.0 -O`, edition 2021). Each program runs in all four editions.
//! One layout algorithm (`runtime/layout.rs`) answers every form: handwritten, user macro,
//! generic wrapper, `const`/`static` initializer and `assert_eq!` operand.
use super::format_trait_facade_tests::prints;

/// Scalars, thin and fat pointers, `Box`, fn pointers, arrays and tuples (`rustc` repr(Rust) field reordering).
#[test]
fn size_and_align_of_scalars_pointers_arrays_and_tuples() {
    prints(
        "mem-scalars",
        r##"use std::mem::{align_of, size_of};
macro_rules! show { ($t:ty) => { println!("{} {}", size_of::<$t>(), align_of::<$t>()); } }
fn main() {
    show!(());
    show!(bool);
    show!(char);
    show!(u8);
    show!(i16);
    show!(u32);
    show!(i64);
    show!(usize);
    show!(f32);
    show!(f64);
    show!(&u8);
    show!(&str);
    show!(&[u8]);
    show!(*const u8);
    show!(*const [u8]);
    show!(Box<u8>);
    show!(Box<str>);
    show!(Box<[u8]>);
    show!(fn());
    show!(fn(u8) -> u8);
    show!([u8; 0]);
    show!([u32; 3]);
    show!([u16; 5]);
    show!([bool; 2]);
    show!((u8, u32));
    show!((u8, u16, u8));
    show!((u8, (u32, u8)));
}
"##,
        r##"0 1
1 1
4 4
1 1
2 2
4 4
8 8
8 8
4 4
8 8
8 8
16 8
16 8
8 8
16 8
8 8
16 8
16 8
8 8
8 8
0 1
12 4
10 2
2 1
8 4
4 2
12 4
"##,
    );
}

/// Structs under `repr(Rust)`, `repr(C)`, `repr(packed(N))`, `repr(align(N))`, generic and nested.
#[test]
fn structs_follow_repr_rust_c_packed_and_align() {
    prints(
        "mem-structs",
        r##"use std::mem::{align_of, size_of};
struct A { a: u8, b: u32, c: u16 }
#[repr(C)] struct B { a: u8, b: u32, c: u16 }
#[repr(C, packed)] struct C { a: u8, b: u32, c: u16 }
#[repr(packed(2))] struct D { a: u8, b: u32, c: u16 }
#[repr(align(16))] struct E { a: u8 }
#[repr(C, align(8))] struct F { a: u8, b: u16 }
struct G<T>(T, u8);
struct W(u8, bool);
struct Unit;
struct Nested { a: A, b: u8 }
struct Pair<T, U> { first: T, second: U }
fn main() {
    println!("{} {}", size_of::<A>(), align_of::<A>());
    println!("{} {}", size_of::<B>(), align_of::<B>());
    println!("{} {}", size_of::<C>(), align_of::<C>());
    println!("{} {}", size_of::<D>(), align_of::<D>());
    println!("{} {}", size_of::<E>(), align_of::<E>());
    println!("{} {}", size_of::<F>(), align_of::<F>());
    println!("{} {}", size_of::<G<u8>>(), align_of::<G<u8>>());
    println!("{} {}", size_of::<G<u32>>(), align_of::<G<u32>>());
    println!("{} {}", size_of::<W>(), align_of::<W>());
    println!("{} {}", size_of::<Unit>(), align_of::<Unit>());
    println!("{} {}", size_of::<Nested>(), align_of::<Nested>());
    println!("{} {}", size_of::<Pair<u8, u64>>(), align_of::<Pair<u8, u64>>());
    println!("{} {}", size_of::<Pair<Pair<u8, u16>, u8>>(), align_of::<Pair<Pair<u8, u16>, u8>>());
}
"##,
        r##"8 4
12 4
7 1
8 2
16 16
8 8
2 1
8 4
2 1
0 1
12 4
16 8
6 2
"##,
    );
}

/// Fieldless, single-variant, empty, `repr(u8/i32/C)` and data-carrying enums.
#[test]
fn enums_use_the_smallest_tag_explicit_repr_and_variant_packing() {
    prints(
        "mem-enums",
        r##"use std::mem::{align_of, size_of};
enum Empty {}
enum One { X }
enum OneData { X(u32) }
enum Three { X, Y, Z }
#[repr(u8)] enum H { X, Y }
#[repr(i32)] enum I { X = -1, Y = 5 }
#[repr(C)] enum J { X, Y }
enum K { X = 300, Y }
enum L { X(u32), Y }
enum M { X(u8), Y(u16), Z }
enum N { X(bool), Y }
enum O { X(Box<u8>), Y, Z }
enum P { X(u32, u8), Y(u8) }
enum Q { X(u64), Y(u8, u8) }
#[repr(u8)] enum T1 { X(u32), Y }
#[repr(C)] enum T2 { X(u32), Y(u8) }
#[repr(C, u8)] enum T3 { X(u64), Y(u8) }
enum Generic<T> { A(T), B }
fn main() {
    println!("{} {}", size_of::<Empty>(), align_of::<Empty>());
    println!("{} {}", size_of::<One>(), align_of::<One>());
    println!("{} {}", size_of::<OneData>(), align_of::<OneData>());
    println!("{} {}", size_of::<Three>(), align_of::<Three>());
    println!("{} {}", size_of::<H>(), align_of::<H>());
    println!("{} {}", size_of::<I>(), align_of::<I>());
    println!("{} {}", size_of::<J>(), align_of::<J>());
    println!("{} {}", size_of::<K>(), align_of::<K>());
    println!("{} {}", size_of::<L>(), align_of::<L>());
    println!("{} {}", size_of::<M>(), align_of::<M>());
    println!("{} {}", size_of::<N>(), align_of::<N>());
    println!("{} {}", size_of::<O>(), align_of::<O>());
    println!("{} {}", size_of::<P>(), align_of::<P>());
    println!("{} {}", size_of::<Q>(), align_of::<Q>());
    println!("{} {}", size_of::<T1>(), align_of::<T1>());
    println!("{} {}", size_of::<T2>(), align_of::<T2>());
    println!("{} {}", size_of::<T3>(), align_of::<T3>());
    println!("{} {}", size_of::<Generic<u8>>(), size_of::<Generic<u32>>());
    println!("{} {}", size_of::<Generic<bool>>(), size_of::<Generic<char>>());
    println!("{} {}", size_of::<Generic<&u8>>(), size_of::<Generic<Generic<bool>>>());
}
"##,
        r##"0 1
0 1
4 4
1 1
1 1
4 4
4 4
2 2
8 4
4 2
1 1
16 8
8 4
16 8
8 4
8 4
16 8
2 8
1 4
8 1
"##,
    );
}

/// Niche filling: references, `Box`, fn pointers, `bool`, `char`, enum tags, fat pointers.
#[test]
fn niche_optimization_matches_rustc() {
    prints(
        "mem-niches",
        r##"use std::mem::size_of;
#[derive(Clone, Copy)] enum G { X, Y, Z }
enum L { X(u32), Y }
enum M { X(u8), Y(u16), Z }
enum O { X(Box<u8>), Y, Z }
fn main() {
    println!("{} {} {}", size_of::<Option<u8>>(), size_of::<Option<u32>>(), size_of::<Option<usize>>());
    println!("{} {} {}", size_of::<Option<bool>>(), size_of::<Option<Option<bool>>>(), size_of::<Option<char>>());
    println!("{} {} {}", size_of::<Option<&u8>>(), size_of::<Option<Box<u8>>>(), size_of::<Option<fn()>>());
    println!("{} {}", size_of::<Option<*const u8>>(), size_of::<Option<()>>());
    println!("{} {} {}", size_of::<Option<G>>(), size_of::<Option<Option<G>>>(), size_of::<Option<L>>());
    println!("{} {} {}", size_of::<Option<M>>(), size_of::<Option<O>>(), size_of::<Option<Option<O>>>());
    println!("{} {} {}", size_of::<Result<G, ()>>(), size_of::<Result<u8, ()>>(), size_of::<Result<(), ()>>());
    println!("{} {} {}", size_of::<Result<u32, u8>>(), size_of::<Result<u32, G>>(), size_of::<Result<bool, bool>>());
    println!("{} {}", size_of::<Result<&u8, ()>>(), size_of::<Option<(u8, bool)>>());
    println!("{} {}", size_of::<Option<[bool; 2]>>(), size_of::<Option<&str>>());
    println!("{} {}", size_of::<Option<&[u8]>>(), size_of::<Option<Box<[u8]>>>());
}
"##,
        r##"2 8 16
1 1 4
8 8 8
16 1
1 1 8
4 16 16
1 2 1
8 8 2
8 2
2 16
16 16
"##,
    );
}

/// `Vec`, `String`, `Rc`, collections, `Cell`/`RefCell`, `ManuallyDrop`, `Range`.
#[test]
fn library_types_have_their_std_layouts() {
    prints(
        "mem-library",
        r##"use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::mem::{align_of, size_of};
use std::rc::Rc;
fn main() {
    println!("{} {}", size_of::<Vec<u8>>(), size_of::<String>());
    println!("{} {}", size_of::<Option<Vec<u8>>>(), size_of::<Option<String>>());
    println!("{} {}", size_of::<Vec<Vec<u8>>>(), size_of::<Option<Option<String>>>());
    println!("{} {}", size_of::<Rc<u8>>(), size_of::<Option<Rc<u8>>>());
    println!("{} {}", size_of::<Rc<RefCell<u8>>>(), size_of::<Option<Rc<RefCell<u8>>>>());
    println!("{} {} {}", size_of::<HashMap<u8, u8>>(), size_of::<BTreeMap<u8, u8>>(), size_of::<VecDeque<u8>>());
    println!("{} {}", size_of::<Option<HashMap<u8, u8>>>(), size_of::<Option<BTreeMap<u8, u8>>>());
    println!("{} {} {}", size_of::<Cell<u32>>(), size_of::<Cell<bool>>(), size_of::<Option<Cell<bool>>>());
    println!("{} {}", size_of::<RefCell<u32>>(), align_of::<RefCell<u8>>());
    println!("{} {}", size_of::<std::mem::ManuallyDrop<u32>>(), size_of::<Option<std::mem::ManuallyDrop<&u8>>>());
    println!("{} {}", size_of::<std::marker::PhantomData<u32>>(), size_of::<std::num::Wrapping<u32>>());
    println!("{} {}", size_of::<std::cmp::Ordering>(), size_of::<Option<std::cmp::Ordering>>());
    println!("{} {}", size_of::<std::ops::Range<u32>>(), size_of::<std::ops::Range<usize>>());
}
"##,
        r##"24 24
24 24
24 24
8 8
8 8
48 24 32
48 32
4 1 2
16 8
4 8
0 4
1 1
8 16
"##,
    );
}

/// `size_of_val`/`align_of_val` of structs, arrays, slices, `str`, trait objects, closures and fn items.
#[test]
fn size_of_val_and_align_of_val_read_the_referent() {
    prints(
        "mem-values",
        r##"use std::mem::{align_of_val, size_of_val};
struct A { a: u8, b: u32, c: u16 }
trait Shape { fn area(&self) -> u32; }
struct Sq(u32);
struct Big([u8; 40]);
impl Shape for Sq { fn area(&self) -> u32 { self.0 } }
impl Shape for Big { fn area(&self) -> u32 { 1 } }
fn main() {
    let a = A { a: 1, b: 2, c: 3 };
    println!("{} {}", size_of_val(&a), align_of_val(&a));
    let arr = [1u16, 2, 3, 4];
    println!("{} {}", size_of_val(&arr), align_of_val(&arr));
    let slice: &[u16] = &arr[1..];
    println!("{} {}", size_of_val(slice), align_of_val(slice));
    let text = "héllo";
    println!("{} {}", size_of_val(text), align_of_val(text));
    println!("{} {}", size_of_val(&5u8), size_of_val(&5u64));
    println!("{}", size_of_val(&1));
    println!("{}", size_of_val(&1.5));
    println!("{}", size_of_val(&()));
    println!("{}", size_of_val(&(1u8, 2u32)));
    println!("{}", size_of_val(&Some(3u8)));
    println!("{}", size_of_val(&Some(Box::new(3u8))));
    let shapes: Vec<Box<dyn Shape>> = vec![Box::new(Sq(2)), Box::new(Big([0; 40]))];
    for shape in &shapes {
        println!("{} {}", size_of_val(&**shape), shape.area());
    }
    let r: &dyn Shape = &Sq(1);
    println!("{} {}", size_of_val(r), align_of_val(r));
    let n = 3u32;
    let by_ref = || n + 1;
    println!("{}", size_of_val(&by_ref));
    let captured = String::from("x");
    let by_move = move || captured.len();
    println!("{}", size_of_val(&by_move));
    let none = || 1;
    println!("{}", size_of_val(&none));
    println!("{}", size_of_val(&main));
    let v = vec![1u32, 2, 3];
    println!("{} {}", size_of_val(&v), size_of_val(&v[..]));
    let s = String::from("abc");
    println!("{} {}", size_of_val(&s), size_of_val(s.as_str()));
}
"##,
        r##"8 4
8 2
6 2
6 1
1 8
4
8
0
8
2
8
4 2
40 1
4 4
8
24
0
0
24 12
24 3
"##,
    );
}

/// `needs_drop` over primitives, owning library types, user `Drop` impls, aggregates and `ManuallyDrop`.
#[test]
fn needs_drop_follows_drop_glue() {
    prints(
        "mem-drop",
        r##"use std::mem::{needs_drop, ManuallyDrop};
use std::rc::Rc;
use std::cell::RefCell;
struct Trivial(u8, f32);
struct NonTrivial(u8, String);
struct WithDrop;
impl Drop for WithDrop { fn drop(&mut self) {} }
struct Wraps(WithDrop);
enum E { A, B(u8) }
enum F { A, B(String) }
struct Generic<T>(T);
fn main() {
    println!("{} {} {}", needs_drop::<u8>(), needs_drop::<bool>(), needs_drop::<char>());
    println!("{} {} {}", needs_drop::<String>(), needs_drop::<Vec<u8>>(), needs_drop::<Box<u8>>());
    println!("{} {}", needs_drop::<&String>(), needs_drop::<&mut String>());
    println!("{} {} {}", needs_drop::<Trivial>(), needs_drop::<NonTrivial>(), needs_drop::<WithDrop>());
    println!("{} {}", needs_drop::<Wraps>(), needs_drop::<(u8, String)>());
    println!("{} {}", needs_drop::<(u8, u16)>(), needs_drop::<[String; 2]>());
    println!("{} {}", needs_drop::<[String; 0]>(), needs_drop::<[u8; 4]>());
    println!("{} {}", needs_drop::<E>(), needs_drop::<F>());
    println!("{} {}", needs_drop::<Option<u8>>(), needs_drop::<Option<String>>());
    println!("{} {}", needs_drop::<Generic<u8>>(), needs_drop::<Generic<String>>());
    println!("{} {}", needs_drop::<ManuallyDrop<String>>(), needs_drop::<ManuallyDrop<u8>>());
    println!("{} {}", needs_drop::<Rc<u8>>(), needs_drop::<RefCell<Vec<u8>>>());
    println!("{} {}", needs_drop::<RefCell<u8>>(), needs_drop::<std::marker::PhantomData<String>>());
    println!("{} {}", needs_drop::<fn()>(), needs_drop::<*const String>());
    println!("{} {}", needs_drop::<str>(), needs_drop::<[String]>());
}
"##,
        r##"false false false
true true true
false false
false true true
true true
false true
false false
false true
false true
false true
false false
true true
false false
false false
false true
"##,
    );
}

/// `mem::discriminant` is `Copy`/`Eq`/`Hash`/`Debug` and distinguishes exactly the variants.
#[test]
fn mem_discriminant_compares_hashes_and_prints_by_variant() {
    prints(
        "mem-discriminants",
        r##"use std::collections::HashSet;
use std::mem::{discriminant, Discriminant};
#[derive(Debug)]
enum Test { A(u8), B, C { a: u8, b: u8 } }
enum Fieldless { P = 5, Q = 9 }
fn main() {
    let a1 = discriminant(&Test::A(1));
    let a2 = discriminant(&Test::A(200));
    let b = discriminant(&Test::B);
    let c = discriminant(&Test::C { a: 1, b: 2 });
    println!("{} {} {}", a1 == a2, a1 == b, b == c);
    println!("{} {}", a1 != b, a1 != a2);
    let copy = a1;
    println!("{}", copy == a1);
    println!("{:?} {:?} {:?}", a1, b, c);
    println!("{:?} {:?}", discriminant(&Fieldless::P), discriminant(&Fieldless::Q));
    let mut seen: HashSet<Discriminant<Test>> = HashSet::new();
    for t in [Test::A(1), Test::A(2), Test::B, Test::C { a: 0, b: 0 }, Test::B] {
        seen.insert(discriminant(&t));
    }
    println!("{}", seen.len());
    println!("{}", discriminant(&Some(1)) == discriminant(&Some(2)));
    println!("{}", discriminant(&Some(1)) == discriminant(&None));
}
"##,
        r##"true false false
true false
true
Discriminant(0) Discriminant(1) Discriminant(2)
Discriminant(5) Discriminant(9)
3
true
false
"##,
    );
}

/// `mem::transmute`/`mem::zeroed` over scalars, raw pointers, arrays and tuples of them.
#[test]
fn transmute_and_zeroed_between_scalars_pointers_and_arrays() {
    prints(
        "mem-transmute",
        r##"use std::mem::{transmute, zeroed};
fn main() {
    let bits: u32 = unsafe { transmute::<f32, u32>(1.0f32) };
    println!("{:#x}", bits);
    let f: f32 = unsafe { transmute::<u32, f32>(0x40490fdb) };
    println!("{}", f);
    let s: u32 = unsafe { transmute::<i32, u32>(-1) };
    println!("{}", s);
    let arr: [u8; 4] = unsafe { transmute::<u32, [u8; 4]>(0x01020304) };
    println!("{:?}", arr);
    let n: u32 = unsafe { transmute::<[u8; 4], u32>([1, 0, 0, 0]) };
    println!("{}", n);
    let z: u64 = unsafe { zeroed::<u64>() };
    println!("{}", z);
    let za: [u16; 3] = unsafe { zeroed::<[u16; 3]>() };
    println!("{:?}", za);
    let zt: (u8, u32, bool) = unsafe { zeroed::<(u8, u32, bool)>() };
    println!("{:?}", zt);
    let p: usize = unsafe { transmute::<*const u8, usize>(std::ptr::null()) };
    println!("{}", p);
    let x = 5u8;
    let addr = &x as *const u8;
    let a: usize = unsafe { transmute::<*const u8, usize>(addr) };
    let b: *const u8 = unsafe { transmute::<usize, *const u8>(a) };
    println!("{}", unsafe { *b });
    let c: char = unsafe { transmute::<u32, char>(65) };
    println!("{}", c);
    let t: bool = unsafe { transmute::<u8, bool>(1) };
    println!("{}", t);
}
"##,
        r##"0x3f800000
3.1415927
4294967295
[4, 3, 2, 1]
1
0
[0, 0, 0]
(0, 0, false)
0
5
A
true
"##,
    );
}

/// Synthetic pointer strides use the same layout `size_of` reports.
#[test]
fn pointer_strides_agree_with_size_of() {
    prints(
        "mem-strides",
        r##"use std::mem::size_of;
struct T { a: u8, b: u32, c: u16 }
#[repr(C)] struct U { a: u8, b: u32, c: u16 }
macro_rules! stride { ($x:expr) => { (&$x[1] as *const _ as usize) - (&$x[0] as *const _ as usize) } }
fn main() {
    let t = [T { a: 1, b: 2, c: 3 }, T { a: 4, b: 5, c: 6 }];
    println!("{} {}", size_of::<T>(), stride!(t));
    let u = [U { a: 1, b: 2, c: 3 }, U { a: 4, b: 5, c: 6 }];
    println!("{} {}", size_of::<U>(), stride!(u));
    let o = [Some(1u8), None];
    println!("{} {}", size_of::<Option<u8>>(), stride!(o));
    let x = [(1u8, 2u32), (3, 4)];
    println!("{} {}", size_of::<(u8, u32)>(), stride!(x));
    let y = [Some(&x), None];
    println!("{} {}", size_of::<Option<&[(u8, u32); 2]>>(), stride!(y));
}
"##,
        r##"8 8
12 12
2 2
8 8
8 8
"##,
    );
}

/// User types holding `Vec`, `HashMap`, `Rc<RefCell<..>>`, `Box<dyn Fn>`, slices, `str`, recursion through `Box`, and a data-carrying enum.
#[test]
fn composite_user_types_with_library_and_trait_object_fields() {
    prints(
        "mem-composites",
        r##"use std::collections::HashMap;
use std::mem::{size_of, align_of, size_of_val};
use std::rc::Rc;
use std::cell::RefCell;
struct List { head: Option<Box<List>>, v: u32 }
struct Node { next: Option<Rc<RefCell<Node>>>, val: i32 }
struct Holder { v: Vec<String>, m: HashMap<u8, u8>, c: char }
struct Slice<'a> { r: &'a [u8], s: &'a str }
struct Dynamic { f: Box<dyn Fn(u8) -> u8>, n: u8 }
#[derive(Clone, Copy)] struct P { x: f64, y: f64 }
enum Shape { Circle(f64), Rect(P, P), Empty }
fn main() {
    println!("{} {}", size_of::<List>(), size_of::<Node>());
    println!("{} {}", size_of::<Holder>(), align_of::<Holder>());
    println!("{} {}", size_of::<Slice>(), size_of::<Dynamic>());
    println!("{} {}", size_of::<Shape>(), size_of::<Option<Shape>>());
    println!("{}", size_of_val(&Shape::Empty));
    let c = |x: u8| x + 1;
    let d: Dynamic = Dynamic { f: Box::new(c), n: 0 };
    println!("{}", size_of_val(&d));
    println!("{}", size_of_val(&c));
}
"##,
        r##"16 16
80 8
32 24
40 40
40
24
0
"##,
    );
}

/// Handwritten, in `const`/`static` initializers, generic functions, user macros and `assert_eq!`.
#[test]
fn layout_queries_in_const_static_generic_macro_and_assert_contexts() {
    prints(
        "mem-contexts",
        r##"use std::mem::{needs_drop, size_of};
struct S { a: u8, b: u32 }
const SIZE: usize = size_of::<u64>();
const S_SIZE: usize = size_of::<S>();
const DROPS: bool = needs_drop::<String>();
static STATIC_SIZE: usize = size_of::<Option<Box<u8>>>();
static STATIC_DROPS: bool = needs_drop::<S>();
fn generic<T>() -> usize { size_of::<T>() }
fn pair<T, U>() -> (usize, usize) { (size_of::<T>(), size_of::<U>()) }
macro_rules! sizes { ($($t:ty),*) => { [$(size_of::<$t>()),*] } }
macro_rules! wrapped { ($t:ty) => { generic::<$t>() } }
fn main() {
    println!("{} {} {}", SIZE, S_SIZE, DROPS);
    println!("{} {}", STATIC_SIZE, STATIC_DROPS);
    println!("{} {} {}", generic::<u8>(), generic::<S>(), generic::<[u16; 3]>());
    println!("{:?}", pair::<u8, u64>());
    println!("{:?}", sizes!(u8, u16, S, (u8, u8)));
    println!("{}", wrapped!(Option<u32>));
    assert_eq!(size_of::<S>(), 8);
    assert_ne!(size_of::<S>(), 9);
    assert!(size_of::<u8>() < size_of::<u16>());
    let arr = [0u8; 8];
    assert_eq!(size_of::<[u8; 8]>(), arr.len());
    let total: usize = [size_of::<u8>(), size_of::<u16>(), size_of::<u32>()].iter().sum();
    println!("{}", total);
    let mut b = 1usize;
    while b < size_of::<usize>() { b <<= 1; }
    println!("{}", b);
    let n = size_of::<u32>() * 2 + size_of::<S>();
    println!("{}", n);
}
"##,
        r##"8 8 true
8 false
1 8 6
(1, 8)
[1, 2, 8, 2]
8
7
8
16
"##,
    );
}

/// A transmute size mismatch names the actual source and destination layouts.
#[test]
fn transmute_size_mismatch_names_its_types() {
    use super::format_trait_facade_tests::run;
    let refused = |source: &str, expected: &str| {
        for edition in [2021, 2024] {
            match run("mem-refused", source, edition) {
                Err(error) => assert!(
                    error.message.contains(expected),
                    "{edition}: {} does not contain {expected}",
                    error.message
                ),
                Ok(_) => panic!("{edition}: expected a refusal containing {expected}"),
            }
        }
    };
    refused(
        "fn main() { let w: u64 = unsafe { std::mem::transmute::<u32, u64>(1u32) }; println!(\"{}\", w); }\n",
        "different sizes: u32 is 4 bytes, u64 is 8 bytes",
    );
}

#[test]
fn upstream_nominal_transmute_uses_ordinary_const_vm_all_origins() {
    use crate::cli::StandaloneOutcome;
    use super::format_trait_facade_tests::run;
    let original = include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/consts/transmute-const.rs");
    let emitted = format!("macro_rules! emit {{ ($($t:tt)*) => {{$($t)*}} }} emit!{{{original}}}");
    let wrapped = format!("macro_rules! emit {{ ($($t:tt)*) => {{$($t)*}} }} macro_rules! wrap {{ ($($t:tt)*) => {{emit!{{$($t)*}}}} }} wrap!{{{original}}}");
    let nested = original.replace("assert_eq!(TRANSMUTED_U32, 3);", "assert_eq!(unsafe { mem::transmute::<Foo, u32>(Foo(3)) }, TRANSMUTED_U32);");
    assert_ne!(nested, original);
    for edition in [2015, 2018, 2021, 2024] {
        for (origin, text) in [original, &emitted, &wrapped, &nested].into_iter().enumerate() {
            let report = match run("upstream-nominal-image", text, edition) {
                Ok(StandaloneOutcome::Completed(report)) => report,
                Err(error) => panic!("edition={edition} origin={origin}: {error:?}"),
                Ok(_) => panic!("edition={edition} origin={origin}: not completed"),
            };
            assert!(report.output.is_empty());
            assert!(report.opcode_counters.started_by_opcode[22] > 0);
            assert_eq!(report.opcode_counters.started_by_opcode[22], report.opcode_counters.started_by_opcode[19]);
            eprintln!("nominal image edition={edition} origin={origin}: ordinary const VM and calls, empty streams");
        }
    }
}

#[test]
fn transmute_consumes_source_without_drop_and_owns_destination_drop() {
    prints("nominal-image-ownership", r#"
struct Source(u32);
impl Drop for Source { fn drop(&mut self) { println!("SOURCE-DROP"); } }
struct Destination(u32);
impl Drop for Destination { fn drop(&mut self) { println!("destination {}", self.0); } }
fn make() -> Source { println!("make"); Source(7) }
fn main() {
    let value: Destination = unsafe { std::mem::transmute(make()) };
    println!("value {}", value.0);
    { let zero: Destination = unsafe { std::mem::zeroed() }; assert_eq!(zero.0, 0); }
}
"#, "make\nvalue 7\ndestination 0\ndestination 7\n");
}

#[test]
fn upstream_union_memory_uses_shared_views_all_editions_and_origins() {
    use crate::cli::StandaloneOutcome;
    use super::format_trait_facade_tests::run;
    let original=include_str!("../../../../fixtures/upstream/rustc-checkout/tests/ui/union/union-transmute.rs");
    let emitted=format!("macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} emit!{{{original}}}");
    let wrapped=format!("macro_rules! emit {{($($t:tt)*)=>{{$($t)*}}}} macro_rules! wrap {{($($t:tt)*)=>{{emit!{{$($t)*}}}}}} wrap!{{{original}}}");
    let nested=original.replace("u.b = (2 << 8) + 2;", "assert!({ u.b = (2 << 8) + 2; true });");
    let aliased=original.replace("u.b = (2 << 8) + 2;", "let retained = &u.a.0 as *const u8; u.b = (2 << 8) + 2; assert_eq!(*retained, 2u8);");
    let patterned=original.replace("assert_eq!(u.a, (2, 2));", "let U { a: (left, right) } = u; assert_eq!(left, 2); assert_eq!(right, 2);");
    assert_ne!(nested,original);
    assert_ne!(aliased,original);
    assert_ne!(patterned,original);
    for edition in [2015,2018,2021,2024] {
        for(origin,text)in [original,&emitted,&wrapped,&nested,&aliased,&patterned].into_iter().enumerate(){
            let report=match run("upstream-union-memory",text,edition){
                Ok(StandaloneOutcome::Completed(report))=>report,
                Err(error)=>panic!("edition={edition} origin={origin}: {error:?}"),
                Ok(_)=>panic!("edition={edition} origin={origin}: not complete"),
            };
            assert!(report.output.is_empty());
            assert!(report.opcode_counters.started_by_opcode[29]>=3);
            eprintln!("union memory edition={edition} origin={origin}: ordinary construction finishes={}",report.opcode_counters.started_by_opcode[29]);
        }
    }
}
