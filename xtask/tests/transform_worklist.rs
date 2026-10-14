//! M010 deterministic transforms; unresolved slots and stale results cannot publish.
use lrr::{
    core::digest::{Digest, StableEncode},
    expand::{
        identity::{ExpansionKey, GrammarSlot, MacroIdentity, ProceduralKind},
        record::ExpansionRecord,
        worklist::{AttributeSlot, DeriveSlot, ItemPlan, ItemTransformWorklist, TransformStep},
    },
    syntax::token_wire::TokenForest,
};
use std::sync::Arc;
fn tokens(text: &str) -> Arc<TokenForest> {
    use lrr_expand_adapter::token_wire::{
        self, ContextRecord, Edition, Key, SourceIdentity, Transparency,
    };
    let forest = token_wire::parse_source(
        text,
        SourceIdentity {
            unit: Key([1; 32]),
            content: Key(Digest::of_bytes(text.as_bytes()).0),
            path: "worklist.rs".into(),
        },
        ContextRecord {
            parent: None,
            expansion: None,
            transparency: Transparency::Opaque,
            defining_unit: Key([1; 32]),
            edition: Edition::E2024,
        },
        Key([2; 32]),
    )
    .unwrap();
    Arc::new(TokenForest::decode(&forest.encode().unwrap()).unwrap())
}
fn key(id: u8, kind: ProceduralKind) -> ExpansionKey {
    ExpansionKey {
        macro_identity: MacroIdentity::Procedural {
            artifact: Digest([3; 32]),
            export: format!("macro_{id}"),
            kind,
        },
        invocation: Digest([id; 32]),
        input_tokens: Digest([4; 32]),
        consumer_compile_env: Digest([5; 32]),
        parent_context: Digest([6; 32]),
        definition_edition: 2024,
        call_edition: 2024,
        grammar: GrammarSlot::Items,
        engine_version: Digest([7; 32]),
        procedural_epoch: Some(1),
    }
}
fn plan(input: Arc<TokenForest>, attrs: Vec<AttributeSlot>, derives: Vec<DeriveSlot>) -> ItemPlan {
    ItemPlan::new(Digest([8; 32]), Digest([9; 32]), input, attrs, derives).unwrap()
}
#[test]
fn worklist_suspends_lexically_and_replacement_requires_exact_reanalysis() {
    let first = key(10, ProceduralKind::Attribute);
    let stale = key(11, ProceduralKind::Attribute);
    let mut work = ItemTransformWorklist::new(
        plan(
            tokens("fn original() {}"),
            vec![
                AttributeSlot::Unresolved(Digest([12; 32])),
                AttributeSlot::expand(stale.clone()),
            ],
            vec![DeriveSlot::expand(key(13, ProceduralKind::Derive))],
        ),
        Digest([5; 32]),
        1,
    )
    .unwrap();
    assert!(matches!(
        work.poll(),
        TransformStep::NeedAttribute { index: 0, .. }
    ));
    assert!(work.resolve_attribute(1, 0, AttributeSlot::Inert).is_err());
    work.resolve_attribute(0, 0, AttributeSlot::expand(first.clone()))
        .unwrap();
    let replacement = tokens("struct Generated;");
    assert!(
        work.admit(ExpansionRecord::from_tokens(stale, &replacement).unwrap())
            .is_err()
    );
    work.admit(ExpansionRecord::from_tokens(first, &replacement).unwrap())
        .unwrap();
    let ticket = match work.poll() {
        TransformStep::NeedReanalysis { ticket, .. } => *ticket,
        _ => panic!("replacement must suspend"),
    };
    assert!(
        work.replan(Digest([0; 32]), plan(replacement.clone(), vec![], vec![]))
            .is_err()
    );
    assert!(
        work.replan(ticket, plan(tokens("struct Wrong;"), vec![], vec![]))
            .is_err()
    );
    work.replan(ticket, plan(replacement.clone(), vec![], vec![]))
        .unwrap();
    match work.poll() {
        TransformStep::Complete { item, appended } => {
            assert_eq!(item.as_ref(), replacement.as_ref());
            assert!(appended.is_empty());
        }
        _ => panic!("old actions must be discarded"),
    }
}
#[test]
fn worklist_attributes_precede_derives_and_siblings_append_to_unchanged_item() {
    let original = tokens("struct Original;");
    let first = key(21, ProceduralKind::Derive);
    let second = key(22, ProceduralKind::Derive);
    let mut work = ItemTransformWorklist::new(
        plan(
            original.clone(),
            vec![AttributeSlot::Unresolved(Digest([23; 32]))],
            vec![
                DeriveSlot::expand(first.clone()),
                DeriveSlot::Unresolved(Digest([24; 32])),
            ],
        ),
        Digest([5; 32]),
        1,
    )
    .unwrap();
    assert!(matches!(
        work.poll(),
        TransformStep::NeedAttribute { index: 0, .. }
    ));
    work.resolve_attribute(0, 0, AttributeSlot::Inert).unwrap();
    assert!(matches!(work.poll(),TransformStep::Expand(k) if k==&first));
    let first_record =
        ExpansionRecord::from_tokens(first.clone(), &tokens("fn from_first() {}")).unwrap();
    work.admit(first_record.clone()).unwrap();
    assert!(matches!(
        work.poll(),
        TransformStep::NeedDerive { index: 1, .. }
    ));
    assert!(work.admit(first_record).is_err());
    work.resolve_derive(1, 0, second.clone()).unwrap();
    work.admit(ExpansionRecord::from_tokens(second, &tokens("fn from_second() {}")).unwrap())
        .unwrap();
    match work.poll() {
        TransformStep::Complete { item, appended } => {
            assert_eq!(item.as_ref(), original.as_ref());
            assert_eq!(appended.len(), 2);
            assert_eq!(appended[0].key(), &first);
        }
        _ => panic!("derives append"),
    }
    let mut wrong = key(25, ProceduralKind::Derive);
    wrong.procedural_epoch = Some(2);
    assert!(
        ItemTransformWorklist::new(
            plan(original, vec![], vec![DeriveSlot::expand(wrong)]),
            Digest([5; 32]),
            1
        )
        .is_err()
    );
    assert_ne!(
        first.fingerprint(),
        key(22, ProceduralKind::Derive).fingerprint()
    );
}

#[test]
fn worklist_validates_builtin_identity_duplicate_invocations_and_slot_bounds() {
    let input = tokens("struct Value;");
    let mut builtin = key(31, ProceduralKind::Derive);
    builtin.macro_identity = MacroIdentity::Builtin {
        definition: Digest([32; 32]),
    };
    builtin.procedural_epoch = None;
    let mut work = ItemTransformWorklist::new(
        plan(
            input.clone(),
            vec![],
            vec![DeriveSlot::expand(builtin.clone())],
        ),
        Digest([5; 32]),
        1,
    )
    .unwrap();
    work.admit(ExpansionRecord::from_tokens(builtin.clone(), &tokens("impl Value {}")).unwrap())
        .unwrap();
    assert!(matches!(work.poll(),TransformStep::Complete{appended,..} if appended.len()==1));
    assert!(
        ItemTransformWorklist::new(
            plan(input.clone(), vec![AttributeSlot::expand(builtin)], vec![]),
            Digest([5; 32]),
            1
        )
        .is_err()
    );
    let duplicate = key(33, ProceduralKind::Derive);
    assert!(
        ItemTransformWorklist::new(
            plan(
                input.clone(),
                vec![],
                vec![
                    DeriveSlot::expand(duplicate.clone()),
                    DeriveSlot::expand(duplicate)
                ]
            ),
            Digest([5; 32]),
            1
        )
        .is_err()
    );
    assert!(
        ItemPlan::new(
            Digest([8; 32]),
            Digest([9; 32]),
            input.clone(),
            vec![AttributeSlot::Inert; 4097],
            vec![]
        )
        .is_err()
    );
    let oversized: Vec<_> = (0..1024)
        .map(|_| {
            let mut k = key(36, ProceduralKind::Derive);
            if let MacroIdentity::Procedural { export, .. } = &mut k.macro_identity {
                *export = "x".repeat(64 * 1024);
            }
            DeriveSlot::expand(k)
        })
        .collect();
    assert!(
        ItemPlan::new(
            Digest([8; 32]),
            Digest([9; 32]),
            input.clone(),
            vec![],
            oversized
        )
        .is_err()
    );
    let attribute = key(34, ProceduralKind::Attribute);
    let mut work = ItemTransformWorklist::new(
        plan(
            input.clone(),
            vec![AttributeSlot::expand(attribute.clone())],
            vec![],
        ),
        Digest([5; 32]),
        1,
    )
    .unwrap();
    work.admit(ExpansionRecord::from_tokens(attribute.clone(), &input).unwrap())
        .unwrap();
    let ticket = match work.poll() {
        TransformStep::NeedReanalysis { ticket, .. } => *ticket,
        _ => unreachable!(),
    };
    assert!(
        work.replan(
            ticket,
            plan(
                input.clone(),
                vec![AttributeSlot::expand(attribute)],
                vec![]
            )
        )
        .is_err()
    );
    assert!(matches!(work.poll(), TransformStep::NeedReanalysis { .. }));
    let empty = Arc::new(TokenForest::new(vec![], vec![], vec![]).unwrap());
    let mut work = ItemTransformWorklist::new(
        plan(
            input,
            vec![AttributeSlot::expand(key(35, ProceduralKind::Attribute))],
            vec![],
        ),
        Digest([5; 32]),
        1,
    )
    .unwrap();
    work.admit(ExpansionRecord::from_tokens(key(35, ProceduralKind::Attribute), &empty).unwrap())
        .unwrap();
    let ticket = match work.poll() {
        TransformStep::NeedReanalysis { ticket, .. } => *ticket,
        _ => unreachable!(),
    };
    work.replan(ticket, plan(empty, vec![], vec![])).unwrap();
    assert!(
        matches!(work.poll(),TransformStep::Complete{item,appended} if item.roots().is_empty()&&appended.is_empty())
    );
}
