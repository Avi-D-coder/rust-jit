//! Stock-token control for physical capture versus declaration arena prefixes.

#[cfg(test)]
mod tests {
    use crate::expand::scopes::{MacroDefinition, MacroName, ScopeKey, TextualScopes};
    use crate::{
        core::digest::Digest,
        frontend::{
            compile_input_catalog::CompileInputCatalog,
            macro_names::MacroNamespaces,
            macro_origins::ExpansionAncestry,
            macro_scopes::MacroDeclarations,
            nested_compile_inputs::{
                EagerMacroResolver, InheritedCompileInputResolver, NestedCompileInputResolver,
                NestedExpansionContext, NestedLexicalContext, NestedMacroLookup,
            },
        },
        syntax::{outline::ScopeKey as SourceScope, parse::ParseKey},
    };
    use lrr_expand_adapter::token_wire as wire;

    #[test]
    fn rustc_nested_operand_retains_argument_lexical_prefix_and_physical_source() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        if let Some(expected) = std::env::var_os("LRR_VALIDATION_SOURCE_ROOT") {
            assert_eq!(
                std::fs::canonicalize(root).unwrap(),
                std::fs::canonicalize(expected).unwrap()
            );
            assert_eq!(
                include_bytes!("../frontend/nested_compile_inputs.rs").as_slice(),
                std::fs::read(root.join("crates/lrr/src/frontend/nested_compile_inputs.rs"))
                    .unwrap()
            );
            assert_eq!(
                include_bytes!("../expand/source_declarative.rs").as_slice(),
                std::fs::read(root.join("crates/lrr/src/expand/source_declarative.rs")).unwrap()
            );
        }
        let relative = "tests/ui/macros/macro-deep_expansion.rs";
        let source =
            std::fs::read(root.join("fixtures/upstream/rustc-checkout").join(relative)).unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("fixtures/upstream/rustc-checkout/manifest.json")).unwrap(),
        )
        .unwrap();
        let entry = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["path"] == relative)
            .unwrap();
        assert_eq!(
            source.as_slice(),
            include_bytes!(
                "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-deep_expansion.rs"
            )
        );
        assert_eq!(
            entry["bytes"].as_u64().unwrap(),
            u64::try_from(source.len()).unwrap()
        );
        assert_eq!(
            manifest["commit"],
            "ae0fe511b15ab760d7e963b13389d1bc2ab42b18"
        );
        let unit = Digest::of_bytes(b"m016-stock-nested-prefix-unit");
        let content = Digest::of_bytes(&source);
        let forest = wire::parse_source(
            std::str::from_utf8(&source).unwrap(),
            wire::SourceIdentity {
                unit: wire::Key(unit.0),
                content: wire::Key(content.0),
                path: relative.into(),
            },
            wire::ContextRecord {
                parent: None,
                expansion: None,
                transparency: wire::Transparency::Opaque,
                defining_unit: wire::Key(unit.0),
                edition: wire::Edition::E2024,
            },
            wire::Key(Digest::of_bytes(b"m016-stock-nested-prefix-outline").0),
        )
        .unwrap();
        fn concat_group(trees: &[wire::TokenTree]) -> Option<&wire::TokenTree> {
            for window in trees.windows(3) {
                if matches!(&window[0], wire::TokenTree::Ident { text, .. } if text == "concat")
                    && matches!(&window[1], wire::TokenTree::Punct { scalar: '!', .. })
                    && matches!(&window[2], wire::TokenTree::Group { .. })
                {
                    return Some(&window[2]);
                }
            }
            trees.iter().find_map(|tree| match tree {
                wire::TokenTree::Group { children, .. } => concat_group(children),
                _ => None,
            })
        }
        let wire::TokenTree::Group {
            delimiter,
            open,
            close,
            children,
        } = concat_group(forest.roots()).unwrap()
        else {
            panic!("stock concat group absent")
        };
        // Retain the original foo!() tokens and physical spans. The synthetic
        // declaration arena below is an LRR prefix/capture control, not a full
        // execution claim for the upstream main/assert_eq! run-pass case.
        assert!(matches!(&children[0], wire::TokenTree::Ident { text, .. } if text == "foo"));
        let operand = wire::TokenForest::new(
            vec![wire::TokenTree::Group {
                delimiter: *delimiter,
                open: *open,
                close: *close,
                children: children[..3].to_vec(),
            }],
            forest.contexts().to_vec(),
            forest.spans().to_vec(),
        )
        .unwrap();
        let operand =
            crate::syntax::token_wire::TokenForest::decode(&operand.encode().unwrap()).unwrap();
        let arguments = crate::expand::include_arguments::IncludeArguments::parse(
            &crate::expand::compile_input_identity::descriptor("include")
                .unwrap()
                .unwrap(),
            &operand,
        )
        .unwrap();
        let nested = arguments.nested_invocation().unwrap();
        let reference = nested
            .arguments()
            .spans()
            .iter()
            .find(|s| s.key() == nested.reference_span())
            .unwrap();
        assert!(reference.start > 6);
        let facts = crate::expand::source_hygiene::output_facts(&operand).unwrap();
        let ancestry = ExpansionAncestry::default();
        let early = MacroDefinition {
            identity: Digest::of_bytes(b"m016-prefix-early"),
            defining_unit: unit,
            definition_edition: 2024,
        };
        let later = MacroDefinition {
            identity: Digest::of_bytes(b"m016-prefix-later"),
            defining_unit: unit,
            definition_edition: 2024,
        };
        let scope = ScopeKey::root(unit);
        let mut textual = TextualScopes::default();
        textual.add_root(scope).unwrap();
        let mut declarations = MacroDeclarations::default();
        for (at, definition) in [(2, &early), (6, &later)] {
            textual
                .define(
                    scope,
                    at,
                    MacroName::parse("foo", 2024).unwrap(),
                    definition.clone(),
                )
                .unwrap();
            declarations
                .register(
                    definition.clone(),
                    reference.context,
                    None,
                    &facts,
                    &ancestry,
                )
                .unwrap();
        }
        let namespaces = MacroNamespaces::default();
        let compiler = CompileInputCatalog::new(
            Digest::of_bytes(b"m016-prefix-compiler"),
            2024,
            [crate::expand::compile_input_identity::descriptor("include")
                .unwrap()
                .unwrap()],
        )
        .unwrap();
        let caller = NestedLexicalContext {
            source: ParseKey {
                content,
                profile: crate::core::profile::ProfileIdentity::current(),
            },
            scope: SourceScope {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            },
            textual_scope: scope,
            origin: None,
        };
        let inner = NestedCompileInputResolver {
            namespaces: &namespaces,
            textual: &textual,
            declarations: &declarations,
            hygiene: &facts,
            ancestry: &ancestry,
            compiler: &compiler,
        };
        assert!(
            matches!(inner.resolve(nested, &caller).unwrap(), NestedMacroLookup::Other(definition) if definition == later)
        );
        let resolver = InheritedCompileInputResolver {
            inner,
            outers: &[],
            lexical_before: Some(5),
        };
        let env = crate::project::tool_manifest::CompileEnv {
            unit,
            host: "aarch64-apple-darwin".into(),
            target: "aarch64-apple-darwin".into(),
            cfg: Default::default(),
            cargo: Default::default(),
            rustc_env: Default::default(),
            declared: Default::default(),
            out_dir: None,
        };
        let environment =
            crate::expand::builtins::FrozenCompileEnvironment::new(unit, &env).unwrap();
        let context = NestedExpansionContext {
            environment: &environment,
            engine_version: Digest::of_bytes(b"m016-prefix-engine"),
        };
        for selection in [
            resolver
                .resolve_expansion(nested, &caller, &context)
                .unwrap(),
            resolver
                .resolve_returned_expansion(
                    nested,
                    &caller,
                    &context,
                    &facts,
                    u64::from(reference.start),
                )
                .unwrap(),
        ] {
            assert!(
                matches!(selection.lookup(), NestedMacroLookup::Other(definition) if definition == &early)
            );
            assert!(
                matches!(selection.key().unwrap().macro_identity, crate::expand::identity::MacroIdentity::Declaration { definition } if definition == early.identity)
            );
            assert_eq!(selection.key().unwrap().invocation, nested.identity());
            assert_eq!(
                selection.key().unwrap().parent_context,
                Digest(reference.context.0)
            );
        }
        let mut wrong = caller;
        wrong.source.content = Digest::of_bytes(b"unrelated-source");
        assert_eq!(
            resolver
                .resolve_expansion(nested, &wrong, &context)
                .err()
                .unwrap()
                .code,
            "LRR-E-MACRO-SCOPE"
        );
        wrong.source.content = content;
        wrong.scope.crate_key = Digest::of_bytes(b"unrelated-unit");
        assert_eq!(
            resolver
                .resolve_expansion(nested, &wrong, &context)
                .err()
                .unwrap()
                .code,
            "LRR-E-MACRO-SCOPE"
        );
        assert_eq!(
            std::fs::read(root.join("fixtures/upstream/rustc-checkout").join(relative)).unwrap(),
            source
        );
    }
}
