//! Overloaded operators through the ordinary standalone pipeline with the
//! LRR standard facade: `==`/`!=`/`<`.. on non-primitive operands select
//! `PartialEq`/`PartialOrd` (autoref'd operands, never moved), `+`/`-`
//! select `Add`/`Sub` (by value), `-x`/`!x` select `Neg`/`Not` and `x += y`
//! selects `AddAssign` (`&mut x`), each through the member index and an
//! ordinary source call. Programs print what they proved.
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
            "lrr-operators-{label}-{}-{}",
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

pub(super) fn run(
    label: &str,
    source: &str,
) -> Result<StandaloneOutcome, crate::core::diagnostic::Diagnostic> {
    let temp = Temp::new(label);
    let root = temp.0.join("main.rs");
    std::fs::write(&root, source).unwrap();
    let profile = ProfileIdentity {
        edition: 2021,
        ..ProfileIdentity::current()
    };
    let (prepared, view, sources, mut snapshot) = capture_with(
        &root,
        false,
        2021,
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
    match run(label, source) {
        Ok(StandaloneOutcome::Completed(report)) => {
            assert_eq!(report.output.stdout, expected, "{source}")
        }
        Ok(StandaloneOutcome::Panicked(report)) => panic!("panicked: {:?}\n{source}", report.panic),
        Ok(_) => panic!("unexpected outcome\n{source}"),
        Err(error) => panic!("{error}\n{source}"),
    }
}

#[test]
fn partial_eq_impl_compares_autoref_operands_without_moving_them() {
    // `a` and `b` are compared four times: `==` and `!=` (the trait's
    // provided `ne`) borrow both operands; neither is moved.
    prints(
        "eq",
        "struct P { x: i32, y: i32 }
impl PartialEq for P {
    fn eq(&self, other: &P) -> bool { self.x == other.x && self.y == other.y }
}
fn main() {
    let a = P { x: 1, y: 2 };
    let b = P { x: 1, y: 2 };
    let c = P { x: 3, y: 2 };
    if a == b { println!(\"eq\"); }
    if a != c { println!(\"ne\"); }
    if !(a == c) { println!(\"not eq\"); }
    if b == a { println!(\"again\"); }
    let _keep = (a, b, c);
}
",
        "eq\nne\nnot eq\nagain\n",
    );
}

#[test]
fn partial_eq_dispatch_matches_through_macro_operands_and_enum_impls() {
    // The same operator inside `assert!` (a standard macro expansion), a
    // user macro wrapper and an enum impl whose body matches its operands.
    prints(
        "eq-macro",
        "enum E { A(i32), B }
impl PartialEq for E {
    fn eq(&self, other: &E) -> bool {
        match (self, other) {
            (E::A(a), E::A(b)) => *a == *b,
            (E::B, E::B) => true,
            _ => false,
        }
    }
}
macro_rules! same { ($l:expr, $r:expr) => { $l == $r }; }
fn main() {
    let a = E::A(1);
    let b = E::A(1);
    assert!(a == b);
    assert!(a != E::B);
    if same!(a, b) { println!(\"wrapped\"); }
    if !same!(E::B, a) { println!(\"wrapped ne\"); }
}
",
        "wrapped\nwrapped ne\n",
    );
}

#[test]
fn partial_ord_provided_comparisons_use_partial_cmp() {
    prints(
        "ord",
        "use std::cmp::Ordering;
struct V(i32);
impl PartialEq for V { fn eq(&self, o: &V) -> bool { self.0 == o.0 } }
impl PartialOrd for V {
    fn partial_cmp(&self, other: &V) -> Option<Ordering> {
        if self.0 < other.0 { Some(Ordering::Less) }
        else if self.0 > other.0 { Some(Ordering::Greater) }
        else { Some(Ordering::Equal) }
    }
}
fn main() {
    let a = V(1);
    let b = V(2);
    if a < b { println!(\"lt\"); }
    if a <= b { println!(\"le\"); }
    if b > a { println!(\"gt\"); }
    if b >= a { println!(\"ge\"); }
    if !(a > b) { println!(\"not gt\"); }
}
",
        "lt\nle\ngt\nge\nnot gt\n",
    );
}

#[test]
fn arithmetic_unary_and_compound_operators_select_their_impls() {
    prints(
        "arith",
        "use std::ops::{Add, AddAssign, Neg, Not, Sub};
struct P { x: i32, y: i32 }
impl Clone for P { fn clone(&self) -> P { P { x: self.x, y: self.y } } }
impl Copy for P {}
impl Add for P {
    type Output = P;
    fn add(self, other: P) -> P { P { x: self.x + other.x, y: self.y + other.y } }
}
impl Sub for P {
    type Output = P;
    fn sub(self, other: P) -> P { P { x: self.x - other.x, y: self.y - other.y } }
}
impl Neg for P {
    type Output = P;
    fn neg(self) -> P { P { x: -self.x, y: -self.y } }
}
impl AddAssign for P {
    fn add_assign(&mut self, other: P) { self.x += other.x; self.y += other.y; }
}
struct B(bool);
impl Not for B {
    type Output = B;
    fn not(self) -> B { B(!self.0) }
}
fn main() {
    let p = P { x: 1, y: 2 };
    let q = P { x: 10, y: 20 };
    let r = p + q;
    if r.x == 11 && r.y == 22 { println!(\"add\"); }
    let s = r - p;
    if s.x == 10 { println!(\"sub\"); }
    let n = -p;
    if n.y == -2 { println!(\"neg\"); }
    let mut m = p;
    m += q;
    m += q;
    if m.x == 21 { println!(\"add_assign\"); }
    let b = !B(false);
    if b.0 { println!(\"not\"); }
}
",
        "add\nsub\nneg\nadd_assign\nnot\n",
    );
}

#[test]
fn facade_tuple_impl_compares_its_fields() {
    prints(
        "tuple",
        "fn main() {
    let t = (1i32, 'a');
    let u = (1i32, 'a');
    if t == u { println!(\"tuple\"); }
    assert_eq!(t, (1i32, 'a'));
    assert_ne!(t, (2i32, 'a'));
    if t == u { println!(\"still\"); }
}
",
        "tuple\nstill\n",
    );
}

#[test]
fn panicking_operator_method_unwinds_through_the_operator_frame() {
    let source = "struct P(i32);
impl PartialEq for P {
    fn eq(&self, _other: &P) -> bool { panic!(\"in eq\") }
}
fn main() {
    let a = P(1);
    let b = P(2);
    let _x = a == b;
}
";
    match run("panic", source) {
        Ok(StandaloneOutcome::Panicked(report)) => {
            assert_eq!(report.panic.message.as_deref(), Some("in eq"))
        }
        Ok(_) => panic!("operator method panic did not unwind"),
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn reference_comparisons_select_cores_reference_impl() {
    // `&A == &B` is core's `impl PartialEq<&B> for &A`, which compares the
    // referents; it is never the builtin primitive comparison.
    prints(
        "refs",
        "struct P(i32);
impl PartialEq for P { fn eq(&self, o: &P) -> bool { self.0 == o.0 } }
fn same<T: PartialEq>(a: &T, b: &T) -> bool { a == b }
fn main() {
    let x = 4i32;
    let y = 4i32;
    if &x == &y { println!(\"prim\"); }
    if same(&P(1), &P(1)) { println!(\"generic\"); }
    let r = &P(2);
    if r == &P(2) { println!(\"nominal\"); }
    // `!=` is the reference impl's provided `ne`, whose `self.eq(other)`
    // (`self: &&P`) selects the `&A` impl at that autoderef step.
    if r != &P(3) && !(r != &P(2)) { println!(\"provided\"); }
}
",
        "prim\ngeneric\nnominal\nprovided\n",
    );
}

#[test]
fn str_and_array_comparisons_forward_through_core_impls() {
    // `&str == &str` is core's `&A: PartialEq<&B>`, which calls
    // `PartialEq::eq(*self, *other)` for `str`; `[T; N] == [U; N]` is the
    // array impl, whose `N` the reached arrays bind.
    prints(
        "str-array",
        r#"
fn main() {
    let s = "xy";
    assert!("ab" == "ab");
    assert!("ab" != "ac");
    assert_eq!(s, "xy");
    let a = [1u8, 2];
    let b = [1u8, 2];
    println!("{} {}", a == b, a != [1u8, 3u8]);
}
"#,
        "true true\n",
    );
}

#[test]
fn operator_impls_are_selected_by_the_right_operand() {
    // Several impls of one operator trait for one self type differ only by
    // the trait's `Rhs` argument: `PartialEq<i32>`/`PartialEq<M>` and
    // `Add<i32>`/`Add<M>`. The right operand's reached type (or an
    // unresolved integer literal) selects among them, for comparisons and
    // by-value arithmetic alike.
    prints(
        "rhs",
        "use std::ops::Add;
struct M(i32);
impl PartialEq<i32> for M { fn eq(&self, o: &i32) -> bool { self.0 == *o } }
impl PartialEq<M> for M { fn eq(&self, o: &M) -> bool { self.0 == o.0 } }
impl Add<i32> for M { type Output = i32; fn add(self, o: i32) -> i32 { self.0 + o } }
impl Add<M> for M { type Output = i32; fn add(self, o: M) -> i32 { self.0 + o.0 * 2 } }
fn main() {
    println!(\"{} {} {} {}\", M(3) == 3, M(3) == M(3), M(3) == 4, M(3) == M(3));
    println!(\"{} {} {}\", M(1) + 2, M(1) + M(2), M(4) + 1);
}
",
        "true true false true\n3 5 5\n",
    );
}

#[test]
fn provided_ne_selects_the_impl_by_the_trait_arguments() {
    // `ne` is not defined by either impl: both share `PartialEq::ne`, whose
    // `!self.eq(other)` selects `PartialEq<i32>` or `PartialEq<M>` from the
    // reached type of `other`, not from the method's signature.
    prints(
        "provided-ne",
        "struct M(i32);
impl PartialEq<i32> for M { fn eq(&self, o: &i32) -> bool { self.0 == *o } }
impl PartialEq<M> for M { fn eq(&self, o: &M) -> bool { self.0 == o.0 } }
fn main() {
    let a = M(3);
    println!(\"{} {} {} {}\", a != M(4), a != M(3), a != 3, a != 4);
    println!(\"{} {}\", a.ne(&M(4)), a.ne(&3));
}
",
        "true false false true\ntrue false\n",
    );
}

#[test]
fn unresolved_literal_right_operand_takes_the_selected_impls_parameter_type() {
    // The only `PartialEq` impl compares with `u8`: the literal `3` is
    // that `u8`, not the integer fallback.
    prints(
        "literal-rhs",
        "struct M(i32);
impl PartialEq<u8> for M { fn eq(&self, o: &u8) -> bool { self.0 == *o as i32 } }
fn main() {
    let a = M(3);
    println!(\"{} {}\", a == 3, a == 4);
}
",
        "true false\n",
    );
}

#[test]
fn operands_bound_by_patterns_from_references_compare_through_the_impl() {
    // `assert_eq!` expands to `match (&left, &right) { (l, r) => *l == *r }`:
    // the pattern bindings hold references with no retained source view, so
    // the operator's autoref'd operands state their own reached reference
    // Types. A derived `PartialEq` and a manual one select the same way.
    prints(
        "pattern-refs",
        "#[derive(PartialEq, Debug)]
struct P { x: i32 }
#[derive(PartialEq, Debug)]
enum E { A(i32), B }
struct Q(i32);
impl PartialEq for Q { fn eq(&self, o: &Q) -> bool { self.0 == o.0 } }
fn main() {
    let a = P { x: 1 };
    let b = P { x: 1 };
    assert_eq!(a, b);
    assert_ne!(E::A(1), E::B);
    let (q1, q2) = (Q(3), Q(3));
    let t = (&q1, &q2);
    let (l, r) = t;
    println!(\"{} {}\", *l == *r, *l != *r);
    match (&a, &b) { (l, r) => println!(\"{}\", *l == *r) }
}
",
        "true false\ntrue\n",
    );
}

#[test]
fn qualified_operator_paths_select_among_impls_by_the_trait_arguments() {
    // `i32` implements `Mul<i32>` and `Mul<&i32>`: `<i32 as Mul>::mul`,
    // `<i32 as Mul<&i32>>::mul` (explicit trait arguments, also through a
    // macro) and the method form select by the call's arguments; a literal
    // argument takes the selected impl's parameter Type.
    prints(
        "qualified-ops",
        "use std::ops::{Add, Mul};
macro_rules! tmul { ($t:ty, $a:expr, $b:expr) => { <$t as Mul<$t>>::mul($a, $b) } }
fn gen<T: Mul<T, Output = T>>(a: T, b: T) -> T { <T as Mul<T>>::mul(a, b) }
fn main() {
    let a: i32 = 6;
    let b: i32 = 7;
    let c: i64 = 6;
    println!(\"{}\", <i32 as Mul>::mul(a, b));
    println!(\"{}\", <i32 as Mul<i32>>::mul(a, b));
    println!(\"{}\", <i32 as Mul<&i32>>::mul(a, &b));
    println!(\"{}\", <i64 as Mul>::mul(c, 7));
    println!(\"{}\", tmul!(i64, 6, 7));
    println!(\"{}\", c.add(7));
    println!(\"{}\", gen(1.5f64, 2.0));
}
",
        "42\n42\n42\n42\n42\n13\n3\n",
    );
}

#[test]
fn explicit_trait_arguments_fix_an_unresolved_literal_argument() {
    prints(
        "qualified-explicit",
        "trait Conv<T> { fn conv(self, t: T) -> u64; }
impl Conv<u8> for u32 { fn conv(self, t: u8) -> u64 { 1000 + t as u64 } }
impl Conv<u16> for u32 { fn conv(self, t: u16) -> u64 { 2000 + t as u64 } }
fn main() {
    println!(\"{} {}\", <u32 as Conv<u16>>::conv(1, 5), <u32 as Conv<u8>>::conv(1, 5));
}
",
        "2005 1005\n",
    );
}

#[test]
fn operators_on_references_to_a_rigid_referent_dispatch_the_bound() {
    // `&'a T: Add<&'a T, Output = T>`: the operands' views carry the
    // instantiated referent, the result is statically the rigid `T`.
    prints(
        "rigid-ref-ops",
        "use std::ops::{Add, Neg};
fn addr<'a, T>(a: &'a T, b: &'a T) -> T where &'a T: Add<&'a T, Output = T> { a + b }
fn negr<T>(a: &T) -> T where for<'a> &'a T: Neg<Output = T> { -a }
fn main() {
    println!(\"{} {} {}\", addr(&3i32, &4i32), addr(&1.5f64, &2.0f64), negr(&5i64));
}
",
        "7 3.5 -5\n",
    );
}

#[test]
fn unit_receivers_select_the_empty_tuple_impls() {
    prints(
        "unit-receiver",
        "use std::cmp::Ordering;
fn main() {
    let u = ().clone();
    println!(\"{:?} {:?} {} {} {}\", ().partial_cmp(&()), ().cmp(&()), ().eq(&()), u == (), ().ne(&()));
    assert_eq!(().partial_cmp(&()), Some(Ordering::Equal));
}
",
        "Some(Equal) Equal true true false\n",
    );
}
