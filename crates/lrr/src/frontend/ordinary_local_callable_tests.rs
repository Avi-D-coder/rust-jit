//! Pure source-call proof controls over the unchanged complete official source.
//! Registered under declaration_field_type_tests to reuse its actual completed
//! source/discovery/entered-plan setup. These do not execute a guest call.
use super::*;
use crate::{
    frontend::callables::{
        EnteredLocalCall, ordinary_local::OrdinaryLocalAdmission, prepare_entered_local_function,
    },
    ir::pc::{PcKey, Phase},
    syntax::body_index::{ExpressionKind, NodeRole},
};
const SUFFIX: &str = include_str!(
    "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
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
fn local(db: &mut Database, site: &PcKey) -> Box<OrdinaryLocalAdmission> {
    match ready(prepare_entered_local_function(db, site)) {
        EnteredLocalCall::Local(proof) => proof,
        other => panic!("actual local callable source goal: {other:?}"),
    }
}
fn no_child_code(db: &Database, parent: &PreparedSourceLocalPlan, start: usize) {
    let body = parent.context().source().index().body();
    for event in &db.trace.events[start..] {
        assert!(
            !matches!(
                &event.key,
                QueryKey::Thunk(_) | QueryKey::PatternPlan(_) | QueryKey::Constant(_)
            ),
            "callable proof prepared executable code"
        );
        assert!(
            !matches!(&event.key, QueryKey::BodyStructure(other) if other != body),
            "callable proof indexed a cold child"
        );
    }
}
#[test]
fn ordinary_entered_callable_proofs_share_value_goals_all_editions_and_source_modes() {
    for edition in [2015, 2018, 2021, 2024] {
        for mode in 0..4 {
            let text = match mode {
                0 | 1 => SUFFIX.to_owned(),
                2 => format!("macro_rules! emit{{()=>{{{SUFFIX}}}}}emit!();"),
                _ => format!(
                    "macro_rules! emit{{()=>{{{SUFFIX}}}}}macro_rules! wrap{{()=>{{emit!();}}}}wrap!();"
                ),
            };
            let (mut db, parent) = world(&text, edition, mode == 1);
            let sites = calls(&parent);
            assert_eq!(sites.len(), 18);
            let start = db.trace.events.len();
            for site in sites {
                let proof = local(&mut db, &site);
                assert!(Arc::ptr_eq(proof.parent(), &parent));
                assert_eq!(proof.site(), &site);
                assert_eq!(proof.source(), &parent.context().source().parsed().key());
                assert_eq!(proof.item().def_key.item_kind, ItemKind::Function);
                assert!(
                    db.published_input(&InputKey::DefinitionBody(
                        proof.item().def_key.fingerprint()
                    ))
                    .is_none()
                );
                let guard_start = db.trace.events.len();
                proof.revalidate(&db).unwrap();
                proof.revalidate(&db).unwrap();
                assert_eq!(
                    guard_start,
                    db.trace.events.len(),
                    "installed currentness re-entered source planning"
                );
                assert!(proof.metadata_bytes() <= 32 * 1024 * 1024);
            }
            no_child_code(&db, &parent, start);
        }
    }
}
#[test]
fn ordinary_callable_requires_actual_reached_body_publication_and_entry_phase() {
    let (mut db, parent) = world(SUFFIX, 2024, false);
    let site = calls(&parent).remove(0);
    let proof = local(&mut db, &site);
    let definition = proof.item().def_key.fingerprint();
    let revision = db.revision();
    let error = proof.callable(&mut db).unwrap_err();
    assert_eq!(error.code, "LRR-E-CALL-CONTEXT");
    assert_eq!(error.message, "ordinary local callable publication absent");
    assert_eq!(db.revision(), revision);
    assert!(
        db.published_input(&InputKey::DefinitionBody(definition))
            .is_none()
    );
    let mut wrong = site;
    wrong.phase = 1;
    match prepare_entered_local_function(&mut db, &wrong) {
        QueryPoll::Ready(Err(error)) => assert_eq!(
            error.message,
            "ordinary local callable requires exact call entry"
        ),
        other => panic!("non-entry callable proof accepted: {other:?}"),
    }
    wrong.phase = 0;
    wrong.node = u32::MAX;
    match prepare_entered_local_function(&mut db, &wrong) {
        QueryPoll::Ready(Err(error)) => {
            assert_eq!(error.message, "ordinary local call node absent")
        }
        other => panic!("foreign call node accepted: {other:?}"),
    }
}
#[test]
fn ordinary_callable_proof_retains_real_expansion_epoch_and_revoked_parent_dependencies() {
    let text = format!("macro_rules! emit{{()=>{{{SUFFIX}}}}}emit!();");
    let (mut db, parent) = world(&text, 2024, false);
    let proof = local(&mut db, &calls(&parent).remove(0));
    proof.revalidate(&db).unwrap();
    db.begin_expansion_epoch(2).unwrap();
    assert!(
        proof.revalidate(&db).is_err(),
        "stale generated source guard accepted"
    );
    let (mut db, parent) = world(SUFFIX, 2024, false);
    let proof = local(&mut db, &calls(&parent).remove(0));
    db.publish_inputs(BTreeMap::new()).unwrap();
    assert!(
        proof.revalidate(&db).is_err(),
        "revoked parent guard accepted"
    );
}
#[test]
fn ordinary_callable_source_proof_has_no_primitive_or_nongeneric_signature_profile() {
    let end = SUFFIX.rfind('}').unwrap();
    let text = format!(
        "{}fn generic<T>(value:T)->T{{value}} fn borrowed(value:&u8)->&u8{{value}} generic(1); borrowed(&1);{}",
        &SUFFIX[..end],
        &SUFFIX[end..]
    );
    let (mut db, parent) = world(&text, 2024, false);
    let sites = calls(&parent);
    assert_eq!(sites.len(), 20);
    let start = db.trace.events.len();
    let generic = local(&mut db, &sites[18]);
    let borrowed = local(&mut db, &sites[19]);
    assert_eq!(generic.item().def_key.declared_name, "generic");
    assert_eq!(borrowed.item().def_key.declared_name, "borrowed");
    assert_ne!(
        generic.item().def_key.fingerprint(),
        borrowed.item().def_key.fingerprint()
    );
    assert!(
        generic.item().body.as_ref().unwrap().key().definition
            != parent.context().source().index().body().definition
    );
    no_child_code(&db, &parent, start);
}
