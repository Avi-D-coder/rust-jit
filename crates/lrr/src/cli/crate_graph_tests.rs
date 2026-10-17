//! Standalone dependency crates through the ordinary capture/discovery/run
//! pipeline. Guest programs divide by zero exactly when the cross-crate value
//! is the expected one, so the panic kind proves the executed result.
use super::{ExternArg, ExternCrates, capture, capture_with, parse_extern, parse_extern_edition};
use crate::{
    cli::{StandaloneOutcome, runtime_capture::Request, standalone_captured},
    core::profile::ProfileIdentity,
    source::embedded_crates::{EmbeddedCrate, EmbeddedFile},
    vm::numeric::PanicKind,
};
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new(label: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lrr-crate-graph-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn externs(rows: &[(&str, &Path, bool, Option<u16>)]) -> ExternCrates {
    ExternCrates::new(
        rows.iter()
            .map(|(name, path, prelude, _)| ExternArg {
                name: (*name).into(),
                path: path.to_path_buf(),
                prelude: *prelude,
                proc_macro: false,
            })
            .collect(),
        rows.iter()
            .filter_map(|(name, _, _, edition)| edition.map(|e| ((*name).into(), e)))
            .collect(),
    )
    .unwrap()
}

fn run(
    world: super::CapturedWorld,
    path: &Path,
) -> Result<StandaloneOutcome, crate::core::diagnostic::Diagnostic> {
    let (prepared, view, sources, mut snapshot) = world;
    standalone_captured(
        path,
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

fn proves(outcome: Result<StandaloneOutcome, crate::core::diagnostic::Diagnostic>) {
    match outcome {
        Ok(StandaloneOutcome::Panicked(report)) => {
            assert_eq!(report.panic.kind, PanicKind::DivisionByZero)
        }
        Ok(StandaloneOutcome::Completed(_)) => panic!("cross-crate value differs"),
        Ok(_) => panic!("unexpected outcome"),
        Err(error) => panic!("{error}"),
    }
}

const CHECK: &str =
    "fn check(v: i32, e: i32) { let _ = 1_i32 / (if v == e { 0_i32 } else { 1_i32 }); }\n";

#[test]
fn extern_argument_shapes_follow_rustc() {
    assert_eq!(
        parse_extern("dep=aux/dep.rs").unwrap(),
        ExternArg {
            name: "dep".into(),
            path: "aux/dep.rs".into(),
            prelude: true,
            proc_macro: false,
        }
    );
    assert!(!parse_extern("noprelude:dep=aux/dep.rs").unwrap().prelude);
    let procedural = parse_extern("noprelude,proc-macro:dep=aux/dep.rs").unwrap();
    assert!(procedural.proc_macro && !procedural.prelude);
    assert!(parse_extern("priv,noprelude:dep=x.rs").is_ok());
    assert!(parse_extern("bogus:dep=x.rs").is_err());
    assert!(parse_extern("dep").is_err());
    assert!(parse_extern("1dep=x.rs").is_err());
    assert_eq!(
        parse_extern_edition("dep=2018").unwrap(),
        ("dep".into(), 2018)
    );
    assert!(ExternCrates::new(vec![], vec![("missing".into(), 2018)]).is_err());
}

#[test]
fn extern_crate_call_executes_dependency_function_with_its_edition() {
    let temp = Temp::new("call");
    let dependency = temp.write(
        "auxiliary/dep.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n\
         pub fn twice(a: i32) -> i32 { add(a, a) }\n",
    );
    let root = temp.write(
        "main.rs",
        &format!(
            "extern crate dep;\n{CHECK}fn main() {{ check(dep::twice(dep::add(1_i32, 2_i32)), 6_i32); }}\n"
        ),
    );
    let set = externs(&[("dep", &dependency, false, Some(2018))]);
    let world = capture(&root, false, 2015, &set).unwrap().0;
    // Root crate and dependency each own one captured module, in their own editions.
    let editions = world
        .2
        .iter()
        .map(|view| {
            (
                view.selection().outline.crate_key,
                view.outline().profile.edition,
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(editions.len(), 2);
    assert!(editions.iter().any(|(_, edition)| *edition == 2018));
    let dependencies = world.1.source_dependencies().unwrap();
    assert_eq!(dependencies.rows().count(), 1);
    proves(run(world, &root));
    // Control: a wrong expectation completes without the division.
    let wrong = temp.write(
        "wrong.rs",
        &format!(
            "extern crate dep;\n{CHECK}fn main() {{ check(dep::add(1_i32, 2_i32), 4_i32); }}\n"
        ),
    );
    match run(capture(&wrong, false, 2015, &set).unwrap().0, &wrong) {
        Ok(StandaloneOutcome::Completed(_)) => {}
        Ok(_) => panic!("control did not complete"),
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn prelude_extern_and_dependency_module_tree_resolve_in_2018() {
    let temp = Temp::new("prelude");
    let dependency = temp.write(
        "auxiliary/lib/lib.rs",
        "mod helpers;\npub use helpers::scale;\npub mod inner { pub fn sub(a: i32, b: i32) -> i32 { a - b } }\n\
         #[macro_export]\nmacro_rules! bump { ($e:expr) => { $e + 1_i32 } }\n",
    );
    temp.write(
        "auxiliary/lib/helpers.rs",
        "pub fn scale(a: i32) -> i32 { a * 10_i32 }\n",
    );
    let root = temp.write(
        "main.rs",
        &format!(
            "use lib::scale;\n{CHECK}fn main() {{ check(lib::inner::sub(scale(2_i32), lib::bump!(1_i32)), 18_i32); }}\n"
        ),
    );
    let set = externs(&[("lib", &dependency, true, None)]);
    let world = capture(&root, false, 2021, &set).unwrap().0;
    // Both dependency module files are captured under the dependency's unit.
    assert_eq!(world.2.len(), 3);
    proves(run(world, &root));
    // `noprelude:` leaves the name to `extern crate` only.
    let hidden = externs(&[("lib", &dependency, false, None)]);
    let world = capture(&root, false, 2021, &hidden).unwrap().0;
    assert_eq!(world.1.source_dependencies().unwrap().rows().count(), 0);
    assert!(run(world, &root).is_err());
}

#[test]
fn macro_use_extern_crate_imports_dependency_macros() {
    let temp = Temp::new("macro-use");
    let dependency = temp.write(
        "auxiliary/mac.rs",
        "#[macro_export]\nmacro_rules! triple { ($e:expr) => { $e * 3_i32 } }\n",
    );
    let root = temp.write(
        "main.rs",
        &format!(
            "#[macro_use]\nextern crate mac;\n{CHECK}fn main() {{ check(triple!(2_i32), 6_i32); }}\n"
        ),
    );
    let set = externs(&[("mac", &dependency, false, Some(2015))]);
    proves(run(capture(&root, false, 2015, &set).unwrap().0, &root));
}

#[test]
fn embedded_crate_is_captured_with_its_unit_and_reachable_by_extern_crate() {
    let temp = Temp::new("embedded");
    let profile = ProfileIdentity {
        edition: 2018,
        ..ProfileIdentity::current()
    };
    let unit = crate::core::digest::Digest::of_bytes(b"lrr test embedded crate");
    let embedded = EmbeddedCrate {
        name: "embedded_helpers",
        unit,
        edition: 2021,
        files: vec![
            EmbeddedFile {
                module_path: vec![],
                path: "embedded_helpers/lib.rs",
                text: "pub mod num;\n",
            },
            EmbeddedFile {
                module_path: vec!["num".into()],
                path: "embedded_helpers/num.rs",
                text: "pub fn double(a: i32) -> i32 { a * 2_i32 }\n",
            },
        ],
    };
    let root = temp.write(
        "main.rs",
        &format!(
            "extern crate embedded_helpers;\n{CHECK}fn main() {{ check(embedded_helpers::num::double(4_i32), 8_i32); }}\n"
        ),
    );
    let world = capture_with(
        &root,
        false,
        profile.edition,
        &ExternCrates::default(),
        vec![embedded],
        None,
    )
    .unwrap()
    .0;
    assert!(world.2.iter().any(|view| {
        view.selection().outline.crate_key == unit && view.outline().profile.edition == 2021
    }));
    proves(run(world, &root));
}

#[test]
fn embedded_standard_facade_units_resolve_implicit_std_and_core_paths() {
    let temp = Temp::new("embedded-std");
    let profile = ProfileIdentity {
        edition: 2021,
        ..ProfileIdentity::current()
    };
    let core = EmbeddedCrate {
        name: "core",
        unit: crate::facade::unit(&profile, "core"),
        edition: 2021,
        files: vec![EmbeddedFile {
            module_path: vec![],
            path: "core/lib.rs",
            text: "#![no_std]\npub mod lrr_probe { pub fn triple(a: i32) -> i32 { a * 3_i32 } }\n",
        }],
    };
    let std = EmbeddedCrate {
        name: "std",
        unit: crate::facade::unit(&profile, "std"),
        edition: 2021,
        files: vec![EmbeddedFile {
            module_path: vec![],
            path: "std/lib.rs",
            text: "#![no_std]\npub mod lrr_probe { pub fn double(a: i32) -> i32 { a * 2_i32 } }\n",
        }],
    };
    let root = temp.write(
        "main.rs",
        &format!(
            "use std::lrr_probe::double;\n{CHECK}fn main() {{ check(double(core::lrr_probe::triple(1_i32)), 6_i32); }}\n"
        ),
    );
    let world = capture_with(
        &root,
        false,
        profile.edition,
        &ExternCrates::default(),
        vec![core, std],
        None,
    )
    .unwrap()
    .0;
    proves(run(world, &root));
}

#[test]
fn dependency_fault_renders_in_the_dependency_source() {
    let temp = Temp::new("render");
    let dependency = temp.write(
        "auxiliary/h.rs",
        "\n\npub fn f() -> i32 { let v = [1_i32; 3]; v[0] }\n",
    );
    let root = temp.write(
        "main.rs",
        "extern crate h;\nfn main() { let _x = h::f(); }\n",
    );
    let set = externs(&[("h", &dependency, false, None)]);
    let error =
        match crate::cli::standalone_edition_with_capture(&root, None, 2015, &set, false, None) {
            Err(error) => error,
            Ok(_) => panic!("unsupported dependency operation completed"),
        };
    assert!(error.to_string().contains("h.rs:3"), "{error}");
}

/// The LRR-provided standard facade crates, explicitly (the default world
/// enables them only with `LRR_EMBEDDED_FACADE=1` while they are staged).
fn facade_world(root: &Path, edition: u16) -> super::CapturedWorld {
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    capture_with(
        root,
        false,
        edition,
        &ExternCrates::default(),
        crate::source::embedded_crates::standard_facade_crates(&profile),
        None,
    )
    .unwrap()
    .0
}

fn fails_with(
    outcome: Result<StandaloneOutcome, crate::core::diagnostic::Diagnostic>,
    code: &str,
) -> crate::core::diagnostic::Diagnostic {
    match outcome {
        Err(error) => {
            assert_eq!(error.code, code, "{error}");
            error
        }
        Ok(_) => panic!("expected {code}"),
    }
}

#[test]
fn standard_prelude_supplies_option_in_every_edition() {
    for edition in [2015, 2018, 2021, 2024] {
        let temp = Temp::new("facade-prelude");
        let root = temp.write(
            "main.rs",
            &format!(
                "{CHECK}fn main() {{ let s = Some(3_i32); if let Some(x) = s {{ check(x, 3_i32); }} }}\n"
            ),
        );
        proves(run(facade_world(&root, edition), &root));
    }
}

#[test]
fn standard_prelude_types_resolve_in_annotations_and_impl_paths() {
    let temp = Temp::new("facade-types");
    let root = temp.write(
        "main.rs",
        &format!(
            "use std::fmt;\nstruct P;\nimpl fmt::Display for P {{ fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {{ Ok(()) }} }}\nuse std::collections::HashMap;\n{CHECK}fn main() {{ let o: Option<i32> = Some(2_i32); if let Some(v) = o {{ check(v, 2_i32); }} }}\n"
        ),
    );
    proves(run(facade_world(&root, 2021), &root));
}

#[test]
fn module_items_shadow_the_standard_prelude() {
    let temp = Temp::new("facade-shadow");
    let root = temp.write(
        "main.rs",
        &format!("#[allow(non_snake_case)]\nfn Some(v: i32) -> i32 {{ v * 2_i32 }}\n{CHECK}fn main() {{ check(Some(2_i32), 4_i32); }}\n"),
    );
    proves(run(facade_world(&root, 2021), &root));
}

#[test]
fn no_implicit_prelude_removes_the_standard_prelude() {
    let temp = Temp::new("facade-no-prelude");
    let root = temp.write(
        "main.rs",
        "#![no_implicit_prelude]\nfn main() { let _s = Some(3_i32); }\n",
    );
    fails_with(run(facade_world(&root, 2021), &root), "LRR-E-NAME-MISSING");
}

#[test]
fn unexecutable_facade_items_fault_naming_the_item() {
    let temp = Temp::new("facade-unsupported");
    let root = temp.write("main.rs", "fn main() { std::process::exit(0); }\n");
    let error = fails_with(run(facade_world(&root, 2015), &root), "LRR-E-FACADE-MISSING");
    assert!(error.message.contains("std::process::exit"), "{error}");
}

#[test]
fn imported_types_resolve_in_local_annotations() {
    let temp = Temp::new("imported-annotation");
    let root = temp.write(
        "main.rs",
        &format!(
            "mod m {{ pub struct P {{ pub x: i32 }} }}\nuse m::P;\n{CHECK}fn main() {{ let p: P = P {{ x: 5_i32 }}; check(p.x, 5_i32); }}\n"
        ),
    );
    proves(run(
        capture_with(&root, false, 2021, &ExternCrates::default(), vec![], None)
            .unwrap()
            .0,
        &root,
    ));
}

#[test]
fn crate_root_extern_crate_rename_joins_the_extern_prelude() {
    let temp = Temp::new("extern-rename");
    let dependency = temp.write("auxiliary/dep.rs", "pub fn value() -> i32 { 7_i32 }\n");
    let root = temp.write(
        "main.rs",
        &format!(
            "extern crate dep as renamed;\nmod inner {{ pub fn get() -> i32 {{ renamed::value() }} }}\n{CHECK}fn main() {{ check(inner::get(), 7_i32); }}\n"
        ),
    );
    let set = externs(&[("dep", &dependency, false, Some(2018))]);
    proves(run(
        capture_with(&root, false, 2018, &set, vec![], None)
            .unwrap()
            .0,
        &root,
    ));
}

#[test]
fn chained_and_cyclic_globs_import_their_source_members() {
    let temp = Temp::new("globs");
    let root = temp.write(
        "main.rs",
        &format!(
            "mod a {{ pub fn f() -> i32 {{ 1_i32 }} pub mod b {{ pub fn g() -> i32 {{ 2_i32 }} }} pub use self::b::*; pub use super::c::*; }}\nmod c {{ pub use super::a::*; pub fn h() -> i32 {{ 4_i32 }} }}\nuse c::*;\n{CHECK}fn main() {{ check(f() + g() + h(), 7_i32); }}\n"
        ),
    );
    proves(run(
        capture_with(&root, false, 2021, &ExternCrates::default(), vec![], None)
            .unwrap()
            .0,
        &root,
    ));
}
