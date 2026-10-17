//! Actual selected local-body source admission. Call selection stays in the
//! ordinary callable service; this joins its proof to the retained parent plan.
use super::*;
use crate::{
    core::digest::StableEncoder,
    frontend::{callables::SourceCallable, source_locals::PreparedSourceLocalPlan},
    syntax::parse::ParseKey,
};

pub(super) struct LocalBodyContext {
    parent: Arc<PreparedSourceLocalPlan>,
    owner: Arc<InputValue>,
    owner_stamp: Digest,
    definition: Digest,
    depth: u16,
    retained_readiness: Option<Digest>,
    identity: Digest,
}
impl LocalBodyContext {
    pub(super) fn parent(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.parent
    }
    pub(super) fn owner_input(&self) -> &Arc<InputValue> {
        &self.owner
    }
    pub(super) fn identity(&self) -> Digest {
        self.identity
    }
    pub(super) fn has_retained_readiness(&self) -> bool {
        self.retained_readiness.is_some()
    }
    fn owner_matches(&self, db: &mut Database) -> bool {
        let key = InputKey::LocalDeclarationOwner(self.definition);
        matches!(db.input(&key).as_deref(), Some(InputValue::LocalDeclarationOwner(body))
            if body == self.parent.context().source().index().body())
            && db
                .published_input(&key)
                .is_some_and(|(_, stamp)| stamp == self.owner_stamp)
    }
    pub(super) fn has_current_inputs(&self, db: &mut Database) -> Result<bool, Diagnostic> {
        let Some(environment) = self.parent.environment() else {
            return Ok(false);
        };
        if !self.owner_matches(db)
            || !matches!(db.input(&InputKey::BodyEnvironment(environment.body().clone())).as_deref(),
                Some(InputValue::BodyEnvironment(current)) if current.identity() == environment.identity())
        {
            return Ok(false);
        }
        self.parent.context().source().has_current_inputs(db)
    }
    pub(super) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        match self.parent.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            other => return other,
        }
        if !self.owner_matches(db) {
            return QueryPoll::Ready(Err(fault("local body actual owning declaration changed")));
        }
        QueryPoll::Ready(Ok(()))
    }
    pub(super) fn capture(
        db: &mut Database,
        parent: Arc<PreparedSourceLocalPlan>,
        callee: &SourceCallable,
        parse: &ParseKey,
        handle: &BodyHandle,
        declaration: &DeclarationContext,
        header: &HeaderRecipe,
    ) -> QueryPoll<Self> {
        match parent.require_current(db) {
            QueryPoll::Ready(Ok(())) => {}
            other => {
                return match other {
                    QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
                    _ => unreachable!(),
                };
            }
        }
        QueryPoll::Ready((|| {
            let environment = parent
                .environment()
                .ok_or_else(|| fault("local body parent environment is not published"))?;
            let proof = callee
                .local_source_contract()
                .ok_or_else(|| fault("local body requires its actual selected source admission"))?;
            // This proves source/body facts, never a current Value selection.
            // The ordinary selected-call contract owns callable revalidation;
            // do not repeat its legacy whole-namespace census here.
            let source = parent.context().source();
            let caller = source.body();
            if proof.caller() != caller
                || proof.source() != parse
                || source.parsed().key() != *parse
                || proof.caller_context() != source.context()
                || proof.caller_header() != source.header()
                || callee.body() != Some(handle)
                || handle.key().definition != callee.identity()
                || !caller.range.contains(declaration.range)
                || !declaration.range.contains(handle.range)
                || !(caller.range.start < handle.range.start && handle.range.end < caller.range.end)
            {
                return Err(fault("local body selected caller/source/body differs"));
            }
            let mut rows = parent
                .context()
                .facts()
                .items
                .iter()
                .filter(|row| row.item.def_key.fingerprint() == callee.identity());
            let row = rows
                .next()
                .ok_or_else(|| fault("local body is absent from its actual parent"))?;
            if rows.next().is_some()
                || row.attribute_input.is_some()
                || row.item != *proof.item()
                || row.scope != proof.scope()
                || row.item.body.as_ref() != Some(handle)
                || row.item.header != *header
                || row.item.scope != declaration.scope
                || row.item.declaration_span != declaration.range
                || row.item.def_key != *callee.definition()
            {
                return Err(fault("local body canonical parent declaration differs"));
            }
            let definition = callee.identity();
            let owner = db
                .input(&InputKey::LocalDeclarationOwner(definition))
                .ok_or_else(|| fault("local body actual owning declaration is absent"))?;
            if !matches!(owner.as_ref(),InputValue::LocalDeclarationOwner(body) if body == &caller.key())
            {
                return Err(fault("local body actual owning declaration differs"));
            }
            let depth = source
                .local_body
                .as_ref()
                .map_or(Some(1), |local| local.depth.checked_add(1))
                .filter(|depth| *depth <= 256)
                .ok_or_else(crate::runtime::arena::resource)?;
            let owner_stamp = owner.fingerprint();
            if db
                .published_input(&InputKey::LocalDeclarationOwner(definition))
                .is_none_or(|(_, stamp)| stamp != owner_stamp)
            {
                return Err(fault("local body owning declaration fingerprint differs"));
            }
            // Ancestor entry readiness does not prepare this function's own
            // pending macros. Inspect the actual residual structural calls
            // once, at cold admission, and retain the immutable proof.
            let retained_readiness = match source.source_receipt.as_deref() {
                Some(InputValue::RetainedSyntax(unit)) => {
                    if unit.key() != parse {
                        return Err(fault("local readiness retained source key differs"));
                    }
                    Some(
                        unit.prepared()
                            .require_closed_function_body(source.parsed(), handle.range)?,
                    )
                }
                _ => None,
            };
            let mut identity = StableEncoder::new("lrr/v1/entered-local-body-parent");
            identity.digest(environment.identity());
            identity.digest(parent.source_identity());
            identity.digest(owner_stamp);
            // The selected call contract owns call-site/currentness issuance;
            // these source facts are deliberately reusable across
            // actual calls to the same declaration under the same parent.
            identity.record(callee.definition());
            identity.record(parse);
            identity.u32(row.scope);
            identity.record(&handle.key());
            identity.option(&retained_readiness, |e, proof| e.digest(*proof));
            Ok(Self {
                parent,
                owner,
                owner_stamp,
                definition,
                depth,
                retained_readiness,
                identity: identity.finish(),
            })
        })())
    }
}

/// Called only for the already selected, actually entered ordinary callee. This
/// uses capture's existing exact input/parser/index checks and current query Arc.
pub(crate) fn capture_local(
    db: &mut Database,
    parent: Arc<PreparedSourceLocalPlan>,
    callee: &SourceCallable,
    entered_index: &Arc<BodyIndex>,
    entered_pc: &PcKey,
) -> QueryPoll<AdmittedBodyContext> {
    let entry = match entered_index.pc(NodeId(0), Phase::Entry) {
        Ok(entry) => entry,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if entry != *entered_pc
        || callee
            .body()
            .is_none_or(|body| body.key() != entered_pc.body)
    {
        return QueryPoll::Ready(Err(fault("local body requires its actual callee entry PC")));
    }
    match super::capture_inner(db, &[], entered_index, entered_pc, Some((parent, callee))) {
        Ok(poll) => poll,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
