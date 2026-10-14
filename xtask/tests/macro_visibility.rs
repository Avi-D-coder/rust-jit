use lrr::{
    core::{digest::Digest, profile::ProfileIdentity},
    expand::scopes::MacroName,
    frontend::{
        hygiene::HygieneFacts,
        macro_names::{MacroPath, PathLookup},
        source_macro_routes::SourceMacroRoutes,
    },
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{outline::ScopeKey, parse::ParsedFile, token_wire as runtime},
};
use lrr_expand_adapter::{hygiene::HygieneStore, token_wire::*};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};
fn name(s: &str) -> MacroName {
    MacroName::parse(s, 2024).unwrap()
}
#[test]
fn source_macro_binding_visibility_matches_native_macos_positive_and_private_rejections() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-visibility-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), &version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! m {()=>{17_u64};} pub mod area {pub mod inner {pub(in crate::area) use crate::m as limited; pub(super) use crate::m as parent; pub(crate) use crate::m as unit_only; use crate::m as private; pub(self) use crate::m as same; pub use crate::m as public; pub fn own()->[u64;3]{[private!(),same!(),limited!()]}} pub mod sibling {pub fn check()->[u64;2]{[super::inner::limited!(),super::inner::parent!()]}}} pub fn root()->u64{area::inner::unit_only!()}";
    fs::write(work.join("provider.rs"), source).unwrap();
    let build = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=rlib",
            "--crate-name=provider",
            "provider.rs",
            "-o",
            "libprovider.rlib",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("provider.stderr"), &build.stderr).unwrap();
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let caller = "fn main(){let own=dep::area::inner::own();let sibling=dep::area::sibling::check();println!(\"{:?}\",[own[0],own[1],own[2],sibling[0],sibling[1],dep::root(),dep::area::inner::public!()]);}";
    fs::write(work.join("consumer.rs"), caller).unwrap();
    let compile = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "consumer.rs",
            "--extern",
            "dep=libprovider.rlib",
            "-o",
            "consumer",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("consumer.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let native = Command::new(work.join("consumer")).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let unit = Digest([11; 32]);
    let other = Digest([12; 32]);
    let mut store = HygieneStore::default();
    let ctx = store.root(Key(unit.0), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[ctx]).unwrap();
    let names = store.name_contexts(&[ctx]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = HygieneFacts::new(
        runtime::TokenForest::decode(&wire)
            .unwrap()
            .contexts()
            .to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let outline = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(unit, vec![], &CfgSet::default())
    .unwrap();
    let graph = SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(ctx.0),
        facts,
        &BTreeMap::new(),
    )
    .unwrap()])
    .unwrap();
    let scope = |unit, path: &[&str]| ScopeKey {
        crate_key: unit,
        module_path: path.iter().map(|s| (*s).into()).collect(),
        owner: None,
    };
    let route =
        |member| MacroPath::new(unit, vec![name("area"), name("inner"), name(member)]).unwrap();
    let mut identities = Vec::new();
    for (member, caller) in [
        ("private", scope(unit, &["area", "inner"])),
        ("same", scope(unit, &["area", "inner"])),
        ("limited", scope(unit, &["area", "inner"])),
        ("limited", scope(unit, &["area", "sibling"])),
        ("parent", scope(unit, &["area", "sibling"])),
        ("unit_only", scope(unit, &[])),
        ("public", scope(other, &[])),
    ] {
        let PathLookup::Defined(def) = graph.lookup_from(&route(member), &caller).unwrap() else {
            panic!("visibility mismatch: {member}")
        };
        identities.push(def.identity);
    }
    assert_eq!(identities.len(), values.len());
    assert!(identities.iter().all(|id| *id == identities[0]));
    let mut rejected = Vec::new();
    for member in ["limited", "parent", "unit_only", "private", "same"] {
        assert!(matches!(
            graph
                .lookup_from(&route(member), &scope(other, &[]))
                .unwrap(),
            PathLookup::Inaccessible(_)
        ));
        let file = format!("denied_{member}.rs");
        fs::write(
            work.join(&file),
            format!("fn main(){{let _=dep::area::inner::{member}!();}}"),
        )
        .unwrap();
        let output = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                &file,
                "--extern",
                "dep=libprovider.rlib",
                "-o",
                "denied",
            ])
            .current_dir(&work)
            .output()
            .unwrap();
        fs::write(work.join(format!("denied_{member}.stderr")), &output.stderr).unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("E0603"),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        rejected.push(member);
    }
    let widening = source.replace("pub fn own()", "pub use private as escaped; pub fn own()");
    fs::write(work.join("widening.rs"), &widening).unwrap();
    let result = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=rlib",
            "widening.rs",
            "-o",
            "widening.rlib",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("widening.stderr"), &result.stderr).unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("E0364"));
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"actual macOS native positive/privacy compilation matrix and parsed caller-aware macro binding lookup; module-prefix visibility and guest VM pending","selected":identities,"native":values,"private_rejections":rejected,"widening_rejection":"E0364","passed":true})).unwrap()).unwrap();
}

fn source_facts(unit: Digest) -> (runtime::Key, HygieneFacts) {
    let mut store = HygieneStore::default();
    let ctx = store.root(Key(unit.0), Edition::E2024).unwrap();
    let rows = store.resolution_contexts(&[ctx]).unwrap();
    let names = store.name_contexts(&[ctx]).unwrap();
    let wire = TokenForest::new(vec![], store.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let facts = HygieneFacts::new(
        runtime::TokenForest::decode(&wire)
            .unwrap()
            .contexts()
            .to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    (runtime::Key(ctx.0), facts)
}
fn source_graph(unit: Digest, source: &str) -> lrr::frontend::macro_names::MacroNamespaces {
    let (ctx, facts) = source_facts(unit);
    let outline = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .outline(unit, vec![], &CfgSet::default())
    .unwrap();
    SourceMacroRoutes::candidate_graph(&[SourceMacroRoutes::from_outline(
        &outline,
        runtime::Key(ctx.0),
        facts,
        &BTreeMap::new(),
    )
    .unwrap()])
    .unwrap()
}
#[test]
fn source_module_prefix_and_alias_privacy_matches_native_macos() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-module-path-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! m {()=>{17_u64};} mod hidden {pub mod visible {pub use crate::m as member;}} pub use crate::hidden::visible as exposed; use crate::hidden::visible as local_alias; pub mod outer {use crate::hidden::visible as inner_alias; pub use inner_alias::member as callable;} pub fn own()->u64{local_alias::member!()}";
    fs::write(work.join("provider.rs"), source).unwrap();
    let compile = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=rlib",
            "--crate-name=provider",
            "provider.rs",
            "-o",
            "libprovider.rlib",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("provider.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    fs::write(work.join("consumer.rs"),"fn main(){println!(\"{:?}\",[dep::exposed::member!(),dep::outer::callable!(),dep::own()]);}").unwrap();
    let compile = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "consumer.rs",
            "--extern",
            "dep=libprovider.rlib",
            "-o",
            "consumer",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("consumer.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let native = Command::new(work.join("consumer")).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let unit = Digest([41; 32]);
    let other = Digest([42; 32]);
    let graph = source_graph(unit, source);
    let scope = |unit| ScopeKey {
        crate_key: unit,
        module_path: vec![],
        owner: None,
    };
    let path =
        |parts: &[&str]| MacroPath::new(unit, parts.iter().map(|s| name(s)).collect()).unwrap();
    for (parts, caller) in [
        (vec!["exposed", "member"], scope(other)),
        (vec!["outer", "callable"], scope(other)),
        (vec!["local_alias", "member"], scope(unit)),
    ] {
        assert!(matches!(
            graph.lookup_from(&path(&parts), &caller).unwrap(),
            PathLookup::Defined(_)
        ));
    }
    let mut rejections = Vec::new();
    for (index, parts) in [
        vec!["hidden", "visible", "member"],
        vec!["local_alias", "member"],
    ]
    .iter()
    .enumerate()
    {
        assert!(matches!(
            graph.lookup_from(&path(parts), &scope(other)).unwrap(),
            PathLookup::Inaccessible(_)
        ));
        let file = format!("denied_{index}.rs");
        fs::write(
            work.join(&file),
            format!("fn main(){{let _=dep::{}!();}}", parts.join("::")),
        )
        .unwrap();
        let output = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                &file,
                "--extern",
                "dep=libprovider.rlib",
                "-o",
                "denied",
            ])
            .current_dir(&work)
            .output()
            .unwrap();
        fs::write(work.join(format!("denied_{index}.stderr")), &output.stderr).unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("E0603"));
        rejections.push(parts.join("::"));
    }
    let invalids = [
        (
            "module_width",
            "mod hidden{} pub use crate::hidden as exposed;",
            "E0365",
            vec!["exposed", "missing"],
        ),
        (
            "macro_width",
            "#[macro_export] macro_rules! m {()=>{17};} pub mod area {use crate::m as private; use crate::area as same_area; pub use same_area::private as invalid;}",
            "E0364",
            vec!["area", "invalid"],
        ),
    ];
    for (label, source, code, parts) in &invalids {
        assert!(matches!(
            source_graph(unit, source)
                .lookup_from(&path(parts), &scope(other))
                .unwrap(),
            PathLookup::Inaccessible(_)
        ));
        let file = format!("{label}.rs");
        fs::write(work.join(&file), source).unwrap();
        let result = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                "--crate-type=rlib",
                &file,
                "-o",
                "invalid.rlib",
            ])
            .current_dir(&work)
            .output()
            .unwrap();
        fs::write(work.join(format!("{label}.stderr")), &result.stderr).unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"retained original module alias paths, direct qualified macro lookup, prefix privacy, public escapes through private ancestors, module and macro reexport width; actual macOS native compilation/execution, no guest VM/completeness claim","native":values,"prefix_rejections":rejections,"width_rejections":["E0365","E0364"],"passed":true})).unwrap()).unwrap();
}

#[test]
fn macro_glob_resolution_matches_native_macos_shadowing_cycles_and_ambiguity() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-glob-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! one {()=>{17_u64};} #[macro_export] macro_rules! two {()=>{31_u64};} pub mod a {pub use crate::one as chosen;} pub mod b {pub use crate::two as chosen;} pub mod forwarded {pub use crate::a::*;} pub mod collision {pub use crate::a::*;pub use crate::b::*;} pub mod explicit {pub use crate::a::*;pub use crate::b::chosen;} pub mod cyclic_a {pub use crate::cyclic_b::*;pub use crate::a::*;} pub mod cyclic_b {pub use crate::cyclic_a::*;} pub mod restricted {pub(crate) use crate::one as local;pub use crate::one as visible;} pub mod copied {pub use crate::restricted::*;}";
    fs::write(work.join("provider.rs"), source).unwrap();
    let compile = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=rlib",
            "--crate-name=provider",
            "provider.rs",
            "-o",
            "libprovider.rlib",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("provider.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    fs::write(work.join("consumer.rs"),"fn main(){println!(\"{:?}\",[dep::forwarded::chosen!(),dep::explicit::chosen!(),dep::cyclic_b::chosen!(),dep::copied::visible!()]);}").unwrap();
    let compile = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "consumer.rs",
            "--extern",
            "dep=libprovider.rlib",
            "-o",
            "consumer",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("consumer.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let native = Command::new(work.join("consumer")).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let unit = Digest([51; 32]);
    let caller = ScopeKey {
        crate_key: Digest([52; 32]),
        module_path: vec![],
        owner: None,
    };
    let graph = source_graph(unit, source);
    let p = |parts: &[&str]| MacroPath::new(unit, parts.iter().map(|n| name(n)).collect()).unwrap();
    let first = graph.lookup_from(&p(&["one"]), &caller).unwrap();
    let second = graph.lookup_from(&p(&["two"]), &caller).unwrap();
    for (parts, expected) in [
        (vec!["forwarded", "chosen"], first.clone()),
        (vec!["explicit", "chosen"], second),
        (vec!["cyclic_b", "chosen"], first.clone()),
        (vec!["copied", "visible"], first),
    ] {
        assert_eq!(graph.lookup_from(&p(&parts), &caller).unwrap(), expected);
    }
    for (label, path, code) in [
        (
            "ambiguous",
            vec!["collision", "chosen"],
            "ambiguous_glob_imports",
        ),
        ("private", vec!["copied", "local"], "E0603"),
    ] {
        let actual = graph.lookup_from(&p(&path), &caller).unwrap();
        if label == "ambiguous" {
            assert!(matches!(actual,PathLookup::Ambiguous(defs) if defs.len()==2));
        } else {
            assert!(matches!(actual, PathLookup::Inaccessible(_)));
        }
        let file = format!("{label}.rs");
        fs::write(
            work.join(&file),
            format!("fn main(){{let _=dep::{}!();}}", path.join("::")),
        )
        .unwrap();
        let result = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                &file,
                "--extern",
                "dep=libprovider.rlib",
                "-o",
                "invalid",
            ])
            .current_dir(&work)
            .output()
            .unwrap();
        fs::write(work.join(format!("{label}.stderr")), &result.stderr).unwrap();
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"parsed macro-only globs, strong shadowing, rooted glob cycles, canonical definition identity, private visibility and ambiguity; actual native macOS execution and ambiguous_glob_imports/E0603 diagnostics, no guest VM or membership seal claim","native":native,"diagnostics":["ambiguous_glob_imports","E0603"],"passed":true})).unwrap()).unwrap();
}

#[test]
fn module_globs_match_native_macos_forwarding_shadowing_cycles_and_privacy() {
    use lrr::frontend::macro_names::ModuleLookup;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-module-glob-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! one {()=>{17_u64};} #[macro_export] macro_rules! two {()=>{31_u64};} mod a {pub mod member {pub use crate::one as call;} pub(crate) mod local {pub use crate::one as call;}} mod b {pub mod member {pub use crate::two as call;}} pub mod forwarded {pub use crate::a::*;} pub mod chain {pub use crate::forwarded::*;} pub mod explicit {pub use crate::a::*; pub use crate::b::member;} pub mod conflict {pub use crate::a::*; pub use crate::b::*;} pub mod cyclic_a {pub use crate::cyclic_b::*;pub use crate::a::*;} pub mod cyclic_b {pub use crate::cyclic_a::*;} pub use chain::member as named;";
    fs::write(work.join("provider.rs"), source).unwrap();
    let provider = Command::new("rustc")
        .args([
            "+1.99.0",
            "--edition=2024",
            "--crate-type=rlib",
            "--crate-name=provider",
            "provider.rs",
            "-o",
            "libprovider.rlib",
        ])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("provider.stderr"), &provider.stderr).unwrap();
    assert!(
        provider.status.success(),
        "{}",
        String::from_utf8_lossy(&provider.stderr)
    );
    let compile = |label: &str, source: &str| {
        fs::write(work.join(format!("{label}.rs")), source).unwrap();
        let output = Command::new("rustc")
            .args([
                "+1.99.0",
                "--edition=2024",
                &format!("{label}.rs"),
                "--extern",
                "dep=libprovider.rlib",
                "-o",
                label,
            ])
            .current_dir(&work)
            .output()
            .unwrap();
        fs::write(work.join(format!("{label}.stderr")), &output.stderr).unwrap();
        output
    };
    let consumer = compile(
        "consumer",
        "fn main(){println!(\"{:?}\",[dep::forwarded::member::call!(),dep::chain::member::call!(),dep::explicit::member::call!(),dep::named::call!(),dep::cyclic_b::member::call!()]);}",
    );
    assert!(
        consumer.status.success(),
        "{}",
        String::from_utf8_lossy(&consumer.stderr)
    );
    let native = Command::new(work.join("consumer")).output().unwrap();
    assert!(native.status.success());
    fs::write(work.join("native.stdout"), &native.stdout).unwrap();
    let values: Vec<u64> = serde_json::from_slice(&native.stdout).unwrap();
    let unit = Digest([90; 32]);
    let caller = ScopeKey {
        crate_key: Digest([91; 32]),
        module_path: vec![],
        owner: None,
    };
    let graph = source_graph(unit, source);
    let components = |parts: &[&str]| parts.iter().map(|p| name(p)).collect::<Vec<_>>();
    let p = |parts: &[&str]| MacroPath::new(unit, components(parts)).unwrap();
    for (route, target, definition) in [
        (&["forwarded", "member"][..], &["a", "member"][..], "one"),
        (&["chain", "member"], &["a", "member"], "one"),
        (&["explicit", "member"], &["b", "member"], "two"),
        (&["named"], &["a", "member"], "one"),
        (&["cyclic_b", "member"], &["a", "member"], "one"),
    ] {
        assert!(
            matches!(graph.lookup_module_from(unit,&components(route),&caller).unwrap(),ModuleLookup::Defined {unit:selected,components:names,..} if selected==unit && names==components(target))
        );
        let mut invocation = route.to_vec();
        invocation.push("call");
        assert_eq!(
            graph.lookup_from(&p(&invocation), &caller).unwrap(),
            graph.lookup_from(&p(&[definition]), &caller).unwrap()
        );
    }
    assert!(
        matches!(graph.lookup_module_from(unit,&components(&["conflict","member"]),&caller).unwrap(),ModuleLookup::Ambiguous(paths) if paths.len()==2)
    );
    assert!(matches!(
        graph
            .lookup_module_from(unit, &components(&["forwarded", "local"]), &caller)
            .unwrap(),
        ModuleLookup::Inaccessible { .. }
    ));
    for (label, path, diagnostic) in [
        (
            "ambiguous",
            "conflict::member::call",
            "ambiguous_glob_imports",
        ),
        ("private", "forwarded::local::call", "E0603"),
    ] {
        let output = compile(label, &format!("fn main(){{let _=dep::{path}!();}}"));
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"parsed source module glob forwarding, named aliases, strong shadowing, rooted cycles, canonical module/macro identity, ambiguity and privacy; actual Rust1.99 macOS native compilation/execution; no guest VM/seal claim","native":values,"diagnostics":["ambiguous_glob_imports","E0603"],"passed":true})).unwrap()).unwrap();
}

#[test]
fn original_item_invocation_paths_match_native_macos_and_textual_shadowing() {
    use lrr::frontend::source_macros::SourceMacroIndex;
    use lrr::syntax::spans::SourceRange;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-invocation-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! root {()=>{pub fn value()->u64{17}};} mod first {pub use crate::root as r#gen; self::r#gen!();} mod second {super::root!();} mod third {crate::root!();} mod local {use crate::root as chosen; macro_rules! chosen {()=>{pub fn value()->u64{31}};} chosen!();} fn main(){println!(\"{:?}\",[first::value(),second::value(),third::value(),local::value()]);}";
    fs::write(work.join("native.rs"), source).unwrap();
    let compile = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "native.rs", "-o", "native"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(work.join("native")).output().unwrap();
    assert!(output.status.success());
    fs::write(work.join("native.stdout"), &output.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    let unit = Digest([99; 32]);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
    let (ctx, facts) = source_facts(unit);
    let index = SourceMacroIndex::from_outline(&outline, ctx, facts).unwrap();
    let graph = source_graph(unit, source);
    let root = graph
        .lookup(&MacroPath::new(unit, vec![name("root")]).unwrap())
        .unwrap();
    let mut selected = Vec::new();
    for (text, module) in [
        ("self::r#gen!();", "first"),
        ("super::root!();", "second"),
        ("crate::root!();", "third"),
        ("chosen!();", "local"),
    ] {
        let start = source.find(text).unwrap();
        let call = parsed
            .macro_call_at(SourceRange {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + text.len()).unwrap(),
            })
            .unwrap();
        let result = index
            .lookup_invocation(
                &call,
                &graph,
                &ScopeKey {
                    crate_key: unit,
                    module_path: vec![module.into()],
                    owner: None,
                },
            )
            .unwrap();
        if module == "local" {
            assert_ne!(result, root);
        } else {
            assert_eq!(result, root);
        }
        let PathLookup::Defined(def) = result else {
            panic!("original source invocation did not select a macro");
        };
        selected.push(def.identity.hex());
    }
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"exact selected original item macro invocation AST paths, relative/self/super/crate/raw routes and textual precedence with actual pinned hygiene facts; actual macOS native generated functions; no guest VM or generated-source integration claim","native":native,"selected":selected,"passed":true})).unwrap()).unwrap();
}

#[test]
fn original_item_cfg_invocation_selection_matches_native_macos() {
    use lrr::{frontend::source_macros::SourceMacroIndex, syntax::spans::SourceRange};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned();
    let work = root.join(format!(
        "artifacts/macos-first/m007-invocation-cfg-native-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&work).unwrap();
    let version = Command::new("rustc")
        .args(["+1.99.0", "-Vv"])
        .output()
        .unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("host: aarch64-apple-darwin"));
    fs::write(work.join("rustc-version.txt"), version.stdout).unwrap();
    let source = "#[macro_export] macro_rules! m {()=>{pub fn value()->u64{17}};} fn value()->u64{99} #[cfg(any())] #[missing] crate::m!(); mod enabled {#[cfg_attr(any(), missing)] crate::m!();} fn main(){println!(\"{:?}\",[value(),enabled::value()]);}";
    fs::write(work.join("native.rs"), source).unwrap();
    let compile = Command::new("rustc")
        .args(["+1.99.0", "--edition=2024", "native.rs", "-o", "native"])
        .current_dir(&work)
        .output()
        .unwrap();
    fs::write(work.join("native.stderr"), &compile.stderr).unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let output = Command::new(work.join("native")).output().unwrap();
    assert!(output.status.success());
    fs::write(work.join("native.stdout"), &output.stdout).unwrap();
    let native: Vec<u64> = serde_json::from_slice(&output.stdout).unwrap();
    let unit = Digest([103; 32]);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed.outline(unit, vec![], &CfgSet::default()).unwrap();
    let (ctx, facts) = source_facts(unit);
    let index = SourceMacroIndex::from_outline(&outline, ctx, facts).unwrap();
    let graph = source_graph(unit, source);
    for (text, module, disabled) in [
        ("#[cfg(any())] #[missing] crate::m!();", vec![], true),
        (
            "#[cfg_attr(any(), missing)] crate::m!();",
            vec!["enabled".into()],
            false,
        ),
    ] {
        let start = source.find(text).unwrap();
        let call = parsed
            .macro_call_at(SourceRange {
                start: u32::try_from(start).unwrap(),
                end: u32::try_from(start + text.len()).unwrap(),
            })
            .unwrap();
        let actual = index
            .lookup_invocation(
                &call,
                &graph,
                &ScopeKey {
                    crate_key: unit,
                    module_path: module,
                    owner: None,
                },
            )
            .unwrap();
        if disabled {
            assert_eq!(actual, PathLookup::Absent);
        } else {
            assert!(matches!(actual, PathLookup::Defined(_)));
        }
    }
    fs::write(work.join("report.json"),serde_json::to_vec_pretty(&serde_json::json!({"task":"M007","scope":"original item invocation cfg filtering and inactive cfg_attr with exact AST selection and actual pinned hygiene; actual native macOS compile/execution preserves existing function against disabled duplicate generation; active attribute execution and guest VM pending","native":native,"passed":true})).unwrap()).unwrap();
}
