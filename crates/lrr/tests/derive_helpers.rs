use lrr::{
    core::digest::{Digest, StableEncode},
    expand::{
        derive::{DeriveHelperBinding, HelperLookup, ItemHelperScope},
        identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
    },
};
fn key(export: &str, id: u8) -> ExpansionKey {
    ExpansionKey {
        macro_identity: MacroIdentity::Procedural {
            artifact: Digest([7; 32]),
            export: export.into(),
            kind: ProceduralKind::Derive,
        },
        invocation: Digest([id; 32]),
        input_tokens: Digest([9; 32]),
        consumer_compile_env: Digest([10; 32]),
        parent_context: Digest([11; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([12; 32]),
        procedural_epoch: Some(1),
    }
}
#[test]
fn derive_helpers_require_exact_active_identity_and_item_cfg_scope() {
    let first = key("First", 21);
    let second = key("Second", 22);
    let owner = Digest([23; 32]);
    let cfg = Digest([24; 32]);
    let scope = ItemHelperScope::new(
        owner,
        cfg,
        vec![
            DeriveHelperBinding::new(
                first.clone(),
                Digest([25; 32]),
                ["shared".into(), "r#type".into()],
            )
            .unwrap(),
            DeriveHelperBinding::new(second.clone(), Digest([26; 32]), ["shared".into()]).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(
        scope
            .helper_owners(owner, cfg, &[first.clone(), second.clone()], "shared")
            .unwrap(),
        vec![first.clone(), second]
    );
    assert_eq!(
        scope
            .helper_owners(owner, cfg, std::slice::from_ref(&first), "r#type")
            .unwrap(),
        vec![first.clone()]
    );
    assert!(
        scope
            .helper_owners(owner, cfg, &[], "shared")
            .unwrap()
            .is_empty()
    );
    let mut changed = first.clone();
    changed.procedural_epoch = Some(2);
    assert!(
        matches!(scope.lookup_helpers(owner,cfg,std::slice::from_ref(&changed),"shared").unwrap(),HelperLookup::NeedDeclarations(keys) if keys==vec![changed])
    );
    let mut alias = first.clone();
    alias.macro_identity = key("Other", 21).macro_identity;
    assert!(
        matches!(scope.lookup_helpers(owner,cfg,std::slice::from_ref(&alias),"shared").unwrap(),HelperLookup::NeedDeclarations(keys) if keys==vec![alias])
    );
    assert!(
        scope
            .helper_owners(
                Digest([27; 32]),
                cfg,
                std::slice::from_ref(&first),
                "shared"
            )
            .is_err()
    );
    assert!(
        scope
            .helper_owners(
                owner,
                Digest([28; 32]),
                std::slice::from_ref(&first),
                "shared"
            )
            .is_err()
    );
    let empty = ItemHelperScope::new(owner, cfg, vec![]).unwrap();
    assert_ne!(scope.fingerprint(), empty.fingerprint());
}
#[test]
fn helper_declarations_canonicalize_duplicates_and_reject_non_derive_or_malformed_names() {
    let mut wrong = key("Wrong", 31);
    wrong.macro_identity = MacroIdentity::Procedural {
        artifact: Digest([7; 32]),
        export: "Wrong".into(),
        kind: ProceduralKind::Attribute,
    };
    assert!(DeriveHelperBinding::new(wrong, Digest([8; 32]), ["helper".into()]).is_err());
    for names in [vec!["path::helper".into()], vec!["not valid".into()]] {
        assert!(DeriveHelperBinding::new(key("First", 32), Digest([8; 32]), names).is_err());
    }
    let binding = DeriveHelperBinding::new(
        key("First", 32),
        Digest([8; 32]),
        ["helper".into(), "r#helper".into()],
    )
    .unwrap();
    assert_eq!(
        binding
            .helpers()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["helper"]
    );
}
