//! Actual reached module body ownership, using the same admitted source and
//! checked BodyEnvironment/SourceLocalPlan publication as ordinary local calls.
use super::*;
use crate::{
    db::dependency::{InputKey, InputValue},
    expand::source_discovery::{SealedBodyMacroEnvironment, SourceScopeOwner},
    frontend::source_body_context::AdmittedBodyContext,
    syntax::body_index::{ExpressionKind, NodeRole},
};

/// Fields are private: only the reached canonical Module call can construct it.
/// No lexical parent is minted for a module declaration.
pub(crate) struct ReachedModuleBody<'a> {
    callee: &'a SourceCallable,
    caller: Arc<BodyIndex>,
    site: NodeId,
    source: Arc<AdmittedBodyContext>,
    macros: Arc<SealedBodyMacroEnvironment>,
}
impl ReachedModuleBody<'_> {
    pub(crate) fn source(&self) -> &Arc<AdmittedBodyContext> {
        &self.source
    }
    pub(crate) fn macros(&self) -> &Arc<SealedBodyMacroEnvironment> {
        &self.macros
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        if let Err(error) = (|| {
            self.callee.revalidate(db)?;
            let proof = self
                .callee
                .ordinary_module_contract()
                .ok_or_else(|| call_fault("reached module source admission absent"))?;
            proof.revalidate(db)?;
            let declaration = proof.declaration();
            let parent = proof.parent().context().source();
            if self.source.local_parent().is_some()
                || self.callee.body() != Some(self.source.body())
                || self.callee.signature().header != *self.source.header()
                || self.source.parsed().key() != *declaration.source()
                || self.source.context() != declaration.context()
                || self.source.body().def_key != self.callee.identity()
                || parent.index().body() != self.caller.body()
                || parent.index().semantic_identity() != self.caller.semantic_identity()
                || !self
                    .caller
                    .nodes()
                    .get(self.site.0 as usize)
                    .is_some_and(|node| {
                        node.id == self.site
                            && node.role == NodeRole::Expression(ExpressionKind::Call)
                    })
                || !matches!(db.input(&InputKey::DefinitionBody(self.caller.body().definition)).as_deref(),
                    Some(InputValue::DefinitionBody(body)) if body.key() == *self.caller.body())
                || db
                    .input(&InputKey::LocalDeclarationOwner(self.callee.identity()))
                    .is_some()
            {
                return Err(call_fault("reached module source/caller identity differs"));
            }
            let owner = self.source.module_source().ok_or_else(|| {
                call_fault("reached module requires its actual generated boundary")
            })?;
            self.macros.index().require_publication(owner)?;
            Ok(())
        })() {
            return QueryPoll::Ready(Err(error));
        }
        match self.source.require_current(db) {
            QueryPoll::Ready(Ok(())) => self.macros.require_current(db),
            other => other,
        }
    }
}
impl RunMachine {
    /// Reached body preparation only, before revision-bound contracts. Existing
    /// current owning receipts are reused; no caller macro context substitution.
    pub(super) fn prepare_reached_module_source(
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
        if let Err(error) = callee.revalidate(db) {
            return QueryPoll::Ready(Err(error));
        }
        let Some(proof) = callee.ordinary_module_contract() else {
            return QueryPoll::Ready(Err(call_fault("reached module source admission absent")));
        };
        let Some(RunNames::Completed(completed)) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "reached module requires completed owning source environment",
            )));
        };
        let site = NodeId(self.frame.pc().node);
        if callee
            .body()
            .is_none_or(|handle| handle.key() != *body.body())
            || proof.parent().context().source().index().body() != self.frame.body.body()
            || !self
                .frame
                .body
                .nodes()
                .get(site.0 as usize)
                .is_some_and(|node| {
                    node.id == site && node.role == NodeRole::Expression(ExpressionKind::Call)
                })
        {
            return QueryPoll::Ready(Err(call_fault(
                "module body preparation requires actual reached call",
            )));
        }
        let (source, macros) = match db.input(&InputKey::BodyEnvironment(body.body().clone())) {
            Some(input) => {
                let InputValue::BodyEnvironment(environment) = input.as_ref() else {
                    return QueryPoll::Ready(Err(call_fault(
                        "reached module environment variant differs",
                    )));
                };
                if environment.body() != body.body()
                    || environment.source().index().semantic_identity() != body.semantic_identity()
                    || !Arc::ptr_eq(environment.macros().source_environment(), completed)
                {
                    return QueryPoll::Ready(Err(call_fault(
                        "reached module environment owner differs",
                    )));
                }
                ready!(environment.require_current(db));
                (
                    Arc::clone(environment.source()),
                    Arc::clone(environment.macros()),
                )
            }
            None => {
                // The selected declaration already supplies its exact owning
                // scope. Reuse the completed ownership index; a reached call
                // neither needs nor grants a census of all source roots.
                let owner = match completed.scope_owner(&proof.declaration().context().scope) {
                    SourceScopeOwner::Selected(owner) => owner,
                    SourceScopeOwner::Ambiguous(_) => {
                        return QueryPoll::Ready(Err(call_fault(
                            "reached module source owner is ambiguous",
                        )));
                    }
                    SourceScopeOwner::Missing => {
                        return QueryPoll::Ready(Err(call_fault(
                            "reached module source owner is absent",
                        )));
                    }
                };
                let entry = match body.pc(NodeId(0), Phase::Entry) {
                    Ok(entry) => entry,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                let source = Arc::new(ready!(crate::frontend::source_body_context::capture(
                    db,
                    &[owner],
                    body,
                    &entry
                )));
                // The common source receipt requires a genuine generated outer
                // boundary. Missing export remains held; never infer a parent.
                let Some(owner) = source.module_source() else {
                    return QueryPoll::Ready(Err(call_fault(
                        "reached module requires its actual generated boundary",
                    )));
                };
                let macros = ready!(completed.sealed_body_macro_environment(db, owner.selection()));
                (source, macros)
            }
        };
        let receipt = ReachedModuleBody {
            callee,
            caller: Arc::clone(&self.frame.body),
            site,
            source,
            macros,
        };
        db.prepare_reached_module_plan(&receipt)
    }
}
impl RunMachine {
    /// Reached body preparation for a module/impl/trait callee selected by
    /// ordinary name or member resolution. Its declaration context supplies
    /// the exact owning scope; an existing current environment is reused.
    pub(super) fn prepare_selected_module_source(
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
        if db
            .input(&InputKey::BodyEnvironment(body.body().clone()))
            .is_some()
            || db
                .input(&InputKey::LocalDeclarationOwner(callee.identity()))
                .is_some()
        {
            return QueryPoll::Ready(Ok(()));
        }
        let Some(RunNames::Completed(completed)) = &self.graph else {
            // Without a completed owning environment the body keeps its
            // existing (environment-free) preparation.
            return QueryPoll::Ready(Ok(()));
        };
        if callee
            .body()
            .is_none_or(|handle| handle.key() != *body.body())
        {
            return QueryPoll::Ready(Err(call_fault(
                "selected module body differs from its callee",
            )));
        }
        // An impl/trait member is owned by its enclosing module's source.
        let scope = &callee.declaration_context().scope;
        let module_scope;
        let scope = if scope.owner.is_some()
            && matches!(completed.scope_owner(scope), SourceScopeOwner::Missing)
        {
            module_scope = crate::syntax::outline::ScopeKey {
                owner: None,
                ..scope.clone()
            };
            &module_scope
        } else {
            scope
        };
        let owner = match completed.scope_owner(scope) {
            SourceScopeOwner::Selected(owner) => owner,
            SourceScopeOwner::Ambiguous(_) => {
                return QueryPoll::Ready(Err(call_fault(
                    "selected module source owner is ambiguous",
                )));
            }
            SourceScopeOwner::Missing => return QueryPoll::Ready(Ok(())),
        };
        let entry = match body.pc(NodeId(0), Phase::Entry) {
            Ok(entry) => entry,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let source = Arc::new(ready!(crate::frontend::source_body_context::capture(
            db,
            &[owner],
            body,
            &entry
        )));
        let Some(owner) = source.module_source() else {
            return QueryPoll::Ready(Ok(()));
        };
        let macros = ready!(completed.sealed_body_macro_environment(db, owner.selection()));
        db.prepare_selected_body_plan(&source, &macros)
    }
}
