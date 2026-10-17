//! Whole programs constructing and using unit/tuple structs and enum variants
//! through the ordinary pipeline, handwritten and emitted by a user macro.
use super::*;
use crate::ir::op::VALUE_PATH_OPCODE;

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
        "lrr-nominal-values-{}-{}.rs",
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
fn completed(text: &str, edition: u16) -> crate::engine::run::RunReport {
    let owned = source(text);
    match standalone_edition(&owned.0, None, edition) {
        Ok(StandaloneOutcome::Completed(report)) => report,
        Ok(StandaloneOutcome::Panicked(report)) => {
            panic!("edition {edition} panicked: {:?}\n{text}", report.panic)
        }
        Ok(_) => panic!("edition {edition} produced another outcome\n{text}"),
        Err(error) => panic!("edition {edition} faulted: {error:?}\n{text}"),
    }
}
fn panicked(text: &str, edition: u16) {
    let owned = source(text);
    assert!(
        matches!(
            standalone_edition(&owned.0, None, edition),
            Ok(StandaloneOutcome::Panicked(_))
        ),
        "edition {edition} did not panic\n{text}"
    );
}
const EDITIONS: [u16; 2] = [2015, 2021];

#[test]
fn rustc_enum_export_inheritance_whole_program_selects_variant_through_module_and_enum() {
    let path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/upstream/rustc-checkout/tests/ui/structs-enums/enum-export-inheritance.rs"
    ));
    // Unchanged original `let _x = a::Foo::Bar;` under rustc's default 2015
    // edition: the enum is a module-like namespace of its variants.
    let StandaloneOutcome::Completed(report) = standalone_edition(path, None, 2015).unwrap() else {
        panic!("original enum-export-inheritance did not complete")
    };
    assert_eq!(report.status, "completed");
    assert_eq!(
        report.opcode_counters.started_by_opcode[usize::from(VALUE_PATH_OPCODE)],
        1
    );
}

#[test]
fn unit_and_tuple_struct_values_construct_project_and_move() {
    for edition in EDITIONS {
        completed("struct S; fn main() { let _s = S; }", edition);
        completed(
            "struct T(i32, bool); fn main() { let t = T(3, true); assert!(t.0 == 3); assert!(t.1); }",
            edition,
        );
        completed(
            "struct S; impl S {} fn mk() -> S { S } fn main() { let _s = mk(); }",
            edition,
        );
        completed(
            "struct T(i32, bool); fn f(t: T) -> i32 { t.0 } fn main() { let t = T(9, true); let r = f(t); assert!(r == 9); }",
            edition,
        );
        // Braced initialization of a tuple struct names fields by ordinal.
        completed(
            "struct S(u8, u16); fn main() { let s = S { 1: 10, 0: 11 }; assert!(s.0 == 11); assert!(s.1 == 10); }",
            edition,
        );
        panicked(
            "struct T(i32, bool); fn main() { let t = T(3, true); assert!(t.0 == 4); }",
            edition,
        );
    }
}

#[test]
fn enum_variant_values_construct_through_paths_imports_and_globs() {
    for edition in EDITIONS {
        let report = completed(
            "enum E { A, B } fn main() { let e = E::A; let f = E::B; }",
            edition,
        );
        assert_eq!(
            report.opcode_counters.started_by_opcode[usize::from(VALUE_PATH_OPCODE)],
            2
        );
        completed(
            "enum E { A(i32), B { x: u8 } } fn main() { let a = E::A(1); let b = E::B { x: 2 }; }",
            edition,
        );
        completed(
            "use E::*; enum E { X, Y(i32), Z { q: bool } } fn main() { let v = Y(4); let w = Z { q: true }; let x = X; }",
            edition,
        );
        completed(
            "mod m { pub enum E { A, B(u8) } pub struct S; pub struct T(pub i32); }\n\
             use m::E;\n\
             fn main() { let a = E::A; let b = m::E::B(3); let s = m::S; let t = m::T(5); assert!(t.0 == 5); }",
            edition,
        );
    }
}

#[test]
fn fieldless_enum_casts_read_the_bound_discriminant() {
    for edition in EDITIONS {
        completed(
            "enum E { A = 1, B = 5 } fn main() { let n = E::B as i32; assert!(n == 5); let m = E::A as u8; assert!(m == 1); }",
            edition,
        );
        completed(
            "enum E { A = -2, B, C = 0x10 } fn main() { let b = E::B as i32; let c = E::C as i32; assert!(b == -1); assert!(c == 16); }",
            edition,
        );
        completed(
            "enum E { A, B } fn pick(b: bool) -> E { if b { E::B } else { E::A } } fn main() { let e = pick(true); let n = e as i32; assert!(n == 1); }",
            edition,
        );
        panicked(
            "enum E { A = 1, B = 5 } fn main() { let n = E::B as i32; assert!(n == 4); }",
            edition,
        );
    }
}

#[test]
fn macro_emitted_constructors_share_the_handwritten_route() {
    for edition in EDITIONS {
        let handwritten = completed(
            "enum E { A, B(i32) } struct T(i32, bool);\n\
             fn main() { let b = E::B(2); let t = T(1, false); assert!(t.0 == 1); let c = E::A; }",
            edition,
        );
        let emitted = completed(
            "macro_rules! mk { ($e:expr) => { $e } }\n\
             enum E { A, B(i32) } struct T(i32, bool);\n\
             fn main() { let b = mk!(E::B(2)); let t = mk!(T(1, false)); assert!(mk!(t.0) == 1); let c = mk!(E::A); }",
            edition,
        );
        // The same ordinary ValuePath and call-seal operations run in both.
        for tag in [usize::from(VALUE_PATH_OPCODE), 19, 22] {
            assert_eq!(
                handwritten.opcode_counters.started_by_opcode[tag],
                emitted.opcode_counters.started_by_opcode[tag],
                "opcode {tag} differs between handwritten and macro-emitted constructors"
            );
        }
    }
}

#[test]
fn type_paths_inside_macro_output_resolve_through_the_type_service() {
    for edition in EDITIONS {
        completed(
            "fn main() { assert!(1u8 as i32 == 1); let c = 97u8 as char; assert!(c as u32 == 97); }",
            edition,
        );
        completed(
            "enum E { A = 1, B = 5 } fn main() { assert!(E::B as i32 == 5); }",
            edition,
        );
        completed(
            "struct P { x: i32 } fn f(p: P) -> i32 { p.x } fn main() { assert!(f(P { x: 1 }) == 1); }",
            edition,
        );
        completed(
            "macro_rules! mk { ($e:expr) => { $e } } struct P { x: i32 }\n\
             fn main() { let p = mk!(P { x: 1 }); assert!(p.x == 1); }",
            edition,
        );
        completed(
            "mod m { pub struct P { pub x: i32 } } macro_rules! mk { () => { m::P { x: 3 } } }\n\
             fn main() { let p = mk!(); assert!(p.x == 3); }",
            edition,
        );
        completed(
            "enum E { A, C { x: u8 } } fn f(e: E) -> i32 { match e { E::A => 1, E::C { x } => x as i32 } }\n\
             fn main() { assert!(f(E::C { x: 9 }) == 9); }",
            edition,
        );
        panicked("fn main() { assert!(1u8 as i32 == 2); }", edition);
    }
}

#[test]
fn annotated_lets_select_nominal_tuple_and_array_types() {
    for edition in EDITIONS {
        completed(
            "enum E { A, B } fn main() { let e: E = E::B; let n = e as i32; assert!(n == 1); }",
            edition,
        );
        completed(
            "struct P { x: i32 } fn main() { let t: (i32, P) = (1, P { x: 2 }); assert!(t.0 == 1); let _: P = P { x: 3 }; }",
            edition,
        );
        completed(
            "enum E { A, B } fn main() { let a: [E; 2] = [E::A, E::B]; let t: (u8, (i64, bool)) = (1, (2, false)); assert!((t.1).0 == 2); }",
            edition,
        );
        completed(
            "mod m { pub struct S(pub u8); } fn main() { let s: m::S = m::S(4); assert!(s.0 == 4); }",
            edition,
        );
        completed(
            "struct L { a: i32, b: i32 } fn f() -> i32 { let x: L = L { a: 1, b: 2 }; x.a + x.b } fn main() { assert!(f() == 3); }",
            edition,
        );
        // Assignment drops the replaced nominal value through drop glue.
        completed(
            "enum E { A, B } struct T(i32); fn main() { let mut e = E::A; e = E::B; let n = e as i32; assert!(n == 1);\n\
             let mut t = T(1); t = T(2); assert!(t.0 == 2); }",
            edition,
        );
    }
}

#[test]
fn generic_nominal_values_infer_arguments_from_fields_or_context() {
    for edition in EDITIONS {
        completed(
            "struct W<T> { x: T } fn main() { let w = W { x: 5 }; assert!(w.x == 5); let v = W { x: true }; assert!(v.x); }",
            edition,
        );
        completed(
            "struct P<T>(T, T); fn main() { let p = P(1u8, 2); assert!(p.0 == 1); assert!(p.1 == 2); }",
            edition,
        );
        completed(
            "enum Opt<T> { No, Yes(T) } fn main() { let o = Opt::Yes(3); let n = match o { Opt::Yes(v) => v, Opt::No => 0 }; assert!(n == 3); }",
            edition,
        );
        completed(
            "enum Opt<T> { No, Yes(T) } fn main() { let o: Opt<i32> = Opt::No; let p: Opt<u8> = Opt::Yes(7);\n\
             let n = match p { Opt::Yes(v) => v, Opt::No => 0 }; assert!(n == 7); }",
            edition,
        );
        completed(
            "struct W<T> { x: T } fn get(w: W<i32>) -> i32 { w.x } fn main() { let w = W { x: 4 }; assert!(get(w) == 4);\n\
             let n = W { x: W { x: 2u16 } }; assert!(n.x.x == 2); let a: W<(i32, bool)> = W { x: (1, true) }; assert!(a.x.1); }",
            edition,
        );
        completed(
            "enum E<T> { A(T), B { v: T } } fn main() { let b = E::B { v: 1.5 }; let a = E::A(9i64); }",
            edition,
        );
        panicked(
            "struct W<T> { x: T } fn main() { let w = W { x: 5 }; assert!(w.x == 6); }",
            edition,
        );
        let owned = source("struct P<T>(T, T); fn main() { let p = P(1u8, true); }");
        assert!(
            matches!(standalone_edition(&owned.0, None, edition), Err(error) if error.code == "LRR-E-CALL-ARGUMENT-TYPE")
        );
    }
}

#[test]
fn nominal_copy_impls_copy_and_other_nominal_values_move() {
    for edition in EDITIONS {
        completed(
            "struct P { x: i32 } impl Clone for P { fn clone(&self) -> P { P { x: self.x } } } impl Copy for P {}\n\
             fn main() { let p = P { x: 1 }; let q = p; assert!(p.x == 1); assert!(q.x == 1); }",
            edition,
        );
        let owned =
            source("struct P { x: i32 } fn main() { let p = P { x: 1 }; let q = p; let r = p; }");
        assert!(
            matches!(standalone_edition(&owned.0, None, edition), Err(error) if error.code == "LRR-E-USE-AFTER-MOVE")
        );
    }
}

#[test]
fn written_reference_str_slice_inferred_and_trait_object_annotations_share_the_type_service() {
    for edition in EDITIONS {
        // References, arrays and `str` pointees: every composite annotation
        // is selected leaf by leaf through the entered Type service.
        completed(
            "struct P { x: i32 } fn main() { let y: &str = \"there\"; let z: &'static str = \"a\";\n\
             let r: &u8 = &5; assert!(*r == 5); let a: [u8; 3] = [1, 2, 3]; assert!(a[0] == 1);\n\
             let p = P { x: 1 }; let q: &P = &p; assert!(q.x == 1); let t: &(i32, P) = &(1, P { x: 2 }); assert!(t.0 == 1);\n\
             let mut v = 1; let m: &mut i32 = &mut v; *m = 3; assert!(v == 3); let _: &'_ u8 = &7; }",
            edition,
        );
        // `_` leaves keep the written structure; the initializer supplies them.
        completed(
            "enum Opt<T> { No, Yes(T) } fn main() { let x: _ = 5; assert!(x == 5); let o: Opt<_> = Opt::Yes(3u8);\n\
             let n = match o { Opt::Yes(v) => v, Opt::No => 0 }; assert!(n == 3); let r: &_ = &true; assert!(*r); }",
            edition,
        );
        // An array result keeps its declared descriptor without the old
        // spelling fast path.
        completed(
            "fn f() -> [u8; 3] { [1, 2, 3] } fn main() { let a = f(); assert!(a[1] == 2); }",
            edition,
        );
    }
    // A trait-object place keeps its value's concrete Type.
    completed(
        "trait T { fn f(&self) -> i32; } struct S; impl T for S { fn f(&self) -> i32 { 4 } }\n\
         fn main() { let s = S; let o: &dyn T = &s; assert!(o.f() == 4); }",
        2021,
    );
}

#[test]
fn compound_field_types_bind_leaf_by_leaf_through_the_declaration_type_service() {
    for edition in EDITIONS {
        // Reference, tuple, array and qualified field Types.
        completed(
            "struct S<'a> { v: &'a isize } struct P { t: (i32, u8), a: [u8; 3] }\n\
             fn main() { let x = 3; let s = S { v: &x }; assert!(*s.v == 3);\n\
             let p = P { t: (1, 2), a: [1, 2, 3] }; assert!(p.t.1 == 2); assert!(p.a[2] == 3); }",
            edition,
        );
        completed(
            "mod m { pub struct S(pub u8); pub struct W<T> { pub x: T } }\n\
             struct H { s: (m::S, u8), w: m::W<i32>, r: [m::S; 2] }\n\
             fn main() { let h = H { s: (m::S(1), 2), w: m::W { x: 3 }, r: [m::S(4), m::S(5)] };\n\
             assert!((h.s.0).0 == 1); assert!(h.w.x == 3); assert!(h.r[1].0 == 5); }",
            edition,
        );
        // Generic applications over the declaration's own parameters; a
        // field's declared `W<i32>` is the nested constructor's context.
        completed(
            "struct W<T> { x: T } struct V<T> { w: W<T>, p: (T, u8) }\n\
             fn main() { let v = V { w: W { x: 5 }, p: (6, 1) }; assert!(v.w.x == 5); assert!(v.p.0 == 6); }",
            edition,
        );
        completed(
            "pub struct W<T> { pub x: T } struct H { w: W<i32> }\n\
             fn main() { let h = H { w: W { x: 3 } }; assert!(h.w.x == 3); }",
            edition,
        );
        // Only the reached variant's fields are bound.
        completed(
            "enum E<'a> { A(&'a str), B(Box<u8>) } fn main() { let e = E::A(\"x\"); }",
            edition,
        );
    }
}

#[test]
fn partial_annotations_tuple_member_views_and_qualified_generic_annotations() {
    for edition in EDITIONS {
        // A partially written Type hints its exactly known members.
        completed(
            "fn main() { let x = 5i64; let t: (_, u8) = (x, 2); assert!(t.1 == 2);\n\
             let a: [_; 3] = [1u8, 2, 3]; let r: &(u8, _) = &(255, 1i64); assert!(r.0 == 255); }",
            edition,
        );
        // A tuple member's closed nominal Type is its projected view.
        completed(
            "struct P { x: i32 } fn main() { let t = &(1, P { x: 2 }); assert!((t.1).x == 2);\n\
             let u: &(i32, P) = &(1, P { x: 3 }); assert!((u.1).x == 3); }",
            edition,
        );
        completed(
            "mod m { pub struct W<T> { pub x: T } pub enum O<T> { N, Y(T) } }\n\
             fn main() { let w: m::W<u8> = m::W { x: 1 }; assert!(w.x == 1); let o: m::O<i64> = m::O::N;\n\
             let p: m::O<u8> = m::O::Y(3); }",
            edition,
        );
    }
}

#[test]
fn declared_nominal_results_are_the_root_result_context() {
    for edition in EDITIONS {
        // A generic unit variant in tail or `return` position takes its type
        // arguments from the definition's declared result.
        completed(
            "enum Opt<T> { No, Yes(T) } fn f() -> Opt<u32> { Opt::No }\n\
             fn g(b: bool) -> Opt<u8> { if b { return Opt::Yes(1); } Opt::No }\n\
             fn main() { match f() { Opt::No => {}, Opt::Yes(_) => panic!() }\n\
             match g(true) { Opt::Yes(v) => assert!(v == 1), Opt::No => panic!() } }",
            edition,
        );
        // A structural result with a nominal member.
        completed(
            "struct W<T> { x: T } fn f() -> (W<u8>, i32) { (W { x: 1 }, 2) }\n\
             fn main() { let t = f(); assert!((t.0).x == 1); }",
            edition,
        );
        // A qualified or aliased result with a unit argument fixes the
        // constructor's undetermined parameter (`fmt::Result`'s shape).
        completed(
            "mod m { pub struct E; pub enum Res<T, U> { Good(T), Bad(U) }\n\
             pub type R = crate::m::Res<(), E>; }\n\
             fn f() -> m::Res<(), m::E> { m::Res::Good(()) } fn g() -> m::R { m::Res::Good(()) }\n\
             fn main() { if let m::Res::Good(()) = f() {} else { panic!() }\n\
             if let m::Res::Good(()) = g() {} else { panic!() } }",
            edition,
        );
    }
}

#[test]
fn declared_parameter_types_are_their_arguments_context() {
    for edition in EDITIONS {
        // A closed nominal parameter gives its argument its descriptor; an
        // open one (`Res<T, u32>`, `(T, Opt<u8>)`) a partial context whose
        // known parts hint the argument's own construction.
        completed(
            "enum Opt<T> { No, Yes(T) } enum Res<T, E> { Good(T), Bad(E) } enum Test { A, B }\n\
             fn test(x: Opt<Test>) -> bool { match x { Opt::Yes(Test::B) => true, _ => false } }\n\
             fn r<T>(x: Res<T, u32>) -> bool { match x { Res::Good(_) => true, Res::Bad(_) => false } }\n\
             fn pair<T>(p: (T, Opt<u8>)) -> bool { match p.1 { Opt::No => true, _ => false } }\n\
             fn main() { assert!(test(Opt::Yes(Test::B))); assert!(!test(Opt::No));\n\
             assert!(!test(Opt::Yes(Test::A))); assert!(r(Res::Good(3)));\n\
             assert!(pair((1u16, Opt::No))); }",
            edition,
        );
        // A parameter the result context fixes through the declared result,
        // and one an earlier argument fixed.
        completed(
            "enum Opt<T> { No, Yes(T) } fn wrap<T>(x: T) -> Opt<T> { Opt::Yes(x) }\n\
             fn first<T: Copy>(a: T, b: T) -> T { a }\n\
             fn main() { let o: Opt<u64> = wrap(5);\n\
             match o { Opt::Yes(v) => assert!(v == 5u64), Opt::No => panic!() }\n\
             let x = first(3u8, 4); assert!(x == 3u8); }",
            edition,
        );
    }
}

#[test]
fn body_local_compound_types_select_each_leaf_occurrence() {
    for edition in EDITIONS {
        // Parameter, result and field Types of body-local declarations
        // written around leaf names, each resolved at its own occurrence by
        // the owning body plan's header service (a generic base's written
        // arguments apply to the declaration it selects).
        completed(
            "enum Opt<T> { No, Yes(T) }\n\
             fn main() {\n\
               struct Q { p: (u8, u16), r: [u16; 2], o: Opt<u32> }\n\
               fn sum(t: (u16, u16), a: [u16; 2], s: &[u16], r: &mut i64) -> u16 {\n\
                 *r += 1; t.0 + t.1 + a[0] + a[1] + s[1] }\n\
               fn mk(x: u8) -> Opt<u8> { Opt::Yes(x) }\n\
               fn none(o: Opt<u32>) -> bool { match o { Opt::No => true, _ => false } }\n\
               let mut r = 0i64;\n\
               assert!(sum((1, 2), [3, 4], &[5, 6], &mut r) == 16);\n\
               assert!(r == 1);\n\
               match mk(7) { Opt::Yes(v) => assert!(v == 7), Opt::No => panic!() }\n\
               assert!(none(Opt::No));\n\
               let q = Q { p: (1, 2), r: [3, 4], o: Opt::Yes(5) };\n\
               assert!(q.p.1 == 2 && q.r[1] == 4);\n\
               match q.o { Opt::Yes(v) => assert!(v == 5u32), Opt::No => panic!() }\n\
             }",
            edition,
        );
    }
}
