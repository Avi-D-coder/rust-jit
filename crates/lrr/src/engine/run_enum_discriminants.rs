//! Enum discriminant expressions run through the ordinary interpreter
//! (E-R5-013). A variant's explicit `= expr` that is not a plain integer
//! literal is an anonymous constant of the enum's representation type. The
//! source outline (for a block-local enum, the entered body's declaration
//! facts) publishes it as an initializer declaration, and this step demands
//! it exactly like a reached `const`: `demand_initializer` enters its body as
//! a zero-parameter source call, the scalar result is cached in `const_cells`,
//! and the value is merged into the enum descriptor's discriminant table. The
//! cast itself (`GuestStore::enum_cast_operand`) only reads the table.
use super::*;
use crate::{db::dependency::InputKey, facade::functions::LibraryFunction, runtime::shape::{TypeId, TypeKey}};

impl RunMachine {
    /// Probe retained ordinary cast positions after the frame's resume point.
    pub(super) fn prepare_enum_discriminants(
        &mut self, db: &mut Database, id: crate::engine::install::RecipeId,
    ) -> QueryPoll<()> {
        match self.prepare_enum_discriminants_inner(db, id) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    fn prepare_enum_discriminants_inner(
        &mut self, db: &mut Database, id: crate::engine::install::RecipeId,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        if !self.store.has_unevaluated_discriminants() {
            return Ok(QueryPoll::Ready(Ok(())));
        }
        let resume = self.frame.resume_operation_index();
        let operands = self.installed.enum_casts(db, id)?;
        for &(index, operand) in operands.iter() {
            if index < resume { continue; }
            // Earlier operations may not have produced this operand yet.
            let Ok(cell) = self.frame.cell(operand) else { continue; };
            let Ok(Some((enumeration, variant))) = self.store.pending_enum_discriminant(cell)
            else { continue; };
            match self.prepare_enum_discriminant_inner(db, enumeration, variant)? {
                QueryPoll::Ready(Ok(())) => {},
                poll => return Ok(poll),
            }
            if self.initializer_demand.is_some() { break; }
        }
        Ok(QueryPoll::Ready(Ok(())))
    }
    /// Called only after ordinary member selection, before argument sealing.
    /// The actual enum place and the existing initializer continuation are shared
    /// with casts; no argument is moved, sealed or evaluated a second time.
    pub(super) fn prepare_facade_discriminants(
        &mut self, db: &mut Database, site: u32, arguments: &[SlotId],
    ) -> QueryPoll<()> {
        match self.prepare_facade_discriminants_inner(db, site, arguments) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    fn prepare_facade_discriminants_inner(
        &mut self, db: &mut Database, site: u32, arguments: &[SlotId],
    ) -> Result<QueryPoll<()>, Diagnostic> {
        let identity = self.frame.call_identity(site);
        if !matches!(self.frame.facade_calls.get(&identity), Some((LibraryFunction::DiscriminantValue, _)))
            || !self.store.has_unevaluated_discriminants()
        {
            return Ok(QueryPoll::Ready(Ok(())));
        }
        let [slot] = arguments else { return Err(call_fault("discriminant call argument count differs")); };
        let value = self.store.live_value(self.frame.cell(*slot)?)?;
        let Some((enumeration, variant)) = self.store.pending_referent_discriminant(value)? else {
            return Ok(QueryPoll::Ready(Ok(())));
        };
        self.prepare_enum_discriminant_inner(db, enumeration, variant)
    }
    /// One demand/refinement coordinator for all discriminant consumers.
    fn prepare_enum_discriminant_inner(
        &mut self, db: &mut Database, enumeration: TypeId, variant: u32,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        let TypeKey::Nominal { identity, definition, .. } = self.store.type_descriptor(enumeration)?.key.as_ref()
        else { return Err(call_fault("pending discriminant lacks its enum declaration")); };
        let (identity, definition) = (*identity, definition.clone());
        let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(identity)) {
            QueryPoll::Ready(Ok(captured)) => captured,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return Err(call_fault("enum discriminant schema outcome differs"));
        };
        let anchor = schema.expression_anchor(variant as usize)
            .ok_or_else(|| call_fault("enum discriminant has no expression to evaluate"))?;
        let name = schema.variants[anchor].name.as_deref()
            .ok_or_else(|| call_fault("enum variant lacks its name"))?;
        let row = crate::syntax::outline::discriminant_def_key(&definition, name).fingerprint();
        let Some(stored) = self.const_cells.get(&row).cloned() else {
            if db.input(&InputKey::DefinitionSource(row)).is_none() {
                return Err(Diagnostic::new("LRR-E-UNSUPPORTED", format!(
                    "the discriminant expression of `{}::{name}` has no admitted ordinary initializer source",
                    definition.declared_name)));
            }
            self.demand_initializer(row, false);
            return Ok(QueryPoll::Ready(Ok(())));
        };
        let value = self.store.read_scalar(stored.cell)?;
        let value = self.store.integer_bits(value)? as i128;
        let table = schema.anchored_discriminants(anchor, value);
        self.store.attach_enum_discriminants(enumeration, &table)?;
        Ok(QueryPoll::Ready(Ok(())))
    }
}
