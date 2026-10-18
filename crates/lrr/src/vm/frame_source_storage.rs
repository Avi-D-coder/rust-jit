//! Checked ownership of composite source-view heap. Existing fixed frame
//! accounting reserves two term nodes per view; this records actual excess.
//! Source changes remain separate from guest ownership effects.
use super::*;
use crate::traits::term::TyTerm;

impl Frame {
    /// Closed retained source shapes have an allocation-free checked measure.
    /// This inspects source terms, never source syntax or a runtime descriptor.
    fn retained_source_heap(source: &TyTerm) -> Option<usize> {
        match source {
            TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Param(_) => Some(0),
            TyTerm::Nominal(_, args) => {
                let mut bytes = args
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::traits::term::GenericArg>())?;
                // An instantiated generic nominal's closed type arguments.
                for arg in args {
                    let crate::traits::term::GenericArg::Type(ty) = arg else {
                        return None;
                    };
                    bytes = bytes.checked_add(Self::retained_source_heap(ty)?)?;
                }
                Some(bytes)
            }
            TyTerm::Ref(_, _, target)
            | TyTerm::Slice(target)
            | TyTerm::Array(target, crate::traits::term::ConstTerm::Value(_)) => {
                std::mem::size_of::<TyTerm>().checked_add(Self::retained_source_heap(target)?)
            }
            TyTerm::Tuple(members) => {
                let mut bytes = members
                    .capacity()
                    .checked_mul(std::mem::size_of::<TyTerm>())?;
                for member in members {
                    bytes = bytes.checked_add(Self::retained_source_heap(member)?)?;
                }
                Some(bytes)
            }
            // A closed fn-pointer view (E-R5-008).
            TyTerm::Function(function) => {
                let mut bytes = std::mem::size_of::<
                    crate::traits::term::Binder<crate::traits::term::FunctionSignature>,
                >()
                .checked_add(
                    function
                        .value
                        .parameters
                        .capacity()
                        .checked_mul(std::mem::size_of::<TyTerm>())?,
                )?
                .checked_add(std::mem::size_of::<TyTerm>())?;
                for parameter in &function.value.parameters {
                    bytes = bytes.checked_add(Self::retained_source_heap(parameter)?)?;
                }
                bytes.checked_add(Self::retained_source_heap(&function.value.result)?)
            }
            _ => None,
        }
    }
    pub(super) fn source_heap(source: Option<&TyTerm>) -> Result<usize, Diagnostic> {
        source
            .map_or(Some(0), Self::retained_source_heap)
            .ok_or_else(crate::runtime::arena::resource)
    }
    pub(super) fn source_excess(source: Option<&TyTerm>) -> Result<usize, Diagnostic> {
        let heap = Self::source_heap(source)?;
        Ok(heap.saturating_sub(2 * std::mem::size_of::<TyTerm>()))
    }
    pub(super) fn source_storage_limit(
        &self,
        excess: usize,
        transient: usize,
    ) -> Result<(), Diagnostic> {
        Self::metadata_bytes(&self.body, self.parameter_metadata_count())
            .and_then(|bytes| bytes.checked_add(self.constructor_mapping_bytes))
            .and_then(|bytes| bytes.checked_add(self.projection_bytes))
            .and_then(|bytes| bytes.checked_add(self.auxiliary_metadata_bytes()?))
            .and_then(|bytes| bytes.checked_add(excess))
            .and_then(|bytes| bytes.checked_add(transient))
            .ok_or_else(crate::runtime::arena::resource)?;
        Ok(())
    }
    pub(super) fn preflight_source_replacement(
        &self,
        old: Option<&TyTerm>,
        new: Option<&TyTerm>,
    ) -> Result<usize, Diagnostic> {
        let new_excess = Self::source_excess(new)?;
        let excess = self
            .source_extra_bytes
            .checked_sub(Self::source_excess(old)?)
            .and_then(|bytes| bytes.checked_add(new_excess))
            .ok_or_else(|| invalid("source-view accounting overflow/underflow"))?;
        self.source_storage_limit(excess, 0)?;
        Ok(excess)
    }
    pub(super) fn clone_source(
        &self,
        source: Option<&TyTerm>,
    ) -> Result<Option<TyTerm>, Diagnostic> {
        let heap = Self::source_heap(source)?;
        self.source_storage_limit(self.source_extra_bytes, heap)?;
        Ok(source.cloned())
    }
    /// Check the entire publication before CallPrepare can consume fuel or
    /// install its first argument. Pending owned facts and the sorted identity
    /// scratch remain charged while all source replacements are considered.
    pub(super) fn preflight_call_source_views(
        &self,
        hints: &Vec<(SlotId, Option<TypeId>, Option<TyTerm>)>,
    ) -> Result<(), Diagnostic> {
        let mut transient = hints
            .capacity()
            .checked_mul(std::mem::size_of::<(SlotId, Option<TypeId>, Option<TyTerm>)>())
            .and_then(|bytes| {
                hints
                    .len()
                    .checked_mul(std::mem::size_of::<SlotId>())
                    .and_then(|scratch| bytes.checked_add(scratch))
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut excess = self.source_extra_bytes;
        let mut peak = excess;
        for (slot, _, source) in hints {
            transient = transient
                .checked_add(Self::source_heap(source.as_ref())?)
                .ok_or_else(crate::runtime::arena::resource)?;
            excess = excess
                .checked_sub(Self::source_excess(self.slot(*slot)?.source.as_ref())?)
                .and_then(|bytes| bytes.checked_add(Self::source_excess(source.as_ref()).ok()?))
                .ok_or_else(|| invalid("call source accounting overflow/underflow"))?;
            peak = peak.max(excess);
        }
        transient = transient
            .checked_add(
                usize::from(self.call_progress.len() == self.call_progress.capacity())
                    .checked_mul(std::mem::size_of::<(
                        crate::vm::call::CallIdentity,
                        (u32, u32),
                    )>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(crate::runtime::arena::resource)?;
        self.source_storage_limit(peak, transient)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(hints.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        let extra = slots
            .capacity()
            .checked_sub(hints.len())
            .and_then(|count| count.checked_mul(std::mem::size_of::<SlotId>()))
            .and_then(|extra| transient.checked_add(extra))
            .ok_or_else(crate::runtime::arena::resource)?;
        self.source_storage_limit(peak, extra)?;
        slots.extend(hints.iter().map(|(slot, _, _)| slot.0));
        slots.sort_unstable();
        if slots.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("call source arguments alias a temporary slot"));
        }
        Ok(())
    }
    pub(super) fn preflight_binding_source_view(
        &self,
        binding: NodeId,
        source: Option<&TyTerm>,
    ) -> Result<usize, Diagnostic> {
        let old = self
            .binding_sources
            .get(binding.0 as usize)
            .ok_or_else(|| invalid("binding source absent"))?;
        let excess = self.preflight_source_replacement(old.as_ref(), source)?;
        self.source_storage_limit(excess, Self::source_heap(source)?)?;
        Ok(excess)
    }
    pub(super) fn set_binding_source_view(
        &mut self,
        binding: NodeId,
        source: Option<TyTerm>,
    ) -> Result<(), Diagnostic> {
        let excess = self.preflight_binding_source_view(binding, source.as_ref())?;
        self.binding_sources[binding.0 as usize] = source;
        self.source_extra_bytes = excess;
        Ok(())
    }
    pub(super) fn preflight_borrow_source_storage(
        &self,
        place: SlotId,
        destination: SlotId,
    ) -> Result<(), Diagnostic> {
        if let Some(written) = self.source_view_ref(destination)? {
            return self.preflight_source_view(destination, Some(written));
        }
        let source = self.place_slot(place)?.source.as_ref();
        if !matches!(
            source,
            Some(
                TyTerm::Primitive(_)
                    | TyTerm::Nominal(_, _)
                    | TyTerm::Array(_, _)
                    | TyTerm::Tuple(_)
            )
        ) {
            return Ok(());
        }
        let heap = Self::source_heap(source)?
            .checked_add(std::mem::size_of::<TyTerm>())
            .ok_or_else(crate::runtime::arena::resource)?;
        let excess = self
            .source_extra_bytes
            .checked_add(heap.saturating_sub(2 * std::mem::size_of::<TyTerm>()))
            .ok_or_else(crate::runtime::arena::resource)?;
        self.source_storage_limit(excess, heap)
    }
    pub(super) fn clear_parameter_source_views(&mut self) -> Result<(), Diagnostic> {
        let released = self
            .parameter_sources
            .iter()
            .try_fold(0usize, |bytes, source| {
                bytes
                    .checked_add(Self::source_excess(Some(source))?)
                    .ok_or_else(crate::runtime::arena::resource)
            })?;
        self.source_extra_bytes = self
            .source_extra_bytes
            .checked_sub(released)
            .ok_or_else(|| invalid("parameter source cleanup accounting underflow"))?;
        self.parameter_sources.clear();
        Ok(())
    }
    pub(crate) fn set_return_source_view(
        &mut self,
        source: Option<TyTerm>,
    ) -> Result<(), Diagnostic> {
        let excess =
            self.preflight_source_replacement(self.return_source.as_ref(), source.as_ref())?;
        self.return_source = source;
        self.source_extra_bytes = excess;
        Ok(())
    }
}
