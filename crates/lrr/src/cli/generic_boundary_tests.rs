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

#[test]
fn integer_try_from_errors_format_like_core() {
    prints(
        "try-from-errors",
        "fn main() {
    println!(\"{:?} {:?}\", u8::try_from(256u32), u8::try_from(-1i32));
    println!(\"{}\", u8::try_from(256u32).unwrap_err());
}
",
        "Err(TryFromIntError(PosOverflow)) Err(TryFromIntError(NegOverflow))\nnumber too large to fit in target type\n",
    );
}

#[test]
fn range_patterns_take_constant_bounds() {
    // Range bounds written as bare constants, module paths and half-open
    // forms resolve through the ordinary Value namespace like constant
    // patterns.
    prints(
        "range-pattern-constants",
        "const LOW: i32 = 10;
const HIGH: i32 = 20;
mod limits {
    pub const MAX: u8 = 200;
    pub const MIN: u8 = 100;
}
const FIRST: char = 'a';
const LAST: char = 'f';

fn classify(n: i32) -> &'static str {
    match n {
        i32::MIN..=-1 => \"negative\",
        0 => \"zero\",
        1..LOW => \"small\",
        LOW..=HIGH => \"medium\",
        _ => \"large\",
    }
}

fn byte(b: u8) -> &'static str {
    match b {
        0..limits::MIN => \"low\",
        limits::MIN..=limits::MAX => \"mid\",
        _ => \"high\",
    }
}

fn letter(c: char) -> bool {
    matches!(c, FIRST..=LAST)
}

fn main() {
    for n in [-5, 0, 3, 10, 15, 20, 21] {
        println!(\"{} {}\", n, classify(n));
    }
    for b in [0u8, 99, 100, 200, 201] {
        println!(\"{} {}\", b, byte(b));
    }
    println!(\"{} {} {}\", letter('a'), letter('f'), letter('g'));
    let x = 7u8;
    if let ..=limits::MAX = x {
        println!(\"upto\");
    }
    if let limits::MIN.. = 150u8 {
        println!(\"from\");
    }
}
",
        "-5 negative\n0 zero\n3 small\n10 medium\n15 medium\n20 medium\n21 large\n0 low\n99 low\n100 mid\n200 mid\n201 high\ntrue true false\nupto\nfrom\n",
    );
}

#[test]
fn boxed_closures_are_called_through_the_box() {
    // The callee operand autoderefs through `Box` like it does through
    // references (`b(2)`, `foo()()`, `(*f)(2)`).
    prints(
        "boxed-closure-calls",
        "fn foo() -> Box<dyn FnMut() -> isize + 'static> {
    let r = || 22;
    Box::new(r)
}
fn make(k: i32) -> Box<dyn Fn(i32) -> i32> {
    Box::new(move |x| x + k)
}
fn main() {
    let b: Box<dyn Fn(i32) -> i32> = Box::new(|x| x + 1);
    println!(\"{}\", b(2));
    println!(\"{}\", foo()());
    let mut n = 0;
    let mut c: Box<dyn FnMut()> = Box::new(|| n += 1);
    c();
    c();
    drop(c);
    println!(\"{}\", n);
    let f = make(10);
    println!(\"{} {}\", f(1), (*f)(2));
}
",
        "3\n22\n2\n11 12\n",
    );
}

#[test]
fn closures_move_their_by_value_captures_out_when_called_once() {
    // A closure body consuming a captured variable moves it out of the
    // closure's own field (an `FnOnce` closure); the closure's remaining
    // captures drop with the closure value.
    prints(
        "closure-capture-move-out",
        "struct D(i32);
impl Drop for D {
    fn drop(&mut self) {
        println!(\"drop {}\", self.0);
    }
}
fn apply(f: impl FnOnce() -> String) -> String {
    f()
}
fn main() {
    let s = String::from(\"a\");
    let f = move || s;
    println!(\"{}\", f());
    let t = String::from(\"bc\");
    let g = || drop(t);
    g();
    println!(\"{}\", apply(move || String::from(\"z\")));
    let u = String::from(\"u\");
    println!(\"{}\", apply(move || u));
    let d = D(1);
    let e = D(2);
    let h = move || {
        let x = d;
        println!(\"in {}\", x.0);
    };
    let k = move || println!(\"k {}\", e.0);
    h();
    k();
    println!(\"end\");
}
",
        "a\nz\nu\nin 1\ndrop 1\nk 2\nend\ndrop 2\n",
    );
}

#[test]
fn shorthand_projection_retains_the_declaring_bounds_type_arguments() {
    // The common Type service must select Convert<u8>, not Convert<bool>,
    // through a where-clause or an inline bound in every source context.
    // This is a route control; unchanged stock projection tests remain the
    // primary language witnesses, including sized/coinductive-2.rs.
    prints(
        "projection-bound-arguments",
        r#"
trait Marker {}
trait Other<T> {}
trait Convert<T> { type Error; }
struct Target;
impl Marker for Target {}
impl Other<u16> for Target {}
impl Convert<u8> for Target { type Error = u16; }
impl Convert<bool> for Target { type Error = u32; }
fn projected<U>(value: U::Error) -> U::Error
where U: Other<U::Error> + Marker + Convert<u8> { value }
fn inline<U: Marker + Convert<bool>>(value: U::Error) -> U::Error { value }
fn open<T, U>(value: U::Error) -> U::Error where U: Convert<T> { value }
struct Holder<U>(U);
impl<U> Holder<U> where U: Marker + Convert<u8> {
    fn inherited(&self, value: U::Error) -> U::Error { value }
}
macro_rules! declare {
    () => {
        fn generated<U>(value: U::Error) -> U::Error
        where U: Other<U::Error> + Marker + Convert<u8> { value }
    }
}
declare!();
macro_rules! call { ($value:expr) => { projected::<Target>($value) }; }
macro_rules! wrapper { ($value:expr) => { call!($value) }; }
fn main() {
    println!("{}", projected::<Target>(65535));
    println!("{}", inline::<Target>(65536));
    println!("{}", generated::<Target>(65534));
    println!("{}", call!(65533));
    println!("{}", wrapper!(65532));
    assert_eq!(wrapper!(65531), 65531u16);
    let holder = Holder(Target);
    println!("{}", holder.inherited(65530));
    println!("{}", open::<u8, Target>(65529));
    println!("{}", open::<bool, Target>(65537));
}
"#,
        "65535\n65536\n65534\n65533\n65532\n65530\n65529\n65537\n",
    );
}

fn cold_associated_source(branch: &str, route: u8) -> String {
    // Reuse the unchanged upstream declarations; adaptations stay here.
    const ORIGINAL: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/associated-types/associated-types-struct-field-numbered.rs"
    );
    let (declarations, _) = ORIGINAL.split_once("pub fn main()").unwrap();
    let operation = format!("if true {{ 0xffffffff }} else {{ {branch} }}");
    let (macros, body) = match route {
        0 => (
            String::new(),
            format!("let value = {operation}; assert!(value > 0);"),
        ),
        1 => (
            format!("macro_rules! emit {{ () => {{ {operation} }}; }}"),
            "let value = emit!(); assert!(value > 0);".into(),
        ),
        2 => (
            format!(
                "macro_rules! emit {{ () => {{ {operation} }}; }} macro_rules! wrapper {{ () => {{ emit!() }}; }}"
            ),
            "let value = wrapper!(); assert!(value > 0);".into(),
        ),
        3 => (String::new(), format!("assert!(({operation}) > 0);")),
        _ => unreachable!(),
    };
    format!(
        "{declarations}\nfn main() {{ let node: Node<i32> = Node(1, Some(22)); {macros} {body} }}"
    )
}

fn cold_local_associated_source(branch: &str, route: u8) -> String {
    // Move the exact upstream Fn declaration; the imported source is unchanged.
    let source = cold_associated_source(branch, route);
    let start = source.find("fn foo<").unwrap();
    let end = source[start..].find("impl UnifyKey for i32").unwrap() + start;
    let declaration = source[start..end].to_owned();
    let mut source = format!("{}{}", &source[..start], &source[end..]);
    let main = source.find("fn main() {").unwrap() + "fn main() {".len();
    source.insert_str(main, &declaration);
    source
}

#[test]
fn cold_local_associated_result_keeps_unentered_source_calls_cold() {
    for edition in [2015, 2018, 2021, 2024] {
        for route in 0..4 {
            for branch in ["0_u32", "foo(&node).unwrap()"] {
                let source = cold_local_associated_source(branch, route);
                match run("cold-local-associated-result", &source, edition) {
                    Ok(StandaloneOutcome::Completed(report)) => {
                        assert_eq!(report.output.stdout, "")
                    }
                    Ok(StandaloneOutcome::Panicked(report)) => panic!(
                        "edition {edition}, route {route}, branch {branch}: {:?}",
                        report.panic
                    ),
                    Ok(_) => panic!(
                        "edition {edition}, route {route}, branch {branch}: unexpected outcome"
                    ),
                    Err(error) => {
                        panic!("edition {edition}, route {route}, branch {branch}: {error}")
                    }
                }
            }
        }
    }
}

#[test]
fn cold_associated_result_keeps_unentered_source_calls_cold() {
    for edition in [2015, 2018, 2021, 2024] {
        for route in 0..4 {
            for branch in ["0_u32", "foo(&node).unwrap()"] {
                let source = cold_associated_source(branch, route);
                match run("cold-associated-result", &source, edition) {
                    Ok(StandaloneOutcome::Completed(report)) => {
                        assert_eq!(report.output.stdout, "")
                    }
                    Ok(StandaloneOutcome::Panicked(report)) => panic!(
                        "edition {edition}, route {route}, branch {branch}: {:?}",
                        report.panic
                    ),
                    Ok(_) => panic!(
                        "edition {edition}, route {route}, branch {branch}: unexpected outcome"
                    ),
                    Err(error) => {
                        panic!("edition {edition}, route {route}, branch {branch}: {error}")
                    }
                }
            }
        }
    }
}

#[test]
fn cold_associated_type_goal_retains_selected_source_without_callee_preparation() {
    check_cold_associated_type_goal(false);
}

#[test]
fn cold_local_associated_type_goal_retains_selected_source_without_callee_preparation() {
    check_cold_associated_type_goal(true);
}

fn check_cold_associated_type_goal(local: bool) {
    use crate::{
        core::digest::StableEncode,
        db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            memo::Database,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        },
        frontend::{
            associated_equations::EquationOwner, binding_constraints::BindingConstraintState,
            callables::CallSelection,
        },
        ir::{op::PrimitiveType, pc::Phase},
        source::manifest::Namespace,
        syntax::body_index::{ExpressionKind, NodeRole},
    };
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("cold source goal: {error:?}"),
            QueryPoll::NeedInput(input) => panic!("cold source goal input: {input:?}"),
        }
    }
    for edition in [2015, 2018, 2021, 2024] {
        for route in [0, 1] {
            for branch in ["0_u32", "foo(&node).unwrap()"] {
                let temp = Temp::new("cold-associated-type-goal");
                let root = temp.0.join("main.rs");
                std::fs::write(
                    &root,
                    if local {
                        cold_local_associated_source(branch, route)
                    } else {
                        cold_associated_source(branch, route)
                    },
                )
                .unwrap();
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
                let (compiler, prepared, view, sources) =
                    crate::cli::compile_inputs::PreparedCompileInputs::prepare(
                        &root, prepared, view, sources,
                    )
                    .unwrap();
                let mut db = Database::default();
                db.trace.detailed = true;
                prepared.publish(&mut db).unwrap();
                db.begin_expansion_epoch(1).unwrap();
                crate::cli::type_preparation::prepare(
                    &compiler,
                    &mut db,
                    &view,
                    &sources,
                    &mut snapshot,
                    None,
                )
                .unwrap();
                let mut discovery =
                    crate::expand::source_discovery::SourceDiscovery::new_deferred_imports(
                        sources,
                        view.outline().root_scope.clone(),
                        Namespace::Value,
                        view.selection().outline.crate_key,
                        1,
                    )
                    .unwrap()
                    .with_declarative_compiler(
                        compiler.environment_identity(),
                        crate::expand::source_declarative::engine_version(),
                        compiler.catalog.clone(),
                    )
                    .unwrap();
                compiler
                    .discover_diagnostic(&mut discovery, &mut db, &mut snapshot)
                    .unwrap();
                discovery.publish(&mut db).unwrap();
                let CallSelection::Source(main) = ready(discovery.entry_function(&mut db, "main"))
                else {
                    panic!("main source selection")
                };
                let main =
                    crate::cli::retained_entry::prepare(&mut db, &discovery, &view, "main", main)
                        .unwrap();
                let body = main.body().unwrap().key();
                let plan = ready(db.prepare_source_local_plan(&discovery, &[&view], &body));
                let source = plan.context().source();
                let literal = source
                    .index()
                    .nodes()
                    .iter()
                    .find(|node| {
                        node.role == NodeRole::Expression(ExpressionKind::Literal)
                            && source
                                .parsed()
                                .entered_node_syntax(source.body(), source.index(), node.id)
                                .unwrap()
                                .is_some_and(|syntax| syntax.text().to_string() == "0xffffffff")
                    })
                    .expect("actual reached literal");
                let foo = if local {
                    &plan
                        .context()
                        .facts()
                        .declarations()
                        .find(|row| row.item.def_key.declared_name == "foo")
                        .unwrap()
                        .item
                } else {
                    view.outline()
                        .items
                        .iter()
                        .find(|item| item.def_key.declared_name == "foo")
                        .unwrap()
                };
                let definition = foo.def_key.fingerprint();
                let cold_body = foo.body.as_ref().unwrap().key();
                let local_metadata = |db: &mut Database| {
                    assert!(
                        matches!(db.input(&InputKey::DefinitionHeader(definition)).as_deref(),
                    Some(InputValue::DefinitionHeader(header)) if header == &foo.header)
                    );
                    assert!(
                        matches!(db.input(&InputKey::DefinitionSource(definition)).as_deref(),
                    Some(InputValue::DefinitionSource(parse)) if *parse == source.parsed().key())
                    );
                    assert!(
                        matches!(db.input(&InputKey::DeclarationContext(definition)).as_deref(),
                    Some(InputValue::DeclarationContext(context))
                        if context.scope == foo.scope && context.range == foo.declaration_span)
                    );
                    assert!(
                        matches!(db.input(&InputKey::LocalDeclarationOwner(definition)).as_deref(),
                    Some(InputValue::LocalDeclarationOwner(owner)) if owner == &body)
                    );
                    assert!(
                        db.input(&InputKey::DefinitionBody(definition)).is_none(),
                        "local free Fn body locator was prepared before call"
                    );
                    [
                        InputKey::DefinitionHeader(definition),
                        InputKey::DefinitionSource(definition),
                        InputKey::DeclarationContext(definition),
                        InputKey::LocalDeclarationOwner(definition),
                    ]
                    .into_iter()
                    .map(|key| {
                        let (_, stamp) = db
                            .published_input(&key)
                            .expect("actual local metadata input");
                        (key, stamp)
                    })
                    .collect::<Vec<_>>()
                };
                let metadata = local.then(|| local_metadata(&mut db));
                let input_revision = db.revision();
                assert!(
                    db.input(&InputKey::BodyEnvironment(cold_body.clone()))
                        .is_none()
                );
                assert_eq!(
                    db.trace.truncated, 0,
                    "initial cold Type proof trace incomplete"
                );
                assert!(!db.trace.events.iter().any(|event| match &event.key {
                    QueryKey::BodyStructure(body) | QueryKey::SourceLocalPlan(body) =>
                        body.definition == definition,
                    QueryKey::Thunk(pc) | QueryKey::PatternPlan(pc) =>
                        pc.body.definition == definition,
                    _ => false,
                }));
                // Scope the diagnostic trace to this metadata demand. The complete
                // trace above also covers preparation before that demand.
                db.trace.events.clear();
                db.trace.truncated = 0;
                db.trace.detailed = true;
                let selected = ready(db.fetch(
                    &SyntaxProvider,
                    QueryKey::BindingConstraints(
                        source.index().pc(literal.id, Phase::Entry).unwrap(),
                    ),
                ));
                let QueryValue::BindingConstraints(goal) = selected.value.as_ref() else {
                    panic!("binding constraint variant")
                };
                if goal.state() != BindingConstraintState::Resolved(PrimitiveType::U32) {
                    // Failure-only source observations: the same actual retained
                    // If subtree, selected Value occurrences and immutable facts.
                    // Nothing lowers or executes the cold function's body.
                    let facts =
                        ready(db.fetch(&SyntaxProvider, QueryKey::SourceConstraints(body.clone())));
                    let QueryValue::SourceConstraints(facts) = facts.value.as_ref() else {
                        panic!()
                    };
                    let operation = source
                        .index()
                        .nodes()
                        .iter()
                        .filter(|node| {
                            node.role == NodeRole::Expression(ExpressionKind::If)
                                && node.range.contains(literal.range)
                        })
                        .min_by_key(|node| node.range.len())
                        .map(|node| node.range)
                        .expect("actual containing If");
                    eprintln!(
                        "cold source goal local={local}, edition={edition}, route={route}, branch={branch}, actual={:?}, body={body:?}, parsed={:?}, operation={operation:?}",
                        goal.state(),
                        source.parsed().key()
                    );
                    for node in source
                        .index()
                        .nodes()
                        .iter()
                        .filter(|node| operation.contains(node.range))
                    {
                        let syntax = source
                            .parsed()
                            .entered_node_syntax(source.body(), source.index(), node.id)
                            .unwrap();
                        eprintln!(
                            "actual node {:?} {:?} range={:?}, syntax={:?}, fact={:?}, uses={:?}",
                            node.id,
                            node.role,
                            node.range,
                            syntax.as_ref().map(|node| node.text().to_string()),
                            facts.fact(node.id),
                            facts.uses(node.id)
                        );
                        if node.role == NodeRole::Expression(ExpressionKind::Path) {
                            match db.fetch(
                                &SyntaxProvider,
                                QueryKey::EnteredValue(
                                    source.index().pc(node.id, Phase::Entry).unwrap(),
                                ),
                            ) {
                                QueryPoll::Ready(Ok(selected)) => {
                                    if let QueryValue::EnteredValue(selected) =
                                        selected.value.as_ref()
                                    {
                                        eprintln!(
                                            "actual Value {:?}, selected={:?}",
                                            selected,
                                            selected.declaration().map(|declaration| (
                                                declaration.identity(),
                                                declaration.source(),
                                                declaration.context(),
                                                declaration.owner()
                                            ))
                                        );
                                    }
                                }
                                other => eprintln!("actual Value unavailable: {other:?}"),
                            }
                        }
                    }
                    eprintln!(
                        "actual selected Fn metadata: {:?}; equation queries: {:?}",
                        db.input(&InputKey::DeclarationContext(definition)),
                        db.trace
                            .events
                            .iter()
                            .filter_map(|event| match &event.key {
                                QueryKey::AssociatedEquations(key)
                                    if key.definition == definition =>
                                    Some(key),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    );
                }
                assert_eq!(
                    goal.state(),
                    BindingConstraintState::Resolved(PrimitiveType::U32),
                    "local={local}, edition {edition}, route {route}, branch {branch}"
                );
                assert!(db.input(&InputKey::BodyEnvironment(cold_body)).is_none());
                assert_eq!(db.trace.truncated, 0, "cold Type proof trace incomplete");
                assert!(!db.trace.events.iter().any(|event| match &event.key {
                    QueryKey::BodyStructure(body) | QueryKey::SourceLocalPlan(body) =>
                        body.definition == definition,
                    QueryKey::Thunk(pc) | QueryKey::PatternPlan(pc) =>
                        pc.body.definition == definition,
                    _ => false,
                }));
                if branch.starts_with("foo") {
                    assert!(db.trace.events.iter().any(|event| matches!(&event.key,
                    QueryKey::AssociatedEquations(key) if key.definition == definition
                        && matches!(&key.owner, EquationOwner::SelectedCall(_)))));
                }
                if let Some(metadata) = metadata {
                    assert_eq!(
                        db.revision(),
                        input_revision,
                        "pure local Type goal published inputs"
                    );
                    assert_eq!(
                        local_metadata(&mut db),
                        metadata,
                        "local metadata changed during the pure Type goal"
                    );
                } else {
                    // Existing module metadata rows are not executable owners.
                    assert!(matches!(
                        db.input(&InputKey::DefinitionBody(definition)).as_deref(),
                        Some(InputValue::DefinitionBody(_))
                    ));
                }
            }
        }
    }
}
