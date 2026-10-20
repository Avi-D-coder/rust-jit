//! Source application proof for an actual entered nongeneric caller. This is
//! neither a caller activation token nor solver/root/commit authorization.
use super::{
    parameters::{DeclarationParameters, DeclaredParameterPreparation, ParameterPending},
    root_input::CallerSourceContext,
    source_caller_clauses::{PreparedSourceCallerClauses, SourceCallerClausesPending},
    source_definition_headers::{DefinitionHeaderRole, SourceHeaderParent},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::callables::SourceCallable,
    vm::context::GenericEnv,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum CallerApplicationPending {
    Source(SourceCallerClausesPending),
    Identity,
    LexicalOwner,
    Parameters(ParameterPending),
    NonemptyParameters,
    NonemptyBindings,
    Signature,
    SourceReceipt,
    Currentness,
    ActiveHandles,
}
#[derive(Debug)]
pub(super) enum CallerApplicationPreparation {
    Verified(VerifiedCallerApplication),
    Pending(CallerApplicationPending),
}
/// Private construction prevents a DTO/application digest from being promoted
/// into proof. Source preparation owns all immutable evidence; the actual root
/// caller still owns active-attempt identity and currentness before consumption.
#[derive(Debug)]
pub(super) struct VerifiedCallerApplication {
    source: Arc<PreparedSourceCallerClauses>,
    environment: Arc<GenericEnv>,
    callable: Arc<SourceCallable>,
    signature: Arc<QueryValue>,
    signature_guard: PreparedInputGuard,
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
impl VerifiedCallerApplication {
    pub(super) fn source(&self) -> &Arc<PreparedSourceCallerClauses> {
        &self.source
    }
    /// The source encoder can consume this join only for the exact retained
    /// plan and actual descriptor. This pure check supplies no live-run proof.
    pub(super) fn matches_caller(
        &self,
        source: &PreparedSourceCallerClauses,
        caller: &CallerSourceContext,
    ) -> Result<(), CallerApplicationPending> {
        if !std::ptr::eq(source, self.source.as_ref()) {
            return Err(CallerApplicationPending::SourceReceipt);
        }
        self.matches_descriptor(caller)
    }
    fn matches_descriptor(
        &self,
        caller: &CallerSourceContext,
    ) -> Result<(), CallerApplicationPending> {
        let definition = self.source.key().definition;
        if caller.body_definition != definition || caller.generic_definition != Some(definition) {
            return Err(CallerApplicationPending::Identity);
        }
        if caller.signature != Some(self.environment.signature_key()) {
            return Err(CallerApplicationPending::Signature);
        }
        if !caller.substitutions.is_empty()
            || caller.application.is_some()
            || !self.environment.has_empty_bindings()
            || self.environment.application_identity().is_some()
        {
            return Err(CallerApplicationPending::NonemptyBindings);
        }
        Ok(())
    }
    /// The actual consumer supplies its current entered handles, not copies
    /// reconstructed from names/digests. Repeated activations of these handles
    /// still require the run owner's distinct attempt/activation certification.
    pub(super) fn require_current(
        &self,
        db: &mut Database,
        environment: &Arc<GenericEnv>,
        callable: &Arc<SourceCallable>,
        caller: &CallerSourceContext,
    ) -> Result<(), CallerApplicationPending> {
        if !Arc::ptr_eq(environment, &self.environment) || !Arc::ptr_eq(callable, &self.callable) {
            return Err(CallerApplicationPending::ActiveHandles);
        }
        self.matches_descriptor(caller)?;
        self.source
            .require_current(db)
            .map_err(|_| CallerApplicationPending::Currentness)?;
        self.signature_guard
            .require_current(db)
            .map_err(|_| CallerApplicationPending::Currentness)?;
        self.callable
            .revalidate(db)
            .map_err(|_| CallerApplicationPending::Currentness)?;
        self.environment
            .require_current(db, &self.callable)
            .map_err(|_| CallerApplicationPending::Currentness)
    }
    pub(super) fn prepare(
        db: &mut Database,
        source: Arc<PreparedSourceCallerClauses>,
        environment: Arc<GenericEnv>,
        callable: Arc<SourceCallable>,
        caller: &CallerSourceContext,
    ) -> QueryPoll<CallerApplicationPreparation> {
        macro_rules! pending {
            ($why:expr) => {
                return QueryPoll::Ready(Ok(CallerApplicationPreparation::Pending($why)))
            };
        }
        if let Some(why) = source.pending() {
            pending!(CallerApplicationPending::Source(why));
        }
        let definition = source.key().definition;
        if caller.body_definition != definition
            || caller.generic_definition != Some(definition)
            || environment.definition().definition != definition
            || callable.identity() != definition
        {
            pending!(CallerApplicationPending::Identity);
        }
        let header = source.header();
        if header.role() != DefinitionHeaderRole::Function {
            pending!(CallerApplicationPending::SourceReceipt);
        }
        match header.source_parent() {
            SourceHeaderParent::Lexical(scope) if scope.owner.is_none() => {}
            _ => pending!(CallerApplicationPending::LexicalOwner),
        }
        match DeclarationParameters::prepare(header.declaration()) {
            Ok(DeclaredParameterPreparation::Ready(parameters)) if parameters.count() == 0 => {}
            Ok(DeclaredParameterPreparation::Ready(_)) => {
                pending!(CallerApplicationPending::NonemptyParameters)
            }
            Ok(DeclaredParameterPreparation::Pending(why)) => {
                pending!(CallerApplicationPending::Parameters(why))
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
        if !environment.has_empty_bindings()
            || !caller.substitutions.is_empty()
            || environment.application_identity().is_some()
            || caller.application.is_some()
        {
            pending!(CallerApplicationPending::NonemptyBindings);
        }
        // Direct source metadata, not a signature synthesized from a body or
        // reconstructed by SignatureSubstitution's legacy unifier.
        let Some((_, recipe, context, _)) = header.declaration().source_parts() else {
            pending!(CallerApplicationPending::SourceReceipt);
        };
        if callable.signature().header != *recipe
            || callable.declaration_context() != context
            || callable.signature().scope != context.scope
            || callable.signature().generic_source_layout != header.source_generic_layout()
        {
            pending!(CallerApplicationPending::SourceReceipt);
        }
        if source.require_current(db).is_err()
            || callable.revalidate(db).is_err()
            || environment.require_current(db, &callable).is_err()
        {
            pending!(CallerApplicationPending::Currentness);
        }
        let key = QueryKey::FunctionSignature(definition);
        let outcome = ready!(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::FunctionSignature(signature) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("actual caller signature query differs")));
        };
        if !outcome.cacheable {
            pending!(CallerApplicationPending::Currentness);
        }
        if signature.as_ref() != callable.signature()
            || signature.as_ref() != environment.definition()
            || signature.fingerprint() != environment.signature_key()
            || caller.signature != Some(signature.fingerprint())
        {
            pending!(CallerApplicationPending::Signature);
        }
        if !signature.generics.is_empty()
            || signature.generic_source_layout != header.source_generic_layout()
        {
            pending!(CallerApplicationPending::NonemptyParameters);
        }
        let signature_guard =
            ready!(db.prepare_input_guard(&SyntaxProvider, &key, outcome.fingerprint));
        let result = Self {
            source,
            environment,
            callable,
            signature: outcome.value,
            signature_guard,
        };
        if let Err(why) = result.require_current(db, &result.environment, &result.callable, caller)
        {
            pending!(why);
        }
        QueryPoll::Ready(Ok(CallerApplicationPreparation::Verified(result)))
    }
    /// The retained full source query remains owned for the receipt's lifetime.
    pub(super) fn signature(&self) -> &crate::syntax::signatures::FunctionSignature {
        let QueryValue::FunctionSignature(signature) = self.signature.as_ref() else {
            unreachable!("retained actual caller signature query")
        };
        signature
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALLER-APPLICATION", message.into())
}
