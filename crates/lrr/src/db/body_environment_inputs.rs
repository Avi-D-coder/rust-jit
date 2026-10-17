//! Coordinator-issued owning environment. The payload cannot be minted from a
//! caller digest, index equality, namespace answer or source-only rebind.
use super::*;
use crate::{
    core::digest::{StableEncode, StableEncoder},
    expand::source_discovery::SealedBodyMacroEnvironment,
    frontend::source_body_context::AdmittedBodyContext,
    source::manifest::BodyKey,
};

pub struct BodyEnvironmentInput {
    source: Arc<AdmittedBodyContext>,
    macros: Arc<SealedBodyMacroEnvironment>,
    body: BodyKey,
    semantic_identity: Digest,
    identity: Digest,
}
impl std::fmt::Debug for BodyEnvironmentInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BodyEnvironmentInput")
            .field("body", &self.body)
            .field("identity", &self.identity)
            .finish()
    }
}
impl BodyEnvironmentInput {
    pub(crate) fn body(&self) -> &BodyKey {
        &self.body
    }
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn semantic_identity(&self) -> Digest {
        self.semantic_identity
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }
    pub(crate) fn source(&self) -> &Arc<AdmittedBodyContext> {
        &self.source
    }
    pub(crate) fn macros(&self) -> &Arc<SealedBodyMacroEnvironment> {
        &self.macros
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        match self.source.require_current(db) {
            QueryPoll::Ready(Ok(())) => self.macros.require_current(db),
            other => other,
        }
    }
}
impl Database {
    /// Coordinator's common cold owning-body demand. Pure consumers use
    /// QueryKey::SourceLocalPlan; they cannot create this environment.
    pub(crate) fn prepare_source_local_plan(
        &mut self,
        discovery: &crate::expand::source_discovery::SourceDiscovery,
        sources: &[&crate::source::changes::PreparedSourceView],
        body: &BodyKey,
    ) -> QueryPoll<Arc<crate::frontend::source_locals::PreparedSourceLocalPlan>> {
        use crate::{
            db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
            ir::pc::{NodeId, Phase},
        };
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        if self.run_active {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "source-local preparation during run",
            )));
        }
        let reusable = match self
            .input(&InputKey::BodyEnvironment(body.clone()))
            .as_deref()
        {
            Some(InputValue::BodyEnvironment(input))
                if input.body() == body && input.macros().seal() == discovery.key() =>
            {
                Some(Arc::clone(input))
            }
            _ => None,
        };
        let reusable = if let Some(input) = reusable {
            // The key alone does not certify a completion publication. A real
            // replacement under the same key requires a fresh opaque envelope.
            let seal = ready!(self.fetch(
                &SyntaxProvider,
                QueryKey::DiscoverySeal(discovery.key().clone())
            ));
            let source_current = match input.source().has_current_inputs(self) {
                Ok(current) => current,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            if source_current && seal.fingerprint == input.macros().seal_value() {
                ready!(input.require_current(self));
                true
            } else {
                false
            }
        } else {
            false
        };
        if !reusable {
            let structure =
                ready!(self.fetch(&SyntaxProvider, QueryKey::BodyStructure(body.clone())));
            let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
                return QueryPoll::Ready(Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "owning body structure result differs",
                )));
            };
            let pc = match index.pc(NodeId(0), Phase::Entry) {
                Ok(pc) => pc,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let mut source = Arc::new(ready!(crate::frontend::source_body_context::capture(
                self, sources, index, &pc
            )));
            if let Some(unit) = source.generated_source() {
                let boundary = match discovery.generated_body_macro_boundary(unit) {
                    Ok(boundary) => boundary,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                source = ready!(source.attach_generated_boundary(self, boundary));
            }
            let Some(owner) = source.module_source() else {
                return QueryPoll::Ready(Err(fault(
                    "LRR-E-BODY-ENVIRONMENT",
                    "owning module environment requires its actual boundary",
                )));
            };
            let macros = ready!(discovery.sealed_body_macro_environment(self, owner.selection()));
            ready!(self.publish_body_environment(source, macros));
        }
        let result = ready!(self.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body.clone())));
        let plan = match result.value.as_ref() {
            QueryValue::SourceLocalPlan(plan) if plan.context().source().index().body() == body => {
                Arc::clone(plan)
            }
            _ => {
                return QueryPoll::Ready(Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "owning local-plan result differs",
                )));
            }
        };
        // These are already-admitted declaration headers, not executable
        // preparation or TypeSchema demands. Opaque attribute owners remain
        // pending in the same prefix index and acquire no declaration rows.
        for row in &plan.context().facts().items {
            if row.attribute_input.is_some() {
                continue;
            }
            let definition = row.item.def_key.fingerprint();
            match ready!(self.publish_local_declaration(plan.context(), definition)) {
                super::local_declaration_inputs::LocalDeclarationPublication::Published(actual)
                    if actual == definition => {}
                _ => {
                    return QueryPoll::Ready(Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "owning local header publication differs",
                    )));
                }
            }
        }
        QueryPoll::Ready(Ok(plan))
    }
    /// Cold coordinator publication from two opaque, current source receipts.
    /// Replacing a completed environment is a derived revision, never a query
    /// side effect or a mutation of an executing frame.
    pub(crate) fn publish_body_environment(
        &mut self,
        source: Arc<AdmittedBodyContext>,
        macros: Arc<SealedBodyMacroEnvironment>,
    ) -> QueryPoll<()> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        if self.run_active {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "body environment publication during run",
            )));
        }
        self.publish_body_environment_checked(source, macros)
    }
    /// Shared publication core. Its callers enforce either cold coordinator
    /// authority or the engine's opaque, current reached-call receipt.
    fn publish_body_environment_checked(
        &mut self,
        source: Arc<AdmittedBodyContext>,
        macros: Arc<SealedBodyMacroEnvironment>,
    ) -> QueryPoll<()> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        match source.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        match macros.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        QueryPoll::Ready((|| {
            let owner = source.module_source().ok_or_else(|| {
                fault(
                    "LRR-E-BODY-ENVIRONMENT",
                    "body requires its actual generated module boundary",
                )
            })?;
            macros.index().require_publication(owner)?;
            let body = source.index().body().clone();
            let mut semantic = StableEncoder::new("lrr/v1/owning-body-source-environment");
            semantic.digest(source.identity());
            semantic.digest(macros.identity());
            let semantic_identity = semantic.finish();
            let key = InputKey::BodyEnvironment(body.clone());
            let previous = match self.inputs.get(&key).map(|row| row.value.as_ref()) {
                Some(InputValue::BodyEnvironment(old))
                    if old.semantic_identity() == semantic_identity
                        && old.macros().seal() == macros.seal()
                        && old.macros().seal_value() == macros.seal_value() =>
                {
                    Some(Arc::clone(old))
                }
                _ => None,
            };
            if let Some(previous) = previous {
                // Reuse only a still-current opaque lease. Fresh genuine
                // receipts above are required even when semantic stamps match.
                if previous.source().has_current_inputs(self)? {
                    return Ok(());
                }
            }
            let revision = self.revision.checked_add(1).ok_or_else(|| {
                fault("LRR-E-ID-EXHAUSTED", "body environment revision exhausted")
            })?;
            let mut envelope = StableEncoder::new("lrr/v2/owning-body-environment-publication");
            envelope.digest(semantic_identity);
            // Opaque issuance identity, separate from the semantic projection.
            // It is not a global revision dependency or runtime lookup key.
            envelope.u64(revision);
            envelope.record(macros.seal());
            envelope.digest(macros.seal_value());
            let input = Arc::new(BodyEnvironmentInput {
                source,
                macros,
                body: body.clone(),
                semantic_identity,
                identity: envelope.finish(),
            });
            let value = InputValue::BodyEnvironment(Arc::clone(&input));
            let fingerprint = value.fingerprint();
            // E-R5-006: no byte accounting.
            self.source_plan_pins.register_input(&input)?;
            self.inputs.insert(
                key,
                InputRecord {
                    value: Arc::new(value),
                    fingerprint,
                },
            );
            self.revision = revision;
            Ok(())
        })())
    }
    /// Derived no-I/O source facts for the exact reached local callee. The
    /// original cold APIs remain unavailable during an active run.
    pub(crate) fn prepare_reached_local_plan(
        &mut self,
        receipt: &crate::engine::run::source_preparation::ReachedLocalBody<'_>,
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
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        if !self.run_active {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "reached local preparation requires active run",
            )));
        }
        ready!(receipt.require_current(self));
        self.prepare_reached_body_plan(receipt.source(), receipt.macros())
    }
    /// Exact reached module receipt shares the same derived owning-body core.
    pub(crate) fn prepare_reached_module_plan(
        &mut self,
        receipt: &crate::engine::run::module_source_preparation::ReachedModuleBody<'_>,
    ) -> QueryPoll<()> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        if !self.run_active {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "reached module preparation requires active run",
            )));
        }
        match receipt.require_current(self) {
            QueryPoll::Ready(Ok(())) => {
                self.prepare_reached_body_plan(receipt.source(), receipt.macros())
            }
            other => other,
        }
    }
    /// Every published macro-generated item unit, for the run's one-time
    /// impl member index (cold; never on an operation path).
    pub(crate) fn generated_syntax_units(
        &self,
    ) -> Vec<Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>> {
        self.inputs
            .iter()
            .filter_map(|(key, record)| match (key, record.value.as_ref()) {
                (InputKey::GeneratedSyntax(_), InputValue::GeneratedSyntax(unit)) => {
                    Some(Arc::clone(unit))
                }
                _ => None,
            })
            .collect()
    }
    /// A source callee selected by ordinary name/member resolution (no
    /// entered-goal receipt) shares the same derived owning-body core. The
    /// caller has already revalidated the selected identity.
    pub(crate) fn prepare_selected_body_plan(
        &mut self,
        source: &Arc<AdmittedBodyContext>,
        macros: &Arc<SealedBodyMacroEnvironment>,
    ) -> QueryPoll<()> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        // Outside an active run (direct machine harnesses) the selected body
        // keeps its environment-free preparation.
        if !self.run_active {
            return QueryPoll::Ready(Ok(()));
        }
        match source.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        match macros.require_current(self) {
            QueryPoll::Ready(Ok(())) => self.prepare_reached_body_plan(source, macros),
            other => other,
        }
    }
    fn prepare_reached_body_plan(
        &mut self,
        source: &Arc<AdmittedBodyContext>,
        macros: &Arc<SealedBodyMacroEnvironment>,
    ) -> QueryPoll<()> {
        use crate::db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue};
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        ready!(self.publish_body_environment_checked(Arc::clone(source), Arc::clone(macros),));
        let body = source.index().body();
        let value = ready!(self.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body.clone())));
        let QueryValue::SourceLocalPlan(plan) = value.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "reached local plan variant differs",
            )));
        };
        if plan.context().source().identity() != source.identity()
            || plan.context().source().index().body() != body
            || plan.environment().is_none()
        {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-BODY-ENVIRONMENT",
                "reached local plan source differs",
            )));
        }
        for row in &plan.context().facts().items {
            if row.attribute_input.is_some() {
                continue;
            }
            let definition = row.item.def_key.fingerprint();
            match ready!(self.publish_local_declaration_checked(plan.context(), definition)) {
                super::local_declaration_inputs::LocalDeclarationPublication::Published(actual)
                    if actual == definition => {}
                _ => {
                    return QueryPoll::Ready(Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "reached local header publication differs",
                    )));
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }
}

#[cfg(test)]
#[path = "body_environment_inputs_tests.rs"]
mod tests;
