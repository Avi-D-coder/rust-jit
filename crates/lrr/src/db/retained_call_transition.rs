//! Checked continuation across target-only retained-body publication. This is
//! not an executable call proof, an input-guard refresh or a second resolver.
use super::*;
use crate::{
    db::retained_syntax::RetainedSyntaxInput,
    db::{provider::SyntaxProvider, value::QueryValue},
    frontend::explicit_type_arguments::PreparedExplicitTypeArguments,
    ir::pc::PcKey,
    source::manifest::BodyKey,
    syntax::{body_index::BodyIndex, outline::BodyHandle},
};

const MAX_BYTES: usize = 32 * 1024 * 1024;
fn invalid(message: &str) -> Diagnostic {
    fault("LRR-E-INTERNAL-SNAPSHOT", message)
}
fn target_keys(definition: Digest) -> [InputKey; 4] {
    [
        InputKey::DefinitionSource(definition),
        InputKey::DefinitionHeader(definition),
        InputKey::DefinitionBody(definition),
        InputKey::DeclarationContext(definition),
    ]
}
fn stamps(db: &Database, definition: Digest) -> Result<[Digest; 4], Diagnostic> {
    let mut result = [Digest([0; 32]); 4];
    for (key, stamp) in target_keys(definition).into_iter().zip(&mut result) {
        *stamp = db
            .published_input(&key)
            .ok_or_else(|| invalid("call target publication row absent"))?
            .1;
    }
    Ok(result)
}

/// Private fields can only be minted by the actual checked publisher below.
/// Retains one shared certified guard, no source parser/body-index/plan clone.
/// A runtime owner must charge metadata_bytes while retaining this receipt.
pub(crate) struct RetainedCallBodyTransition {
    source_revision: u64,
    site: PcKey,
    before_plan: Digest,
    definition: Digest,
    before: [Digest; 4],
    after: [Digest; 4],
    body: BodyHandle,
    syntax: Digest,
    syntax_stamp: Digest,
    guard: Arc<PreparedInputGuard>,
    prepared_source: Arc<PreparedInputGuard>,
    bytes: usize,
}
impl RetainedCallBodyTransition {
    pub(crate) fn body(&self) -> BodyKey {
        self.body.key()
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(super) fn permits_replacement(
        &self,
        key: &InputKey,
        before: Digest,
        after: Digest,
    ) -> bool {
        target_keys(self.definition)
            .iter()
            .position(|candidate| candidate == key)
            .is_some_and(|i| self.before[i] == before && self.after[i] == after)
    }
    /// Relevant old dependencies must be unchanged except for the exact four
    /// target replacements this receipt witnessed. The old guard stays stale.
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        if db.source_revision() != self.source_revision
            || stamps(db, self.definition)? != self.after
            || !matches!(db.published_input(&InputKey::DefinitionBody(self.definition)),
                Some((InputValue::DefinitionBody(body), _)) if body == &self.body)
            || !matches!(db.published_input(&InputKey::DefinitionSource(self.definition)),
                Some((InputValue::DefinitionSource(parse), _)) if parse.content == self.syntax)
            || !matches!(db.published_input(&InputKey::RetainedSyntax(self.syntax)),
                Some((InputValue::RetainedSyntax(unit), stamp))
                    if stamp == self.syntax_stamp && unit.prepared().permits_body(self.definition))
        {
            return Err(invalid(
                "retained call target transition publication changed",
            ));
        }
        self.prepared_source.require_current(db)?;
        self.guard.require_current_across_call_body(db, self)
    }
    /// Consume fresh ordinary evidence. No old callable, signature, Type row or
    /// input stamp is edited to make it current after the publication.
    pub(crate) fn require_rebound(
        &self,
        db: &mut Database,
        before: &PreparedExplicitTypeArguments,
        fresh: &PreparedExplicitTypeArguments,
        index: &BodyIndex,
        pc: &PcKey,
    ) -> Result<(), Diagnostic> {
        self.require_current(db)?;
        if pc != &self.site
            || before.fingerprint() != self.before_plan
            || !before.same_call_contract(fresh)
        {
            return Err(invalid(
                "retained call renewal changed the selected contract",
            ));
        }
        fresh.require_current(db, index, pc)?;
        let callee = fresh.callable(db)?;
        if callee.identity() != self.definition || callee.body() != Some(&self.body) {
            return Err(invalid(
                "retained call renewal selected another target body",
            ));
        }
        Ok(())
    }
}

impl Database {
    /// Cold coordinator boundary before argument transfer/frame mutation. The
    /// caller supplies its genuine current selected call and execution lease.
    /// Syntax input publication is separately authorized by the existing API;
    /// the existing body publisher still checks original anchors, source/seal,
    /// epoch, exact prior rows and that the target is not active.
    pub(crate) fn publish_retained_call_body(
        &mut self,
        selected: &PreparedExplicitTypeArguments,
        index: &BodyIndex,
        pc: &PcKey,
        caller: &BodyExecutionLease,
        input: Arc<RetainedSyntaxInput>,
    ) -> QueryPoll<RetainedCallBodyTransition> {
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        checked!(self.require_coordinator());
        checked!(selected.require_current(self, index, pc));
        if caller.body() != index.body() || pc.body != *caller.body() {
            return QueryPoll::Ready(Err(invalid("retained call caller lease differs")));
        }
        match caller.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
        let callee = checked!(selected.callable(self));
        let definition = callee.identity();
        let before = checked!(stamps(self, definition));
        let source_revision = self.source_revision();
        // Capture the new unit's complete source/epoch/seal closure before
        // replacing any target rows. Replay is already memoized by admission;
        // this is cold guard capture, never a runtime parse or stamp refresh.
        let parse_key = QueryKey::ParseFile(input.key().clone());
        let parsed = match self.fetch(&SyntaxProvider, parse_key.clone()) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        if !parsed.cacheable
            || !matches!(parsed.value.as_ref(), QueryValue::ParseFile(actual) if actual.key() == *input.key())
        {
            return QueryPoll::Ready(Err(invalid("retained call prepared parse differs")));
        }
        let prepared_source =
            match self.prepare_input_guard(&SyntaxProvider, &parse_key, parsed.fingerprint) {
                QueryPoll::Ready(Ok(guard)) => Arc::new(guard),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            };
        let bytes = checked!(
            std::mem::size_of::<RetainedCallBodyTransition>()
                .checked_add(selected.guard().metadata_bytes())
                .and_then(|bytes| bytes.checked_add(prepared_source.metadata_bytes()))
                .and_then(|bytes| bytes.checked_add(4 * std::mem::size_of::<usize>()))
                .filter(|bytes| *bytes <= MAX_BYTES)
                .ok_or_else(crate::runtime::arena::resource)
        );
        let body = match self.publish_retained_body(input, definition) {
            QueryPoll::Ready(Ok(body)) => body,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let after = checked!(stamps(self, definition));
        let Some((InputValue::DefinitionSource(parse), _)) =
            self.published_input(&InputKey::DefinitionSource(definition))
        else {
            return QueryPoll::Ready(Err(invalid("retained call source row variant differs")));
        };
        let syntax = parse.content;
        let syntax_stamp = checked!(
            self.published_input(&InputKey::RetainedSyntax(syntax))
                .map(|(_, stamp)| stamp)
                .ok_or_else(|| invalid("retained call syntax publication absent"))
        );
        let Some((InputValue::DefinitionBody(handle), _)) =
            self.published_input(&InputKey::DefinitionBody(definition))
        else {
            return QueryPoll::Ready(Err(invalid("retained call body row variant differs")));
        };
        if handle.key() != body {
            return QueryPoll::Ready(Err(invalid("retained call published body differs")));
        }
        let result = RetainedCallBodyTransition {
            source_revision,
            site: pc.clone(),
            before_plan: selected.fingerprint(),
            definition,
            before,
            after,
            body: handle.clone(),
            syntax,
            syntax_stamp,
            guard: Arc::clone(selected.guard()),
            prepared_source,
            bytes,
        };
        QueryPoll::Ready(result.require_current(self).map(|()| result))
    }
}
