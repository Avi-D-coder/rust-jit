//! T054 reached scalar recipe adapter. It never fetches a query or prepares source.
//! Borrowed suspension remains distinct; only a genuine current success owns a recipe.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::DependencyStamp,
        key::QueryKey,
        memo::{Database, Outcome},
        poll::{InputRequest, QueryPoll},
        value::QueryValue,
    },
    engine::constant_array_publication::AdmittedConstantUse,
    frontend::{
        constant_construction::CurrentConstantConstruction,
        constant_path_selection::ConstantPathSelection,
        constant_result_type::CurrentConstantResultType,
        constants::{ConstLimits, ConstValue, query::fetch_current_constant_data},
    },
    ir::pc::{PcKey, Phase, SlotId},
    runtime::{
        cell::{FrameId, GuestStore},
        shape::{CopyStatus, PrimitiveIdentity},
    },
    source::changes::PreparedSourceView,
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        spans::BodySpan,
    },
    traits::term::TyTerm,
    vm::frame::{Frame, FrameStatus},
};
const MAX_BYTES: usize = 32 << 20;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-USE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "scalar constant use metadata limit".into(),
    )
}
/// Input bundle is not a certificate. Capture privately replays each real proof.
#[derive(Clone, Copy)]
pub(crate) struct ScalarUseSources<'a> {
    pub caller: &'a PreparedSourceView,
    pub views: &'a [&'a PreparedSourceView],
    pub target: &'a PreparedSourceView,
    pub result: &'a CurrentConstantResultType,
    pub path: &'a ConstantPathSelection,
}
/// Does not borrow the DB, so the driver can perform/resume its authorized query.
/// No serializer, raw digest constructor, complete flag or guest value is exposed.
pub(crate) struct ScalarConstantUseSite<'a> {
    sources: ScalarUseSources<'a>,
    frame: FrameId,
    pc: PcKey,
    span: BodySpan,
    body_index: Digest,
    destination: SlotId,
    session: crate::runtime::cell::StoreSession,
    epoch: u32,
    revision: u64,
    result_identity: Digest,
    scalar_identity: Digest,
    universe: Digest,
    bytes: usize,
    byte_limit: usize,
}
pub(crate) enum ScalarUsePreparation<'a> {
    NeedInput(&'a InputRequest),
    Ready(Result<PreparedScalarConstantUse<'a>, Diagnostic>),
}
pub(crate) struct PreparedScalarConstantUse<'a> {
    site: &'a ScalarConstantUseSite<'a>,
    construction: CurrentConstantConstruction<'a>,
    value: ConstValue,
}
impl<'a> ScalarConstantUseSite<'a> {
    pub(crate) fn capture_current(
        db: &Database,
        frame: &Frame,
        store: &GuestStore,
        sources: ScalarUseSources<'a>,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        let bytes = std::mem::size_of::<Self>()
            .checked_add(sources.result.metadata_bytes())
            .and_then(|n| n.checked_add(sources.path.metadata_bytes()))
            .and_then(|n| {
                n.checked_add(
                    sources
                        .views
                        .len()
                        .checked_mul(std::mem::size_of::<&PreparedSourceView>())?,
                )
            })
            .filter(|n| *n <= byte_limit && byte_limit <= MAX_BYTES)
            .ok_or_else(limit)?;
        frame.require_store(store)?;
        sources.result.revalidate(db, sources.target)?;
        sources.path.revalidate(db, sources.caller, sources.views)?;
        let scalar = sources
            .result
            .scalar()
            .ok_or_else(|| fault("scalar use requires exact closed source scalar readiness"))?;
        let plan = sources.result.admission().plan();
        let (constant, call, length) = plan.entry()?;
        if constant.is_none()
            || call.is_some()
            || length.is_some()
            || sources.path.target() != sources.target.selection()
            || sources.path.definition() != plan.owner_definition()
            || sources.path.context() != plan.owner_context()
            || Some(sources.path.body()) != plan.owner_body()
            || frame.status != FrameStatus::Running
            || frame.pc().body != sources.path.site().body().key()
            || frame.pc().phase != Phase::Entry.code()?
            || frame.body.profile() != &sources.caller.selection().outline.parse.profile
            || frame.body.cfg().fingerprint() != sources.caller.selection().outline.cfg
        {
            return Err(fault(
                "scalar source/caller/role/body/profile/cfg/phase differs",
            ));
        }
        let node = frame
            .body
            .nodes()
            .get(frame.pc().node as usize)
            .ok_or_else(|| fault("scalar reached node absent"))?;
        let destination = node
            .value
            .ok_or_else(|| fault("scalar source path has no result slot"))?;
        let span = frame.body.span(node.id)?;
        let body = sources.path.site().body();
        let range = sources.path.site().range();
        if node.role != NodeRole::Expression(ExpressionKind::Path)
            || body.range.start.checked_add(span.relative_start) != Some(range.start)
            || body.range.start.checked_add(span.relative_end) != Some(range.end)
        {
            return Err(fault("scalar selected original path/span differs"));
        }
        // Descriptor agreement follows the actual enclosing source proof; it is
        // never a member/type selector or a substitute for that proof.
        let selected = PrimitiveIdentity::from_recipe(scalar.primitive())?.id();
        if store.type_descriptor(selected)?.copy != CopyStatus::Copy {
            return Err(fault("scalar use requires admitted Copy storage"));
        }
        let source = TyTerm::Primitive(scalar.primitive());
        frame.preflight_source_view(destination, Some(&source))?;
        if frame
            .source_view_ref(destination)?
            .is_some_and(|actual| actual != &source)
            || frame
                .hint(destination)?
                .is_some_and(|actual| actual != selected)
        {
            return Err(fault(
                "scalar source result differs from reached destination context",
            ));
        }
        frame.preflight_write_type(destination, selected, store)?;
        Ok(Self {
            sources,
            frame: frame.constant_frame_anchor(),
            pc: frame.pc().clone(),
            span,
            body_index: frame.body.fingerprint(),
            destination,
            session: store.pattern_binding_session(),
            epoch: store.budget().epoch(),
            revision: db.revision(),
            result_identity: sources.result.identity(),
            scalar_identity: scalar.source_identity(),
            universe: db.constant_input_universe()?,
            bytes,
            byte_limit,
        })
    }
    fn revalidate(
        &self,
        db: &Database,
        frame: &Frame,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        if db.revision() != self.revision
            || db.constant_input_universe()? != self.universe
            || store.pattern_binding_session() != self.session
            || store.budget().epoch() != self.epoch
            || frame.constant_frame_anchor() != self.frame
            || frame.pc() != &self.pc
            || frame.body.fingerprint() != self.body_index
        {
            return Err(fault(
                "scalar use site is stale or foreign; resume requires fresh source capture",
            ));
        }
        let replay = Self::capture_current(db, frame, store, self.sources, self.byte_limit)?;
        if replay.destination != self.destination
            || replay.span != self.span
            || replay.bytes != self.bytes
            || replay.result_identity != self.result_identity
            || replay.scalar_identity != self.scalar_identity
        {
            return Err(fault("scalar use current source/site changed"));
        }
        Ok(())
    }
    /// Authorized coordinator query step, never a provider/source/guest effect.
    /// The site owns no DB borrow; the returned poll is an owned query result.
    pub(crate) fn fetch_current(
        &self,
        db: &mut Database,
        frame: &Frame,
        store: &GuestStore,
        limits: ConstLimits,
    ) -> QueryPoll<Outcome> {
        if let Err(error) = self.revalidate(db, frame, store) {
            return QueryPoll::Ready(Err(error));
        }
        let Some(scalar) = self.sources.result.scalar() else {
            return QueryPoll::Ready(Err(fault("scalar source readiness absent")));
        };
        let Some(bytes) = self.byte_limit.checked_sub(self.bytes).filter(|n| *n != 0) else {
            return QueryPoll::Ready(Err(limit()));
        };
        fetch_current_constant_data(
            db,
            self.sources.target,
            self.sources.result.admission(),
            &scalar.wire_type(),
            limits,
            bytes.min(1 << 20),
        )
    }
    fn diagnostic(&self, error: &Diagnostic) -> Diagnostic {
        if error
            .heap_bytes()
            .and_then(|n| n.checked_add(std::mem::size_of::<Diagnostic>()))
            .and_then(|n| n.checked_add(self.bytes))
            .is_none_or(|n| n > self.byte_limit)
        {
            limit()
        } else {
            error.clone()
        }
    }
    /// No query call, input publication, trace/pin mutation, fuel or guest allocation.
    pub(crate) fn accept_poll<'b>(
        &'b self,
        db: &'b Database,
        frame: &Frame,
        store: &GuestStore,
        poll: &'b QueryPoll<Outcome>,
    ) -> ScalarUsePreparation<'b> {
        if let Err(error) = self.revalidate(db, frame, store) {
            return ScalarUsePreparation::Ready(Err(error));
        }
        let outcome = match poll {
            QueryPoll::NeedInput(input) => {
                return match input.validate() {
                    Ok(()) => ScalarUsePreparation::NeedInput(input),
                    Err(error) => ScalarUsePreparation::Ready(Err(error)),
                };
            }
            QueryPoll::Ready(Err(error)) => {
                return ScalarUsePreparation::Ready(Err(self.diagnostic(error)));
            }
            QueryPoll::Ready(Ok(outcome)) => outcome,
        };
        let selected = || -> Result<PreparedScalarConstantUse<'b>, Diagnostic> {
            let key = match outcome.value.as_ref() {
                QueryValue::Constant(value) => value.key(),
                QueryValue::ConstantFailure(value) => value.key(),
                QueryValue::Error(error) => return Err(self.diagnostic(error)),
                _ => return Err(fault("scalar query returned another value variant")),
            };
            key.validate()?;
            let admission = self.sources.result.admission();
            let plan = admission.plan();
            let expected = self
                .sources
                .result
                .scalar()
                .ok_or_else(|| fault("scalar source readiness absent"))?
                .wire_type();
            if key.definition() != plan.owner_definition()
                || key.selection() != plan.identity()
                || key.source() != plan.source().source()
                || key.cfg() != plan.source().cfg()
                || key.profile() != plan.profile()
                || key.admission() != admission.identity()
                || key.result_type() != expected.fingerprint_checked()?
                || key.universe() != Some(self.universe)
            {
                return Err(fault(
                    "scalar outcome is not its exact current selected query",
                ));
            }
            let memo = db
                .memo(&QueryKey::Constant(key.clone()))
                .ok_or_else(|| fault("scalar use requires actual current query memo"))?;
            if !outcome.cacheable
                || memo.verified_revision != db.revision()
                || outcome.fingerprint != outcome.value.fingerprint()
                || outcome.fingerprint != memo.fingerprint
                || memo.fingerprint != memo.value.fingerprint()
                || memo.dependencies.len() != admission.dependencies().len()
            {
                return Err(fault("scalar returned outcome/current memo differs"));
            }
            for (key, fingerprint) in admission.dependencies() {
                if !memo.dependencies.contains(&DependencyStamp::Input {
                    key: key.clone(),
                    fingerprint: *fingerprint,
                }) || db
                    .published_input(key)
                    .is_none_or(|(_, actual)| actual != *fingerprint)
                {
                    return Err(fault("scalar current admitted input dependency differs"));
                }
            }
            let value = match outcome.value.as_ref() {
                QueryValue::Constant(value) => value,
                QueryValue::ConstantFailure(failure) => {
                    return Err(self.diagnostic(failure.diagnostic()));
                }
                _ => unreachable!("checked constant query variants"),
            };
            let remaining = self
                .byte_limit
                .checked_sub(self.bytes)
                .and_then(|n| n.checked_sub(std::mem::size_of::<PreparedScalarConstantUse<'_>>()))
                .ok_or_else(limit)?;
            let construction = CurrentConstantConstruction::capture_current(
                db,
                self.sources.target,
                self.sources.result,
                value,
                None,
                remaining,
            )?;
            let value = value.payload().decode_value()?;
            value
                .scalar()
                .ok_or_else(|| fault("successful scalar recipe contains an aggregate"))?
                .validate_scalar()?;
            Ok(PreparedScalarConstantUse {
                site: self,
                construction,
                value,
            })
        };
        ScalarUsePreparation::Ready(selected())
    }
}
impl PreparedScalarConstantUse<'_> {
    /// Conservative retained source/query/recipe bytes for the driver budget.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.site.bytes)
            .and_then(|n| n.checked_add(self.construction.metadata_bytes()))
            .filter(|n| *n <= self.site.byte_limit)
            .ok_or_else(limit)
    }

    /// Preparation precedes the dispatcher's single ordinary charge callback.
    /// Existing field charges/reservation/abort/temporary transaction stay unchanged.
    /// No borrowed proof/transaction escapes the local synchronous scope.
    pub(crate) fn publish_with_opcode_charge(
        &self,
        frame: &mut Frame,
        store: &mut GuestStore,
        charge: impl FnOnce(&mut Frame) -> Result<(), Diagnostic>,
    ) -> Result<(), Diagnostic> {
        self.metadata_bytes()?;
        self.site
            .revalidate(self.construction.database(), frame, store)?;
        let binding = self.construction.bind_guest_descriptor(store)?;
        let proof = AdmittedConstantUse::capture_current(
            frame,
            self.site.destination,
            self.site.sources.caller,
            self.site.sources.views,
            self.site.sources.path,
            binding,
            store,
        )?;
        let prepared = frame.prepare_constant_publication(
            &proof,
            &self.value,
            self.construction.wire(),
            store,
        )?;
        charge(frame)?;
        frame.publish_constant_to_temporary(prepared, store)
    }
}
#[cfg(test)]
#[path = "scalar_constant_use_tests.rs"]
mod tests;
