//! Pure plans from unchanged original source paths; no original body execution.
use crate::{
    core::{
        digest::{Digest, StableEncode},
        profile::ProfileIdentity,
    },
    db::{key::OutlineKey, memo::Database},
    frontend::{
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
    syntax::{parse::ParseKey, spans::BodySpan},
};

pub(super) const ORIGINAL: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/float-nan.rs"
);

pub(super) fn world(
    source: &str,
    function: &str,
    edition: u16,
) -> (
    Database,
    PreparedSourceView,
    PublishedMacroIndex,
    BodyKey,
    NameNamespaces,
    Vec<crate::frontend::runtime_macro_plan::RuntimeNoItemProof>,
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
    let (context, facts) = crate::syntax::floating_fixture_hygiene::root(unit, edition);
    let published = PublishedMacroIndex::from_view(view.share(), context, facts).unwrap();
    let mut proofs = Vec::new();
    let published = if source == ORIGINAL {
        use crate::expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep};
        let mut discovery = SourceDiscovery::new(
            vec![view.share()],
            view.outline().root_scope.clone(),
            crate::source::manifest::Namespace::Type,
            Digest::of_bytes(b"floating constant completed discovery"),
            1,
        )
        .unwrap()
        .with_declarative(
            Digest::of_bytes(b"floating constant source expansions"),
            Digest::of_bytes(b"floating constant source generations"),
        )
        .unwrap()
        .with_compiler(std::sync::Arc::new(
            crate::frontend::compile_input_catalog::CompileInputCatalog::standalone(
                &view.selection().outline.parse.profile,
            )
            .unwrap(),
        ))
        .unwrap();
        assert!(matches!(
            discovery.poll().unwrap(),
            SourceDiscoveryStep::Sealed(_)
        ));
        discovery.publish(&mut db).unwrap();
        let input = db
            .input(&crate::db::dependency::InputKey::DiscoverySeal(
                discovery.key().clone(),
            ))
            .unwrap();
        let crate::db::dependency::InputValue::DiscoverySeal(snapshot) = input.as_ref() else {
            panic!("sealed discovery input absent")
        };
        proofs.extend(
            snapshot
                .runtime_no_item_proofs
                .iter()
                .map(|(_, proof)| proof.clone()),
        );
        assert!(
            !proofs.is_empty(),
            "original assertion source requires exact no-item certificates"
        );
        let mut indexes = discovery.completed_runtime_macro_indexes().unwrap();
        assert_eq!(indexes.len(), 1);
        let (completed_view, index) = indexes.pop().unwrap();
        assert_eq!(completed_view.selection(), view.selection());
        index
    } else {
        published
    };
    (db, view, published, body, graph, proofs)
}

pub(super) fn span(view: &PreparedSourceView, body: &BodyKey, at: usize, path: &str) -> BodySpan {
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
fn original_nan_path_plans_keep_exact_source_owner_and_each_official_edition() {
    let at = ORIGINAL.find("f64::NAN").unwrap();
    let mut identities = std::collections::BTreeSet::new();
    for edition in [2015, 2018, 2021, 2024] {
        let (mut db, view, published, body, graph, proofs) = world(ORIGINAL, "main", edition);
        let mut entered = published.enter_body(view.selection(), &body).unwrap();
        let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
            &view.selection().outline.parse.profile,
        )
        .unwrap();
        for proof in proofs.iter().filter(|proof| proof.body() == &body) {
            entered
                .accept_no_item_proof(view.selection(), &body, proof, &catalog)
                .unwrap();
        }
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
        let FloatingConstantOutcome::Ready(plan) = outcome else {
            panic!("original NAN did not plan: {outcome:?}")
        };
        assert_eq!(plan.source_span(), &source);
        assert_eq!(plan.edition(), edition);
        assert_eq!(plan.hygiene_context(), locals.context());
        assert_eq!(plan.scalar().type_id, 15);
        assert!(f64::from_bits(plan.scalar().payload).is_nan());
        plan.scalar().validate_scalar().unwrap();
        assert!(identities.insert(plan.fingerprint()));
        assert!(db.trace.events.iter().all(|event| !matches!(
            event.key,
            crate::db::key::QueryKey::Thunk(_) | crate::db::key::QueryKey::BodyStructure(_)
        )));
    }
    // This plans the original path only. Root body, is_nan methods, assertions,
    // infinity/arithmetic and full compiletest/native oracle remain unexecuted.
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
            let (mut db, view, published, body, graph, proofs) = world(&source, "live", 2024);
            let mut entered = published.enter_body(view.selection(), &body).unwrap();
            let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
                &view.selection().outline.parse.profile,
            )
            .unwrap();
            for proof in proofs.iter().filter(|proof| proof.body() == &body) {
                entered
                    .accept_no_item_proof(view.selection(), &body, proof, &catalog)
                    .unwrap();
            }
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
        let (mut db, view, published, body, graph, proofs) = world(source, "live", 2024);
        let mut entered = published.enter_body(view.selection(), &body).unwrap();
        let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
            &view.selection().outline.parse.profile,
        )
        .unwrap();
        for proof in proofs.iter().filter(|proof| proof.body() == &body) {
            entered
                .accept_no_item_proof(view.selection(), &body, proof, &catalog)
                .unwrap();
        }
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
        let (mut db, view, published, body, graph, proofs) = world(source, "live", 2024);
        let mut entered = published.enter_body(view.selection(), &body).unwrap();
        let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
            &view.selection().outline.parse.profile,
        )
        .unwrap();
        for proof in proofs.iter().filter(|proof| proof.body() == &body) {
            entered
                .accept_no_item_proof(view.selection(), &body, proof, &catalog)
                .unwrap();
        }
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
fn floating_plans_reject_foreign_hygiene_span_and_rigid_result_context() {
    use crate::{
        frontend::call_contracts::ArgumentType,
        runtime::shape::TypeId,
        traits::term::{ParamKey, TyTerm},
    };
    let at = ORIGINAL.find("f64::NAN").unwrap();
    let (mut db, view, published, body, graph, proofs) = world(ORIGINAL, "main", 2024);
    let mut entered = published.enter_body(view.selection(), &body).unwrap();
    let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
        &view.selection().outline.parse.profile,
    )
    .unwrap();
    for proof in proofs.iter().filter(|proof| proof.body() == &body) {
        entered
            .accept_no_item_proof(view.selection(), &body, proof, &catalog)
            .unwrap();
    }
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
    let mut wrong_span = source_span.clone();
    wrong_span.body_key.definition = Digest([5; 32]);
    let error = plan_floating_constant(
        &mut db,
        &graph,
        &locals,
        FloatingConstantSite {
            source: &view,
            body: &body,
            at: at as u64,
            span: &wrong_span,
            qualifier: "f64",
            member: "NAN",
            context: locals.context(),
            expected: None,
        },
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
    let error = plan_floating_constant(
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
            context: crate::syntax::token_wire::Key([7; 32]),
            expected: None,
        },
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
    let wrong = ArgumentType {
        source: TyTerm::Param(ParamKey {
            definition: Digest([9; 32]),
            ordinal: 0,
        }),
        descriptor: TypeId(15),
    };
    let error = plan_floating_constant(
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
            expected: Some(&wrong),
        },
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-GENERIC-BINDING");
    let wrong = ArgumentType {
        source: TyTerm::Primitive(crate::ir::op::PrimitiveType::F64),
        descriptor: TypeId(14),
    };
    let error = plan_floating_constant(
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
            expected: Some(&wrong),
        },
    )
    .into_ready()
    .unwrap()
    .unwrap_err();
    assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
}

#[test]
fn floating_plan_revalidation_never_reuses_a_scalar_under_a_changed_source_owner() {
    let at = ORIGINAL.find("f64::NAN").unwrap();
    let (mut db, view, published, body, graph, proofs) = world(ORIGINAL, "main", 2024);
    let mut entered = published.enter_body(view.selection(), &body).unwrap();
    let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
        &view.selection().outline.parse.profile,
    )
    .unwrap();
    for proof in proofs.iter().filter(|proof| proof.body() == &body) {
        entered
            .accept_no_item_proof(view.selection(), &body, proof, &catalog)
            .unwrap();
    }
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
    let make_site = || FloatingConstantSite {
        source: &view,
        body: &body,
        at: at as u64,
        span: &source_span,
        qualifier: "f64",
        member: "NAN",
        context: locals.context(),
        expected: None,
    };
    let FloatingConstantOutcome::Ready(plan) =
        plan_floating_constant(&mut db, &graph, &locals, make_site())
            .into_ready()
            .unwrap()
            .unwrap()
    else {
        panic!("original source was not planned")
    };
    plan.revalidate(&mut db, &graph, &locals, make_site())
        .into_ready()
        .unwrap()
        .unwrap();
    let new_source = "type f64=u64;fn main(){let value=f64::NAN;}";
    let new_at = new_source.find("f64::NAN").unwrap();
    let (mut new_db, new_view, new_published, new_body, new_graph, new_proofs) =
        world(new_source, "main", 2024);
    assert!(new_proofs.is_empty());
    let new_entered = new_published
        .enter_body(new_view.selection(), &new_body)
        .unwrap();
    let new_locals = new_entered
        .local_index(new_view.selection(), &new_body, &Default::default(), &[])
        .unwrap();
    let new_span = span(&new_view, &new_body, new_at, "f64::NAN");
    let error = plan
        .revalidate(
            &mut new_db,
            &new_graph,
            &new_locals,
            FloatingConstantSite {
                source: &new_view,
                body: &new_body,
                at: new_at as u64,
                span: &new_span,
                qualifier: "f64",
                member: "NAN",
                context: new_locals.context(),
                expected: None,
            },
        )
        .into_ready()
        .unwrap()
        .unwrap_err();
    assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
}

#[test]
fn original_no_item_certificates_retire_only_exact_validated_body_producers() {
    let (mut db, view, published, body, graph, proofs) = world(ORIGINAL, "main", 2024);
    let mut entered = published.enter_body(view.selection(), &body).unwrap();
    let catalog = crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::standard(
        &view.selection().outline.parse.profile,
    )
    .unwrap();
    let forged_catalog =
        crate::frontend::runtime_macro_catalog::RuntimeMacroCatalog::new(Digest([19; 32]), 2024)
            .unwrap();
    let proof = proofs.iter().find(|proof| proof.body() == &body).unwrap();
    assert!(
        entered
            .accept_no_item_proof(view.selection(), &body, proof, &forged_catalog)
            .is_err()
    );
    let mut stale = view.selection().clone();
    stale.outline.parse.content = Digest([20; 32]);
    assert!(
        entered
            .accept_no_item_proof(&stale, &body, proof, &catalog)
            .is_err()
    );
    let mut foreign_body = body.clone();
    foreign_body.definition = Digest([21; 32]);
    assert!(
        entered
            .accept_no_item_proof(view.selection(), &foreign_body, proof, &catalog)
            .is_err()
    );
    let resolutions = entered
        .facts()
        .bindings
        .iter()
        .map(|binding| (binding.identity, PatternResolution::Binding))
        .collect();
    let at = ORIGINAL.find("f64::NAN").unwrap();
    let source = span(&view, &body, at, "f64::NAN");
    let planned =
        |db: &mut Database, entered: &crate::frontend::source_macros::EnteredMacroIndex<'_>| {
            let locals = entered
                .local_index(view.selection(), &body, &resolutions, &[])
                .unwrap();
            plan_floating_constant(
                db,
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
            .unwrap()
        };
    assert!(matches!(
        planned(&mut db, &entered),
        FloatingConstantOutcome::NeedType(_)
    ));
    for proof in proofs.iter().filter(|proof| proof.body() == &body) {
        entered
            .accept_no_item_proof(view.selection(), &body, proof, &catalog)
            .unwrap();
    }
    assert!(matches!(
        planned(&mut db, &entered),
        FloatingConstantOutcome::Ready(_)
    ));
    // A proof for the original intrinsic cannot certify a user macro producing an item.
    let nonempty =
        "macro_rules! assert { () => { struct Hidden; } } fn live(){assert!();let value=f64::NAN;}";
    let (mut user_db, user_view, user_index, user_body, user_graph, user_proofs) =
        world(nonempty, "live", 2024);
    assert!(user_proofs.is_empty());
    let mut user_entered = user_index
        .enter_body(user_view.selection(), &user_body)
        .unwrap();
    assert!(
        user_entered
            .accept_no_item_proof(user_view.selection(), &user_body, proof, &catalog)
            .is_err()
    );
    let user_locals = user_entered
        .local_index(user_view.selection(), &user_body, &Default::default(), &[])
        .unwrap();
    let user_at = nonempty.find("f64::NAN").unwrap();
    let user_span = span(&user_view, &user_body, user_at, "f64::NAN");
    assert!(matches!(
        plan_floating_constant(
            &mut user_db,
            &user_graph,
            &user_locals,
            FloatingConstantSite {
                source: &user_view,
                body: &user_body,
                at: user_at as u64,
                span: &user_span,
                qualifier: "f64",
                member: "NAN",
                context: user_locals.context(),
                expected: None,
            }
        )
        .into_ready()
        .unwrap()
        .unwrap(),
        FloatingConstantOutcome::NeedType(_)
    ));
}
