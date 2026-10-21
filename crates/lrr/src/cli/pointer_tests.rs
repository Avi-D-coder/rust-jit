//! Raw pointers and function pointers (E-R5-008). Expected output is real
//! rustc 1.99.0's. Each program runs in all four editions. The same casts reach
//! the interpreter hand-written, emitted by user macros and nested in
//! `assert_eq!`/`println!`; there is one cast and one coercion route.
use super::format_trait_facade_tests::prints;

/// Raw pointers (E-R5-008): `&T as *const T`, a typed `let`, `&mut T as *mut T` writes, `p as usize` and `usize as *const T`, null, `==`, and the sign-extended integer cast `!0usize as *const ()`.
#[test]
fn raw_pointers_cast_dereference_compare_and_round_trip() {
    prints(
        "raw-basics",
        "fn main() {
    let x = 5i32;
    let p = &x as *const i32;
    let q: *const i32 = &x;
    println!(\"{} {}\", unsafe { *p }, p == q);
    let mut y = 1i32;
    let m = &mut y as *mut i32;
    unsafe { *m = 7; *m += 1; }
    println!(\"{}\", y);
    let a = p as usize;
    let r = a as *const i32;
    println!(\"{} {}\", unsafe { *r }, a != 0);
    println!(\"{} {}\", p.is_null(), std::ptr::null::<i32>().is_null());
    let n: *const i32 = std::ptr::null();
    println!(\"{} {}\", n.is_null(), n == p);
    println!(\"{}\", !0usize as *const () == (0i8 - 1) as *const ());
}
",
        "5 true\n8\n5 true\nfalse true\ntrue false\ntrue\n",
    );
}

/// `a.as_ptr()`, `add`, `offset`, `sub`, `offset_from` over array element cells; a write through `as_mut_ptr`.
#[test]
fn pointer_arithmetic_walks_array_elements() {
    prints(
        "raw-arith",
        "fn main() {
    let a = [10i32, 20, 30, 40];
    let p = a.as_ptr();
    unsafe {
        println!(\"{}\", *p);
        println!(\"{}\", *p.add(1));
        println!(\"{}\", *p.offset(2));
        let e = p.add(3);
        println!(\"{}\", *e);
        println!(\"{}\", e.offset_from(p));
        println!(\"{}\", p.add(4).sub(1) == e);
    }
    let mut b = [1u8, 2, 3];
    let m = b.as_mut_ptr();
    unsafe { *m.add(1) = 9; }
    println!(\"{:?}\", b);
}
",
        "10\n20\n30\n40\n3\ntrue\n[1, 9, 3]\n",
    );
}

/// Function items coerce to `fn` pointers (struct field, parameter, `as`, typed `let`); fn pointers compare by code address and call through the ordinary call machinery.
#[test]
fn function_pointers_coerce_compare_and_call() {
    prints(
        "fn-pointers",
        "struct S { f: fn(i32) -> i32 }
fn dbl(x: i32) -> i32 { x * 2 }
fn inc(x: i32) -> i32 { x + 1 }
fn apply(f: fn(i32) -> i32, v: i32) -> i32 { f(v) }
fn main() {
    let s = S { f: dbl };
    println!(\"{}\", (s.f)(4));
    println!(\"{}\", apply(inc, 4));
    let g = dbl as fn(i32) -> i32;
    println!(\"{}\", g == (dbl as fn(i32) -> i32));
    println!(\"{}\", g == (inc as fn(i32) -> i32));
    let h: fn(i32) -> i32 = inc;
    println!(\"{}\", h(1));
}
",
        "8\n5\ntrue\nfalse\n2\n",
    );
}

/// `ptr::read`/`write`/`eq`/`null_mut` over pointers to struct fields.
#[test]
fn ptr_read_write_and_eq_through_field_pointers() {
    prints(
        "ptr-fns",
        "use std::ptr;
struct P { a: i32, b: i32 }
fn main() {
    let mut s = P { a: 1, b: 2 };
    let pb = &mut s.b as *mut i32;
    unsafe { *pb = 20; ptr::write(pb, 21); }
    println!(\"{} {}\", s.a, s.b);
    let pa = &s.a as *const i32;
    println!(\"{}\", unsafe { ptr::read(pa) });
    println!(\"{}\", ptr::eq(pa, &s.a));
    let m: *mut i32 = ptr::null_mut();
    println!(\"{}\", m.is_null());
}
",
        "1 21\n1\ntrue\ntrue\n",
    );
}

/// The same casts and calls emitted by user `macro_rules!` and nested in `assert_eq!`/`println!`.
#[test]
fn pointer_casts_in_user_macros_and_assert_arguments() {
    prints(
        "ptr-macros",
        "macro_rules! as_ptr { ($e:expr) => { $e as *const i32 } }
macro_rules! deref { ($p:expr) => { unsafe { *$p } } }
macro_rules! callf { ($f:expr, $v:expr) => { ($f as fn(i32) -> i32)($v) } }
macro_rules! make_ptr_fn {
    ($name:ident, $target:ident) => {
        fn $name() -> fn(i32) -> i32 { $target }
    };
}
fn inc(x: i32) -> i32 { x + 1 }
make_ptr_fn!(get_inc, inc);
fn main() {
    let x = 41i32;
    let p = as_ptr!(&x);
    println!(\"{}\", deref!(p));
    assert_eq!(unsafe { *(&x as *const i32) }, 41);
    assert!(p == &x as *const i32);
    assert_eq!(deref!(as_ptr!(&x)), 41);
    let a = callf!(inc, 1);
    println!(\"{} {}\", a, (inc as fn(i32) -> i32)(callf!(inc, 2)));
    let f = get_inc();
    println!(\"{}\", f(9));
    println!(\"{}\", f == (inc as fn(i32) -> i32));
}
",
        "41\n2 4\n10\ntrue\n",
    );
}

/// `x as i32`, `&x as &i32`, `A(1) as A` and `&x as *const _` take the cast-coercion route; an untyped literal local meets a typed pointer annotation.
#[test]
fn identity_and_reference_casts_and_inferred_pointee() {
    prints(
        "identity-casts",
        "struct A(u32);
impl Drop for A { fn drop(&mut self) { self.0 = 0; } }
fn main() {
    let mut a = 0i32;
    let b = &(a as i32);
    a = 1;
    println!(\"{} {}\", a, *b);
    let r = &a as &i32;
    println!(\"{}\", *r);
    let s = &(A(1) as A);
    println!(\"{}\", s.0);
    let x = 5;
    let p: *const i32 = &x;
    println!(\"{}\", unsafe { *p });
    let h = &x as *const _;
    println!(\"{}\", h == p);
}
",
        "1 0\n1\n1\n5\ntrue\n",
    );
}

/// `&raw const` / `&raw mut` and the library `ptr::addr_of!` / `addr_of_mut!` macros (which expand to them): the pointer designates the place's own cell, so fields of a `repr(packed)` struct, array elements, tuple fields and `(*p).field` places read and write through it, and no reference exists in between.
#[test]
fn raw_borrows_and_addr_of_macros_designate_places() {
    prints(
        "raw-borrow-places",
        "use std::ptr::{addr_of, addr_of_mut};
#[repr(packed)]
struct P { a: u8, b: u32 }
struct S { a: u32, t: (i64, i64), arr: [u8; 3] }
fn main() {
    let p = P { a: 1, b: 7 };
    let r = addr_of!(p.b);
    println!(\"{} {}\", unsafe { r.read_unaligned() }, p.a);
    let mut s = S { a: 1, t: (4, 5), arr: [1, 2, 3] };
    let pa = &raw mut s.a;
    let pt = &raw mut s.t.1;
    let pe = &raw const s.arr[1];
    unsafe { *pa += 10; *pt = 50; }
    println!(\"{} {} {}\", s.a, s.t.1, unsafe { *pe });
    let sp: *mut S = &raw mut s;
    let q = unsafe { addr_of_mut!((*sp).a) };
    unsafe { q.write(77); }
    println!(\"{} {}\", s.a, unsafe { addr_of!((*sp).t.0).read() });
    let x = 3u32;
    println!(\"{}\", &raw const x == &x as *const u32);
}
",
        "7 1\n11 50 2\n77 4\ntrue\n",
    );
}
