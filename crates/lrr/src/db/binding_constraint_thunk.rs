//! Cold shared source-constraint selection. Installed execution consumes the
//! ordinary verified recipe and its retained relevant-input guard only.
use super::{
    dependency::{InputKey, InputValue},
    key::QueryKey,
    memo::Database,
    poll::QueryPoll,
    provider::{Provided, SyntaxProvider},
    value::QueryValue,
};
use crate::{
    core::diagnostic::Diagnostic,
    frontend::binding_constraints::BindingConstraintState,
    ir::pc::PcKey,
    syntax::{
        body_index::{BodyIndex, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
pub(super) fn thunk(
    db: &mut Database,
    parsed: &ParsedFile,
    body: &BodyHandle,
    index: &BodyIndex,
    pc: &PcKey,
) -> QueryPoll<Option<Provided>> {
    if !index
        .nodes()
        .get(pc.node as usize)
        .is_some_and(|node| node.role == NodeRole::Statement)
    {
        return QueryPoll::Ready(Ok(None));
    }
    // The legacy unsealed entry is explicitly held. A real publication selects
    // the common source services for original and retained syntax identically.
    let environment = db.input(&InputKey::BodyEnvironment(pc.body.clone()));
    let Some(InputValue::BodyEnvironment(_)) = environment.as_deref() else {
        return QueryPoll::Ready(Ok(None));
    };
    let key = QueryKey::PreparedBindingPlan(PcKey {
        body: pc.body.clone(),
        node: pc.node,
        phase: 0,
    });
    let prepared = match db.fetch(&SyntaxProvider, key.clone()) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::PreparedBindingPlan(plan) = prepared.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("binding constraint query variant differs")));
    };
    let proof = plan.constraints();
    match proof.state() {
        BindingConstraintState::NoNumericDemand | BindingConstraintState::Unconstrained => {
            return QueryPoll::Ready(Ok(None));
        }
        BindingConstraintState::Pending { .. } => {
            return QueryPoll::Ready(Err(fault(
                "numeric source constraint requires a missing common source goal",
            )));
        }
        BindingConstraintState::Resolved(_) => {}
    }
    if !prepared.cacheable {
        return QueryPoll::Ready(Err(fault(
            "numeric source constraint has unretained dependencies",
        )));
    }
    let recipe = match parsed.initial_thunk_with_source_constraints(body, index, pc, proof) {
        Ok(recipe) => recipe,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let Some(guard) = plan.guard() else {
        return QueryPoll::Ready(Err(fault(
            "prepared numeric source has no retained currentness guard",
        )));
    };
    QueryPoll::Ready(
        recipe
            .with_shared_prepared_inputs(std::sync::Arc::clone(guard))
            .map(|recipe| {
                Some(Provided {
                    value: QueryValue::Thunk(Box::new(recipe)),
                    cacheable: true,
                })
            }),
    )
}
