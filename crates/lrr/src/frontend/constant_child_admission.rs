//! Source-owned child requests retain the parent's actual opaque evidence.
use super::*;
use crate::frontend::constant_result_type::CurrentArrayLengthDemand;
impl ConstantSourceAdmission {
    pub(crate) fn reselect_array_length_current(
        &self,
        db: &Database,
        source: &PreparedSourceView,
        demand: &CurrentArrayLengthDemand,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        // No current-stamp collection or source/proof clone before reservation.
        // Identical republish may require a fresh parent; revision is not evidence.
        if db.revision() != self.publication_revision {
            return Err(fault(
                "constant child requires a freshly admitted publication revision",
            ));
        }
        if byte_limit < self.metadata_bytes() {
            return Err(limit());
        }
        self.revalidate(db, source)?;
        demand.validate_parent(self)?;
        let child = Self::capture_current_impl(
            db,
            source,
            self.definition,
            ConstantRequest::ArrayLength(demand.range()),
            byte_limit.min(self.byte_limit),
            &self.macro_proofs,
            &self.sources.views(),
            self.local.clone(),
        )?;
        if child.plan.owner_definition() != self.plan.owner_definition()
            || child.plan.owner_context() != self.plan.owner_context()
            || child.plan.owner_body() != self.plan.owner_body()
            || child.selection != self.selection
            || child.value_seal != self.value_seal
            || child.type_seal != self.type_seal
            || child.value_fingerprint != self.value_fingerprint
            || child.type_fingerprint != self.type_fingerprint
            || child.stamps != self.stamps
            || child.plan.range() != demand.range()
        {
            return Err(fault(
                "constant child current source/header/type/value world changed",
            ));
        }
        let (_, call, length) = child.plan.entry()?;
        if call.is_some() || length.is_none_or(|actual| actual.range() != demand.range()) {
            return Err(fault(
                "constant child is not its exact source-owned length request",
            ));
        }
        Ok(child)
    }
}
