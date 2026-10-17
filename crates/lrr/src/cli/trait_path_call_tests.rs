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

#[test]
fn impls_differing_by_trait_arguments_are_selected_by_the_argument_types() {
    // `Scale<i32>`, `Scale<f64>` and `Scale<&str>` of one type provide
    // `scale`: the evaluated argument selects the impl (an unresolved
    // literal by its integer/float family), also inside the trait's provided
    // `twice`, whose `self.scale(by)` has only the bound `T`.
    prints(
        "trait-arg-impls",
        r#"
trait Scale<T> {
    fn scale(&self, by: T) -> i32;
    fn twice(&self, by: T) -> i32 { self.scale(by) * 2 }
}
struct S(i32);
impl Scale<i32> for S { fn scale(&self, by: i32) -> i32 { self.0 * by } }
impl Scale<f64> for S { fn scale(&self, by: f64) -> i32 { (self.0 as f64 * by) as i32 } }
impl Scale<&str> for S { fn scale(&self, by: &str) -> i32 { self.0 + by.len() as i32 } }
fn main() {
    let s = S(3);
    println!("{} {} {}", s.scale(2), s.scale(1.5), s.scale("abcd"));
    println!("{} {}", s.twice(2), s.twice(2.0f64));
    let x: i32 = 5;
    let y = 2.5f64;
    println!("{} {}", s.scale(x), s.scale(y));
}
"#,
        "6 4 7\n12 12\n15 7\n",
    );
}

#[test]
fn owner_type_arguments_bind_the_impl_parameters() {
    // `W::<u8>::n()` has no argument or expected result to fix `T`: the
    // Type argument written on the owner segment does, through the same
    // explicit-Type selection a call's own arguments use. The owner's
    // written structure (`P::<_, u64>` hole, `Cell2::<Cell2<u16>>` nested
    // against an impl self Type `Cell2<Cell2<T>>`) binds the impl's `T`
    // too; a caller's own parameter is its reached instantiation.
    prints(
        "owner-args",
        r#"
struct W<T> { v: T }
impl<T: Default> W<T> {
    fn unit() -> W<T> { W { v: T::default() } }
    fn get(&self) -> &T { &self.v }
}
trait Name { fn name() -> i32; }
impl Name for u8 { fn name() -> i32 { 8 } }
impl Name for u32 { fn name() -> i32 { 32 } }
impl<T: Name> W<T> { fn n() -> i32 { T::name() } }
struct P<A, B> { a: A, b: B }
impl<A: Default, B: Default> P<A, B> {
    fn make() -> P<A, B> { P { a: A::default(), b: B::default() } }
}
struct Cell2<T>(T);
impl<T: Default> Cell2<Cell2<T>> {
    fn deep() -> Cell2<Cell2<T>> { Cell2(Cell2(T::default())) }
}
fn through<T: Name>() -> i32 { W::<T>::n() }
fn main() {
    println!("{} {}", W::<u8>::n(), W::<u32>::n());
    println!("{} {}", through::<u8>(), through::<u32>());
    let w = W::<u64>::unit();
    println!("{}", *w.get());
    let p = P::<u8, i16>::make();
    println!("{} {}", p.a, p.b);
    let q: P<i8, u64> = P::<_, u64>::make();
    println!("{}", q.b);
    let d = Cell2::<Cell2<u16>>::deep();
    println!("{}", (d.0).0);
    let b = Box::<u8>::new(7);
    println!("{} {}", *b, Vec::<u8>::new().len());
}
"#,
        "8 32\n8 32\n0\n0 0\n0\n0\n7 0\n",
    );
}

#[test]
fn owner_type_arguments_in_macro_generated_bodies_use_the_declaring_context() {
    // A macro-generated function has no published body environment: the
    // owner argument resolves in the declaration's own signature context,
    // including the generated function's own parameter.
    prints(
        "owner-args-generated",
        r#"
struct W<T>(T);
impl<T: Default> W<T> {
    fn make() -> W<T> { W(T::default()) }
    fn get(&self) -> &T { &self.0 }
}
macro_rules! gen {
    ($name:ident, $t:ty) => {
        fn $name() -> u8 { let w = W::<$t>::make(); let _ = w.get(); 1 }
    };
}
gen!(g8, u8);
macro_rules! gen_generic {
    ($name:ident) => {
        fn $name<T: Default>() -> u8 { let w = W::<T>::make(); let _ = w; 6 }
    };
}
gen_generic!(gg);
#[derive(Debug)]
struct D { x: i32 }
fn main() {
    println!("{} {} {:?}", g8(), gg::<u16>(), D { x: 1 });
}
"#,
        "1 6 D { x: 1 }\n",
    );
}

#[test]
fn inclusive_ranges_are_the_range_inclusive_new_call() {
    // `a..=b` is `RangeInclusive::new(a, b)`: ordinary call lowering and
    // argument typing, so an unsuffixed bound takes its sibling bound's type
    // (`1..=3u8`, `0..=foo()`, `0_usize..=4 + 4 - 3`), exactly as it does
    // for any generic call, and the value iterates through the facade's
    // `Iterator for RangeInclusive`.
    prints(
        "range-inclusive",
        r#"
fn foo() -> isize { 42 }
fn main() {
    let mut total = 0;
    for i in 1..=3 { total += i; }
    for i in 0_usize..=2 { total += i * 10; }
    let r = 0..=foo();
    let s = 1..=3u8;
    let t = 0_usize..=4 + 4 - 3;
    println!("{} {} {} {}", total, r.end(), s.start(), t.end());
}
"#,
        "36 42 1 5\n",
    );
}
