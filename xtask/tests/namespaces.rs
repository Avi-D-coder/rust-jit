use lrr::{
    core::digest::{Digest, StableEncode},
    frontend::names::{NameDefinition, NameLookup, NameNamespaces},
    source::manifest::Namespace,
    syntax::{outline::ScopeKey, visibility::VisibilityScope},
};
fn unit() -> Digest {
    Digest([1; 32])
}
fn caller() -> ScopeKey {
    ScopeKey {
        crate_key: unit(),
        module_path: vec![],
        owner: None,
    }
}
fn path(parts: &[&str]) -> lrr::frontend::names::NamePath {
    NameNamespaces::path(unit(), parts, 2024).unwrap()
}
fn definition(namespace: Namespace, id: u8) -> NameDefinition {
    NameDefinition {
        namespace,
        identity: Digest([id; 32]),
        defining_unit: unit(),
        definition_edition: 2024,
    }
}
fn graph() -> NameNamespaces {
    let mut g = NameNamespaces::default();
    g.declare_module(unit(), &[], 2024, VisibilityScope::Public)
        .unwrap();
    g
}
#[test]
fn namespace_tables_preserve_equal_names_and_named_over_glob_precedence() {
    let mut g = graph();
    for module in ["left", "right"] {
        g.declare_module(unit(), &[module], 2024, VisibilityScope::Public)
            .unwrap();
    }
    for namespace in [Namespace::Value, Namespace::Type, Namespace::Macro] {
        g.define(
            path(&["Thing"]),
            definition(namespace, namespace.tag() as u8 + 10),
            VisibilityScope::Public,
        )
        .unwrap();
        assert!(
            matches!(g.lookup(namespace,&path(&["Thing"]),&caller()).unwrap(),NameLookup::Defined(d) if d.namespace==namespace && d.identity==Digest([namespace.tag() as u8+10;32]))
        );
    }
    g.define(
        path(&["left", "value"]),
        definition(Namespace::Value, 20),
        VisibilityScope::Public,
    )
    .unwrap();
    g.define(
        path(&["right", "value"]),
        definition(Namespace::Value, 21),
        VisibilityScope::Public,
    )
    .unwrap();
    g.glob_member(
        Namespace::Value,
        path(&["value"]),
        path(&["left", "value"]),
        VisibilityScope::Public,
    )
    .unwrap();
    g.glob_member(
        Namespace::Value,
        path(&["value"]),
        path(&["right", "value"]),
        VisibilityScope::Public,
    )
    .unwrap();
    assert!(
        matches!(g.lookup(Namespace::Value,&path(&["value"]),&caller()).unwrap(),NameLookup::Ambiguous(ds) if ds.len()==2)
    );
    g.named_import(
        Namespace::Value,
        path(&["value"]),
        path(&["left", "value"]),
        VisibilityScope::Public,
    )
    .unwrap();
    assert!(
        matches!(g.lookup(Namespace::Value,&path(&["value"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==Digest([20;32]))
    );
    assert_eq!(
        g.lookup(Namespace::Type, &path(&["value"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
    assert_ne!(
        definition(Namespace::Value, 1).fingerprint(),
        definition(Namespace::Type, 1).fingerprint()
    );
}
#[test]
fn namespace_import_cycles_privacy_and_macro_blockers_remain_distinct() {
    let mut g = graph();
    g.named_import(
        Namespace::Type,
        path(&["a"]),
        path(&["b"]),
        VisibilityScope::Public,
    )
    .unwrap();
    g.named_import(
        Namespace::Type,
        path(&["b"]),
        path(&["a"]),
        VisibilityScope::Public,
    )
    .unwrap();
    assert_eq!(
        g.lookup(Namespace::Type, &path(&["a"]), &caller()).unwrap(),
        NameLookup::Absent
    );
    g.pending(Namespace::Type, path(&["b"]), Digest([30; 32]))
        .unwrap();
    assert_eq!(
        g.lookup(Namespace::Type, &path(&["a"]), &caller()).unwrap(),
        NameLookup::NeedInput(Digest([30; 32]))
    );
    g.declare_module(unit(), &["inner"], 2024, VisibilityScope::Public)
        .unwrap();
    g.define(
        path(&["inner", "hidden"]),
        definition(Namespace::Value, 31),
        VisibilityScope::Within {
            unit: unit(),
            module_path: vec!["inner".into()],
        },
    )
    .unwrap();
    assert!(matches!(
        g.lookup(Namespace::Value, &path(&["inner", "hidden"]), &caller())
            .unwrap(),
        NameLookup::Inaccessible(_)
    ));
    let inside = ScopeKey {
        module_path: vec!["inner".into()],
        ..caller()
    };
    assert!(matches!(
        g.lookup(Namespace::Value, &path(&["inner", "hidden"]), &inside)
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert_eq!(
        g.lookup(Namespace::Value, &path(&["missing"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
}
#[test]
fn actual_native_namespace_separation_and_import_precedence_agree() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-names-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    let source = "mod left{pub fn value()->i64{3}} mod right{pub fn value()->i64{5}} use left::*;use right::*;use left::value;type Thing=i64;#[allow(non_snake_case)]fn Thing()->Thing{7}macro_rules! Thing{()=>{9}}fn main(){println!(\"{}:{}:{}\",value(),Thing(),Thing!());}";
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3:7:9");
    namespace_tables_preserve_equal_names_and_named_over_glob_precedence();
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"scope":"owned namespace separation/strong import component and standalone native oracle; no LRR execution/source import integration claim"})).unwrap()).unwrap();
}

fn outline(source: &str) -> lrr::syntax::outline::Outline {
    outline_in(source, unit())
}
fn outline_in(source: &str, source_unit: Digest) -> lrr::syntax::outline::Outline {
    let blob = lrr::source::snapshot::SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    lrr::syntax::parse::ParsedFile::parse(&blob, lrr::core::profile::ProfileIdentity::current())
        .unwrap()
        .outline(
            source_unit,
            vec![],
            &lrr::source::snapshot::CfgSet::guest_default(),
        )
        .unwrap()
}
#[test]
fn source_namespace_views_use_cfg_membership_constructors_and_cold_headers() {
    let source = outline(
        "fn enabled()->i64{3} #[cfg(any())] fn disabled(){missing();} fn cold(){undefined_type_error();} struct Tuple(i64); mod child {pub fn inside()->i64{5}} ",
    );
    let mut missing_root = source.clone();
    missing_root.root_scope.module_path.push("child".into());
    assert!(NameNamespaces::from_outlines(&[&missing_root]).is_err());
    let graph = NameNamespaces::from_outlines(&[&source]).unwrap();
    for name in ["enabled", "cold"] {
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&[name]), &caller())
                .unwrap(),
            NameLookup::Defined(_)
        ));
    }
    assert_eq!(
        graph
            .lookup(Namespace::Value, &path(&["disabled"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
    let value = graph
        .lookup(Namespace::Value, &path(&["Tuple"]), &caller())
        .unwrap();
    let ty = graph
        .lookup(Namespace::Type, &path(&["Tuple"]), &caller())
        .unwrap();
    assert!(
        matches!((value,ty),(NameLookup::Defined(v),NameLookup::Defined(t)) if v.identity==t.identity && v.namespace!=t.namespace)
    );
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["child", "inside"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    let mut forged = source.clone();
    forged
        .scopes
        .get_mut(&caller())
        .unwrap()
        .candidates
        .insert((Namespace::Value, "forged".into()), vec![Digest([40; 32])]);
    assert!(NameNamespaces::from_outlines(&[&forged]).is_err());
}
#[test]
fn source_namespace_frontiers_block_missing_and_disclosed_names_without_affecting_siblings() {
    let source =
        outline("mod waiting { produce!(); pub fn known(){} } mod complete {pub fn known(){}} ");
    let graph = NameNamespaces::from_outlines(&[&source]).unwrap();
    for name in ["known", "missing"] {
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&["waiting", name]), &caller())
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["complete", "known"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    let imports = outline("use other::value; fn known(){} ");
    let graph = NameNamespaces::from_outlines(&[&imports]).unwrap();
    assert_eq!(
        graph
            .lookup(Namespace::Value, &path(&["known"]), &caller())
            .unwrap(),
        NameLookup::NeedInput(NameNamespaces::import_identity(&imports.imports[0]))
    );
}
#[test]
fn actual_native_cfg_selected_source_names_match_namespace_views() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-source-names-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    let source = "fn enabled()->i64{3} #[cfg(any())] fn disabled()->i64{99} mod child{pub fn inside()->i64{5}} fn main(){println!(\"{}:{}\",enabled(),child::inside());}";
    let facts = outline(source);
    let graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["enabled"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["child", "inside"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert_eq!(
        graph
            .lookup(Namespace::Value, &path(&["disabled"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(work.join("oracle")).output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert_eq!(stdout.trim(), "3:5");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"scope":"pinned cfg-selected declaration namespace view and valid native source; no LRR execution or complete import integration"})).unwrap()).unwrap();
}

#[test]
fn source_named_imports_resolve_alias_chains_and_preserve_namespace_identity() {
    let facts = outline(
        "mod source {pub fn f()->i64{3} pub type Num=i64;} pub use crate::source::{f as selected,Num as Number}; pub use selected as reexported;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["selected"]), &caller())
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
    let report = graph.resolve_named_imports().unwrap();
    assert_eq!(report.resolved.len(), 3);
    assert!(
        report.pending.is_empty()
            && report.missing.is_empty()
            && report.inaccessible.is_empty()
            && report.ambiguous.is_empty()
    );
    let original = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    for name in ["selected", "reexported"] {
        let found = graph
            .lookup(Namespace::Value, &path(&[name]), &caller())
            .unwrap();
        assert!(
            matches!(&found,NameLookup::Defined(d) if d.identity==original),
            "{name}: {found:?}"
        );
    }
    assert!(matches!(
        graph
            .lookup(Namespace::Type, &path(&["Number"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert_eq!(
        graph
            .lookup(Namespace::Value, &path(&["Number"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
    assert_eq!(graph.resolve_named_imports().unwrap().resolved.len(), 3);
}
#[test]
fn source_named_import_reports_missing_private_and_blocked_targets_without_false_completion() {
    for (source, kind) in [
        ("use a as b;use b as a;", "missing"),
        (
            "mod m {fn hidden(){}} use crate::m::hidden as x;",
            "private",
        ),
        ("mod m {make!();} use crate::m::unknown as x;", "pending"),
    ] {
        let facts = outline(source);
        let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
        let report = graph.resolve_named_imports().unwrap();
        assert!(report.resolved.is_empty());
        match kind {
            "missing" => assert_eq!(report.missing.len(), 2),
            "private" => assert_eq!(report.inaccessible.len(), 1),
            "pending" => assert_eq!(report.pending.len(), 1),
            _ => unreachable!(),
        }
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&["x"]), &caller())
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
    {
        let source = "mod m{pub fn f(){}}use crate::m::*;use f as x;";
        let facts = outline(source);
        let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
        let report = graph.resolve_named_imports().unwrap();
        assert_eq!(report.pending.len(), 2, "{source}");
        assert!(report.resolved.is_empty() && report.missing.is_empty());
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&["x"]), &caller())
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
    let invalid = outline(
        "mod source{pub fn f()->i64{3}}use crate::source::f as selected;pub use selected as reexported;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&invalid]).unwrap();
    let report = graph.resolve_named_imports().unwrap();
    assert_eq!(report.resolved.len(), 1);
    assert_eq!(report.inaccessible.len(), 1);
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["reexported"]), &caller())
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
}
#[test]
fn actual_native_source_named_aliases_match_resolved_definition_identity() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-named-imports-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    let source = "mod source{pub fn f()->i64{3}pub type Num=i64;}pub use crate::source::{f as selected,Num as Number};pub use selected as reexported;fn main(){let n:Number=reexported();println!(\"{}\",n);}";
    let facts = outline(source);
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_named_imports().unwrap();
    assert_eq!(report.resolved.len(), 3);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["reexported"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==facts.items.iter().find(|i|i.def_key.declared_name=="f").unwrap().def_key.fingerprint())
    );
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(work.join("oracle")).output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"resolved_imports":report.resolved.len(),"scope":"ordinary non-module named import source identity and valid native comparison; no LRR execution or complete glob/module import claim"})).unwrap()).unwrap();
}

#[test]
fn source_module_aliases_resolve_chains_and_preserve_member_frontiers() {
    let facts =
        outline("mod m{pub fn f(){}}use second as first;use crate::m as second;use first::f as x;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_named_imports().unwrap();
    assert_eq!(report.resolved.len(), 3);
    assert!(
        report.pending.is_empty() && report.missing.is_empty() && report.inaccessible.is_empty()
    );
    let original = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    for name in [path(&["first", "f"]), path(&["second", "f"]), path(&["x"])] {
        assert!(
            matches!(graph.lookup(Namespace::Value,&name,&caller()).unwrap(),NameLookup::Defined(d) if d.identity==original)
        );
    }
    assert_eq!(graph.resolve_named_imports().unwrap().resolved.len(), 3);
    let facts = outline("mod m{make!();pub fn f(){}}use crate::m as alias;use alias::f as x;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_named_imports().unwrap();
    assert_eq!(report.resolved.len(), 1);
    assert_eq!(report.pending.len(), 1);
    for name in ["f", "missing"] {
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&["alias", name]), &caller())
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
    let facts = outline("pub mod m{pub fn f(){}}use crate::m as alias;pub use alias as exported;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_named_imports().unwrap();
    assert_eq!(report.resolved.len(), 1);
    assert_eq!(report.inaccessible.len(), 1);
}

#[test]
fn actual_native_source_module_aliases_match_member_identity() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-module-aliases-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    let source = "mod source{pub fn f()->i64{3}}use second as first;use crate::source as second;use first::f as chosen;fn main(){println!(\"{}\",chosen());}";
    let facts = outline(source);
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_named_imports().unwrap();
    assert_eq!(report.resolved.len(), 3);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["chosen"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==facts.items.iter().find(|i|i.def_key.declared_name=="f").unwrap().def_key.fingerprint())
    );
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(work.join("oracle")).output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"resolved_imports":report.resolved.len(),"scope":"ordinary source module alias chain/member identity and valid native comparison; no LRR execution or complete import scheduling claim"})).unwrap()).unwrap();
}

#[test]
fn source_globs_preserve_precedence_ambiguity_cycles_and_open_producers() {
    let facts = outline(
        "mod a{pub fn f(){}}mod b{pub fn f(){}}use crate::a::*;use crate::b::*;use crate::a::f;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 3);
    assert!(report.pending.is_empty());
    let original = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f" && i.scope.module_path == ["a"])
        .unwrap()
        .def_key
        .fingerprint();
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["f"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==original)
    );
    let facts = outline("mod a{pub fn f(){}}mod b{pub fn f(){}}use crate::a::*;use crate::b::*;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 2);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["f"]),&caller()).unwrap(),NameLookup::Ambiguous(d) if d.len()==2)
    );
    let facts = outline(
        "mod a{pub use crate::b::*;pub fn f(){}}mod b{pub use crate::a::*;}use crate::b::*;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 3);
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["f"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert_eq!(
        graph
            .lookup(Namespace::Value, &path(&["missing"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 3);
    let facts = outline("mod a{fn private(){}}use crate::a::*;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 1);
    assert_eq!(
        graph
            .lookup(Namespace::Value, &path(&["private"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
    let facts = outline("mod a{pub fn f(){}}use crate::a as alias;use alias::*;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 2);
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["f"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    let facts = outline("mod a{pub use crate::b::*;}mod b{pub use crate::a::*;}use crate::a::*;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 3);
    assert_eq!(
        graph
            .lookup(Namespace::Value, &path(&["missing"]), &caller())
            .unwrap(),
        NameLookup::Absent
    );
    for (source, pending) in [
        ("make!();use missing::*;", true),
        ("use missing::*;", false),
    ] {
        let facts = outline(source);
        let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
        let report = graph.resolve_imports().unwrap();
        assert!(report.resolved.is_empty());
        assert_eq!(report.pending.len(), usize::from(pending));
        assert_eq!(report.missing.len(), usize::from(!pending));
    }
    let facts = outline("mod a{make!();pub fn f(){}}use crate::a::*;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert!(report.resolved.is_empty());
    assert_eq!(report.pending.len(), 1);
    for name in ["f", "missing"] {
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&[name]), &caller())
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
}

#[test]
fn actual_native_source_glob_cycle_matches_member_identity() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-source-globs-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    let source = "mod a{pub use crate::b::*;pub fn f()->i64{3}}mod b{pub use crate::a::*;}use crate::b::*;fn main(){println!(\"{}\",f());}";
    let facts = outline(source);
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 3);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["f"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==facts.items.iter().find(|i|i.def_key.declared_name=="f").unwrap().def_key.fingerprint())
    );
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(work.join("oracle")).output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"resolved_imports":report.resolved.len(),"scope":"ordinary source glob cycle/member identity and valid native comparison; no LRR execution or full import fixed-point claim"})).unwrap()).unwrap();
}

#[test]
fn source_glob_modules_preserve_qualified_members_precedence_and_ambiguity() {
    let facts =
        outline("mod a{pub mod inner{pub fn f(){}}}use crate::a::*;use inner::f as chosen;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 2);
    assert!(report.pending.is_empty());
    let original = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    for target in [path(&["inner", "f"]), path(&["chosen"])] {
        assert!(
            matches!(graph.lookup(Namespace::Value,&target,&caller()).unwrap(),NameLookup::Defined(d) if d.identity==original)
        );
    }
    let facts = outline(
        "mod a{pub mod inner{pub fn f(){}}}mod b{pub mod inner{pub fn f(){}}}use crate::a::*;use crate::b::*;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 2);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["inner","f"]),&caller()).unwrap(),NameLookup::AmbiguousModules(p) if p.len()==2)
    );
    let facts = outline(
        "mod a{pub mod inner{pub fn f(){}}}mod b{pub mod inner{pub fn f(){}}}use crate::a::*;use crate::b::*;use crate::a::inner;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 3);
    let original = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f" && i.scope.module_path == ["a", "inner"])
        .unwrap()
        .def_key
        .fingerprint();
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["inner","f"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==original)
    );
    let facts = outline("mod a{pub mod inner{make!();pub fn f(){}}}use crate::a::*;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 1);
    for name in ["f", "missing"] {
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&["inner", name]), &caller())
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
}

#[test]
fn source_glob_module_ambiguity_and_shadowed_frontiers_remain_distinct() {
    let facts = outline(
        "mod a{pub mod inner{pub fn f(){}}}mod b{pub mod inner{pub fn f(){}}}use crate::a::*;use crate::b::*;use inner::f as chosen;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.ambiguous.len(), 1);
    assert!(report.resolved.is_empty());
    let facts = outline(
        "mod a{pub mod inner{pub fn f(){}}}mod b{pub mod inner{make!();pub fn f(){}}}use crate::a::*;use crate::b::*;use crate::a::inner;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 3);
    let original = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f" && i.scope.module_path == ["a", "inner"])
        .unwrap()
        .def_key
        .fingerprint();
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["inner","f"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==original)
    );
}

#[test]
fn actual_native_source_glob_module_matches_qualified_member_identity() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-glob-modules-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    let source = "mod a{pub mod inner{pub fn f()->i64{3}}}use crate::a::*;use inner::f as chosen;fn main(){println!(\"{}\",chosen());}";
    let facts = outline(source);
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 2);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["chosen"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==facts.items.iter().find(|i|i.def_key.declared_name=="f").unwrap().def_key.fingerprint())
    );
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(work.join("oracle")).output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"resolved_imports":report.resolved.len(),"scope":"ordinary source glob-module qualified member identity and valid native comparison; no LRR execution or full strong/glob fixed-point claim"})).unwrap()).unwrap();
}

#[test]
fn source_root_aliases_and_dependency_renames_preserve_unit_identity() {
    let facts = outline("fn f(){}use crate as own;use own as second;use second::f as chosen;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 3);
    assert_eq!(
        graph
            .lookup(Namespace::Type, &path(&["own"]), &caller())
            .unwrap(),
        NameLookup::ModuleRoot(unit())
    );
    assert_eq!(
        graph
            .lookup(Namespace::Type, &path(&["second"]), &caller())
            .unwrap(),
        NameLookup::ModuleRoot(unit())
    );
    let original = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["chosen"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==original)
    );
    let invalid = outline("use crate as own;pub use own as exported;");
    let mut graph = NameNamespaces::from_outlines(&[&invalid]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 1);
    assert_eq!(report.inaccessible.len(), 1);
    let provider = Digest([2; 32]);
    let blob =
        lrr::source::snapshot::SourceBlob::new(b"pub fn f(){}fn private(){}".to_vec()).unwrap();
    let p = lrr::syntax::parse::ParsedFile::parse(
        &blob,
        lrr::core::profile::ProfileIdentity::current(),
    )
    .unwrap()
    .outline(
        provider,
        vec![],
        &lrr::source::snapshot::CfgSet::guest_default(),
    )
    .unwrap();
    let c = outline("use ::renamed as root;use root::f as chosen;");
    let mut graph = NameNamespaces::from_outlines(&[&p, &c]).unwrap();
    graph.dependency(unit(), "renamed", provider, 2024).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 2);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["chosen"]),&caller()).unwrap(),NameLookup::Defined(d) if d.defining_unit==provider)
    );
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["root", "private"]), &caller())
            .unwrap(),
        NameLookup::Inaccessible(_)
    ));
    assert!(
        graph
            .dependency(unit(), "renamed", Digest([3; 32]), 2024)
            .is_err()
    );
}

#[test]
fn dependency_prelude_shadowing_and_root_frontiers_remain_source_aware() {
    let provider = Digest([2; 32]);
    let p = outline_in("produce!();pub fn f(){}", provider);
    let c = outline("mod renamed{pub fn f(){}}use renamed::f as chosen;");
    let mut graph = NameNamespaces::from_outlines(&[&p, &c]).unwrap();
    graph.dependency(unit(), "renamed", provider, 2024).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 1);
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["chosen"]),&caller()).unwrap(),NameLookup::Defined(d) if d.defining_unit==unit())
    );
    let c = outline("use ::renamed as root;use root::f as chosen;");
    let mut graph = NameNamespaces::from_outlines(&[&p, &c]).unwrap();
    graph.dependency(unit(), "renamed", provider, 2024).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 1);
    assert_eq!(report.pending.len(), 1);
    for name in ["f", "missing"] {
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&["root", name]), &caller())
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
}
#[test]
fn actual_native_dependency_root_alias_matches_provider_identity() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-root-dependencies-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    let provider = Digest([2; 32]);
    let p = "pub fn f()->i64{3}";
    let c = "use ::renamed as root;use root::f as chosen;fn main(){println!(\"{}\",chosen());}";
    let pf = outline_in(p, provider);
    let cf = outline(c);
    let mut graph = NameNamespaces::from_outlines(&[&pf, &cf]).unwrap();
    graph.dependency(unit(), "renamed", provider, 2024).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 2);
    let original = pf
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    assert!(
        matches!(graph.lookup(Namespace::Value,&path(&["chosen"]),&caller()).unwrap(),NameLookup::Defined(d) if d.identity==original&&d.defining_unit==provider)
    );
    fs::write(work.join("provider.rs"), p).unwrap();
    fs::write(work.join("consumer.rs"), c).unwrap();
    let built = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-name=origin",
            "--crate-type=rlib",
        ])
        .arg(work.join("provider.rs"))
        .arg("-o")
        .arg(work.join("liborigin.rlib"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "--crate-name=consumer"])
        .arg(work.join("consumer.rs"))
        .arg("--extern")
        .arg(format!("renamed={}", work.join("liborigin.rlib").display()))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    fs::write(work.join("consumer.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let result = Command::new(work.join("oracle")).output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"scope":"standalone native two-crate dependency rename/root-alias comparison and owned source provider identity; no selected live-unit compilation or LRR execution claim"})).unwrap()).unwrap();
}

fn ordinary_source(
    source: &str,
) -> (
    lrr::frontend::source_macros::PublishedMacroIndex,
    lrr::source::changes::SourceSelection,
    lrr::source::manifest::BodyKey,
    NameNamespaces,
) {
    use lrr::{
        core::profile::ProfileIdentity,
        db::key::OutlineKey,
        frontend::{hygiene::HygieneFacts, source_macros::PublishedMacroIndex},
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::{parse::ParseKey, token_wire as runtime},
    };
    use lrr_expand_adapter::{
        hygiene::HygieneStore,
        token_wire::{Edition, Key, TokenForest},
    };
    let input = SourceUnitInput {
        path: SourcePath::new("ordinary.rs").unwrap(),
        crate_key: unit(),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
    };
    let selection = SourceSelection {
        path: input.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: input.blob.content(),
                profile: input.profile.clone(),
            },
            crate_key: unit(),
            module_path: vec![],
            cfg: input.cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(None, vec![input], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("ordinary source absent")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "main")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut store = HygieneStore::default();
    let context = store.root(Key(unit().0), Edition::E2024).unwrap();
    let keys = [context];
    let rows = store.resolution_contexts(&keys).unwrap();
    let names = store.name_contexts(&keys).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let decoded = runtime::TokenForest::decode(&wire).unwrap();
    let facts = HygieneFacts::new(
        decoded.contexts().to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    (
        PublishedMacroIndex::from_view(view, runtime::Key(context.0), facts).unwrap(),
        selection,
        body,
        graph,
    )
}
#[test]
fn actual_native_ordinary_local_shadowing_matches_source_identity_and_order() {
    use lrr::frontend::{
        names::NameLookup,
        source_locals::{OrdinaryNameSite, PatternResolution},
    };
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let source = "fn x()->i64{3}fn main(){let before=x();let x=7;let outer=x;let inner={let x=11;x};let after=x;{println!(\"{}:{}:{}:{}\",before,outer,inner,after);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let positions = [
        source.find("before=x").unwrap() + 7,
        source.find("outer=x").unwrap() + 6,
        source.find("11;x").unwrap() + 3,
        source.find("after=x").unwrap() + 6,
    ];
    let observed = positions
        .iter()
        .map(|at| {
            locals
                .lookup_ordinary(
                    OrdinaryNameSite {
                        source: &selection,
                        body: &body,
                        at: *at as u64,
                        name: "x",
                        context: locals.context(),
                        namespace: Namespace::Value,
                    },
                    &graph,
                    None,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(matches!(&observed[0], NameLookup::Defined(_)));
    let ids = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.name == "x")
        .map(|b| b.identity)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 2);
    assert_eq!(observed[1], NameLookup::LocalBinding(ids[0]));
    assert_eq!(observed[2], NameLookup::LocalBinding(ids[1]));
    assert_eq!(observed[3], NameLookup::LocalBinding(ids[0]));
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-lexical-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3:7:11:7");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"binding_ids":ids.iter().copied().map(Digest::hex).collect::<Vec<_>>(),"scope":"ordinary source local/module shadowing and source order compared with standalone native oracle; no VM evaluation or complete body-item/prelude claim"})).unwrap()).unwrap();
}
#[test]
fn ordinary_local_lookup_preserves_producer_demands_and_exact_source_validation() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "fn x(){}fn main(){let x=7;produce!();let after=x;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let at = (source.find("after=x").unwrap() + 6) as u64;
    let lookup = |source, body, at| {
        locals.lookup_ordinary(
            OrdinaryNameSite {
                source,
                body,
                at,
                name: "x",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
    };
    assert!(matches!(
        lookup(&selection, &body, at).unwrap(),
        NameLookup::NeedInput(_)
    ));
    let mut stale = selection.clone();
    stale.outline.parse.content = Digest([9; 32]);
    assert!(lookup(&stale, &body, at).is_err());
    let mut stale_body = body.clone();
    stale_body.exact_body_bytes = Digest([9; 32]);
    assert!(lookup(&selection, &stale_body, at).is_err());
    assert!(lookup(&selection, &body, u64::MAX).is_err());
}
#[test]
fn ordinary_module_precedence_and_explicit_prelude_keep_namespaces_separate() {
    let source = outline("fn selected(){}struct Kind{value:i64}mod child{pub fn local(){}}");
    let mut graph = NameNamespaces::from_outlines(&[&source]).unwrap();
    graph
        .declare_module(unit(), &["prelude"], 2024, VisibilityScope::Public)
        .unwrap();
    for (namespace, name, id) in [
        (Namespace::Value, "fallback", 8),
        (Namespace::Value, "selected", 9),
        (Namespace::Type, "Kind", 10),
    ] {
        graph
            .define(
                path(&["prelude", name]),
                definition(namespace, id),
                VisibilityScope::Public,
            )
            .unwrap();
    }
    let prelude = ScopeKey {
        crate_key: unit(),
        module_path: vec!["prelude".into()],
        owner: None,
    };
    assert!(
        matches!(graph.lookup_unqualified(Namespace::Value,"fallback",&caller(),2024,Some(&prelude)).unwrap(),NameLookup::Defined(d) if d.identity==Digest([8;32]))
    );
    assert!(
        matches!(graph.lookup_unqualified(Namespace::Value,"selected",&caller(),2024,Some(&prelude)).unwrap(),NameLookup::Defined(d) if d.identity!=Digest([9;32]))
    );
    assert!(
        matches!(graph.lookup_unqualified(Namespace::Type,"Kind",&caller(),2024,Some(&prelude)).unwrap(),NameLookup::Defined(d) if d.identity!=Digest([10;32]))
    );
    assert_eq!(
        graph
            .lookup_unqualified(Namespace::Value, "Kind", &caller(), 2024, Some(&prelude))
            .unwrap(),
        NameLookup::Absent
    );
    let child = ScopeKey {
        crate_key: unit(),
        module_path: vec!["child".into()],
        owner: None,
    };
    assert_eq!(
        graph
            .lookup_unqualified(Namespace::Value, "selected", &child, 2024, None)
            .unwrap(),
        NameLookup::Absent
    );
    graph
        .pending(
            Namespace::Value,
            path(&["prelude", "waiting"]),
            Digest([11; 32]),
        )
        .unwrap();
    assert_eq!(
        graph
            .lookup_unqualified(Namespace::Value, "waiting", &caller(), 2024, Some(&prelude))
            .unwrap(),
        NameLookup::NeedInput(Digest([11; 32]))
    );
}

#[test]
fn ordinary_source_pairing_rejects_stale_and_unbound_namespace_views() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "fn f()->i64{3}fn main(){let x=7;let after=f();}";
    let (published, selection, body, names) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let at = (source.find("after=f").unwrap() + 6) as u64;
    let stale = outline("fn f()->i64{9}fn main(){let x=7;let after=f();}");
    let stale_names = NameNamespaces::from_outlines(&[&stale]).unwrap();
    let lookup = |names| {
        locals.lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            names,
            None,
        )
    };
    assert!(matches!(lookup(&names).unwrap(), NameLookup::Defined(_)));
    assert!(lookup(&stale_names).is_err());
    assert!(lookup(&graph()).is_err());
    let original = outline(source);
    names.require_source(&selection, &original).unwrap();
    let mut wrong_cfg = original.clone();
    wrong_cfg.cfg = Digest([8; 32]);
    assert!(names.require_source(&selection, &wrong_cfg).is_err());
    let mut wrong_unit = original.clone();
    wrong_unit.root_scope.crate_key = Digest([8; 32]);
    assert!(names.require_source(&selection, &wrong_unit).is_err());
    let mut wrong_profile = original.clone();
    wrong_profile.profile.edition = 2021;
    assert!(names.require_source(&selection, &wrong_profile).is_err());
    let imported = outline("mod m{pub fn f(){}}use crate::m::f as chosen;fn main(){chosen();}");
    let names = NameNamespaces::from_outlines(&[&imported]).unwrap();
    let (_, selection, _, _) =
        ordinary_source("mod m{pub fn f(){}}use crate::m::f as chosen;fn main(){chosen();}");
    names.require_source(&selection, &imported).unwrap();
    for field in 0..9 {
        let mut forged = imported.clone();
        let row = &mut forged.imports[0];
        match field {
            0 => row.identity = Digest([8; 32]),
            1 => row.leaf_ordinal += 1,
            2 => row.scope.crate_key = Digest([8; 32]),
            3 => row.range.start += 1,
            4 => row.range.end += 1,
            5 => row.absolute = !row.absolute,
            6 => row.path.push(lrr::syntax::imports::ImportSegment::Name(
                "different".into(),
            )),
            7 => row.kind = lrr::syntax::imports::ImportKind::Glob,
            8 => row.visibility = lrr::syntax::outline::Visibility::Public,
            _ => unreachable!(),
        }
        assert!(
            names.require_source(&selection, &forged).is_err(),
            "import field {field}"
        );
    }
}

#[test]
fn body_items_shadow_outer_locals_and_module_names_in_exact_scopes() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "fn x()->i64{3}fn main(){let before=x();let x=7;let inner={let call=x();fn x()->i64{11}call};let after=x;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let item = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "x")
        .unwrap()
        .item
        .def_key
        .fingerprint();
    let at = (source.find("call=x").unwrap() + 5) as u64;
    assert!(
        matches!(locals.lookup_ordinary(OrdinaryNameSite{source:&selection,body:&body,at,name:"x",context:locals.context(),namespace:Namespace::Value},&graph,None).unwrap(),NameLookup::Defined(d) if d.identity==item)
    );
    let source = "struct Kind{}fn main(){let before:Kind;struct Kind;let value=Kind;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let item = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "Kind")
        .unwrap()
        .item
        .def_key
        .fingerprint();
    for (at, namespace) in [
        (source.find("before:Kind").unwrap() + 7, Namespace::Type),
        (source.find("value=Kind").unwrap() + 6, Namespace::Value),
    ] {
        assert!(
            matches!(locals.lookup_ordinary(OrdinaryNameSite{source:&selection,body:&body,at:at as u64,name:"Kind",context:locals.context(),namespace},&graph,None).unwrap(),NameLookup::Defined(d) if d.identity==item)
        );
    }
}
#[test]
fn body_item_producers_and_imports_block_hoisted_lookup_before_source_position() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    for source in [
        "fn f(){}fn main(){let before=f();produce!();}",
        "fn f(){}fn main(){let before=f();use other::f;}",
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let resolutions = entered
            .facts()
            .bindings
            .iter()
            .map(|b| (b.identity, PatternResolution::Binding))
            .collect();
        let locals = entered
            .local_index(&selection, &body, &resolutions, &[])
            .unwrap();
        let at = (source.find("before=f").unwrap() + 7) as u64;
        assert!(matches!(
            locals
                .lookup_ordinary(
                    OrdinaryNameSite {
                        source: &selection,
                        body: &body,
                        at,
                        name: "f",
                        context: locals.context(),
                        namespace: Namespace::Value
                    },
                    &graph,
                    None
                )
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
}

#[test]
fn actual_native_body_item_shadowing_matches_hoisted_source_identity() {
    use lrr::frontend::{
        names::NameLookup,
        source_locals::{OrdinaryNameSite, PatternResolution},
    };
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let source = "fn x()->i64{3}fn main(){let before=x();let x=7;let inner={let call=x();fn x()->i64{11}call};let after=x;{println!(\"{}:{}:{}\",before,inner,after);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let positions = [
        source.find("before=x").unwrap() + 7,
        source.find("call=x").unwrap() + 5,
        source.find("after=x").unwrap() + 6,
    ];
    let observed = positions
        .iter()
        .map(|at| {
            locals
                .lookup_ordinary(
                    OrdinaryNameSite {
                        source: &selection,
                        body: &body,
                        at: *at as u64,
                        name: "x",
                        context: locals.context(),
                        namespace: Namespace::Value,
                    },
                    &graph,
                    None,
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(matches!(&observed[0], NameLookup::Defined(_)));
    let ids = entered
        .facts()
        .bindings
        .iter()
        .filter(|b| b.name == "x")
        .map(|b| b.identity)
        .collect::<Vec<_>>();
    assert_eq!(ids.len(), 1);
    let item = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "x")
        .unwrap()
        .item
        .def_key
        .fingerprint();
    assert!(matches!(&observed[1],NameLookup::Defined(d) if d.identity==item));
    assert_eq!(observed[2], NameLookup::LocalBinding(ids[0]));
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-body-items-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3:11:7");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"binding_ids":ids.iter().copied().map(Digest::hex).collect::<Vec<_>>(),"scope":"ordinary block item hoisting and outer local shadowing compared with native source; no VM evaluation or complete body-import/generated-item claim"})).unwrap()).unwrap();
}

#[test]
fn ordinary_body_macro_view_preserves_textual_definition_order() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "fn main(){let before=0;macro_rules! pick{()=>{3}}let after=0;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    for (at, defined) in [
        (source.find("before").unwrap(), false),
        (source.find("after").unwrap(), true),
    ] {
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: at as u64,
                    name: "pick",
                    context: locals.context(),
                    namespace: Namespace::Macro,
                },
                &graph,
                None,
            )
            .unwrap();
        if defined {
            assert!(matches!(found, NameLookup::Defined(_)));
        } else {
            assert_eq!(found, NameLookup::Absent);
        }
    }
}

#[test]
fn nested_functions_capture_enclosing_items_without_enclosing_local_bindings() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    let source = "fn main(){fn nested()->i64{fn leaf()->i64{helper()}leaf()}fn helper()->i64{3}let secret=7;let chosen=nested();}";
    let (published, selection, body, graph) = ordinary_source(source);
    let outer = published.enter_body(&selection, &body).unwrap();
    let helper = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "helper")
        .unwrap()
        .item
        .def_key
        .fingerprint();
    let nested = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let child = outer.enter_nested_body(&selection, &body, &nested).unwrap();
    let leaf = child
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "leaf")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let entered = child.enter_nested_body(&selection, &nested, &leaf).unwrap();
    let locals = entered
        .local_index(&selection, &leaf, &Default::default(), &[])
        .unwrap();
    let at = source.find("helper()").unwrap() as u64;
    for name in ["helper", "secret"] {
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &leaf,
                    at,
                    name,
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        if name == "helper" {
            assert!(matches!(found,NameLookup::Defined(d) if d.identity==helper));
        } else {
            assert_eq!(found, NameLookup::Absent);
        }
    }
}
#[test]
fn nested_item_frontiers_capture_only_enclosing_scopes() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    for (source, pending) in [
        (
            "fn main(){fn nested(){helper();}fn helper(){}produce!();}",
            true,
        ),
        (
            "fn main(){fn nested(){helper();}fn helper(){}{produce!();}}",
            false,
        ),
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let outer = published.enter_body(&selection, &body).unwrap();
        let nested = outer
            .facts()
            .items
            .iter()
            .find(|i| i.item.def_key.declared_name == "nested")
            .unwrap()
            .item
            .body
            .as_ref()
            .unwrap()
            .key();
        let entered = outer.enter_nested_body(&selection, &body, &nested).unwrap();
        let locals = entered
            .local_index(&selection, &nested, &Default::default(), &[])
            .unwrap();
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &nested,
                    at: source.find("helper();").unwrap() as u64,
                    name: "helper",
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        if pending {
            assert!(matches!(found, NameLookup::NeedInput(_)));
        } else {
            assert!(matches!(found, NameLookup::Defined(_)));
        }
    }
}

#[test]
fn actual_native_nested_functions_match_enclosing_item_identity() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    let source = "fn main(){fn nested()->i64{fn leaf()->i64{helper()}leaf()}fn helper()->i64{3}let secret=7;let chosen=nested();{println!(\"{}\",chosen);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let outer = published.enter_body(&selection, &body).unwrap();
    let helper = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "helper")
        .unwrap()
        .item
        .def_key
        .fingerprint();
    let nested = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let child = outer.enter_nested_body(&selection, &body, &nested).unwrap();
    let leaf = child
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "leaf")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let entered = child.enter_nested_body(&selection, &nested, &leaf).unwrap();
    let locals = entered
        .local_index(&selection, &leaf, &Default::default(), &[])
        .unwrap();
    let at = source.find("helper()").unwrap() as u64;
    for name in ["helper", "secret"] {
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &leaf,
                    at,
                    name,
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        if name == "helper" {
            assert!(matches!(found,NameLookup::Defined(d) if d.identity==helper));
        } else {
            assert_eq!(found, NameLookup::Absent);
        }
    }

    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-nested-items-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"helper_identity":helper.hex(),"scope":"original enclosing block-item identity through two nested functions and standalone native comparison; enclosing value locals remain uncaptured; no VM/generated-item/body-import claim"})).unwrap()).unwrap();
}

#[test]
fn body_named_imports_are_hoisted_and_preserve_alias_and_namespace_identity() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "mod m{pub fn f()->i64{3}pub struct Ty{pub value:i64}}fn main(){let before=chosen();use crate::m::{f as first,Ty as T};use first as chosen;let typed:T;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    for (name, namespace) in [("chosen", Namespace::Value), ("T", Namespace::Type)] {
        let at = (source.find("before=chosen").unwrap() + 7) as u64;
        assert!(
            matches!(locals.lookup_ordinary(OrdinaryNameSite{source:&selection,body:&body,at,name,context:locals.context(),namespace},&graph,None).unwrap(),NameLookup::Defined(d) if d.namespace==namespace)
        );
    }
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at: (source.find("typed:T").unwrap() + 6) as u64,
                name: "T",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert_eq!(found, NameLookup::Absent);
}
#[test]
fn body_import_cycles_and_unresolved_targets_retain_typed_frontiers() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    for source in [
        "fn main(){use a as b;use b as a;let selected=a;}",
        "mod m{produce!();}fn main(){use crate::m::f as selected;let value=selected;}",
        "fn main(){use missing::f as selected;let value=selected;}",
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let resolutions = entered
            .facts()
            .bindings
            .iter()
            .map(|b| (b.identity, PatternResolution::Binding))
            .collect();
        let locals = entered
            .local_index(&selection, &body, &resolutions, &[])
            .unwrap();
        let at = source.rfind("selected").unwrap() as u64;
        assert!(matches!(
            locals
                .lookup_ordinary(
                    OrdinaryNameSite {
                        source: &selection,
                        body: &body,
                        at,
                        name: "selected",
                        context: locals.context(),
                        namespace: Namespace::Value
                    },
                    &graph,
                    None
                )
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
}
#[test]
fn actual_native_body_import_alias_chain_matches_captured_nested_identity() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let source = "mod m{pub fn f()->i64{3}}fn main(){fn nested()->i64{chosen()}use crate::m::f as first;use first as chosen;let value=nested();{println!(\"{}\",value);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let outer = published.enter_body(&selection, &body).unwrap();
    let nested = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let entered = outer.enter_nested_body(&selection, &body, &nested).unwrap();
    let locals = entered
        .local_index(&selection, &nested, &Default::default(), &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &nested,
                at: source.find("chosen()").unwrap() as u64,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-body-imports-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"identity":original.hex(),"scope":"hoisted body named-import alias chain captured in nested function preserves original module definition; native standalone oracle; no VM/full body-glob/generated import claim"})).unwrap()).unwrap();
}

#[test]
fn body_module_and_root_alias_chains_resolve_imported_members() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    for source in [
        "mod m{pub fn f(){}}fn main(){use crate::m as first;use first as second;use second::f as chosen;let selected=chosen;}",
        "mod m{pub fn f(){}}fn main(){use crate as root;use root::m as module;use module::f as chosen;let selected=chosen;}",
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let resolutions = entered
            .facts()
            .bindings
            .iter()
            .map(|b| (b.identity, PatternResolution::Binding))
            .collect();
        let locals = entered
            .local_index(&selection, &body, &resolutions, &[])
            .unwrap();
        let original = outline(source)
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "f")
            .unwrap()
            .def_key
            .fingerprint();
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.rfind("chosen").unwrap() as u64,
                    name: "chosen",
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    }
}
#[test]
fn body_module_alias_cycles_and_unknown_targets_cannot_supply_members() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    for source in [
        "fn main(){use a as b;use b as a;use a::f as chosen;let selected=chosen;}",
        "mod m{make!();}fn main(){use crate::m as alias;use alias::f as chosen;let selected=chosen;}",
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let resolutions = entered
            .facts()
            .bindings
            .iter()
            .map(|b| (b.identity, PatternResolution::Binding))
            .collect();
        let locals = entered
            .local_index(&selection, &body, &resolutions, &[])
            .unwrap();
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.rfind("chosen").unwrap() as u64,
                    name: "chosen",
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        assert!(matches!(found, NameLookup::NeedInput(_)));
    }
}

#[test]
fn actual_native_body_module_alias_chain_matches_captured_member_identity() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let source = "mod m{pub fn f()->i64{3}}fn main(){fn nested()->i64{chosen()}use crate as root;use root::m as first;use first as second;use second::f as chosen;let value=nested();{println!(\"{}\",value);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let outer = published.enter_body(&selection, &body).unwrap();
    let nested = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let entered = outer.enter_nested_body(&selection, &body, &nested).unwrap();
    let locals = entered
        .local_index(&selection, &nested, &Default::default(), &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &nested,
                at: source.find("chosen()").unwrap() as u64,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-body-module-aliases-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"identity":original.hex(),"scope":"body root/module alias chain captured in nested function preserves original member identity; native standalone oracle; no VM/full body-glob/generated import claim"})).unwrap()).unwrap();
}

#[test]
fn qualified_body_module_lookup_preserves_identity_over_unrelated_local_names() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    use lrr::syntax::imports::ImportSegment;
    let source = "mod m{pub fn f()->i64{3}}fn main(){use crate as root;use root::m as alias;let f=99;let selected=alias::f();}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let at = source.find("alias::f()").unwrap() as u64;
    let path = [
        ImportSegment::Name("alias".into()),
        ImportSegment::Name("f".into()),
    ];
    let found = locals
        .lookup_qualified_module(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &path,
            false,
            &graph,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    let local = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(local, NameLookup::LocalBinding(_)));
}
#[test]
fn qualified_body_module_lookup_retains_frontiers_and_rejects_stale_sites() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    use lrr::syntax::imports::ImportSegment;
    let source = "mod m{make!();}fn main(){use crate::m as alias;let selected=alias::f();}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let path = [
        ImportSegment::Name("alias".into()),
        ImportSegment::Name("f".into()),
    ];
    let at = source.find("alias::f()").unwrap() as u64;
    let lookup = |source, path| {
        locals.lookup_qualified_module(
            OrdinaryNameSite {
                source,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            path,
            false,
            &graph,
        )
    };
    assert!(matches!(
        lookup(&selection, &path).unwrap(),
        NameLookup::NeedInput(_)
    ));
    assert!(lookup(&selection, &[]).is_err());
    let mut stale = selection.clone();
    stale.outline.parse.content = Digest([9; 32]);
    assert!(lookup(&stale, &path).is_err());
}

#[test]
fn actual_native_qualified_body_member_matches_identity_over_local_collision() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    use lrr::syntax::imports::ImportSegment;
    let source = "mod m{pub fn f()->i64{3}}fn main(){use crate as root;use root::m as alias;let f=99;let selected=alias::f();{println!(\"{}\",selected);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let at = source.find("alias::f()").unwrap() as u64;
    let path = [
        ImportSegment::Name("alias".into()),
        ImportSegment::Name("f".into()),
    ];
    let found = locals
        .lookup_qualified_module(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &path,
            false,
            &graph,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    let local = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(local, NameLookup::LocalBinding(_)));

    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-qualified-body-paths-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"identity":original.hex(),"scope":"direct body-qualified module member retains source identity despite unrelated local spelling; standalone native comparison; no VM/associated-item/generated-path claim"})).unwrap()).unwrap();
}
#[test]
fn qualified_body_member_preserves_original_private_visibility() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    use lrr::syntax::imports::ImportSegment;
    let source =
        "mod m{fn hidden(){}}fn main(){use crate::m as alias;let selected=alias::hidden();}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let path = [
        ImportSegment::Name("alias".into()),
        ImportSegment::Name("hidden".into()),
    ];
    let found = locals
        .lookup_qualified_module(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at: source.find("alias::hidden()").unwrap() as u64,
                name: "hidden",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &path,
            false,
            &graph,
        )
        .unwrap();
    assert!(matches!(found, NameLookup::Inaccessible(_)));
}

#[test]
fn body_globs_preserve_explicit_precedence_ambiguity_and_private_absence() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    for (source, ambiguous) in [
        (
            "mod a{pub fn f(){}}mod b{pub fn f(){}}fn main(){use crate::a::*;use crate::b::*;use crate::a::f;let selected=f;}",
            false,
        ),
        (
            "mod a{pub fn f(){}}mod b{pub fn f(){}}fn main(){use crate::a::*;use crate::b::*;let selected=f;}",
            true,
        ),
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let resolutions = entered
            .facts()
            .bindings
            .iter()
            .map(|b| (b.identity, PatternResolution::Binding))
            .collect();
        let locals = entered
            .local_index(&selection, &body, &resolutions, &[])
            .unwrap();
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.find("selected=f").unwrap() as u64,
                    name: "f",
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        if ambiguous {
            assert!(matches!(found,NameLookup::Ambiguous(ds) if ds.len()==2));
        } else {
            let original = outline(source)
                .items
                .iter()
                .find(|i| i.def_key.declared_name == "f" && i.scope.module_path == ["a"])
                .unwrap()
                .def_key
                .fingerprint();
            assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
        }
    }
    let source = "mod a{fn hidden(){}}fn main(){use crate::a::*;let selected=0;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    assert_eq!(
        locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.find("selected").unwrap() as u64,
                    name: "hidden",
                    context: locals.context(),
                    namespace: Namespace::Value
                },
                &graph,
                None
            )
            .unwrap(),
        NameLookup::Absent
    );
}
#[test]
fn body_glob_unknown_producers_keep_known_and_missing_members_pending() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "mod a{make!();pub fn f(){}}fn main(){use crate::a::*;let selected=0;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    for name in ["f", "missing"] {
        assert!(matches!(
            locals
                .lookup_ordinary(
                    OrdinaryNameSite {
                        source: &selection,
                        body: &body,
                        at: source.find("selected").unwrap() as u64,
                        name,
                        context: locals.context(),
                        namespace: Namespace::Value
                    },
                    &graph,
                    None
                )
                .unwrap(),
            NameLookup::NeedInput(_)
        ));
    }
}

#[test]
fn actual_native_body_glob_member_matches_captured_alias_identity() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let source = "mod m{pub fn f()->i64{3}}fn main(){fn nested()->i64{chosen()}use crate::m::*;use f as chosen;let value=nested();{println!(\"{}\",value);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let outer = published.enter_body(&selection, &body).unwrap();
    let nested = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let entered = outer.enter_nested_body(&selection, &body, &nested).unwrap();
    let locals = entered
        .local_index(&selection, &nested, &Default::default(), &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &nested,
                at: source.find("chosen()").unwrap() as u64,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-body-globs-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"identity":original.hex(),"scope":"body glob member and named alias captured in nested function preserve source identity; standalone native comparison; no VM/full glob-module/generated import claim"})).unwrap()).unwrap();
}

#[test]
fn explicit_body_items_shadow_open_globs_without_sealing_the_glob() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "mod a{make!();pub fn f(){}}fn main(){use crate::a::*;fn f(){}let selected=f;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let original = entered
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "f")
        .unwrap()
        .item
        .def_key
        .fingerprint();
    let at = source.find("selected=f").unwrap() as u64;
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    assert!(matches!(
        locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at,
                    name: "missing",
                    context: locals.context(),
                    namespace: Namespace::Value
                },
                &graph,
                None
            )
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
}

#[test]
fn tuple_constructor_visibility_keeps_type_visibility_and_active_field_restrictions() {
    let facts = outline(
        "mod m{pub struct Private(i64);pub struct Public(pub i64);pub struct Disabled(#[cfg(any())] i64,pub i64);}",
    );
    let graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert!(matches!(
        graph
            .lookup(Namespace::Type, &path(&["m", "Private"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["m", "Private"]), &caller())
            .unwrap(),
        NameLookup::Inaccessible(_)
    ));
    for name in ["Public", "Disabled"] {
        assert!(matches!(
            graph
                .lookup(Namespace::Value, &path(&["m", name]), &caller())
                .unwrap(),
            NameLookup::Defined(_)
        ));
    }
}

#[test]
fn private_tuple_constructor_does_not_prevent_importing_the_public_type_facet() {
    let facts = outline("mod m{pub struct Private(i64);}use crate::m::Private as Alias;");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 1);
    assert!(matches!(
        graph
            .lookup(Namespace::Type, &path(&["Alias"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["Alias"]), &caller())
            .unwrap(),
        NameLookup::Inaccessible(_)
    ));
}
#[test]
fn actual_native_tuple_constructor_visibility_matches_type_and_value_facets() {
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let source = "mod m{pub struct Private(i64);pub struct Public(pub i64);pub struct Disabled(#[cfg(any())] i64,pub i64);}use crate::m::Private as Alias;fn main(){let _:Option<Alias>=None;let a=m::Public(2);let b=m::Disabled(3);println!(\"{}:{}\",a.0,b.0);}";
    let facts = outline(source);
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 1);
    assert!(matches!(
        graph
            .lookup(Namespace::Type, &path(&["Alias"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["Alias"]), &caller())
            .unwrap(),
        NameLookup::Inaccessible(_)
    ));
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-constructor-visibility-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    fs::write(work.join("positive.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "2:3");
    let negative = "mod m{pub struct Private(i64);}fn main(){let _=m::Private(3);}";
    fs::write(work.join("negative.rs"), negative).unwrap();
    let rejected = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("negative.rs"))
        .arg("-o")
        .arg(work.join("negative"))
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let stderr = String::from_utf8(rejected.stderr).unwrap();
    assert!(stderr.contains("E0603") && stderr.contains("tuple struct constructor"));
    fs::write(work.join("negative.stderr"), &stderr).unwrap();
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"negative_code":"E0603","negative_accepted":false,"scope":"public tuple type/private constructor facets and cfg-disabled field visibility, with real positive/negative native comparisons; no VM/type-checking/generated-field claim"})).unwrap()).unwrap();
}

#[test]
fn body_tuple_type_aliases_preserve_private_value_constructor_facets() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source =
        "mod m{pub struct Private(i64);}fn main(){use crate::m::Private as Alias;let selected=0;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let at = source.find("selected").unwrap() as u64;
    let lookup = |namespace| {
        locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at,
                    name: "Alias",
                    context: locals.context(),
                    namespace,
                },
                &graph,
                None,
            )
            .unwrap()
    };
    assert!(matches!(lookup(Namespace::Type), NameLookup::Defined(_)));
    assert!(matches!(
        lookup(Namespace::Value),
        NameLookup::Inaccessible(_)
    ));
}
#[test]
fn body_tuple_globs_filter_private_constructors_without_hiding_types() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    let source = "mod m{pub struct Private(i64);pub struct Public(pub i64);}fn main(){use crate::m::*;let selected=0;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let lookup = |name, namespace| {
        locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.find("selected").unwrap() as u64,
                    name,
                    context: locals.context(),
                    namespace,
                },
                &graph,
                None,
            )
            .unwrap()
    };
    assert!(matches!(
        lookup("Private", Namespace::Type),
        NameLookup::Defined(_)
    ));
    assert_eq!(lookup("Private", Namespace::Value), NameLookup::Absent);
    assert!(matches!(
        lookup("Public", Namespace::Value),
        NameLookup::Defined(_)
    ));
}
#[test]
fn actual_native_body_constructor_facets_match_captured_imports() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let source = "mod m{pub struct Private(i64);pub struct Public(pub i64);}fn main(){use crate::m::*;use crate::m::Private as Alias;fn nested()->i64{let _:Option<Alias>=None;Public(2).0}let selected=nested();{println!(\"{}\",selected);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let outer = published.enter_body(&selection, &body).unwrap();
    let nested = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let entered = outer.enter_nested_body(&selection, &body, &nested).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| {
            (
                b.identity,
                lrr::frontend::source_locals::PatternResolution::Binding,
            )
        })
        .collect();
    let locals = entered
        .local_index(&selection, &nested, &resolutions, &[])
        .unwrap();
    let at = source.find("Public(2)").unwrap() as u64;
    for (name, namespace, accessible) in [
        ("Alias", Namespace::Type, true),
        ("Alias", Namespace::Value, false),
        ("Public", Namespace::Value, true),
    ] {
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &nested,
                    at,
                    name,
                    context: locals.context(),
                    namespace,
                },
                &graph,
                None,
            )
            .unwrap();
        if accessible {
            assert!(matches!(found, NameLookup::Defined(_)));
        } else {
            assert!(matches!(found, NameLookup::Inaccessible(_)));
        }
    }
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-body-constructor-facets-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    fs::write(work.join("positive.stderr"), &built.stderr).unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "2");
    let negative =
        "mod m{pub struct Private(i64);}fn main(){use crate::m::Private as Alias;let _=Alias(3);}";
    fs::write(work.join("negative.rs"), negative).unwrap();
    let rejected = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("negative.rs"))
        .arg("-o")
        .arg(work.join("negative"))
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let stderr = String::from_utf8(rejected.stderr).unwrap();
    assert!(stderr.contains("E0423") && stderr.contains("private fields"));
    fs::write(work.join("negative.stderr"), &stderr).unwrap();
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"negative_accepted":false,"negative_code":"E0423","scope":"body alias/glob/captured tuple type and constructor facets compared with valid native execution and rejected private alias constructor; no VM/generated-field claim"})).unwrap()).unwrap();
}

#[test]
fn body_glob_module_prefixes_resolve_members_and_named_aliases() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    use lrr::syntax::imports::ImportSegment;
    let source = "mod a{pub mod inner{pub fn f(){}}}fn main(){use crate::a::*;use inner::f as chosen;let selected=inner::f;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let at = source.find("selected=inner").unwrap() as u64;
    let path = [
        ImportSegment::Name("inner".into()),
        ImportSegment::Name("f".into()),
    ];
    let found = locals
        .lookup_qualified_module(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &path,
            false,
            &graph,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
}

#[test]
fn body_glob_module_prefix_ambiguity_precedes_member_selection() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    use lrr::syntax::imports::ImportSegment;
    for explicit in [false, true] {
        let source = format!(
            "mod a{{pub mod inner{{pub fn f(){{}}}}}}mod b{{pub mod inner{{pub fn g(){{}}}}}}fn main(){{use crate::a::*;use crate::b::*;{}let selected=inner::f;}}",
            if explicit { "use crate::a::inner;" } else { "" }
        );
        let (published, selection, body, graph) = ordinary_source(&source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let resolutions = entered
            .facts()
            .bindings
            .iter()
            .map(|b| (b.identity, PatternResolution::Binding))
            .collect();
        let locals = entered
            .local_index(&selection, &body, &resolutions, &[])
            .unwrap();
        let path = [
            ImportSegment::Name("inner".into()),
            ImportSegment::Name("f".into()),
        ];
        let found = locals
            .lookup_qualified_module(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.find("selected=inner").unwrap() as u64,
                    name: "f",
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &path,
                false,
                &graph,
            )
            .unwrap();
        if explicit {
            assert!(matches!(found, NameLookup::Defined(_)));
        } else {
            assert!(matches!(found,NameLookup::Ambiguous(ds) if ds.len()==2));
        }
    }
    let source = "mod a{pub mod inner{pub fn f(){}}}mod b{pub mod inner{pub fn g(){}}}fn main(){use crate::a::*;use crate::b::*;use inner::f as chosen;let selected=chosen;}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    assert!(
        matches!(locals.lookup_ordinary(OrdinaryNameSite{source:&selection,body:&body,at:source.find("selected=chosen").unwrap()as u64,name:"chosen",context:locals.context(),namespace:Namespace::Value},&graph,None).unwrap(),NameLookup::Ambiguous(ds) if ds.len()==2)
    );
}

#[test]
fn actual_native_body_glob_module_member_matches_source_identity() {
    use lrr::frontend::source_locals::{OrdinaryNameSite, PatternResolution};
    use lrr::syntax::imports::ImportSegment;
    let source = "mod a{pub mod inner{pub fn f()->i64{3}}}fn main(){use crate::a::*;use inner::f as chosen;let selected=inner::f();{println!(\"{}\",selected);}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|b| (b.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(&selection, &body, &resolutions, &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let at = source.find("selected=inner").unwrap() as u64;
    let path = [
        ImportSegment::Name("inner".into()),
        ImportSegment::Name("f".into()),
    ];
    let found = locals
        .lookup_qualified_module(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "f",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &path,
            false,
            &graph,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));

    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };
    let work = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(format!(
            "artifacts/macos-first/t020-body-glob-modules-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    fs::create_dir(&work).unwrap();
    fs::write(work.join("oracle.rs"), source).unwrap();
    let built = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("oracle.rs"))
        .arg("-o")
        .arg(work.join("oracle"))
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let native = Command::new(work.join("oracle")).output().unwrap();
    assert!(native.status.success());
    let stdout = String::from_utf8(native.stdout).unwrap();
    assert_eq!(stdout.trim(), "3");
    let negative = "mod a{pub mod inner{pub fn f(){}}}mod b{pub mod inner{pub fn g(){}}}fn main(){use crate::a::*;use crate::b::*;inner::f();}";
    fs::write(work.join("negative.rs"), negative).unwrap();
    let rejected = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024"])
        .arg(work.join("negative.rs"))
        .arg("-o")
        .arg(work.join("negative"))
        .output()
        .unwrap();
    assert!(!rejected.status.success());
    let stderr = String::from_utf8(rejected.stderr).unwrap();
    assert!(stderr.contains("E0659"));
    fs::write(work.join("negative.stderr"), &stderr).unwrap();
    fs::write(work.join("evidence.json"),serde_json::to_vec_pretty(&serde_json::json!({"platform":"aarch64-apple-darwin","rust":"1.99.0","native_stdout":stdout,"negative_accepted":false,"negative_code":"E0659","identity":original.hex(),"scope":"body-glob module member/named alias source identity and ambiguous-prefix negative native comparison; full strong/glob fixed point and generated sources pending"})).unwrap()).unwrap();
}

#[test]
fn body_import_readiness_reuses_long_linear_alias_chains() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    let mut source = String::from("fn f(){}fn main(){use crate::f as a1000;");
    for n in (0..1000).rev() {
        source.push_str(&format!("use a{} as a{};", n + 1, n));
    }
    source.push_str("a0();}");
    let (published, selection, body, graph) = ordinary_source(&source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let locals = entered
        .local_index(&selection, &body, &Default::default(), &[])
        .unwrap();
    let original = outline(&source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at: source.find("a0();").unwrap() as u64,
                name: "a0",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(matches!(found,NameLookup::Defined(d) if d.identity==original));
}

#[test]
fn body_chained_glob_bases_resolve_without_recursive_routing() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    let source = "mod a{pub mod inner{pub mod deeper{pub fn f(){}}}}fn main(){use crate::a::*;use inner::*;use deeper::*;use deeper::f as chosen;chosen();}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let locals = entered
        .local_index(&selection, &body, &Default::default(), &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "f")
        .unwrap()
        .def_key
        .fingerprint();
    for name in ["chosen", "f"] {
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.find("chosen();").unwrap() as u64,
                    name,
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        assert!(
            matches!(&found,NameLookup::Defined(d) if d.identity==original),
            "{name}: {found:?}"
        );
    }
}

#[test]
fn body_chained_globs_preserve_ambiguity_explicit_prefixes_and_pending_producers() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    for (source, expected) in [
        (
            "mod a{pub mod inner{pub mod deeper{pub fn f(){}}}}mod b{pub mod inner{pub mod deeper{pub fn g(){}}}}fn main(){use crate::a::*;use crate::b::*;use inner::*;use deeper::f as chosen;chosen();}",
            0,
        ),
        (
            "mod a{pub mod inner{pub mod deeper{pub fn f(){}}}}mod b{pub mod inner{pub mod deeper{pub fn g(){}}}}fn main(){use crate::a::*;use crate::b::*;use crate::a::inner;use inner::*;use deeper::f as chosen;chosen();}",
            1,
        ),
        (
            "mod a{pub mod inner{pub mod deeper{produce!();pub fn f(){}}}}fn main(){use crate::a::*;use inner::*;use deeper::f as chosen;chosen();}",
            2,
        ),
        ("fn main(){use left::*;use right::*;chosen();}", 2),
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let locals = entered
            .local_index(&selection, &body, &Default::default(), &[])
            .unwrap();
        let found = locals
            .lookup_ordinary(
                OrdinaryNameSite {
                    source: &selection,
                    body: &body,
                    at: source.find("chosen();").unwrap() as u64,
                    name: "chosen",
                    context: locals.context(),
                    namespace: Namespace::Value,
                },
                &graph,
                None,
            )
            .unwrap();
        match expected {
            0 => assert!(
                matches!(found, NameLookup::Ambiguous(ref ds) if ds.len()==2),
                "{found:?}"
            ),
            1 => {
                let original = outline(source)
                    .items
                    .iter()
                    .find(|i| i.def_key.declared_name == "f")
                    .unwrap()
                    .def_key
                    .fingerprint();
                assert!(
                    matches!(found,NameLookup::Defined(ref d) if d.identity==original),
                    "{found:?}"
                );
            }
            2 => assert!(matches!(found, NameLookup::NeedInput(_)), "{found:?}"),
            _ => unreachable!(),
        }
    }
}

#[test]
fn global_glob_chains_converge_and_retract_superseded_member_origins() {
    for source in [
        "mod a{pub mod middle{pub mod origin{pub mod inner{pub fn f(){}}}}}use crate::a::*;use middle::*;use origin::*;use inner::*;use f as chosen;",
        "mod a{pub mod middle{pub mod origin{pub mod inner{pub fn f(){}}}}}mod b{pub mod inner{pub fn f(){}}}use crate::a::*;use middle::*;use origin::inner;use crate::b::*;use inner::*;use f as chosen;",
    ] {
        let facts = outline(source);
        let original = facts
            .items
            .iter()
            .find(|i| {
                i.def_key.declared_name == "f"
                    && i.scope.module_path.first().map(String::as_str) == Some("a")
            })
            .unwrap()
            .def_key
            .fingerprint();
        let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
        let report = graph.resolve_imports().unwrap();
        assert_eq!(report.resolved.len(), facts.imports.len(), "{report:?}");
        assert!(report.pending.is_empty());
        for name in ["f", "chosen"] {
            let found = graph
                .lookup(Namespace::Value, &path(&[name]), &caller())
                .unwrap();
            assert!(
                matches!(found,NameLookup::Defined(ref d) if d.identity==original),
                "{name}: {found:?}"
            );
        }
        assert_eq!(graph.resolve_imports().unwrap().resolved, report.resolved);
    }
}

#[test]
fn global_glob_rebuild_preserves_manual_edges_and_repeated_resolution() {
    use lrr::syntax::visibility::VisibilityScope;
    let facts = outline(
        "mod a{pub mod middle{pub mod origin{pub mod inner{pub fn f(){}}}}}mod b{pub mod inner{pub fn f(){}}}use crate::a::*;use middle::*;use origin::inner;use crate::b::*;use inner::*;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    graph
        .glob_member(
            Namespace::Value,
            path(&["f"]),
            path(&["b", "inner", "f"]),
            VisibilityScope::Public,
        )
        .unwrap();
    for _ in 0..3 {
        let report = graph.resolve_imports().unwrap();
        assert_eq!(report.resolved.len(), facts.imports.len());
        let found = graph
            .lookup(Namespace::Value, &path(&["f"]), &caller())
            .unwrap();
        assert!(
            matches!(found,NameLookup::Ambiguous(ref ds) if ds.len()==2),
            "{found:?}"
        );
    }
}

#[test]
fn global_glob_convergence_retains_late_ambiguity_and_open_producer_demands() {
    let facts = outline(
        "mod a{pub mod middle{pub mod origin{pub mod inner{pub fn f(){}}}}}mod b{pub mod inner{pub fn g(){}}}use crate::a::*;use middle::*;use origin::*;use crate::b::*;use inner::*;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert!(!report.ambiguous.is_empty(), "{report:?}");
    let glob = facts
        .imports
        .iter()
        .find(|i| i.path == vec![lrr::syntax::imports::ImportSegment::Name("inner".into())])
        .unwrap();
    assert!(
        matches!(graph.lookup_import_module(glob, 2024).unwrap(), lrr::frontend::macro_names::ModuleLookup::Ambiguous(ref paths) if paths.len()==2)
    );
    // The unresolved glob keeps the containing source namespace unsealed.
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["inner", "f"]), &caller())
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
    let facts = outline(
        "mod a{pub mod middle{pub mod origin{pub mod inner{produce!();pub fn f(){}}}}}use crate::a::*;use middle::*;use origin::*;use inner::*;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert!(!report.pending.is_empty());
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["f"]), &caller())
            .unwrap(),
        NameLookup::NeedInput(_)
    ));
}

#[test]
fn anonymous_source_imports_record_original_facets_without_named_bindings() {
    let facts = outline(
        "mod m{pub trait Trait{}pub fn f(){}pub struct Ty{pub field:i64}}use crate::m::{Trait as _,f as _,Ty as _};",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert_eq!(report.resolved.len(), 3, "{report:?}");
    for (name, namespace) in [
        ("Trait", Namespace::Type),
        ("f", Namespace::Value),
        ("Ty", Namespace::Type),
    ] {
        assert_eq!(
            graph.lookup(namespace, &path(&[name]), &caller()).unwrap(),
            NameLookup::Absent
        );
    }
}

#[test]
fn anonymous_body_imports_do_not_block_ready_ordinary_names() {
    use lrr::frontend::source_locals::OrdinaryNameSite;
    let source = "mod m{pub trait Trait{}pub fn f(){}}fn chosen(){}fn main(){use crate::m::Trait as _;use crate::m::f as _;chosen();}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let locals = entered
        .local_index(&selection, &body, &Default::default(), &[])
        .unwrap();
    let original = outline(source)
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "chosen")
        .unwrap()
        .def_key
        .fingerprint();
    let found = locals
        .lookup_ordinary(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at: source.find("chosen();").unwrap() as u64,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
            None,
        )
        .unwrap();
    assert!(
        matches!(found,NameLookup::Defined(ref d) if d.identity==original),
        "{found:?}"
    );
}

#[test]
fn anonymous_import_records_preserve_facets_encoding_and_typed_failures() {
    use lrr::core::digest::StableEncode;
    let facts = outline(
        "mod m{pub trait Trait{}pub struct Tuple(pub i64);}use crate::m::{Trait as _,Tuple as _};use crate as _;",
    );
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    assert_eq!(graph.resolve_imports().unwrap().resolved.len(), 3);
    let records = graph
        .disclosed_anonymous_imports(&caller())
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 3);
    let trait_id = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "Trait")
        .unwrap()
        .def_key
        .fingerprint();
    let tuple_id = facts
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "Tuple")
        .unwrap()
        .def_key
        .fingerprint();
    assert!(records.iter().any(|r| r.definitions.len() == 1
        && r.definitions[0].identity == trait_id
        && r.definitions[0].namespace == Namespace::Type));
    assert!(records.iter().any(|r| r.definitions.len() == 2
        && r.definitions.iter().all(|d| d.identity == tuple_id)
        && r.definitions[0].namespace != r.definitions[1].namespace));
    assert!(
        records
            .iter()
            .any(|r| r.module_root == Some(caller().crate_key) && r.definitions.is_empty())
    );
    let original = records.iter().find(|r| !r.definitions.is_empty()).unwrap();
    let mut changed = original.clone();
    changed.identity = lrr::core::digest::Digest([91; 32]);
    assert_ne!(original.fingerprint(), changed.fingerprint());
    for (source, expected) in [
        ("mod m{trait Hidden{}}use crate::m::Hidden as _;", 0),
        ("mod m{}use crate::m::Missing as _;", 1),
        (
            "mod m{produce!();pub trait Trait{}}use crate::m::Trait as _;",
            2,
        ),
        (
            "mod a{pub mod inner{pub trait Trait{}}}mod b{pub mod inner{pub trait Trait{}}}use crate::a::*;use crate::b::*;use inner::Trait as _;",
            3,
        ),
    ] {
        let facts = outline(source);
        let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
        let report = graph.resolve_imports().unwrap();
        match expected {
            0 => assert_eq!(report.inaccessible.len(), 1),
            1 => assert_eq!(report.missing.len(), 1),
            2 => assert_eq!(report.pending.len(), 1),
            3 => assert!(!report.ambiguous.is_empty()),
            _ => unreachable!(),
        }
        assert_eq!(graph.disclosed_anonymous_imports(&caller()).count(), 0);
    }
}

#[test]
fn anonymous_body_facets_follow_aliases_and_keep_producers_pending() {
    use lrr::frontend::{names::AnonymousImports, source_locals::OrdinaryNameSite};
    for (source, ready) in [
        (
            "mod m{pub trait Trait{}}fn chosen(){}fn main(){use crate::m::Trait as First;use First as _;chosen();}",
            true,
        ),
        (
            "mod m{produce!();pub trait Trait{}}fn chosen(){}fn main(){use crate::m::Trait as _;chosen();}",
            false,
        ),
        (
            "mod m{trait Trait{}}fn chosen(){}fn main(){use crate::m::Trait as _;chosen();}",
            false,
        ),
    ] {
        let (published, selection, body, graph) = ordinary_source(source);
        let entered = published.enter_body(&selection, &body).unwrap();
        let locals = entered
            .local_index(&selection, &body, &Default::default(), &[])
            .unwrap();
        let site = OrdinaryNameSite {
            source: &selection,
            body: &body,
            at: source.find("chosen();").unwrap() as u64,
            name: "chosen",
            context: locals.context(),
            namespace: Namespace::Value,
        };
        let found = locals.disclosed_anonymous_imports(site, &graph).unwrap();
        if ready {
            let original = outline(source)
                .items
                .iter()
                .find(|i| i.def_key.declared_name == "Trait")
                .unwrap()
                .def_key
                .fingerprint();
            assert!(
                matches!(found,AnonymousImports::Disclosed(ref rs) if rs.len()==1 && rs[0].definitions.len()==1 && rs[0].definitions[0].identity==original),
                "{found:?}"
            );
        } else {
            assert!(matches!(found, AnonymousImports::NeedInput(_)), "{found:?}");
        }
    }
}

#[test]
fn captured_anonymous_body_imports_preserve_origin_and_reject_stale_selection() {
    use lrr::frontend::{names::AnonymousImports, source_locals::OrdinaryNameSite};
    let source = "mod m{pub trait Trait{}}fn chosen(){}fn main(){use crate::m::Trait as _;fn nested(){chosen();}}";
    let (published, selection, body, graph) = ordinary_source(source);
    let outer = published.enter_body(&selection, &body).unwrap();
    let nested = outer
        .facts()
        .items
        .iter()
        .find(|i| i.item.def_key.declared_name == "nested")
        .unwrap()
        .item
        .body
        .as_ref()
        .unwrap()
        .key();
    let origin = &outer.facts().imports[0].source;
    let entered = outer.enter_nested_body(&selection, &body, &nested).unwrap();
    let locals = entered
        .local_index(&selection, &nested, &Default::default(), &[])
        .unwrap();
    let at = source.find("chosen();").unwrap() as u64;
    let found = locals
        .disclosed_anonymous_imports(
            OrdinaryNameSite {
                source: &selection,
                body: &nested,
                at,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
        )
        .unwrap();
    assert!(
        matches!(found,AnonymousImports::Disclosed(ref rs) if rs.len()==1 && rs[0].scope==origin.scope && rs[0].identity==NameNamespaces::import_identity(origin)),
        "{found:?}"
    );
    let mut stale = selection.clone();
    stale.outline.parse.content = lrr::core::digest::Digest([92; 32]);
    assert!(
        locals
            .disclosed_anonymous_imports(
                OrdinaryNameSite {
                    source: &stale,
                    body: &nested,
                    at,
                    name: "chosen",
                    context: locals.context(),
                    namespace: Namespace::Value
                },
                &graph
            )
            .is_err()
    );
}

#[test]
fn inactive_anonymous_imports_do_not_create_bindings_records_or_demands() {
    use lrr::frontend::{names::AnonymousImports, source_locals::OrdinaryNameSite};
    let facts = outline("#[cfg(any())]use missing::Trait as _;fn chosen(){}");
    let mut graph = NameNamespaces::from_outlines(&[&facts]).unwrap();
    let report = graph.resolve_imports().unwrap();
    assert!(report.pending.is_empty());
    assert_eq!(graph.disclosed_anonymous_imports(&caller()).count(), 0);
    assert!(matches!(
        graph
            .lookup(Namespace::Value, &path(&["chosen"]), &caller())
            .unwrap(),
        NameLookup::Defined(_)
    ));
    let source = "fn chosen(){}fn main(){#[cfg(any())]use missing::Trait as _;chosen();}";
    let (published, selection, body, graph) = ordinary_source(source);
    let entered = published.enter_body(&selection, &body).unwrap();
    let locals = entered
        .local_index(&selection, &body, &Default::default(), &[])
        .unwrap();
    let found = locals
        .disclosed_anonymous_imports(
            OrdinaryNameSite {
                source: &selection,
                body: &body,
                at: source.find("chosen();").unwrap() as u64,
                name: "chosen",
                context: locals.context(),
                namespace: Namespace::Value,
            },
            &graph,
        )
        .unwrap();
    assert!(
        matches!(found,AnonymousImports::Disclosed(ref rs) if rs.is_empty()),
        "{found:?}"
    );
}
