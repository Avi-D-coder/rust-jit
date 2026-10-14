//! Pure plans from unchanged original source paths; no original body execution.
use lrr::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database},
    frontend::{
        hygiene::HygieneFacts,
        names::NameNamespaces,
        primitive_constants::{
            FloatingConstantOutcome, FloatingConstantSite, plan_floating_constant,
        },
        source_locals::PatternResolution,
        source_macros::PublishedMacroIndex,
    },
    source::{
        changes::{
            PreparedSourceInputs, PreparedSourceView, SourceSelection, SourceSelectionOutcome,
            SourceUnitInput,
        },
        manifest::BodyKey,
        snapshot::{CfgSet, SourceBlob, SourcePath},
    },
    syntax::{parse::ParseKey, spans::BodySpan, token_wire as runtime},
};
use lrr_expand_adapter::{
    hygiene::HygieneStore,
    token_wire::{Edition, Key, TokenForest},
};

const ORIGINAL: &str =
    include_str!("../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/float-nan.rs");

fn world(
    source: &str,
    function: &str,
    edition: u16,
) -> (
    Database,
    PreparedSourceView,
    PublishedMacroIndex,
    BodyKey,
    NameNamespaces,
) {
    let unit = Digest::of_bytes(b"LRR original floating constant source");
    let profile = ProfileIdentity {
        edition,
        ..ProfileIdentity::current()
    };
    let cfg = CfgSet::guest_default();
    let path = SourcePath::new("scalar-constants.rs").unwrap();
    let blob = SourceBlob::new(source.as_bytes().to_vec()).unwrap();
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
        panic!("source missing")
    };
    let body = view
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.declared_name == function)
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .key();
    let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
    let mut db = Database::default();
    db.trace.detailed = true;
    inputs.publish(&mut db).unwrap();
    let mut hygiene = HygieneStore::default();
    let context = hygiene
        .root(
            Key(unit.0),
            match edition {
                2015 => Edition::E2015,
                2018 => Edition::E2018,
                2021 => Edition::E2021,
                2024 => Edition::E2024,
                _ => unreachable!(),
            },
        )
        .unwrap();
    let keys = [context];
    let rows = hygiene.resolution_contexts(&keys).unwrap();
    let names = hygiene.name_contexts(&keys).unwrap();
    let wire = TokenForest::new(vec![], hygiene.records(), vec![])
        .unwrap()
        .encode()
        .unwrap();
    let decoded = runtime::TokenForest::decode(&wire).unwrap();
    let facts = HygieneFacts::new(
        decoded.contexts().to_vec(),
        serde_json::from_slice(&serde_json::to_vec(&rows).unwrap()).unwrap(),
    )
    .unwrap()
    .with_name_contexts(serde_json::from_slice(&serde_json::to_vec(&names).unwrap()).unwrap())
    .unwrap();
    let published =
        PublishedMacroIndex::from_view(view.share(), runtime::Key(context.0), facts).unwrap();
    (db, view, published, body, graph)
}

fn span(view: &PreparedSourceView, body: &BodyKey, at: usize, path: &str) -> BodySpan {
    let range = view
        .outline()
        .items
        .iter()
        .filter_map(|item| item.body.as_ref())
        .find(|handle| handle.key() == *body)
        .unwrap()
        .range;
    BodySpan {
        body_key: body.clone(),
        relative_start: u32::try_from(at).unwrap() - range.start,
        relative_end: u32::try_from(at + path.len()).unwrap() - range.start,
    }
}

#[test]
fn scalar_constant_widths_and_ieee_classes_are_selected_from_type_identity() {
    for qualifier in ["f32", "f64"] {
        for member in [
            "NAN",
            "INFINITY",
            "NEG_INFINITY",
            "MIN",
            "MAX",
            "MIN_POSITIVE",
            "EPSILON",
        ] {
            let path = format!("{qualifier}::{member}");
            let source = format!("fn live(){{let value={path};}}");
            let at = source.find(&path).unwrap();
            let (mut db, view, published, body, graph) = world(&source, "live", 2024);
            let entered = published.enter_body(view.selection(), &body).unwrap();
            let locals = entered
                .local_index(view.selection(), &body, &Default::default(), &[])
                .unwrap();
            let source_span = span(&view, &body, at, &path);
            let FloatingConstantOutcome::Ready(plan) = plan_floating_constant(
                &mut db,
                &graph,
                &locals,
                FloatingConstantSite {
                    source: &view,
                    body: &body,
                    at: at as u64,
                    span: &source_span,
                    qualifier,
                    member,
                    context: locals.context(),
                    expected: None,
                },
            )
            .into_ready()
            .unwrap()
            .unwrap() else {
                panic!("closed floating member not planned")
            };
            let scalar = plan.scalar();
            scalar.validate_scalar().unwrap();
            let value = if qualifier == "f32" {
                assert_eq!(scalar.type_id, 14);
                f64::from(f32::from_bits(scalar.payload.try_into().unwrap()))
            } else {
                assert_eq!(scalar.type_id, 15);
                f64::from_bits(scalar.payload)
            };
            match member {
                "NAN" => assert!(value.is_nan()),
                "INFINITY" => assert!(value.is_infinite() && value.is_sign_positive()),
                "NEG_INFINITY" => assert!(value.is_infinite() && value.is_sign_negative()),
                "MIN" => assert!(value.is_finite() && value < -1.0),
                "MAX" => assert!(value.is_finite() && value > 1.0),
                "MIN_POSITIVE" => assert!(value.is_finite() && value > 0.0 && value < 1.0),
                "EPSILON" => assert!(value.is_finite() && 1.0 + value > 1.0 && value < 1.0),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn shadowed_alias_and_rigid_type_names_never_become_primitives_by_spelling() {
    for source in [
        "struct f64; fn live(){let value=f64::NAN;}",
        "type f64=u64; fn live(){let value=f64::NAN;}",
        "fn live(){type f64=u64;let value=f64::NAN;}",
        "fn live<f64>(){let value=f64::NAN;}",
        "mod f64{} fn live(){let value=f64::NAN;}",
    ] {
        let at = source.find("f64::NAN").unwrap();
        let (mut db, view, published, body, graph) = world(source, "live", 2024);
        let entered = published.enter_body(view.selection(), &body).unwrap();
        let locals = entered
            .local_index(view.selection(), &body, &Default::default(), &[])
            .unwrap();
        let source_span = span(&view, &body, at, "f64::NAN");
        let outcome = plan_floating_constant(
            &mut db,
            &graph,
            &locals,
            FloatingConstantSite {
                source: &view,
                body: &body,
                at: at as u64,
                span: &source_span,
                qualifier: "f64",
                member: "NAN",
                context: locals.context(),
                expected: None,
            },
        )
        .into_ready()
        .unwrap()
        .unwrap();
        assert!(
            matches!(outcome, FloatingConstantOutcome::NeedType(_)),
            "shadowed source admitted: {source}: {outcome:?}"
        );
    }
}

#[test]
fn primitive_floating_paths_ignore_value_and_lifetime_namespace_names() {
    for source in [
        "fn live<'f64>(){let value=f64::NAN;}",
        "fn live<const f64:usize>(){let value=f64::NAN;}",
        "fn live(){let f64=1;let value=f64::NAN;}",
    ] {
        let at = source.find("f64::NAN").unwrap();
        let (mut db, view, published, body, graph) = world(source, "live", 2024);
        let entered = published.enter_body(view.selection(), &body).unwrap();
        let resolutions = entered
            .facts()
            .bindings
            .iter()
            .map(|binding| (binding.identity, PatternResolution::Binding))
            .collect();
        let locals = entered
            .local_index(view.selection(), &body, &resolutions, &[])
            .unwrap();
        let source_span = span(&view, &body, at, "f64::NAN");
        assert!(matches!(
            plan_floating_constant(
                &mut db,
                &graph,
                &locals,
                FloatingConstantSite {
                    source: &view,
                    body: &body,
                    at: at as u64,
                    span: &source_span,
                    qualifier: "f64",
                    member: "NAN",
                    context: locals.context(),
                    expected: None,
                }
            )
            .into_ready()
            .unwrap()
            .unwrap(),
            FloatingConstantOutcome::Ready(_)
        ));
    }
}

#[test]
fn original_unsealed_macro_producer_remains_a_type_preparation_frontier() {
    let at = ORIGINAL.find("f64::NAN").unwrap();
    let (mut db, view, published, body, graph) = world(ORIGINAL, "main", 2015);
    let entered = published.enter_body(view.selection(), &body).unwrap();
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|binding| (binding.identity, PatternResolution::Binding))
        .collect();
    let locals = entered
        .local_index(view.selection(), &body, &resolutions, &[])
        .unwrap();
    let source = span(&view, &body, at, "f64::NAN");
    let outcome = plan_floating_constant(
        &mut db,
        &graph,
        &locals,
        FloatingConstantSite {
            source: &view,
            body: &body,
            at: at as u64,
            span: &source,
            qualifier: "f64",
            member: "NAN",
            context: locals.context(),
            expected: None,
        },
    )
    .into_ready()
    .unwrap()
    .unwrap();
    assert!(matches!(
        outcome,
        FloatingConstantOutcome::NeedType(
            lrr::frontend::signature_names::SignatureName::Unresolved(
                lrr::frontend::names::NameLookup::NeedInput(_)
            )
        )
    ));
}

#[test]
fn caller_supplied_names_cannot_change_the_written_source_path() {
    let source = "fn live(){let value=f64::NAN;}";
    let (mut db, view, published, body, graph) = world(source, "live", 2024);
    let entered = published.enter_body(view.selection(), &body).unwrap();
    let locals = entered
        .local_index(view.selection(), &body, &Default::default(), &[])
        .unwrap();
    let at = source.find("f64::NAN").unwrap();
    let source_span = span(&view, &body, at, "f64::NAN");
    for (qualifier, member) in [("f32", "NAN"), ("f64", "INFINITY")] {
        let error = plan_floating_constant(
            &mut db,
            &graph,
            &locals,
            FloatingConstantSite {
                source: &view,
                body: &body,
                at: at as u64,
                span: &source_span,
                qualifier,
                member,
                context: locals.context(),
                expected: None,
            },
        )
        .into_ready()
        .unwrap()
        .unwrap_err();
        assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
    }
}
