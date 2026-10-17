//! No-I/O derived preparation for an actually reached ordinary local call.
//! Source capture/name selection remain in the common frontend; no VM action
//! or argument transfer is committed by this preparation receipt.
use super::*;
use crate::{
    db::dependency::{InputKey, InputValue},
    expand::source_discovery::SealedBodyMacroEnvironment,
    frontend::{source_body_context::AdmittedBodyContext, source_locals::PreparedSourceLocalPlan},
    syntax::body_index::{ExpressionKind, NodeRole},
};

pub(crate) struct ReachedLocalBody<'a> {
    callee: &'a SourceCallable,
    caller: Arc<BodyIndex>,
    site: NodeId,
    parent: Arc<PreparedSourceLocalPlan>,
    source: Arc<AdmittedBodyContext>,
    macros: Arc<SealedBodyMacroEnvironment>,
}
impl ReachedLocalBody<'_> {
    pub(crate) fn source(&self) -> &Arc<AdmittedBodyContext> {
        &self.source
    }
    pub(crate) fn macros(&self) -> &Arc<SealedBodyMacroEnvironment> {
        &self.macros
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        if let Err(error) = (|| {
            // This is derived source metadata, not a second name-use proof.
            // The ordinary call contract/dispatch retains its existing selected
            // callable checks; here the opaque admitted child and parent supply
            // exact source/header/body/owner authority without a world census.
            let definition = self.callee.identity();
            let contract = self
                .callee
                .local_source_contract()
                .ok_or_else(|| call_fault("reached local source admission absent"))?;
            if contract.site().body != *self.caller.body()
                || contract.site().node != self.site.0
                || self.callee.body() != Some(self.source.body())
                || self.callee.signature().header != *self.source.header()
            {
                return Err(call_fault("reached local source call identity differs"));
            }
            let actual_parent = self
                .source
                .local_parent()
                .ok_or_else(|| call_fault("reached local source has no admitted parent"))?;
            if actual_parent.source_identity() != self.parent.source_identity()
                || actual_parent
                    .environment()
                    .map(|environment| environment.identity())
                    != self
                        .parent
                        .environment()
                        .map(|environment| environment.identity())
            {
                return Err(call_fault("reached local admitted parent differs"));
            }
            if !matches!(db.input(&InputKey::LocalDeclarationOwner(definition)).as_deref(),
                Some(InputValue::LocalDeclarationOwner(owner))
                    if owner == self.parent.context().source().index().body())
            {
                return Err(call_fault("reached local source owner changed"));
            }
            if !matches!(db.input(&InputKey::DefinitionBody(self.caller.body().definition)).as_deref(),
                Some(InputValue::DefinitionBody(body)) if body.key() == *self.caller.body())
                || !self
                    .caller
                    .nodes()
                    .get(self.site.0 as usize)
                    .is_some_and(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
            {
                return Err(call_fault("reached local caller source changed"));
            }
            Ok(())
        })() {
            return QueryPoll::Ready(Err(error));
        }
        match self.parent.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        match self.source.require_current(db) {
            QueryPoll::Ready(Ok(())) => self.macros.require_current(db),
            other => other,
        }
    }
}

#[cfg(test)]
#[path = "run_source_preparation_tests.rs"]
mod tests;
impl RunMachine {
    /// One ordinary query demand, shared by early derived local preparation and
    /// the existing later module-call boundary. Both retain its canonical Arc.
    pub(super) fn selected_source_body(
        db: &mut Database,
        callee: &SourceCallable,
    ) -> QueryPoll<Arc<BodyIndex>> {
        // A selected identity can outlive argument evaluation or a pause. Check
        // its real source authority before the first executable body demand.
        if let Err(error) = callee.revalidate(db) {
            return QueryPoll::Ready(Err(error));
        }
        let Some(handle) = callee.body() else {
            return QueryPoll::Ready(Err(call_fault("selected source function has no body")));
        };
        match db.fetch(&SyntaxProvider, QueryKey::BodyStructure(handle.key())) {
            QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                QueryValue::BodyStructure(body) if body.body() == &handle.key() => {
                    QueryPoll::Ready(Ok(Arc::clone(body)))
                }
                _ => QueryPoll::Ready(Err(call_fault("callee body query variant mismatch"))),
            },
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    pub(super) fn prepare_reached_local_source(
        &self,
        db: &mut Database,
        callee: &SourceCallable,
        body: &Arc<BodyIndex>,
    ) -> QueryPoll<()> {
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let Some(owner) = db.input(&InputKey::LocalDeclarationOwner(callee.identity())) else {
            // Module source preparation retains its existing entry owner.
            // A missing local owner is never inferred from a lexical rib.
            if callee.local_source_contract().is_some() {
                return QueryPoll::Ready(Err(call_fault("reached local callee owner absent")));
            }
            return QueryPoll::Ready(Ok(()));
        };
        let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
            return QueryPoll::Ready(Err(call_fault(
                "reached local callee owner variant differs",
            )));
        };
        if callee.local_source_contract().is_none() {
            return QueryPoll::Ready(Err(call_fault(
                "reached local callee source admission absent",
            )));
        }
        let site = NodeId(self.frame.pc().node);
        if !self
            .frame
            .body
            .nodes()
            .get(site.0 as usize)
            .is_some_and(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
        {
            return QueryPoll::Ready(Err(call_fault(
                "local body preparation requires reached ordinary call",
            )));
        }
        let caller = Arc::clone(&self.frame.body);
        let value = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(owner.clone())));
        let QueryValue::SourceLocalPlan(parent) = value.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("local parent plan variant differs")));
        };
        let Some(environment) = parent.environment() else {
            return QueryPoll::Ready(Err(call_fault("local parent environment absent")));
        };
        let macros = Arc::clone(environment.macros());
        let entry = match body.pc(NodeId(0), Phase::Entry) {
            Ok(entry) => entry,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // Reuse an actual current input, including completion of any header
        // publication interrupted before call transfer. Never remint stale
        // source authority from an equivalent-looking index or key.
        let source = match db.input(&InputKey::BodyEnvironment(body.body().clone())) {
            Some(input) => {
                let InputValue::BodyEnvironment(environment) = input.as_ref() else {
                    return QueryPoll::Ready(Err(call_fault(
                        "reached local environment variant differs",
                    )));
                };
                if environment.body() != body.body()
                    || environment.macros().identity() != macros.identity()
                {
                    return QueryPoll::Ready(Err(call_fault(
                        "reached local environment owner differs",
                    )));
                }
                ready!(environment.require_current(db));
                Arc::clone(environment.source())
            }
            None => Arc::new(ready!(crate::frontend::source_body_context::capture_local(
                db,
                Arc::clone(parent),
                callee,
                body,
                &entry
            ))),
        };
        let receipt = ReachedLocalBody {
            callee,
            caller,
            site,
            parent: Arc::clone(parent),
            source,
            macros,
        };
        db.prepare_reached_local_plan(&receipt)
    }
}
