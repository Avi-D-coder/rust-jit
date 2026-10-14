use super::*;
use crate::expand::declarative_test_fixture as fixture;
use crate::{
    core::profile::ProfileIdentity,
    expand::identity::MacroIdentity,
    source::snapshot::{SourceBlob, SourcePath},
    syntax::outline::ScopeKey,
};
use lrr_expand_adapter::{
    declarative::{DeclarativeMacro, DeclarativeSite},
    token_wire::{Edition, Key},
};

#[test]
fn generated_module_inert_attribute_replay_preserves_retained_scope_kind() {
    let cfg = CfgSet::guest_default();
    let parent = ParsedFile::parse_declarations(
        &SourceBlob::new(b"make!();".to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap()
    .scan_declarations(
        SourcePath::new("main.rs").unwrap(),
        ScopeKey {
            crate_key: Digest([2; 32]),
            module_path: vec![],
            owner: None,
        },
        &cfg,
        Default::default(),
    )
    .unwrap();
    let input = fixture::input("");
    let key = ExpansionKey {
        macro_identity: MacroIdentity::Declaration {
            definition: Digest([1; 32]),
        },
        invocation: parent.producers()[0].identity,
        input_tokens: Digest::of_bytes(&input.encode().unwrap()),
        consumer_compile_env: Digest([4; 32]),
        parent_context: Digest([5; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([6; 32]),
        procedural_epoch: None,
    };
    let definition = DeclarativeMacro::new(fixture::definition(
        "()=>{#[allow(dead_code)] fn cold(){}};",
        Edition::E2024,
    ))
    .unwrap();
    let (expanded, _) = definition
        .expand_hygienic_with_limits(
            &input,
            fixture::call_site(&input),
            DeclarativeSite {
                expansion: Key(key.fingerprint().0),
                defining_unit: Key([2; 32]),
                definition_edition: Edition::E2024,
            },
            2_000_000,
            32 * 1024 * 1024,
        )
        .unwrap();
    let record = Arc::new(
        ExpansionRecord::from_wire(key.clone(), expanded.unwrap().tokens.encode().unwrap())
            .unwrap(),
    );
    let generated =
        GeneratedDeclarations::admit(&key, record, 1, &parent, &cfg, Default::default()).unwrap();
    assert_eq!(
        generated.scan().lexical_scopes()[0].kind,
        crate::syntax::declaration_scan::DeclarationScopeKind::Module
    );
    let attribute = &generated.scan().producers()[0];
    assert_eq!(attribute.unqualified_attribute.as_deref(), Some("allow"));
    assert!(
        generated
            .inert_attribute_definition(attribute.identity, &cfg)
            .unwrap()
            .is_some()
    );
    let mut changed = cfg.clone();
    changed.atoms.insert("changed_cfg".into());
    assert_eq!(
        generated
            .inert_attribute_definition(attribute.identity, &changed)
            .unwrap_err()
            .code,
        "LRR-E-INTERNAL-SNAPSHOT"
    );
}
