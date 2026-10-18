//! Pure Q47 authority over actual indexed callers; no guest execution claim.
use super::*;
use crate::{
    db::trace::Action,
    frontend::{
        entered_call_goals::EnteredTypeArgument,
        explicit_type_arguments::PreparedExplicitTypeArguments,
        source_locals::owned_plan::type_declarations::{BodyTypeDemand, BodyTypeOutcome},
    },
    ir::pc::{NodeId, PcKey, Phase},
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        signatures::GenericKind,
    },
};

const NESTED_TYPES: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-nested_stmt_macros.rs"
);
fn calls(plan: &PreparedSourceLocalPlan) -> Vec<PcKey> {
    let index = plan.context().source().index();
    index
        .nodes()
        .iter()
        .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        .map(|node| index.pc(node.id, Phase::Entry).unwrap())
        .collect()
}
fn typed(db: &mut Database, site: &PcKey) -> PreparedExplicitTypeArguments {
    let value = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedExplicitTypeArguments(site.clone()),
    ));
    let QueryValue::PreparedExplicitTypeArguments(plan) = value.value.as_ref() else {
        panic!("actual explicit Type plan variant differs")
    };
    plan.as_ref().clone()
}

#[test]
fn official_three_call_plans_keep_nominal_identity_shared_rows_and_guarded_caller() {
    for edition in [2015, 2018, 2021, 2024] {
        for source_change in [false, true] {
            let (mut db, parent) = world(NESTED_TYPES, edition, true);
            let index = parent.context().source().index();
            let sites = calls(&parent);
            assert_eq!(sites.len(), 3);
            let mut prepared = Vec::new();
            for (site, expected_name) in sites.iter().zip(["Bar", "Baz", "Xyzzy"]) {
                let mut declarations = parent.context().facts().items.iter().filter(|row| {
                    row.item.def_key.item_kind == ItemKind::Struct
                        && row.item.def_key.declared_name == expected_name
                });
                let expected = declarations.next().unwrap().item.def_key.fingerprint();
                assert!(declarations.next().is_none());
                let call = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredCall(site.clone())));
                let QueryValue::EnteredCall(call) = call.value.as_ref() else {
                    panic!()
                };
                let ordinal = call
                    .signature()
                    .unwrap()
                    .generics
                    .iter()
                    .find(|parameter| parameter.kind == GenericKind::Type)
                    .unwrap()
                    .ordinal;
                let plan = typed(&mut db, site);
                assert!(plan.retained_plan().same_environment(&parent));
                assert_eq!(plan.rows().len(), 1);
                assert_eq!(plan.rows()[0].ordinal(), ordinal);
                assert_eq!(
                    plan.rows()[0].source().source(),
                    &TyTerm::Nominal(expected, vec![])
                );
                let shared = plan.clone();
                assert_eq!(plan.rows().as_ptr(), shared.rows().as_ptr());
                assert!(Arc::ptr_eq(plan.guard(), shared.guard()));
                assert_eq!(plan.fingerprint(), shared.fingerprint());
                let events = db.trace.events.len();
                shared.require_current(&db, index, site).unwrap();
                let other = sites.iter().find(|other| *other != site).unwrap();
                let error = shared.require_current(&db, index, other).unwrap_err();
                assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
                assert_eq!(error.message, "explicit Type plan caller use differs");
                assert_eq!(db.trace.events.len(), events);
                prepared.push(shared);
            }
            assert!(db.trace.events.iter().all(|event| match &event.key {
                QueryKey::BodyStructure(body) => body == index.body(),
                QueryKey::Thunk(_) => false,
                _ => true,
            }));
            if source_change {
                // Real new captured source publication, not forged guard stamps.
                let changed = format!("{NESTED_TYPES}\n// LRR actual source revision control\n");
                let (_, _, inputs) =
                    crate::expand::source_declarative::assembly::tests::world(&changed, edition);
                inputs.publish(&mut db).unwrap();
            } else {
                db.begin_expansion_epoch(2).unwrap();
            }
            let events = db.trace.events.len();
            for (plan, site) in prepared.iter().zip(&sites) {
                let error = plan.require_current(&db, index, site).unwrap_err();
                assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT");
                assert_eq!(error.message, "prepared operation input changed");
            }
            assert_eq!(
                db.trace.events.len(),
                events,
                "guard refusal must not fetch source plans"
            );
        }
    }
}

#[test]
fn type_argument_plan_keeps_real_unsupported_body_and_field_cold_and_resolves_caller_alias() {
    const COLD: &str = "struct Marker{field:UnsupportedColdLayout} fn callee<'a,T>(){unsupported_cold_body();} fn main(){callee::<Marker>();}";
    let (mut db, parent) = world(COLD, 2024, false);
    let sites = calls(&parent);
    assert_eq!(sites.len(), 1);
    let call = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredCall(sites[0].clone())));
    let QueryValue::EnteredCall(call) = call.value.as_ref() else {
        panic!()
    };
    let signature = call.signature().unwrap();
    let ordinal = signature
        .generics
        .iter()
        .find(|p| p.kind == GenericKind::Type)
        .unwrap()
        .ordinal;
    assert_eq!(
        ordinal, 1,
        "the written lifetime occupies actual source ordinal zero"
    );
    let callee = signature.definition;
    let plan = typed(&mut db, &sites[0]);
    assert_eq!(plan.rows().len(), 1);
    assert_eq!(plan.rows()[0].ordinal(), ordinal);
    assert!(
        matches!(plan.rows()[0].source().source(), TyTerm::Nominal(_, arguments) if arguments.is_empty())
    );
    assert!(
        db.trace.events.iter().all(|event| match &event.key {
            QueryKey::BodyStructure(body) => body.definition != callee,
            QueryKey::Thunk(_) | QueryKey::BoundRecordConstructor(_) => false,
            _ => true,
        }),
        "explicit Type use must not lower the unsupported callee or field layout"
    );

    const ALIAS: &str =
        "fn callee<T>(){unsupported_cold_body();} fn main(){type u8=u16;callee::<u8>();}";
    let (mut db, parent) = world(ALIAS, 2024, false);
    let sites = calls(&parent);
    assert_eq!(sites.len(), 1);
    let call = ready(db.fetch(&SyntaxProvider, QueryKey::EnteredCall(sites[0].clone())));
    let QueryValue::EnteredCall(call) = call.value.as_ref() else {
        panic!()
    };
    let target = call.written_source_name().unwrap();
    let Some(EnteredTypeArgument::Indexed(site)) = target.arguments()[0].type_argument() else {
        panic!()
    };
    assert!(matches!(
        ready(parent.prepare_body_type(&mut db, NodeId(site.key().node))),
        BodyTypeOutcome::Pending(BodyTypeDemand::AliasContext(_))
    ));
    let start = db.trace.events.len();
    // The caller's own alias `u8 = u16` shadows the builtin: the explicit
    // argument is that alias's target, selected without call-entry authority.
    let prepared = ready(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedExplicitTypeArguments(sites[0].clone()),
    ));
    let QueryValue::PreparedExplicitTypeArguments(plan) = prepared.value.as_ref() else {
        panic!()
    };
    assert_eq!(plan.rows().len(), 1);
    assert_eq!(
        plan.rows()[0].source().source(),
        &TyTerm::Primitive(crate::ir::op::PrimitiveType::U16)
    );
    // The caller's alias is read from its own declaration: the unsupported
    // callee stays cold.
    assert!(db.trace.events[start..].iter().all(|event| match &event.key {
        QueryKey::BodyStructure(body) => body.definition != callee,
        QueryKey::Thunk(pc) => pc.body.definition != callee,
        _ => true,
    }));
    assert!(db.trace.events[start..].iter().any(|event| {
        event.key.kind() == "PreparedExplicitTypeArguments" && event.action == Action::ProviderRun
    }));
}
