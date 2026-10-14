use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::key::OutlineKey,
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{
        body_index::{BodyIndex, BodyIndexLimits, ScopeKind},
        parse::{ParseKey, ParsedFile},
    },
};
fn prepare(text: &str) -> (PreparedSourceInputs, PreparedSourceView, BodyIndex) {
    let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
    let profile = ProfileIdentity::current();
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("main.rs").unwrap();
    let unit = Digest([220; 32]);
    let selection = SourceSelection {
        path: path.clone(),
        outline: OutlineKey {
            parse: ParseKey {
                content: blob.content(),
                profile: profile.clone(),
            },
            crate_key: unit,
            module_path: vec![],
            cfg: cfg.fingerprint(),
        },
    };
    let parsed = ParsedFile::parse(&blob, profile.clone()).unwrap();
    let outline = parsed.outline(unit, vec![], &cfg).unwrap();
    let body = outline.items[0].body.as_ref().unwrap();
    let index = parsed.index_body(body, BodyIndexLimits::default()).unwrap();
    let inputs = PreparedSourceInputs::refresh(
        None,
        vec![SourceUnitInput {
            path,
            crate_key: unit,
            module_path: vec![],
            profile,
            cfg,
            blob,
        }],
        Default::default(),
    )
    .unwrap();
    let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
        panic!("source absent")
    };
    (inputs, view, index)
}
fn annotations(text: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let (_, view, index) = prepare(text);
    let offset = view.outline().items[0].body.as_ref().unwrap().range.start as usize;
    index
        .nodes()
        .iter()
        .filter_map(|n| {
            n.extended_to.as_ref().map(|e| {
                assert_eq!(index.scopes()[e.scope.0 as usize].kind, ScopeKind::Block);
                let spelling = text[offset + n.range.start as usize..offset + n.range.end as usize]
                    .to_string();
                let guards = e
                    .constructors
                    .iter()
                    .map(|id| {
                        let node = &index.nodes()[id.0 as usize];
                        text[offset + node.range.start as usize..offset + node.range.end as usize]
                            .to_string()
                    })
                    .collect();
                (spelling, guards)
            })
        })
        .collect()
}
#[test]
fn direct_tuple_array_and_record_borrows_extend_without_function_or_repeat_propagation() {
    let map = annotations(
        "fn case(){let a=&direct();let b=(&tuple(),[&array()],Record{r:&record()});let c=ordinary(&arg());let d=[&repeat();2];let e=1+{&arithmetic()};}",
    );
    for n in ["direct()", "tuple()", "array()", "record()"] {
        assert_eq!(map.get(n), Some(&vec![]));
    }
    assert!(map.get("arg()").is_some_and(|g| g == &["ordinary(&arg())"]));
    assert!(!map.contains_key("repeat()"));
    assert!(!map.contains_key("arithmetic()"));
}
#[test]
fn initializer_frontier_visits_branch_tails_but_never_conditions_scrutinees_or_closures() {
    let map = annotations(
        "fn case(){let a={&block()};let b=if predicate(){&yes()}else{&no()};let c=match scrutinee(){true=>&arm_a(),false=>&arm_b()};let d=||&closure();}",
    );
    for n in ["block()", "yes()", "no()", "arm_a()", "arm_b()"] {
        assert_eq!(map.get(n), Some(&vec![]));
    }
    for n in ["predicate()", "scrutinee()", "closure()"] {
        assert!(!map.contains_key(n));
    }
}
#[test]
fn extending_patterns_include_direct_ref_subpatterns_but_stop_at_reference_wrappers() {
    let map = annotations(
        "fn case(){let ref a=whole();let (ref b,c)=(tuple_pattern(),0);let Record{r:ref d}=record_pattern();let &ref e=wrapped();}",
    );
    for n in ["whole()", "(tuple_pattern(),0)", "record_pattern()"] {
        assert_eq!(map.get(n), Some(&vec![]));
    }
    assert!(!map.contains_key("tuple_pattern()"));
    assert!(!map.contains_key("wrapped()"));
}
#[test]
fn extending_borrow_base_propagation_excludes_index_operands_and_unary_arithmetic() {
    let map = annotations(
        "fn case(){let a=&make()[index()];let b=&field().value.0;let c=&*deref();let d=&-negative();}",
    );
    for n in ["make()", "field()", "deref()"] {
        assert_eq!(map.get(n), Some(&vec![]));
    }
    for n in ["index()", "negative()"] {
        assert!(!map.contains_key(n));
    }
}
#[test]
fn constructor_argument_extensions_remain_guarded_by_exact_source_call_identity() {
    let map = annotations(
        "fn case(){let a=Tuple(&ctor());let b=UppercaseFunction(&ordinary());let c=Tuple(Tuple(&nested()));}",
    );
    assert_eq!(map.get("ctor()"), Some(&vec!["Tuple(&ctor())".into()]));
    assert_eq!(
        map.get("ordinary()"),
        Some(&vec!["UppercaseFunction(&ordinary())".into()])
    );
    assert_eq!(
        map.get("nested()"),
        Some(&vec![
            "Tuple(Tuple(&nested()))".into(),
            "Tuple(&nested())".into()
        ])
    );
}
#[test]
fn extension_annotations_are_deterministic_and_metadata_budget_refusal_keeps_body_identity() {
    let text = "fn case(){let a=Tuple((&one(),&two()));}";
    let (_, _, index) = prepare(text);
    let parsed = ParsedFile::parse(
        &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    let outline = parsed
        .outline(Digest([220; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline.items[0].body.as_ref().unwrap();
    assert_eq!(
        parsed
            .index_body(
                body,
                BodyIndexLimits {
                    metadata_bytes: index.estimated_bytes() - 1,
                    ..Default::default()
                }
            )
            .unwrap_err()
            .code,
        "LRR-E-BODY-INDEX-LIMIT"
    );
    assert_eq!(index.fingerprint(), prepare(text).2.fingerprint());
}

const STOCK_FRONTIERS: &str = include_str!(
    "../../fixtures/upstream/rustc-checkout/tests/ui/lifetimes/temporary-lifetime-extension.rs"
);

fn stock_frontiers() -> (ParsedFile, lrr::syntax::outline::BodyHandle, BodyIndex) {
    let parsed = ParsedFile::parse(
        &SourceBlob::new(STOCK_FRONTIERS.as_bytes().to_vec()).unwrap(),
        ProfileIdentity::current(),
    )
    .unwrap();
    assert!(parsed.errors.is_empty());
    let outline = parsed
        .outline(Digest([220; 32]), vec![], &CfgSet::guest_default())
        .unwrap();
    let body = outline
        .items
        .iter()
        .find(|item| item.def_key.declared_name == "main")
        .unwrap()
        .body
        .clone()
        .unwrap();
    let index = parsed.index_body(&body, Default::default()).unwrap();
    (parsed, body, index)
}

#[test]
fn rustc_original_temporary_frontiers_keep_repeated_calls_distinct_and_scrutinee_unextended() {
    use lrr::syntax::body_index::{ExpressionKind, NodeRole};
    // Index the unchanged original main. Its String/formatting/Drop execution is
    // outside this structural acceptance; the original stdout remains retained.
    let (parsed, body, index) = stock_frontiers();
    let source = |node: &lrr::syntax::body_index::StructuralNode| {
        &STOCK_FRONTIERS[body.range.start as usize + node.range.start as usize
            ..body.range.start as usize + node.range.end as usize]
    };
    let calls = index
        .nodes()
        .iter()
        .filter(|node| {
            node.role == NodeRole::Expression(ExpressionKind::Call) && source(node) == "temp()"
        })
        .collect::<Vec<_>>();
    // Ten original calls cover direct/container/base/block/if/match frontiers;
    // their equal spellings must never collapse their separate source identities.
    assert_eq!(calls.len(), 10);
    let mut identities = std::collections::BTreeSet::new();
    let mut scrutinees = 0;
    for call in calls {
        assert!(identities.insert((call.range.start, call.range.end, call.id)));
        let scrutinee = call.parent.is_some_and(|id| {
            let parent = &index.nodes()[id.0 as usize];
            parent.role == NodeRole::Expression(ExpressionKind::Match)
                && parent.operands.first() == Some(&call.id)
        });
        if scrutinee {
            scrutinees += 1;
            assert!(call.extended_to.is_none());
        } else {
            let extension = call.extended_to.as_ref().unwrap();
            assert_eq!(extension.scope, index.nodes()[0].scope);
            assert_eq!(
                index.scopes()[extension.scope.0 as usize].kind,
                ScopeKind::Block
            );
            assert!(extension.constructors.is_empty());
        }
    }
    assert_eq!(scrutinees, 1);
    let block_operand = index
        .nodes()
        .iter()
        .find(|node| source(node).trim() == "{ owned_non_temporary }")
        .unwrap();
    assert_eq!(
        block_operand.extended_to.as_ref().unwrap().scope,
        index.nodes()[0].scope
    );
    assert_eq!(
        parsed
            .index_body(&body, Default::default())
            .unwrap()
            .fingerprint(),
        index.fingerprint()
    );
}

#[test]
fn rustc_original_temporary_frontiers_fail_closed_at_metadata_limit() {
    let (parsed, body, index) = stock_frontiers();
    let result = parsed.index_body(
        &body,
        BodyIndexLimits {
            metadata_bytes: index.estimated_bytes().checked_sub(1).unwrap(),
            ..Default::default()
        },
    );
    assert_eq!(result.unwrap_err().code, "LRR-E-BODY-INDEX-LIMIT");
    assert_eq!(
        parsed
            .index_body(&body, Default::default())
            .unwrap()
            .fingerprint(),
        index.fingerprint()
    );
}
