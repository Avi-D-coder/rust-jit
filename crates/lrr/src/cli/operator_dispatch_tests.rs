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
