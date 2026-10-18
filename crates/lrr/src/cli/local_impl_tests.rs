//! `impl` blocks and `trait` declarations written inside function bodies
//! (E-R5-009): a block-local impl is published into the run's member index
//! when its owning body is entered, and applies to every later method and
//! associated-function selection. Each shape runs handwritten, emitted by a
//! user macro and emitted through a wrapper macro, in all four editions.
//! Expected output is the program's output under real rustc (`rustc +1.99.0`).
use super::format_trait_facade_tests::prints;

/// The same body written by hand, emitted by a user macro and emitted through
/// a wrapper macro must take the one route and agree.
fn all_forms(label: &str, items: &str, rest: &str, expected: &str) {
    prints(
        &format!("{label}-handwritten"),
        &format!("fn main() {{\n{items}\n{rest}\n}}\n"),
        expected,
    );
    prints(
        &format!("{label}-macro"),
        &format!(
            "macro_rules! decl {{ () => {{\n{items}\n}} }}\nfn main() {{\n    decl!();\n{rest}\n}}\n"
        ),
        expected,
    );
    prints(
        &format!("{label}-wrapped"),
        &format!(
            "macro_rules! decl {{ () => {{\n{items}\n}} }}\nmacro_rules! wrap {{ () => {{ decl!(); }} }}\nfn main() {{\n    wrap!();\n{rest}\n}}\n"
        ),
        expected,
    );
}

#[test]
fn block_local_inherent_impl_methods_and_associated_functions() {
    all_forms(
        "inherent",
        "    struct S(i32);
    impl S {
        fn get(&self) -> i32 { self.0 }
        fn new(v: i32) -> S { S(v) }
    }",
        "    println!(\"{} {}\", S(4).get(), S::new(9).get());",
        "4 9\n",
    );
}

#[test]
fn block_local_trait_with_provided_method_and_local_impl() {
    all_forms(
        "local-trait",
        "    trait Tr { fn f(&self) -> i32; fn g(&self) -> i32 { self.f() + 1 } }
    struct S;
    impl Tr for S { fn f(&self) -> i32 { 9 } }",
        "    println!(\"{} {}\", S.f(), S.g());",
        "9 10\n",
    );
}

#[test]
fn block_local_impl_of_std_iterator_and_operator_traits() {
    all_forms(
        "iter-add",
        "    struct Counter(u32);
    impl Iterator for Counter {
        type Item = u32;
        fn next(&mut self) -> Option<u32> {
            if self.0 < 3 { self.0 += 1; Some(self.0) } else { None }
        }
    }
    struct V(i32);
    impl std::ops::Add for V {
        type Output = V;
        fn add(self, o: V) -> V { V(self.0 + o.0) }
    }",
        "    let s: u32 = Counter(0).sum();
    println!(\"{}\", s);
    println!(\"{}\", (V(1) + V(2)).0);",
        "6\n3\n",
    );
}

/// Macro calls inside member bodies (`write!`, `println!`) run: the nested
/// bodies' calls attach with the enclosing body's own (E-R5-010,
/// `nested_body_tests::block_local_impl_members_with_macro_calls`, which
/// imports the std paths at module level). Known gap kept here: member
/// signatures that write a qualified path (`&mut std::fmt::Formatter`) stay
/// `NeedOwnerContext` (E-R5-009, header shapes capture single names only).
#[test]
#[ignore = "qualified path types in local member signatures (E-R5-009 gap 3)"]
fn block_local_display_default_and_drop_impls_with_macro_bodies() {
    all_forms(
        "display-drop",
        "    struct P(i32);
    impl std::fmt::Display for P {
        fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, \"P<{}>\", self.0)
        }
    }
    impl Default for P { fn default() -> P { P(77) } }
    struct D(i32);
    impl Drop for D { fn drop(&mut self) { println!(\"drop {}\", self.0); } }",
        "    let _d = D(5);
    println!(\"{} {}\", P(3), P::default());",
        "P<3> P<77>\ndrop 5\n",
    );
}

#[test]
fn block_local_impl_on_a_module_type() {
    prints(
        "module-type",
        "struct Foo(i32);
fn main() {
    impl Foo { fn extra(&self) -> i32 { self.0 * 2 } }
    println!(\"{}\", Foo(21).extra());
}
",
        "42\n",
    );
}

#[test]
fn two_bodies_each_declare_a_type_and_an_impl_of_the_same_name() {
    prints(
        "same-names",
        "fn one() -> i32 {
    struct S(i32);
    impl S { fn v(&self) -> i32 { self.0 + 1 } }
    S(1).v()
}
fn two() -> i32 {
    struct S(i32);
    impl S { fn v(&self) -> i32 { self.0 * 10 } }
    S(2).v()
}
fn main() { println!(\"{} {}\", one(), two()); }
",
        "2 20\n",
    );
}

/// Known gap: a block-level `mod` is a container whose members are not yet
/// collected (`modules/mod-inside-fn`): "lexical item callee requires body
/// namespace integration".
#[test]
#[ignore = "block-level mod items: lexical item callee requires body namespace integration"]
fn module_function_body_local_module_is_callable() {
    prints(
        "mod-inside-fn",
        "fn main() {
    mod m { pub fn g() -> i32 { 5 } }
    println!(\"{}\", m::g());
}
",
        "5\n",
    );
}
