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

#[test]
fn range_indexed_slices_share_the_array_element_type() {
    // `base[a..b]` is the slice of the base's elements: the literals of an
    // array only borrowed as a slice take a later use's Type or the i32/f64
    // fallback, through the one static constraint service.
    prints(
        "range-index",
        "fn take(s: &[u8]) -> u8 { s[1] }
fn main() {
    let a = [1, 2, 3];
    let s = &a[..];
    let t = &a[1..2];
    println!(\"{} {} {}\", s.len(), t[0], a[1..3][1]);
    let f = [1.5, 2.0];
    println!(\"{}\", (&f[..])[1]);
    let b = [7, 8, 9];
    println!(\"{}\", take(&b[1..]));
    let c = [1; 2];
    println!(\"{}\", (&c[..]).len());
}
",
        "3 2 3\n2\n9\n2\n",
    );
}

#[test]
fn lifetime_arguments_name_the_bare_nominal_inside_macro_arguments() {
    // `Ctx<'_>` has no Type argument, and a lifetime token inside a macro
    // call keeps the same identity when the call is replayed.
    prints(
        "lifetime-args",
        "use std::marker::PhantomData;
struct Ctx<'a> { x: u8, _m: PhantomData<&'a ()> }
fn size_ok<T>() -> bool { true }
fn main() {
    println!(\"{}\", size_ok::<Ctx<'_>>());
    println!(\"{}\", size_ok::<&Ctx<'static>>());
}
",
        "true\ntrue\n",
    );
}

#[test]
fn primitive_cast_results_carry_their_target_type_as_a_local_source() {
    // `5u64 as i64` has exactly the written target Type, so a local bound to
    // it can be borrowed and compared like an annotated one.
    prints(
        "cast-source",
        "fn main() {
    let x = 5u64 as i64;
    let y = 7u64 as i64;
    println!(\"{:?} {}\", x.cmp(&y), x == y);
    let c = 300i32 as u8;
    let d = c as char;
    let f = c as f64;
    println!(\"{} {} {}\", c, d, f);
    let r = &(x as u8);
    println!(\"{}\", *r + 1);
}
",
        "Less false\n44 , 44\n6\n",
    );
}

#[test]
fn macro_labels_are_hygienic_for_break_and_continue() {
    // A macro's own `'x` is not the caller's `'x`: the call-site `break 'x`
    // and `continue 'x` select the caller's enclosing loop.
    prints(
        "label-hygiene",
        "macro_rules! loop_x {
    ($e: expr) => {
        'x: loop {
            $e
        }
    };
}
fn main() {
    let mut n = 0;
    'x: for _i in 0..3 {
        n += 1;
        loop_x!(break 'x);
        n += 100;
    }
    'x: for _i in 0..3 {
        n += 1;
        loop_x!(continue 'x);
        #[allow(unreachable_code)]
        { n += 100; }
    }
    println!(\"{}\", n);
}
",
        "4\n",
    );
}

#[test]
fn overloaded_deref_in_a_scrutinee_follows_the_mutability_of_its_ref_bindings() {
    // `match *b { ref mut n => .. }` and `let ref mut z = *y` use `DerefMut`;
    // a `ref` binding uses `Deref`; any `ref mut` among the bindings of a
    // pattern selects `DerefMut`.
    prints(
        "deref-ref-patterns",
        "use std::ops::{Deref, DerefMut};
struct DerefOk<T>(T);
struct DerefMutOk<T>(T);
impl<T> Deref for DerefOk<T> {
    type Target = T;
    fn deref(&self) -> &T { &self.0 }
}
impl<T> DerefMut for DerefOk<T> {
    fn deref_mut(&mut self) -> &mut T { panic!(\"wrong: DerefMut\") }
}
impl<T> Deref for DerefMutOk<T> {
    type Target = T;
    fn deref(&self) -> &T { panic!(\"wrong: Deref\") }
}
impl<T> DerefMut for DerefMutOk<T> {
    fn deref_mut(&mut self) -> &mut T { &mut self.0 }
}
fn main() {
    let mut b = DerefMutOk(0i32);
    match *b { ref mut n => { *n += 1i32; } }
    let mut y = DerefMutOk(1i32);
    let ref mut z = *y;
    *z += 1i32;
    let b = DerefOk(2i32);
    match *b { ref n => { assert_eq!(*n, 2i32); } }
    let y = DerefOk(3i32);
    let ref z = *y;
    assert_eq!(*z, 3i32);
    let mut m = DerefMutOk((0i32, 9i32));
    match *m { (ref a, ref mut c) => { *c += *a; } }
    println!(\"ok {} {}\", b.0, y.0);
}
",
        "ok 2 3\n",
    );
}

#[test]
fn type_relative_calls_select_among_impls_of_one_trait_by_argument_type() {
    // `W::conv(x)` with `Conv<i32>`, `Conv<bool>`, `Conv<&str>` for `W`:
    // the evaluated argument's reached Type selects the impl.
    prints(
        "type-relative-impls",
        "struct W(i32);
trait Conv<T> { fn conv(x: T) -> Self; }
impl Conv<i32> for W { fn conv(x: i32) -> W { W(x) } }
impl Conv<bool> for W { fn conv(x: bool) -> W { W(if x { 100 } else { 0 }) } }
impl Conv<&str> for W { fn conv(x: &str) -> W { W(x.len() as i32) } }
fn main() {
    let a = W::conv(5i32);
    let b = W::conv(true);
    let c = W::conv(\"abc\");
    let d = W::conv(7);
    println!(\"{} {} {} {}\", a.0, b.0, c.0, d.0);
}
",
        "5 100 3 7\n",
    );
}

#[test]
fn numeric_from_try_from_and_into_select_the_impl_by_argument_type() {
    // The facade's lossless `From` and range-checked `TryFrom` matrices
    // between the integer types, `bool`/`char` and floats, through
    // `u64::from(x)`, `T::try_from(x)` and the blanket `Into`.
    prints(
        "numeric-conversions",
        "fn main() {
    let a = u64::from(5u32);
    let b = u16::from(200u8);
    let c = i64::from(-3i32);
    let d = u8::try_from(300i32).unwrap_or(7);
    let e = i8::try_from(100u8).unwrap();
    let f = u32::try_from(-1i64).is_err();
    let g = f64::from(3u8);
    let h = char::from(65u8);
    let i = usize::try_from(5u64).unwrap();
    let j: i64 = 5i32.into();
    let k: u16 = 7u8.into();
    let m = u64::from(9u64);
    let n = u32::from(true);
    let o = i16::try_from(-40000i32).is_err();
    let p = u8::try_from(255u32).unwrap();
    let q = u32::from('A');
    let r = i32::try_from(7usize).unwrap() + i32::from(1i8);
    println!(\"{} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}\", a, b, c, d, e, f, g, h, i, j, k, m, n, o, p, q, r);
}
",
        "5 200 -3 7 100 true 3 A 5 5 7 9 1 true 255 65 8\n",
    );
}
