use lrr::{
    core::digest::Digest,
    expand::scopes::MacroDefinition,
    frontend::macro_origins::{ExpansionAncestry, MacroCandidate, MacroSelection},
    syntax::token_wire::Key,
};
fn candidate(id: u8, origin: Option<Key>) -> MacroCandidate {
    MacroCandidate {
        definition: MacroDefinition {
            identity: Digest([id; 32]),
            defining_unit: Digest([1; 32]),
            definition_edition: 2024,
        },
        origin,
    }
}
#[test]
fn expansion_order_ambiguity_uses_partial_ancestry_not_token_context() {
    let mut graph = ExpansionAncestry::default();
    let a = Key([1; 32]);
    let b = Key([2; 32]);
    let nested = Key([3; 32]);
    graph.register(a, None).unwrap();
    graph.register(b, None).unwrap();
    graph.register(nested, Some(a)).unwrap();
    let source = candidate(10, None);
    let generated = candidate(11, Some(a));
    assert!(matches!(
        graph
            .select_compatible(None, &[generated.clone(), source.clone()])
            .unwrap(),
        MacroSelection::Ambiguous(_)
    ));
    assert_eq!(
        graph
            .select_compatible(Some(a), &[generated.clone(), source.clone()])
            .unwrap(),
        MacroSelection::Defined(generated.definition.clone())
    );
    assert_eq!(
        graph
            .select_compatible(Some(nested), &[generated.clone(), source.clone()])
            .unwrap(),
        MacroSelection::Defined(generated.definition.clone())
    );
    assert_eq!(
        graph
            .select_compatible(None, &[source.clone(), generated.clone()])
            .unwrap(),
        MacroSelection::Defined(source.definition.clone())
    );
    assert_eq!(
        graph
            .select_compatible(None, &[generated.clone(), candidate(12, Some(a))])
            .unwrap(),
        MacroSelection::Defined(generated.definition.clone())
    );
    assert!(matches!(
        graph
            .select_compatible(None, &[candidate(13, Some(b)), generated])
            .unwrap(),
        MacroSelection::Ambiguous(_)
    ));
}
#[test]
fn expansion_order_missing_conflicting_and_depth_inputs_refuse_atomically() {
    let mut graph = ExpansionAncestry::default();
    let start = graph.fingerprint();
    assert!(graph.register(Key([1; 32]), Some(Key([2; 32]))).is_err());
    assert_eq!(graph.fingerprint(), start);
    let mut parent = None;
    for index in 0..256u16 {
        let mut bytes = [0; 32];
        bytes[..2].copy_from_slice(&index.to_le_bytes());
        let key = Key(bytes);
        graph.register(key, parent).unwrap();
        parent = Some(key);
    }
    let fingerprint = graph.fingerprint();
    assert!(graph.register(Key([255; 32]), parent).is_err());
    assert_eq!(graph.fingerprint(), fingerprint);
    assert!(graph.select_compatible(Some(Key([255; 32])), &[]).is_err());
    let old = Key([0; 32]);
    assert!(graph.register(old, Some(Key([1; 32]))).is_err());
    assert_eq!(graph.fingerprint(), fingerprint);
    let mut other = ExpansionAncestry::default();
    let a = Key([1; 32]);
    let b = Key([2; 32]);
    other.register(b, None).unwrap();
    other.register(a, None).unwrap();
    let mut ordered = ExpansionAncestry::default();
    ordered.register(a, None).unwrap();
    ordered.register(b, None).unwrap();
    assert_eq!(other.fingerprint(), ordered.fingerprint());
    let a = candidate(10, None);
    let b = candidate(11, None);
    let mut malformed = b.clone();
    malformed.definition.defining_unit = Digest([9; 32]);
    assert!(ordered.select_compatible(None, &[a, b, malformed]).is_err());
}
