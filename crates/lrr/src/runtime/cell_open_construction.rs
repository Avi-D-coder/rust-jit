//! A generic record whose explicit Type argument still holds a `_` begins with
//! a guard that knows only its declared field count. The reached constructor
//! settles it once, before the first field moves in, from the pinned inference
//! answer: the guard then equals one begun with the closed arguments.
use super::*;

impl GuestStore {
    pub(crate) fn begin_open_nominal_construction(
        &mut self,
        declared_fields: usize,
    ) -> Result<ConstructionId, Diagnostic> {
        self.constructions.reserve(1, &mut self.budget)?;
        let expected = vec![None; declared_fields];
        let record = ConstructionRecord::partial(&expected)?;
        let bytes = record.owned_bytes().ok_or_else(resource)?;
        self.constructions
            .allocate(record, bytes, &mut self.budget)
            .map(ConstructionId::from)
    }
    /// Fix the nominal identity, argument list and every declared field Type of
    /// an open guard that no field has entered. Fails closed on any other state.
    pub(crate) fn settle_open_nominal_construction(
        &mut self,
        guard: ConstructionId,
        bound: &crate::frontend::types::BoundNominalVariant,
        args: &[TypeId],
    ) -> Result<TypeId, Diagnostic> {
        let record = self.constructions.get(guard)?;
        if record.state != ConstructionState::Building
            || record.pending_cleanup.is_some()
            || !record.completed.is_empty()
            || record.nominal.is_some()
            || record.pending_nominal.is_some()
            || record.observed_kind.is_some()
            || record.fields.len() != bound.fields().len()
            || record.expected.iter().any(Option::is_some)
        {
            return Err(ownership());
        }
        let ty = self.intern_nominal_with(bound, args)?;
        let mut fields = Vec::new();
        fields
            .try_reserve_exact(bound.fields().len())
            .map_err(|_| resource())?;
        for field in bound.fields() {
            let field_ty = field.materialize(self, args)?;
            if self.types.descriptor(field_ty)?.shape == ShapeCategory::Unsized {
                return Err(invalid());
            }
            fields.push(Some(field_ty));
        }
        let record = self.constructions.get_mut(guard)?;
        record.expected = fields;
        record.nominal = Some((ty, bound.variant()));
        Ok(ty)
    }
}
