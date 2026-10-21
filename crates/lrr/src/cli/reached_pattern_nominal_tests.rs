//! LRR route/exact-once control supplementing the unchanged rustc enum-field
//! original. It checks each nested goal and repeated generic activation,
//! rather than using the transaction root or source name as a Type cache key.
use super::StandaloneOutcome;

#[test]
fn nested_nominal_pattern_goals_keep_each_actual_application_and_scrutinee_once() {
    const SOURCE: &str = r#"
        trait Project { type Value; }
        impl Project for u8 { type Value = u32; }
        impl Project for u32 { type Value = u8; }
        struct W<T: Project> { marker: T, value: T::Value }
        fn make(calls: &mut u32) -> (W<u8>, W<u32>) {
            *calls += 1;
            (W::<u8> { marker: 1_u8, value: 33_u32 },
             W::<u32> { marker: 2_u32, value: 44_u8 })
        }
        fn take<T: Project>(value: W<T>) -> bool {
            match value { W { value: _, .. } => true }
        }
        macro_rules! consume {
            ($value:expr) => {
                match $value {
                    (W { value: a, .. }, W { value: b, .. }) =>
                        a == 33_u32 && b == 44_u8,
                }
            };
        }
        macro_rules! wrapper { ($value:expr) => { consume!($value) }; }
        fn main() {
            let mut calls = 0_u32;
            let ordinary = match make(&mut calls) {
                (W { value: a, .. }, W { value: b, .. }) =>
                    a == 33_u32 && b == 44_u8,
            };
            assert!(ordinary);
            assert!(consume!(make(&mut calls)));
            assert!(wrapper!(make(&mut calls)));
            assert!(calls == 3_u32);
            assert!(take(W::<u8> { marker: 3_u8, value: 55_u32 }));
            assert!(take(W::<u32> { marker: 4_u32, value: 66_u8 }));
        }
    "#;
    match super::crate_graph::format_trait_facade_tests::run(
        "reached-pattern-nominal",
        SOURCE,
        2021,
    ) {
        Ok(StandaloneOutcome::Completed(report)) => {
            assert!(report.opcode_counters.started_by_opcode[45] > 0);
        }
        Ok(_) => panic!("reached pattern nominal route did not complete"),
        Err(error) => panic!("reached pattern nominal route: {error:?}"),
    }
}

#[test]
fn generated_condition_uses_definition_site_binding_despite_caller_shadowing() {
    // Full coordinator preparation is necessary for retained local-name goals.
    // A bare RunMachine::poll may request that input before lexical lookup.
    const SOURCE: &str = "fn value()->i32{let x=2_i32;macro_rules! test {()=>{if let 1_i32..=3_i32=x{7_i32}else{99_i32}}}let x=8_i32;test!()}fn main(){assert_eq!(value(),7_i32);}";
    for edition in [2015, 2018, 2021, 2024] {
        match super::crate_graph::format_trait_facade_tests::run(
            "definition-site-shadowing",
            SOURCE,
            edition,
        ) {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.opcode_counters.started_by_opcode[19], 1);
                assert_eq!(report.opcode_counters.started_by_opcode[26], 0);
                assert!(report.opcode_counters.started_by_opcode[45] > 0);
                assert_eq!(
                    report.opcode_counters.operations_started,
                    report.opcode_counters.operations_completed
                );
            }
            Ok(_) => panic!("definition-site lookup did not complete in edition {edition}"),
            Err(error) => panic!("definition-site lookup in edition {edition}: {error:?}"),
        }
    }
}

#[test]
fn unchanged_upstream_hygiene_uses_the_full_coordinator() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/hygiene/hygiene.rs"
    ));
    let source = std::fs::read_to_string(path).unwrap();
    for edition in [2015, 2018, 2021, 2024] {
        match super::crate_graph::format_trait_facade_tests::run(
            "upstream-hygiene",
            &source,
            edition,
        ) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(_) => panic!("unchanged hygiene main did not complete in edition {edition}"),
            Err(error) => panic!("unchanged hygiene main in edition {edition}: {error:?}"),
        }
    }
}
#[test]
fn unchanged_upstream_nested_repetition_keeps_source_local_reads() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/array-slice-vec/repeated-vector-syntax.rs"
    ));
    let source = std::fs::read_to_string(path).unwrap();
    match super::crate_graph::format_trait_facade_tests::run("upstream-repetition", &source, 2015) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(_) => panic!("unchanged nested repetition did not complete"),
        Err(error) => panic!("unchanged nested repetition: {error:?}"),
    }
}

#[test]
fn qualified_pattern_paths_share_one_module_route_across_origins_and_editions() {
    // LRR route control: the same three-segment (and module-qualified
    // two-segment) pattern paths, handwritten, emitted by a user macro, through
    // a wrapper and nested in a standard macro, select their constructors
    // through the one qualified-path service that qualified expression paths use.
    const SOURCE: &str = r#"
        mod a {
            pub enum E { X(i32), Y, Z { v: i32 } }
            pub mod b { pub struct S(pub i32); pub struct R { pub q: u8 } pub const K: i32 = 7; }
        }
        macro_rules! first { ($e:expr) => { match $e { a::E::X(n) => n, _ => -1 } }; }
        macro_rules! wrapped { ($e:expr) => { first!($e) }; }
        fn main() {
            let x = a::E::X(3);
            match x { a::E::X(n) => assert_eq!(n, 3), a::E::Y | a::E::Z { .. } => panic!() }
            assert_eq!(first!(x), 3);
            assert_eq!(wrapped!(x), 3);
            assert!(matches!(x, a::E::X(3)));
            assert!(matches!(a::E::Z { v: 5 }, a::E::Z { v: 5 }));
            let a::b::S(k) = a::b::S(4);
            let a::b::R { q } = a::b::R { q: 9 };
            assert_eq!((k, q), (4, 9));
            match 7 { a::b::K => {} _ => panic!() }
            let o = Some(1u8);
            match o { std::option::Option::Some(n) => assert_eq!(n, 1), _ => panic!() }
            let r: Result<(), u8> = Ok(());
            if let std::result::Result::Ok(()) = r {} else { panic!() }
        }
    "#;
    for edition in [2015, 2018, 2021, 2024] {
        match super::crate_graph::format_trait_facade_tests::run(
            "qualified-pattern-paths",
            SOURCE,
            edition,
        ) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(_) => panic!("qualified pattern paths did not complete in edition {edition}"),
            Err(error) => panic!("qualified pattern paths in edition {edition}: {error:?}"),
        }
    }
}

#[test]
fn unchanged_upstream_crate_qualified_patterns_select_their_variants() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/pattern/issue-6449.rs"
    ));
    let source = std::fs::read_to_string(path).unwrap();
    match super::crate_graph::format_trait_facade_tests::run("upstream-issue-6449", &source, 2015) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(_) => panic!("unchanged issue-6449 main did not complete"),
        Err(error) => panic!("unchanged issue-6449 main: {error:?}"),
    }
}

#[test]
fn type_relative_variant_paths_share_one_route_across_origins_and_editions() {
    // LRR route control: `Self::V` in an impl and `Alias::V` for a type alias of
    // an enum (rustc since 1.37), as expressions, constructor calls, record
    // constructors and patterns, handwritten, emitted by a user macro, through a
    // wrapper and nested in a standard macro, select the enum's variant through
    // the one `CallResolver` type-relative lookup that every route shares.
    const SOURCE: &str = r#"
        mod m { #[derive(Debug, PartialEq)] pub enum E { A, B(i32), C { x: i32 } } pub type Al = E; }
        use m::Al;
        #[derive(Debug, PartialEq)] enum L { A, B(i32), C { x: i32 } }
        struct S { f: i32 }
        struct T(i32);
        struct U;
        macro_rules! mk { ($t:ident :: $v:ident) => { $t::$v }; ($t:ident :: $v:ident ($e:expr)) => { $t::$v($e) }; }
        macro_rules! pat { ($x:expr, $p:pat => $r:expr) => { match $x { $p => $r, _ => -1 } }; }
        macro_rules! wrapped { ($x:expr, $p:pat => $r:expr) => { pat!($x, $p => $r) }; }
        impl L {
            const K: Self = Self::B(9);
            fn a() -> Self { Self::A }
            fn b(n: i32) -> Self { Self::B(n) }
            fn c(x: i32) -> Self { Self::C { x } }
            fn m() -> Self { mk!(Self::B(4)) }
            fn q() -> Self { <Self>::A }
            fn val(&self) -> i32 {
                match self { Self::A => 0, Self::B(n) => *n, Self::C { x } => *x }
            }
            fn is_a(&self) -> bool { matches!(self, Self::A) }
            fn wrap(&self) -> i32 { wrapped!(self, Self::B(n) => *n) }
        }
        impl S { fn new() -> Self { Self { f: 4 } } fn get(&self) -> i32 { let Self { f } = self; *f } }
        impl T { fn new() -> Self { Self(5) } fn get(&self) -> i32 { let Self(f) = self; *f } }
        impl U { fn new() -> Self { Self } fn is(&self) -> bool { matches!(self, Self) } }
        fn main() {
            assert_eq!(L::a(), L::A);
            assert_eq!(L::b(2), L::B(2));
            assert_eq!(L::c(3), L::C { x: 3 });
            assert_eq!(L::m(), L::B(4));
            assert_eq!(L::q(), L::A);
            assert_eq!(L::K, L::B(9));
            assert_eq!(L::c(7).val(), 7);
            assert!(L::a().is_a());
            assert_eq!(L::b(6).wrap(), 6);
            assert_eq!(S::new().get(), 4);
            assert_eq!(T::new().get(), 5);
            assert!(U::new().is());
            let a = mk!(Al::A);
            let b = mk!(Al::B(3));
            assert_eq!(a, m::E::A);
            assert_eq!(b, m::Al::B(3));
            assert_eq!(Al::C { x: 8 }, m::E::C { x: 8 });
            assert_eq!(pat!(b, Al::B(n) => n), 3);
            assert_eq!(wrapped!(m::Al::C { x: 8 }, m::Al::C { x } => x), 8);
            assert_eq!(pat!(a, <Al>::A => 100), 100);
            assert!(matches!(m::E::B(1), Al::B(1)));
        }
    "#;
    for edition in [2015, 2018, 2021, 2024] {
        match super::crate_graph::format_trait_facade_tests::run(
            "type-relative-variant-paths",
            SOURCE,
            edition,
        ) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(_) => panic!("type-relative variant paths did not complete in edition {edition}"),
            Err(error) => panic!("type-relative variant paths in edition {edition}: {error:?}"),
        }
    }
}

#[test]
fn block_level_use_roots_continue_through_longer_paths_in_every_origin_and_edition() {
    // LRR route control: a path of three or more segments whose root a
    // block-level `use` binds (a module, here also reached through a type alias)
    // continues from that binding through the one qualified body-module kernel,
    // in expressions, calls, record constructors and patterns, handwritten, from
    // a user macro and nested in a standard macro.
    const SOURCE: &str = r#"
        mod a {
            pub mod b {
                #[derive(Debug, PartialEq)] pub enum E { X, Y(i32), Z { v: i32 } }
                pub type Al = E;
                pub fn f(n: i32) -> i32 { n * 2 }
                pub mod c { pub fn g() -> i32 { 6 } pub const K: i32 = 3; }
            }
        }
        macro_rules! first { ($e:expr) => { match $e { b::E::Y(n) => n, _ => -1 } }; }
        fn main() {
            use a::b;
            use a::b::c;
            use std::cmp;
            let v = b::E::Y(2);
            assert_eq!(first!(b::E::Y(5)), 5);
            assert!(matches!(v, b::E::Y(2)));
            match v { b::E::X => panic!(), b::E::Y(n) => assert_eq!(n, 2), b::E::Z { .. } => panic!() }
            match (b::E::Z { v: 9 }) { b::E::Z { v } => assert_eq!(v, 9), _ => panic!() }
            assert_eq!(b::f(3), 6);
            assert_eq!(c::g(), 6);
            assert_eq!(b::c::g(), 6);
            assert_eq!(b::Al::Y(7), b::E::Y(7));
            match b::Al::Y(7) { b::Al::Y(n) => assert_eq!(n, 7), _ => panic!() }
            let r = match 1.cmp(&2) {
                cmp::Ordering::Less => 10,
                cmp::Ordering::Equal => 20,
                cmp::Ordering::Greater => 30,
            };
            assert_eq!(r, 10);
            assert!(cmp::Ordering::Greater == 2.cmp(&1));
        }
    "#;
    for edition in [2015, 2018, 2021, 2024] {
        match super::crate_graph::format_trait_facade_tests::run(
            "block-use-deep-roots",
            SOURCE,
            edition,
        ) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(_) => panic!("block-level use roots did not complete in edition {edition}"),
            Err(error) => panic!("block-level use roots in edition {edition}: {error:?}"),
        }
    }
}

#[test]
fn unchanged_upstream_self_and_alias_variant_programs_complete() {
    for (relative, edition) in [
        ("pattern/issue-110508.rs", 2015),
        ("type-alias-enum-variants/type-alias-enum-variants-pass.rs", 2015),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/upstream/rustc-checkout/tests/ui")
            .join(relative);
        let source = std::fs::read_to_string(&path).unwrap();
        match super::crate_graph::format_trait_facade_tests::run("upstream-variants", &source, edition) {
            Ok(StandaloneOutcome::Completed(_)) => {}
            Ok(_) => panic!("unchanged {relative} did not complete"),
            Err(error) => panic!("unchanged {relative}: {error:?}"),
        }
    }
}
