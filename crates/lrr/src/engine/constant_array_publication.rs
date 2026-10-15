//! Borrowed T054 publication from actual current source/type/query certificates.
//! Replays source selection read-only; no provider or guest side effects.
#[cfg(test)]
use crate::core::digest::StableEncode;
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    frontend::constants::{
        ConstValue,
        wire::{self, ConstantWireType},
    },
    frontend::{
        constant_construction::SelectedGuestConstantType,
        constant_path_selection::ConstantPathSelection,
    },
    ir::pc::{PcKey, SlotId},
    runtime::{
        cell::{FrameId, GuestStore},
        shape::TypeId,
    },
    source::changes::PreparedSourceView,
    syntax::spans::BodySpan,
    traits::term::TyTerm,
};
// Only the actual privately source-selected construction/descriptor join may
// create production authority. Raw key/payload/descriptor/digests cannot do so.
// The zero-sized physical fixture variant exists only in tests. Keep the
// production source proof inline with its accounted metadata representation.
#[cfg_attr(test, allow(clippy::large_enum_variant))]
enum PublicationAuthority<'a> {
    SelectedSource {
        path: &'a ConstantPathSelection,
        binding: SelectedGuestConstantType<'a, 'a>,
        caller: &'a PreparedSourceView,
        views: &'a [&'a PreparedSourceView],
    },
    #[cfg(test)]
    PhysicalFixtureOnly,
}
pub(crate) struct AdmittedConstantUse<'a> {
    _authority: PublicationAuthority<'a>,
    session: crate::runtime::cell::StoreSession,
    frame: FrameId,
    pc: PcKey,
    destination: SlotId,
    span: BodySpan,
    body_index: Digest,
    source: TyTerm,
    ty: TypeId,
    content: Digest,
}
impl<'a> AdmittedConstantUse<'a> {
    pub(crate) fn revalidate(&self, store: &GuestStore) -> Result<(), Diagnostic> {
        if self.session != store.pattern_binding_session() {
            return Err(crate::runtime::arena::stale());
        }
        match &self._authority {
            PublicationAuthority::SelectedSource {
                path,
                binding,
                caller,
                views,
            } => {
                binding.revalidate(store)?;
                path.revalidate(binding.construction().database(), caller, views)
            }
            #[cfg(test)]
            PublicationAuthority::PhysicalFixtureOnly => Ok(()),
        }
    }
    pub(crate) fn capture_source_for_dispatch(
        frame: &crate::vm::frame::Frame,
        destination: SlotId,
        caller: &'a PreparedSourceView,
        views: &'a [&'a PreparedSourceView],
        path: &'a ConstantPathSelection,
        binding: SelectedGuestConstantType<'a, 'a>,
        store: &GuestStore,
    ) -> Result<Self, Diagnostic> {
        use crate::core::digest::StableEncode;
        frame.require_store(store)?;
        let construction = binding.construction();
        binding.revalidate(store)?;
        path.revalidate(construction.database(), caller, views)?;
        let admission = construction.result().admission();
        let plan = admission.plan();
        let (constant, call, length) = plan.entry()?;
        if constant.is_none()
            || call.is_some()
            || length.is_some()
            || path.target() != construction.source().selection()
            || path.definition() != plan.owner_definition()
            || path.context() != plan.owner_context()
            || Some(path.body()) != plan.owner_body()
            || frame.pc().body != path.site().body().key()
            || frame.body.profile() != &caller.selection().outline.parse.profile
            || frame.body.cfg().fingerprint() != caller.selection().outline.cfg
        {
            return Err(Diagnostic::new(
                "LRR-E-CONST-PUBLICATION-SOURCE",
                "selected constant result/caller/source/body/profile differs".into(),
            ));
        }
        let pc = frame.pc().clone();
        let node = frame.body.nodes().get(pc.node as usize).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-CONST-PUBLICATION-SOURCE",
                "constant publication node absent".into(),
            )
        })?;
        let span = frame.body.span(node.id)?;
        let body = path.site().body();
        let range = path.site().range();
        if node.value != Some(destination)
            || pc.phase != crate::ir::pc::Phase::Entry.code()?
            || body.range.start.checked_add(span.relative_start) != Some(range.start)
            || body.range.start.checked_add(span.relative_end) != Some(range.end)
        {
            return Err(Diagnostic::new(
                "LRR-E-CONST-PUBLICATION-SOURCE",
                "selected constant original site/span/destination differs".into(),
            ));
        }
        std::mem::size_of::<Self>()
            .checked_add(construction.metadata_bytes())
            .and_then(|n| n.checked_add(path.metadata_bytes()))
            .and_then(|n| n.checked_add(construction.source_term().heap_bytes()?))
            .filter(|n| *n <= 32 << 20)
            .ok_or_else(crate::runtime::arena::resource)?;
        let source = construction.source_term().clone();
        let ty = binding.ty();
        let content = Digest::of_bytes(construction.parent().payload().bytes());
        let proof = Self {
            _authority: PublicationAuthority::SelectedSource {
                path,
                binding,
                caller,
                views,
            },
            session: store.pattern_binding_session(),
            frame: frame.constant_frame_anchor(),
            pc,
            destination,
            span,
            body_index: frame.body.fingerprint(),
            source,
            ty,
            content,
        };
        frame.preflight_constant_source(&proof, store)?;
        Ok(proof)
    }
    #[cfg(test)]
    pub(crate) fn capture_current(
        frame: &crate::vm::frame::Frame,
        destination: SlotId,
        caller: &'a PreparedSourceView,
        views: &'a [&'a PreparedSourceView],
        path: &'a ConstantPathSelection,
        binding: SelectedGuestConstantType<'a, 'a>,
        store: &GuestStore,
    ) -> Result<Self, Diagnostic> {
        let proof = Self::capture_source_for_dispatch(
            frame,
            destination,
            caller,
            views,
            path,
            binding,
            store,
        )?;
        frame.preflight_constant_use(&proof, store)?;
        Ok(proof)
    }
    pub(crate) fn matches_instruction(
        &self,
        instruction: &crate::ir::source_constant::SourceConstantInstruction,
    ) -> bool {
        let (path, binding) = match &self._authority {
            PublicationAuthority::SelectedSource { path, binding, .. } => (path, binding),
            #[cfg(test)]
            PublicationAuthority::PhysicalFixtureOnly => return false,
        };
        self.pc == *instruction.key()
            && self.span == *instruction.span()
            && self.body_index == instruction.body_index()
            && self.destination == instruction.destination()
            && self.content == instruction.content()
            && path.identity() == instruction.path()
            && binding.construction().parent().key() == instruction.query()
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        let retained = match &self._authority {
            PublicationAuthority::SelectedSource { path, binding, .. } => binding
                .construction()
                .metadata_bytes()
                .checked_add(path.metadata_bytes())?,
            #[cfg(test)]
            PublicationAuthority::PhysicalFixtureOnly => 0,
        };
        std::mem::size_of::<Self>()
            .checked_add(retained)?
            .checked_add(self.source.heap_bytes()?)
    }
    /// Immutable agreement data only; never a source/type authority constructor.
    pub(crate) fn content_identity(&self) -> Digest {
        self.content
    }
    /// Retain the actual current selected use binding across physical Dispatch.
    /// Every consumer must separately replay this full opaque proof each chunk.
    pub(crate) fn source_use_identity(&self) -> Digest {
        use crate::core::digest::StableEncoder;
        let mut e = StableEncoder::new("lrr/v5/constant-use-agreement");
        match &self._authority {
            PublicationAuthority::SelectedSource { path, binding, .. } => {
                e.u8(1);
                e.digest(path.definition());
                e.record(path.context());
                e.record(&path.body().key());
                e.u32(path.body().range.start);
                e.u32(path.body().range.end);
                e.digest(binding.construction().result().identity());
                e.record(binding.construction().parent().key());
            }
            #[cfg(test)]
            PublicationAuthority::PhysicalFixtureOnly => e.u8(0),
        }
        e.record(&self.pc);
        e.record(&self.span.body_key);
        e.u32(self.span.relative_start);
        e.u32(self.span.relative_end);
        e.u32(self.destination.0);
        e.u64(self.frame.bits());
        e.digest(self.body_index);
        e.record(&self.source);
        e.u32(self.ty.0);
        e.digest(self.content);
        e.finish()
    }
    pub(crate) fn frame(&self) -> FrameId {
        self.frame
    }
    pub(crate) fn pc(&self) -> &PcKey {
        &self.pc
    }
    pub(crate) fn destination(&self) -> SlotId {
        self.destination
    }
    pub(crate) fn span(&self) -> &BodySpan {
        &self.span
    }
    pub(crate) fn body_index(&self) -> Digest {
        self.body_index
    }
    pub(crate) fn source(&self) -> &TyTerm {
        &self.source
    }
    pub(crate) fn ty(&self) -> TypeId {
        self.ty
    }
    pub(crate) fn admits_content(
        &self,
        value: &ConstValue,
        ty: &ConstantWireType,
    ) -> Result<bool, Diagnostic> {
        Ok(Digest::of_bytes(&wire::encode(value, ty, 1 << 20)?) == self.content)
    }
}
/// Test-only physical injection. It is NOT source/namespace/query/type authority
/// and must never become a production constructor or full-fixture pass.
#[cfg(test)]
pub(crate) fn physical_fixture_use<'a>(
    frame: &crate::vm::frame::Frame,
    destination: SlotId,
    source: TyTerm,
    ty: TypeId,
    value: &'a ConstValue,
    wire: &ConstantWireType,
) -> Result<AdmittedConstantUse<'a>, Diagnostic> {
    let pc = frame.pc().clone();
    Ok(AdmittedConstantUse {
        _authority: PublicationAuthority::PhysicalFixtureOnly,
        session: frame.store_session(),
        frame: frame.constant_frame_anchor(),
        span: frame.body.span(crate::ir::pc::NodeId(pc.node))?,
        pc,
        destination,
        body_index: frame.body.fingerprint(),
        source,
        ty,
        content: Digest::of_bytes(&wire::encode(value, wire, 1 << 20)?),
    })
}

#[cfg(test)]
#[path = "constant_source_publication_tests.rs"]
mod tests;
