use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    ir::pc::{NodeId, Phase, SlotId, SlotKind},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::{
        body_index::{BodyIndexLimits, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
fn bodies(source: &str) -> (ParsedFile, Vec<(String, BodyHandle)>) {
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
    let outline = parsed
        .outline(Digest([31; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let bodies = outline
        .items
        .iter()
        .filter_map(|i| i.body.clone().map(|b| (i.def_key.declared_name.clone(), b)))
        .collect();
    (parsed, bodies)
}
#[test]
fn called_body_indexes_cold_invalid_syntax_roles_without_resolving_names() {
    let (parsed, bodies) = bodies(
        "fn live(){if false {unknown::<NoSuchType>(absent());} else {3;}} fn cold(){another_missing();}",
    );
    let body = &bodies.iter().find(|(n, _)| n == "live").unwrap().1;
    let index = parsed.index_body(body, BodyIndexLimits::default()).unwrap();
    assert!(
        index
            .nodes()
            .iter()
            .any(|n| n.role == NodeRole::Expression(ExpressionKind::If))
    );
    assert!(
        index
            .nodes()
            .iter()
            .any(|n| n.role == NodeRole::Expression(ExpressionKind::Path))
    );
    assert_eq!(index.body(), &body.key());
    assert_eq!(
        index,
        parsed.index_body(body, BodyIndexLimits::default()).unwrap()
    );
    assert_eq!(
        index.fingerprint(),
        parsed
            .index_body(body, BodyIndexLimits::default())
            .unwrap()
            .fingerprint()
    );
    for node in index.nodes() {
        assert!(node.parent.is_none_or(|p| p.0 < node.id.0));
        if let (Some(value), Some(place)) = (node.value, node.place) {
            assert_eq!(index.slot_kind(value).unwrap(), SlotKind::Value);
            assert_eq!(index.slot_kind(place).unwrap(), SlotKind::Place);
        }
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        assert_eq!(pc.body, body.key());
        assert_eq!(pc.node, node.id.0);
    }
}
#[test]
fn nested_items_closures_async_and_macro_payloads_are_opaque() {
    let source = "fn live(){fn nested(){hidden_call(42);}let f=||hidden_closure(43);let a=async{hidden_async(44)}; emit!(hidden_tokens(45));0}";
    let (parsed, bodies) = bodies(source);
    let body = &bodies[0].1;
    let index = parsed.index_body(body, BodyIndexLimits::default()).unwrap();
    let mut opaque = 0;
    for node in index.nodes().iter().filter(|n| n.opaque) {
        opaque += 1;
        assert!(
            !index
                .nodes()
                .iter()
                .any(|other| other.parent == Some(node.id))
        );
    }
    assert_eq!(opaque, 4);
    let root_start = body.range.start;
    for hidden in [
        "hidden_call",
        "hidden_closure",
        "hidden_async",
        "hidden_tokens",
    ] {
        let at = source.find(hidden).unwrap() as u32 - root_start;
        assert!(
            !index
                .nodes()
                .iter()
                .any(|n| !n.opaque && n.range.start == at)
        );
    }
}
#[test]
fn node_and_slot_limits_apply_only_to_the_body_explicitly_reached() {
    let source = format!(
        "fn live(){{0}}fn cold(){{{}}}",
        (0..30).map(|_| "missing();").collect::<String>()
    );
    let (parsed, bodies) = bodies(&source);
    let limits = BodyIndexLimits {
        nodes: 8,
        slots: 4,
        ..Default::default()
    };
    let live = &bodies.iter().find(|(n, _)| n == "live").unwrap().1;
    let cold = &bodies.iter().find(|(n, _)| n == "cold").unwrap().1;
    parsed.index_body(live, limits).unwrap();
    assert_eq!(
        parsed.index_body(cold, limits).unwrap_err().code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
    assert_eq!(
        parsed
            .index_body(live, BodyIndexLimits { slots: 3, ..limits })
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
    assert_eq!(
        parsed
            .index_body(
                live,
                BodyIndexLimits {
                    metadata_bytes: 1,
                    ..limits
                }
            )
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
    assert!(
        parsed
            .index_body(
                live,
                BodyIndexLimits {
                    nodes: 100_001,
                    ..Default::default()
                }
            )
            .is_err()
    );
}
#[test]
fn scopes_labels_operands_and_pc_phases_have_stable_owned_identities() {
    let (parsed, bodies) = bodies(
        "fn live(){'outer:loop{'inner:while test(){call(1,2);break 'outer;} 'block:{break 'block;}}}",
    );
    let index = parsed
        .index_body(&bodies[0].1, BodyIndexLimits::default())
        .unwrap();
    assert_eq!(index.labels().len(), 3);
    assert_eq!(index.labels()[0].spelling.as_deref(), Some("'outer"));
    assert_eq!(index.labels()[1].parent, Some(index.labels()[0].id));
    assert!(!index.labels()[2].is_loop);
    for scope in index.scopes() {
        assert!(scope.parent.is_none_or(|p| p.0 < scope.id.0));
    }
    let call = index
        .nodes()
        .iter()
        .find(|n| n.role == NodeRole::Expression(ExpressionKind::Call) && n.operands.len() == 3)
        .unwrap();
    for n in 0..3 {
        assert_eq!(index.pc(call.id, Phase::Operand(n)).unwrap().phase, n + 3);
    }
    assert!(index.pc(call.id, Phase::Operand(3)).is_err());
    assert!(index.pc(NodeId(u32::MAX), Phase::Entry).is_err());
    assert!(index.slot_kind(SlotId(index.slot_count())).is_err());
}
#[test]
fn source_spans_are_body_relative_and_stale_handles_fail_closed() {
    let (parsed, bodies) = bodies("fn live(){call(3);}");
    let body = &bodies[0].1;
    let index = parsed.index_body(body, Default::default()).unwrap();
    let (moved, other) = self::bodies("\n\nfn live(){call(3);}");
    let relocated = moved.index_body(&other[0].1, Default::default()).unwrap();
    assert_eq!(index.fingerprint(), relocated.fingerprint());
    assert_eq!(index.span(NodeId(0)).unwrap().relative_start, 0);
    let mut stale = body.clone();
    stale.body_hash = Digest([99; 32]);
    assert_eq!(
        parsed
            .index_body(&stale, Default::default())
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX"
    );
    stale.range.end = u32::MAX;
    assert_eq!(
        parsed
            .index_body(&stale, Default::default())
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX"
    );
}

#[test]
fn prepared_publication_indexes_only_exact_selected_body_revisions() {
    use lrr::{
        db::key::OutlineKey,
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::SourcePath,
        },
        syntax::parse::ParseKey,
    };
    let input = SourceUnitInput {
        path: SourcePath::new("body.rs").unwrap(),
        crate_key: Digest([31; 32]),
        module_path: vec![],
        profile: ProfileIdentity::current(),
        cfg: CfgSet::guest_default(),
        blob: SourceBlob::new(b"fn live(){if false{missing();}3}fn cold(){absent();}".to_vec())
            .unwrap(),
    };
    let selection = SourceSelection {
        path: input.path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: input.blob.content(),
                profile: input.profile.clone(),
            },
            crate_key: input.crate_key,
            module_path: vec![],
            cfg: input.cfg.fingerprint(),
        },
    };
    let prepared = PreparedSourceInputs::refresh(None, vec![input], Default::default()).unwrap();
    let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap() else {
        panic!("selected body publication absent")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|i| i.def_key.declared_name == "live")
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    assert_eq!(
        view.index_body(&selection, &body, Default::default())
            .unwrap()
            .body(),
        &body
    );
    let mut stale = selection.clone();
    stale.outline.parse.content = Digest([33; 32]);
    assert!(view.index_body(&stale, &body, Default::default()).is_err());
    let mut stale_body = body.clone();
    stale_body.exact_body_bytes = Digest([33; 32]);
    assert!(
        view.index_body(&selection, &stale_body, Default::default())
            .is_err()
    );
}

#[test]
fn fixed_node_limit_accepts_the_real_boundary_and_rejects_one_more_node() {
    let (parsed, bodies) = bodies(&format!("fn live(){{{}}}", "0;".repeat(49_999)));
    let index = parsed.index_body(&bodies[0].1, Default::default()).unwrap();
    assert_eq!(index.nodes().len(), 100_000);
    let (over, other) = self::bodies(&format!("fn live(){{{}}}", "0;".repeat(50_000)));
    assert_eq!(
        over.index_body(&other[0].1, Default::default())
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
}

#[test]
fn pattern_locations_keep_node_scope_and_source_identity_without_binding_resolution() {
    let (parsed, bodies) = bodies("fn live(){let (x,y)=(1,2);match x{v=>v}}");
    let index = parsed.index_body(&bodies[0].1, Default::default()).unwrap();
    let patterns = index.pattern_locations().collect::<Vec<_>>();
    assert_eq!(patterns.len(), 4);
    for node in patterns {
        assert_eq!(node.role, NodeRole::Pattern);
        assert_eq!(index.span(node.id).unwrap().body_key, bodies[0].1.key());
        assert!((node.scope.0 as usize) < index.scopes().len());
    }
    assert!(
        index
            .scopes()
            .iter()
            .any(|scope| scope.kind == lrr::syntax::body_index::ScopeKind::MatchArm)
    );
}
