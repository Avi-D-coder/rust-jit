use super::*;

#[test]
fn unchanged_library_sources_export_real_std_macro_definitions() {
    for edition in [2015, 2018, 2021, 2024] {
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let library = LibraryMacros::for_profile(&profile).unwrap();
        // Ordinary macro_rules exports at the crate roots.
        for (krate, name) in [
            ("core", "assert_eq"),
            ("core", "assert_ne"),
            ("core", "matches"),
            ("core", "unreachable"),
            ("core", "todo"),
            ("core", "write"),
            ("alloc", "vec"),
            ("alloc", "format"),
            ("std", "println"),
            ("std", "print"),
            ("std", "eprintln"),
            ("std", "panic"),
        ] {
            let route = library
                .exported(krate, name)
                .unwrap_or_else(|| panic!("{krate}::{name} is not exported"));
            assert_eq!(route.definition.definition_edition, LIBRARY_EDITION);
            assert_eq!(Some(route.definition.defining_unit), library.unit(krate));
        }
        // Builtin-marked definitions name their compiler expander.
        let builtin = |krate, name| {
            let route = library.exported(krate, name).unwrap();
            library.builtin(&route.definition).map(str::to_owned)
        };
        assert_eq!(builtin("core", "assert").as_deref(), Some("assert"));
        assert_eq!(builtin("core", "format_args").as_deref(), Some("format_args"));
        assert_eq!(builtin("core", "panic").as_deref(), Some("core_panic"));
        assert_eq!(builtin("std", "panic").as_deref(), Some("std_panic"));
        assert_eq!(builtin("core", "assert_eq"), None);
        // Edition panic macros are `macro` 2.0 items in core::panic.
        let core = library.unit("core").unwrap();
        for name in ["panic_2015", "panic_2021", "unreachable_2015", "unreachable_2021"] {
            let path = MacroPath::new(
                core,
                vec![
                    MacroName::parse("panic", LIBRARY_EDITION).unwrap(),
                    MacroName::parse(name, LIBRARY_EDITION).unwrap(),
                ],
            )
            .unwrap();
            assert!(library.defines(&path), "core::panic::{name}");
        }
    }
}

#[test]
fn std_root_reexports_resolve_through_the_ordinary_macro_graph() {
    let profile = ProfileIdentity::current();
    let library = LibraryMacros::for_profile(&profile).unwrap();
    let graph = SourceMacroRoutes::discovery_graph(library.routes()).unwrap();
    let std = library.unit("std").unwrap();
    let caller = crate::syntax::outline::ScopeKey {
        crate_key: std,
        module_path: vec![],
        owner: None,
    };
    let mut missing = Vec::new();
    for name in [
        "assert_eq", "assert_ne", "matches", "todo", "unreachable", "unimplemented",
        "vec", "format", "println", "print", "eprintln", "panic", "assert",
        "format_args", "write", "writeln", "line", "stringify", "concat",
    ] {
        let path = MacroPath::new(std, vec![MacroName::parse(name, LIBRARY_EDITION).unwrap()])
            .unwrap();
        match graph.lookup_from(&path, &caller).unwrap() {
            crate::frontend::macro_names::PathLookup::Defined(_) => {}
            other => missing.push(format!("{name}: {other:?}")),
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}

#[test]
fn edition_panic_macros_resolve_from_another_crate() {
    let profile = ProfileIdentity::current();
    let library = LibraryMacros::for_profile(&profile).unwrap();
    let graph = SourceMacroRoutes::discovery_graph(library.routes()).unwrap();
    let core = library.unit("core").unwrap();
    let caller = crate::syntax::outline::ScopeKey {
        crate_key: crate::core::digest::Digest([9; 32]),
        module_path: vec![],
        owner: None,
    };
    for name in ["panic_2015", "panic_2021", "unreachable_2015", "unreachable_2021"] {
        let path = MacroPath::new(
            core,
            vec![
                MacroName::parse("panic", LIBRARY_EDITION).unwrap(),
                MacroName::parse(name, LIBRARY_EDITION).unwrap(),
            ],
        )
        .unwrap();
        assert!(
            matches!(
                graph.lookup_from(&path, &caller).unwrap(),
                crate::frontend::macro_names::PathLookup::Defined(_)
            ),
            "core::panic::{name}: {:?}",
            graph.lookup_from(&path, &caller).unwrap()
        );
    }
}

#[test]
fn actual_library_definition_value_namespace_requires_source_module_closure() {
    use crate::{
        db::memo::Database,
        expand::{source_discovery::SourceDiscovery, source_declarative},
        frontend::compile_input_catalog::CompileInputCatalog,
        source::manifest::Namespace,
    };
    let profile = ProfileIdentity::current();
    let library = LibraryMacros::for_profile(&profile).unwrap();
    let definition = library.resolve_path("core", &["cmp", "PartialEq"]).unwrap();
    let owner = library.providers().iter().find(|provider| {
        provider.macro_index().definition(definition.identity).is_some()
    }).unwrap();
    let declaration = owner.module_source().outline().items.iter().find(|item| {
        item.def_key.fingerprint() == definition.identity
    }).unwrap();
    let tree = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rustc/rust/library");
    let snapshot = crate::source::loader::SourceLoader::in_source_tree(
        &tree, SourcePath::new("core/src/lib.rs").unwrap(),
        owner.module_source().selection().outline.parse.profile.clone(),
    ).unwrap().capture().expect("actual cfg-selected core module capture");
    let mut units = Vec::new();
    let mut selections = Vec::new();
    crate::source::module_tree::capture_inputs(
        snapshot.files(), &snapshot.project().root_source, definition.defining_unit,
        &owner.module_source().selection().outline.parse.profile,
        owner.module_source().captured_macro_cfg().unwrap(), &mut units, &mut selections,
    ).unwrap();
    let inputs = PreparedSourceInputs::refresh(None, units, BTreeMap::new()).unwrap();
    let sources = selections.iter().map(|selection| {
        let SourceSelectionOutcome::Selected(source) = inputs.select(selection).unwrap() else { panic!() };
        let policy = source.macro_prelude_policy().unwrap();
        assert!(policy.no_core && !policy.standard, "actual core modules inherit their root");
        source
    }).collect();
    // This metadata preparation owns a separate publication, never guest facade
    // definitions or a second runtime interpreter.
    let mut metadata = Database::default();
    inputs.publish(&mut metadata).unwrap();
    metadata.begin_expansion_epoch(1).unwrap();
    let discovery = SourceDiscovery::new_deferred_imports(
        sources, declaration.scope.clone(), Namespace::Value,
        Digest::of_bytes(b"actual library definition source namespace"), 1,
    ).unwrap();
    let mut discovery = discovery.with_declarative_compiler(
        Digest::of_bytes(b"actual library definition compile context"),
        source_declarative::engine_version(),
        Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
    ).expect("real library definition namespace source/module preparation");
    // Construction alone is not a namespace receipt. Drive the ordinary
    // preparation coordinator; missing dependencies remain real failures.
    loop {
        use crate::expand::source_discovery::SourceDiscoveryStep;
        match discovery.poll().expect("actual library definition namespace discovery") {
            SourceDiscoveryStep::Sealed(_) => {
                discovery.publish(&mut metadata).unwrap();
                break;
            }
            SourceDiscoveryStep::NeedExpansion { request } => {
                let record = request.expand().expect("actual library source macro expansion");
                discovery.admit_expansion(Arc::new(record)).unwrap();
            }
            SourceDiscoveryStep::NeedGeneratedExpansion { request } => {
                let record = request.expand().expect("actual library generated macro expansion");
                discovery.admit_expansion(Arc::new(record)).unwrap();
            }
            SourceDiscoveryStep::Incomplete { diagnostic, blocked } => {
                panic!("actual library namespace incomplete: {diagnostic:?}; {blocked:?}");
            }
            _ => panic!("actual library namespace needs an unestablished preparation dependency"),
        }
    }
}
