//! Preregistered pure IR admission controls, not full original-body execution.
use super::{
    FloatingConstantOutcome, FloatingConstantSite, plan_floating_constant,
    tests::{ORIGINAL, span, world},
};
use crate::{
    core::digest::StableEncode,
    db::{key::QueryKey, memo::Database, provider::SyntaxProvider, value::QueryValue},
    frontend::source_locals::PatternResolution,
    ir::{
        op::{OPCODE_COUNT, Op},
        pc::{NodeId, Phase, SlotId},
        recipe::{RecipeDraft, RecipeExit},
        validate::verify,
    },
    syntax::body_index::{BodyIndex, ExpressionKind, NodeRole},
};
fn body_index(db: &mut Database, body: &crate::source::manifest::BodyKey) -> BodyIndex {
    let output = db
        .fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone()))
        .into_ready()
        .unwrap()
        .unwrap();
    let QueryValue::BodyStructure(index) = output.value.as_ref() else {
        panic!("wrong body index variant")
    };
    index.as_ref().clone()
}
fn draft(recipe: &crate::ir::recipe::VerifiedRecipe) -> RecipeDraft {
    let recipe = recipe.recipe();
    RecipeDraft {
        key: recipe.key.clone(),
        origin: recipe.origin.clone(),
        operations: recipe.operations.to_vec(),
        exits: vec![recipe.exit.clone()],
        dependencies: recipe.dependencies.to_vec(),
    }
}
#[test]
fn original_nan_ir_admission_preserves_each_official_edition_and_exact_let_continuation() {
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
        let at = ORIGINAL.find("f64::NAN").unwrap();
        let written = span(&view, &body, at, "f64::NAN");
        let site = FloatingConstantSite {
            source: &view,
            body: &body,
            at: at as u64,
            span: &written,
            qualifier: "f64",
            member: "NAN",
            context: locals.context(),
            expected: None,
        };
        let FloatingConstantOutcome::Ready(plan) =
            plan_floating_constant(&mut db, &graph, &locals, site)
                .into_ready()
                .unwrap()
                .unwrap()
        else {
            panic!("original first path blocked")
        };
        let index = body_index(&mut db, &body);
        let node = index
            .nodes()
            .iter()
            .find(|node| index.span(node.id).unwrap() == written)
            .unwrap();
        let pc = index.pc(node.id, Phase::Entry).unwrap();
        let lowered = plan
            .lower_recipe(&mut db, &graph, &locals, site, &index, &pc)
            .into_ready()
            .unwrap()
            .unwrap();
        let recipe = lowered.recipe().recipe();
        assert_eq!(recipe.key, pc);
        assert_eq!(recipe.origin.span, written);
        let Op::PrimitiveFloatingConstant {
            constant,
            destination,
        } = recipe.operations.last().unwrap()
        else {
            panic!("closed floating operation absent")
        };
        assert_eq!(Some(*destination), node.value);
        assert_eq!(constant.scalar().type_id, 15);
        assert_eq!(constant.scalar().kind, 7);
        assert!(f64::from_bits(constant.scalar().payload).is_nan());
        assert_eq!(constant.source_span(), &written);
        assert_eq!(constant.plan_identity(), plan.fingerprint());
        assert_eq!(constant.edition(), edition);
        assert_eq!(constant.hygiene_context(), locals.context());
        assert_eq!(constant.opcode_tag(), 41);
        assert_eq!(OPCODE_COUNT, 42);
        let RecipeExit::Next(next) = &recipe.exit else {
            panic!("missing structural next")
        };
        assert_eq!(next.phase, Phase::Commit.code().unwrap());
        assert_eq!(index.nodes()[next.node as usize].role, NodeRole::Statement);
        assert_ne!(next.node, 0);
        lowered
            .revalidate(&mut db, &graph, &locals, site, &index, &pc)
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(!recipe.dependencies.is_empty());
        // These are pure source/recipe checks: no constant opcode or original
        // is_nan/assertion/arithmetic body has executed in this test.
    }
}
#[test]
fn closed_ieee_recipe_scalar_keeps_width_and_nonfinite_bits() {
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
            let (mut db, view, published, body, graph, _) = world(&source, "live", 2024);
            let entered = published.enter_body(view.selection(), &body).unwrap();
            let locals = entered
                .local_index(view.selection(), &body, &Default::default(), &[])
                .unwrap();
            let at = source.find(&path).unwrap();
            let written = span(&view, &body, at, &path);
            let site = FloatingConstantSite {
                source: &view,
                body: &body,
                at: at as u64,
                span: &written,
                qualifier,
                member,
                context: locals.context(),
                expected: None,
            };
            let FloatingConstantOutcome::Ready(plan) =
                plan_floating_constant(&mut db, &graph, &locals, site)
                    .into_ready()
                    .unwrap()
                    .unwrap()
            else {
                panic!("closed member blocked")
            };
            let index = body_index(&mut db, &body);
            let node = index
                .nodes()
                .iter()
                .find(|node| index.span(node.id).unwrap() == written)
                .unwrap();
            let pc = index.pc(node.id, Phase::Entry).unwrap();
            let lowered = plan
                .lower_recipe(&mut db, &graph, &locals, site, &index, &pc)
                .into_ready()
                .unwrap()
                .unwrap();
            let Op::PrimitiveFloatingConstant { constant, .. } =
                lowered.recipe().recipe().operations.last().unwrap()
            else {
                panic!("closed operation absent")
            };
            assert_eq!(constant.scalar(), plan.scalar());
            constant.scalar().validate_scalar().unwrap();
            if qualifier == "f32" {
                assert!(constant.scalar().payload <= u64::from(u32::MAX));
            }
            let value = if qualifier == "f32" {
                f64::from(f32::from_bits(constant.scalar().payload as u32))
            } else {
                f64::from_bits(constant.scalar().payload)
            };
            match member {
                "NAN" => assert!(value.is_nan()),
                "INFINITY" => assert_eq!(value, f64::INFINITY),
                "NEG_INFINITY" => assert_eq!(value, f64::NEG_INFINITY),
                _ => assert!(value.is_finite()),
            }
        }
    }
}
#[test]
fn floating_ir_proof_refuses_foreign_pc_origin_destination_exit_and_erased_dependencies() {
    let source = "fn live(){let value=f64::NAN;let other=1.0f64;}";
    let (mut db, view, published, body, graph, _) = world(source, "live", 2024);
    let entered = published.enter_body(view.selection(), &body).unwrap();
    let locals = entered
        .local_index(view.selection(), &body, &Default::default(), &[])
        .unwrap();
    let at = source.find("f64::NAN").unwrap();
    let written = span(&view, &body, at, "f64::NAN");
    let site = FloatingConstantSite {
        source: &view,
        body: &body,
        at: at as u64,
        span: &written,
        qualifier: "f64",
        member: "NAN",
        context: locals.context(),
        expected: None,
    };
    let FloatingConstantOutcome::Ready(plan) =
        plan_floating_constant(&mut db, &graph, &locals, site)
            .into_ready()
            .unwrap()
            .unwrap()
    else {
        panic!("source blocked")
    };
    let index = body_index(&mut db, &body);
    let node = index
        .nodes()
        .iter()
        .find(|node| index.span(node.id).unwrap() == written)
        .unwrap();
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let lowered = plan
        .lower_recipe(&mut db, &graph, &locals, site, &index, &pc)
        .into_ready()
        .unwrap()
        .unwrap();
    for changed in ["pc", "origin", "destination", "exit", "dependencies"] {
        let mut forged = draft(lowered.recipe());
        match changed {
            "pc" => forged.key.phase = Phase::Commit.code().unwrap(),
            "origin" => forged.origin.span.relative_start += 1,
            "destination" => {
                let Op::PrimitiveFloatingConstant { destination, .. } =
                    forged.operations.last_mut().unwrap()
                else {
                    panic!()
                };
                *destination = SlotId(destination.0 + 2);
            }
            "exit" => {
                forged.exits = vec![RecipeExit::Next(index.pc(NodeId(0), Phase::Exit).unwrap())]
            }
            "dependencies" => forged.dependencies.clear(),
            _ => unreachable!(),
        }
        assert!(verify(forged, &index).is_err(), "{changed}");
    }
}
#[test]
fn retained_floating_recipe_requires_current_source_namespace_and_path_context() {
    let source = "fn live(){let value=f64::NAN;}";
    let (mut db, view, published, body, graph, _) = world(source, "live", 2024);
    let entered = published.enter_body(view.selection(), &body).unwrap();
    let locals = entered
        .local_index(view.selection(), &body, &Default::default(), &[])
        .unwrap();
    let at = source.find("f64::NAN").unwrap();
    let written = span(&view, &body, at, "f64::NAN");
    let site = FloatingConstantSite {
        source: &view,
        body: &body,
        at: at as u64,
        span: &written,
        qualifier: "f64",
        member: "NAN",
        context: locals.context(),
        expected: None,
    };
    let FloatingConstantOutcome::Ready(plan) =
        plan_floating_constant(&mut db, &graph, &locals, site)
            .into_ready()
            .unwrap()
            .unwrap()
    else {
        panic!()
    };
    let index = body_index(&mut db, &body);
    let node = index
        .nodes()
        .iter()
        .find(|node| index.span(node.id).unwrap() == written)
        .unwrap();
    let pc = index.pc(node.id, Phase::Entry).unwrap();
    let lowered = plan
        .lower_recipe(&mut db, &graph, &locals, site, &index, &pc)
        .into_ready()
        .unwrap()
        .unwrap();
    let changed = "type f64=u64;fn live(){let value=f64::NAN;}";
    let (mut new_db, new_view, new_index, new_body, new_graph, _) = world(changed, "live", 2024);
    let new_entered = new_index
        .enter_body(new_view.selection(), &new_body)
        .unwrap();
    let new_locals = new_entered
        .local_index(new_view.selection(), &new_body, &Default::default(), &[])
        .unwrap();
    let new_at = changed.find("f64::NAN").unwrap();
    let new_span = span(&new_view, &new_body, new_at, "f64::NAN");
    let new_site = FloatingConstantSite {
        source: &new_view,
        body: &new_body,
        at: new_at as u64,
        span: &new_span,
        qualifier: "f64",
        member: "NAN",
        context: new_locals.context(),
        expected: None,
    };
    let new_body_index = body_index(&mut new_db, &new_body);
    let new_node = new_body_index
        .nodes()
        .iter()
        .find(|node| node.role == NodeRole::Expression(ExpressionKind::Path))
        .unwrap();
    let new_pc = new_body_index.pc(new_node.id, Phase::Entry).unwrap();
    assert!(
        lowered
            .revalidate(
                &mut new_db,
                &new_graph,
                &new_locals,
                new_site,
                &new_body_index,
                &new_pc
            )
            .into_ready()
            .unwrap()
            .is_err()
    );
}
