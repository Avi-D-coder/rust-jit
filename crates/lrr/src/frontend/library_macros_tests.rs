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
