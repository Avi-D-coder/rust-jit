//! Trait-path and generic associated calls through the ordinary standalone
//! pipeline with the LRR standard facade: `Trait::f(x)`, `<T as Trait>::f`,
//! `<T>::f`, `T::f` through a generic parameter and a trait body's `Self::f`
//! select their impl from the call's self type (an argument written as
//! `Self`, an anchor type, a parameter's binding or the expected result)
//! through the member index, then run as ordinary source calls; associated
//! functions of generic impls bind the impl's parameters from their
//! arguments or the expected result. Programs print what they proved.
use super::operator_dispatch_tests::prints;

#[test]
fn trait_paths_select_the_impl_of_their_self_type() {
    prints(
        "trait-paths",
        r#"
trait Shape {
    fn area(&self) -> i32;
    fn twice(&self) -> i32 { Self::area(self) * 2 }
    fn make(v: i32) -> Self;
}
struct Sq(i32);
struct Rect(i32, i32);
impl Shape for Sq {
    fn area(&self) -> i32 { self.0 * self.0 }
    fn make(v: i32) -> Self { Sq(v) }
}
impl Shape for Rect {
    fn area(&self) -> i32 { self.0 * self.1 }
    fn make(v: i32) -> Self { Rect(v, 1) }
}
fn total<T: Shape>(x: &T) -> i32 { T::area(x) + <T as Shape>::area(x) }
fn build<T: Shape>(v: i32) -> T { T::make(v) }
fn main() {
    let s = Sq(3);
    let r = Rect(2, 5);
    println!("{} {} {}", Shape::area(&s), Shape::area(&r), <Sq as Shape>::area(&s));
    println!("{} {}", Sq::area(&s), <Rect>::area(&r));
    println!("{} {}", total(&s), total(&r));
    println!("{} {}", s.twice(), r.twice());
    let t: Sq = build(4);
    let u: Rect = Shape::make(7);
    println!("{} {}", t.area(), Shape::area(&u));
}
"#,
        "9 10 9\n9 10\n18 20\n18 20\n16 7\n",
    );
}

#[test]
fn trait_path_arguments_after_self_take_the_selected_impls_hints() {
    // `Walk::step(start, 1)`: the self argument selects the impl; the later
    // literal takes the impl's `u8` parameter. The provided `twice` body
    // calls through the trait path again with its own `Self` bound.
    prints(
        "trait-path-hints",
        r#"
trait Walk: Sized {
    fn step(start: Self, by: u8) -> Self;
    fn twice(start: Self, by: u8) -> Self { Walk::step(Walk::step(start, by), by) }
}
impl Walk for i64 {
    fn step(start: i64, by: u8) -> i64 { start + by as i64 }
}
fn main() {
    let a = Walk::step(40i64, 1);
    let b = Walk::twice(a, 3);
    println!("{} {}", a, b);
}
"#,
        "41 47\n",
    );
}

#[test]
fn facade_range_iteration_steps_through_the_step_trait() {
    prints(
        "range-step",
        r#"
fn main() {
    let mut total = 0i32;
    for i in 0i32..5i32 { total += i; }
    println!("{}", total);
}
"#,
        "10\n",
    );
}

#[test]
fn generic_impl_associated_functions_bind_impl_parameters() {
    prints(
        "generic-assoc",
        r#"
struct W<T> { v: T }
impl<T> W<T> {
    fn new(v: T) -> Self { W { v } }
    fn get(self) -> T { self.v }
    fn empty() -> Option<T> { None }
}
struct S { v: Option<u16> }
fn main() {
    let a = W::new(5i32);
    let b: W<u8> = W::new(7u8);
    // No argument binds `T`: the expected result does.
    let e: Option<u16> = W::empty();
    let s = S { v: W::empty() };
    println!("{} {} {} {}", a.get(), b.get(), e.is_none(), s.v.is_none());
}
"#,
        "5 7 true true\n",
    );
}

#[test]
fn comparison_temporaries_with_unconstrained_literals_take_the_fallback() {
    prints(
        "literal-temporaries",
        r#"
fn main() {
    assert_eq!((1, 'a'), (1, 'a'));
    assert_ne!((1, 2.5), (1, 3.5));
    println!("ok");
}
"#,
        "ok\n",
    );
}

#[test]
fn const_generic_array_impls_bind_their_length_from_the_reached_array() {
    // `impl<T, const N: usize> Same for [T; N]`: `&Self` is `&[T; N]`; the
    // reached `[u8; 2]` binds `T` and `N` for the method and trait path.
    prints(
        "const-array-impl",
        r#"
trait Same { fn same(&self, other: &Self) -> bool; }
impl<T: PartialEq, const N: usize> Same for [T; N] {
    fn same(&self, other: &Self) -> bool {
        let mut i = 0;
        while i < self.len() {
            if self[i] != other[i] { return false; }
            i += 1;
        }
        true
    }
}
fn main() {
    let a = [1u8, 2];
    let b = [1u8, 2];
    let c = [1u8, 3];
    println!("{} {}", a.same(&b), Same::same(&a, &c));
}
"#,
        "true false\n",
    );
}
