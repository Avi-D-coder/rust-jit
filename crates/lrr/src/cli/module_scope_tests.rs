//! Module scope of function bodies: standard and user macros in the functions
//! of non-root module files, `crate::`/`super::` callee paths, modules written
//! in function bodies (hoisted to the enclosing module under an unspellable
//! segment, `syntax::outline::module_segment`, and bound lexically by an import
//! alias), and `impl`s written in functions that never run. Every shape runs in
//! all four editions through the production pipeline; expected output is the
//! program's output under real rustc (`rustc +1.99.0`).
use super::format_trait_facade_tests::prints;
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
            "lrr-modscope-{label}-{}-{}",
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

/// A crate of several files (the first is the root) run in every edition.
fn prints_files(label: &str, files: &[(&str, &str)], expected: &str) {
    for edition in [2015, 2018, 2021, 2024] {
        let temp = Temp::new(label);
        for (path, text) in files {
            let path = temp.0.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let root = temp.0.join(files[0].0);
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
        let outcome = standalone_captured(
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
        );
        match outcome {
            Ok(StandaloneOutcome::Completed(report)) => {
                assert_eq!(report.output.stdout, expected, "{label} {edition}")
            }
            Ok(StandaloneOutcome::Panicked(report)) => {
                panic!("{label} {edition} panicked: {:?}", report.panic)
            }
            Ok(_) => panic!("{label} {edition} unexpected outcome"),
            Err(error) => panic!("{label} {edition} {error}"),
        }
    }
}

#[test]
fn standard_macros_in_functions_of_out_of_line_module_files() {
    prints_files(
        "file-module-macros",
        &[
            (
                "main.rs",
                "mod foo;\nfn main() { foo::f(); foo::bar::g(); }\n",
            ),
            (
                "foo.rs",
                "pub mod bar;\npub fn f() {\n    assert_eq!(1 + 1, 2);\n    assert!(true, \"x {}\", 1);\n    let v: Vec<i32> = vec![1, 2];\n    println!(\"f {} {}\", v.len(), format!(\"{}-{}\", 3, 4));\n}\n",
            ),
            (
                "foo/bar.rs",
                "pub struct P(pub i32);\nimpl P { pub fn show(&self) -> String { assert_ne!(self.0, 0); format!(\"P{}\", self.0) } }\npub fn g() { println!(\"g {}\", P(7).show()); }\n",
            ),
        ],
        "f 2 3-4\ng P7\n",
    );
}

#[test]
fn macros_in_mod_rs_and_path_attribute_module_files() {
    prints_files(
        "mod-rs-and-path",
        &[
            (
                "main.rs",
                "mod a;\n#[path = \"other/renamed.rs\"]\nmod r;\nmod inl { pub mod deep; }\nfn main() {\n    assert_eq!(a::sub::f(), 10);\n    println!(\"{} {} {}\", a::sub::f(), r::g(), inl::deep::h());\n}\n",
            ),
            ("a/mod.rs", "pub mod sub;\n"),
            (
                "a/sub/mod.rs",
                "pub fn f() -> i32 { let v: Vec<i32> = vec![5, 5]; assert_eq!(v.len(), 2); v[0] + v[1] }\n",
            ),
            (
                "other/renamed.rs",
                "pub fn g() -> String { format!(\"{}\", \"renamed\") }\n",
            ),
            ("inl/deep.rs", "pub fn h() -> i32 { assert_ne!(1, 2); 3 }\n"),
        ],
        "10 renamed 3\n",
    );
}

#[test]
fn user_macros_cross_module_files_with_macro_use() {
    prints_files(
        "macro-use-files",
        &[
            (
                "main.rs",
                "#[macro_use]\nmod macros;\nmod user;\nfn main() { assert_eq!(double!(4), 8); println!(\"{}\", user::go()); }\n",
            ),
            (
                "macros.rs",
                "macro_rules! double { ($x:expr) => { $x * 2 } }\n",
            ),
            (
                "user.rs",
                "pub fn go() -> i32 { assert!(true); double!(5) + 2 }\n",
            ),
        ],
        "12\n",
    );
}

#[test]
fn crate_and_super_rooted_callee_paths() {
    prints(
        "crate-super-callee",
        "fn top() -> i32 { 100 }
mod outer {
    pub fn f() -> i32 { 1 }
    pub mod inner {
        pub fn g() -> i32 { super::f() + crate::top() }
        pub fn h() -> i32 { self::g() + super::super::top() }
    }
}
fn main() {
    println!(\"{} {} {}\", crate::top(), outer::inner::g(), outer::inner::h());
    assert_eq!(crate::top(), 100);
}
",
        "100 101 201\n",
    );
}

#[test]
fn block_module_functions_types_and_nested_modules() {
    prints(
        "block-mod-basic",
        "fn main() {
    mod m {
        pub fn f() -> i32 { 1 }
        pub enum E { A, B(i32) }
        pub struct S { pub a: i32 }
        pub struct T(pub i32);
        pub const C: i32 = 5;
        pub mod inner {
            pub fn g() -> i32 { super::f() + 1 }
            pub fn h() -> i32 { self::g() + crate::top() }
        }
        pub fn pc() -> i32 { 3 }
        pub(crate) fn pcr() -> i32 { 4 }
        pub(super) fn ps() -> i32 { 5 }
    }
    println!(\"{} {} {}\", m::f(), m::inner::g(), m::inner::h());
    println!(\"{} {} {}\", m::pc(), m::pcr(), m::ps());
    match m::E::B(4) { m::E::B(x) => println!(\"B{}\", x), m::E::A => println!(\"A\") }
    let s = m::S { a: 2 };
    let t = m::T(3);
    println!(\"{} {} {}\", s.a, t.0, m::C);
}
fn top() -> i32 { 100 }
",
        "1 2 102\n3 4 5\nB4\n2 3 5\n",
    );
}

#[test]
fn block_module_impls_traits_use_and_derives() {
    prints(
        "block-mod-items",
        "fn main() {
    mod m {
        use std::collections::BTreeMap;
        #[derive(Debug, Clone, PartialEq)]
        pub struct S { pub a: i32 }
        impl S {
            pub fn new() -> S { S { a: 10 } }
            pub fn get(&self) -> i32 { self.a }
        }
        pub trait Tr { fn t(&self) -> i32; }
        impl Tr for S { fn t(&self) -> i32 { self.a + 1 } }
        pub fn count() -> usize { let mut m = BTreeMap::new(); m.insert(1, 2); m.len() }
    }
    use m::Tr;
    let s = m::S::new();
    println!(\"{} {} {}\", s.get(), s.t(), m::count());
    println!(\"{:?} {}\", s.clone(), s.clone() == s);
}
",
        "10 11 1\nS { a: 10 } true\n",
    );
}

#[test]
fn same_named_block_modules_in_different_bodies_are_distinct() {
    prints(
        "block-mod-distinct",
        "struct W;
impl W { fn go(&self) -> i32 { mod m { pub fn f() -> i32 { 3 } } m::f() } }
trait T { fn d(&self) -> i32 { mod m { pub fn f() -> i32 { 4 } } m::f() } }
impl T for W {}
fn one() -> i32 { mod m { pub fn f() -> i32 { 1 } } m::f() }
fn two() -> i32 { mod m { pub fn f() -> i32 { 2 } } m::f() }
fn nested() -> i32 {
    mod a {
        pub fn f() -> i32 {
            mod b { pub fn g() -> i32 { 50 } }
            b::g() + 1
        }
    }
    a::f()
}
fn main() {
    let c = || { mod q { pub fn z() -> i32 { 9 } } q::z() };
    println!(\"{} {} {} {} {} {}\", one(), two(), W.go(), W.d(), nested(), c());
}
",
        "1 2 3 4 51 9\n",
    );
}

#[test]
fn block_modules_in_module_files_resolve_super_to_the_enclosing_module() {
    prints_files(
        "block-mod-file",
        &[
            (
                "main.rs",
                "mod foo;\nfn run() -> i32 { mod m { pub fn f() -> i32 { assert_eq!(1, 1); 1 } } m::f() }\nfn main() { println!(\"{} {} {}\", run(), foo::go(), foo::bar::go()); }\n",
            ),
            (
                "foo.rs",
                "pub mod bar;\npub fn go() -> i32 { mod m { pub fn f() -> i32 { assert!(true); 2 } } m::f() }\n",
            ),
            (
                "foo/bar.rs",
                "pub fn go() -> i32 { mod m { pub fn f() -> i32 { assert_ne!(1, 2); super::go2() + 1 } } m::f() }\nfn go2() -> i32 { 2 }\n",
            ),
        ],
        "1 2 3\n",
    );
}

#[test]
fn impl_written_in_a_function_that_never_runs_is_still_selected() {
    prints(
        "impl-in-cold-fn",
        "struct S;
mod m { fn f() { impl crate::S { pub fn s(&self) -> i32 { 5 } } } }
fn g() { impl S { pub fn t(&self) -> i32 { 6 } } }
fn main() { println!(\"{} {}\", S.s(), S.t()); }
",
        "5 6\n",
    );
}
