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
use crate::db::dependency::InputKey;

impl RunMachine {
    /// Before a recipe with `as` casts runs: install the discriminant of any
    /// enum operand whose table entry is still an unevaluated expression, or
    /// demand its initializer (the caller then enters it and prepares this PC
    /// again).
    pub(super) fn prepare_enum_discriminants(
        &mut self,
        db: &mut Database,
        id: crate::engine::install::RecipeId,
    ) -> QueryPoll<()> {
        match self.prepare_enum_discriminants_inner(db, id) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    fn prepare_enum_discriminants_inner(
        &mut self,
        db: &mut Database,
        id: crate::engine::install::RecipeId,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        use crate::runtime::shape::TypeKey;
        // No enum in the run has an unevaluated expression discriminant: the
        // overwhelmingly common case costs this one counter read.
        if !self.store.has_unevaluated_discriminants() {
            return Ok(QueryPoll::Ready(Ok(())));
        }
        let resume = self.frame.resume_operation_index();
        let operands: Vec<SlotId> = self
            .installed
            .get_current(db, id)?
            .recipe()
            .operations
            .iter()
            .skip(resume)
            .filter_map(|op| match op {
                Op::PrimitiveCast { source, .. } => Some(*source),
                _ => None,
            })
            .collect();
        for operand in operands {
            // An operand an earlier operation of this recipe produces has no
            // storage yet; it is a cast of a value the table already covers
            // or of a non-enum.
            let Ok(cell) = self.frame.cell(operand) else {
                continue;
            };
            let Ok(Some((enumeration, variant))) = self.store.pending_enum_discriminant(cell)
            else {
                continue;
            };
            let TypeKey::Nominal {
                identity,
                definition,
                ..
            } = self.store.type_descriptor(enumeration)?.key.as_ref()
            else {
                continue;
            };
            let (identity, definition) = (*identity, definition.clone());
            let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(identity)) {
                QueryPoll::Ready(Ok(captured)) => captured,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
            };
            let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
                return Err(call_fault("enum discriminant schema outcome differs"));
            };
            let anchor = schema
                .expression_anchor(variant as usize)
                .ok_or_else(|| call_fault("enum discriminant has no expression to evaluate"))?;
            let name = schema.variants[anchor]
                .name
                .as_deref()
                .ok_or_else(|| call_fault("enum variant lacks its name"))?;
            let row = crate::syntax::outline::discriminant_def_key(&definition, name).fingerprint();
            let Some((cell, _)) = self.const_cells.get(&row).cloned() else {
                // Source preparation publishes the rows of module enums, and
                // body entry those of block-local enums; an enum declared by
                // a published generated unit has none (E-R5-013 records the
                // gap).
                if db.input(&InputKey::DefinitionSource(row)).is_none() {
                    return Err(Diagnostic::new(
                        "LRR-E-UNSUPPORTED",
                        format!(
                            "the discriminant expression of `{}::{name}` has no initializer route: only enums declared in module source or in a function body publish their discriminant constants (an enum emitted into a generated unit does not yet)",
                            definition.declared_name
                        ),
                    ));
                }
                self.demand_initializer(row, false);
                return Ok(QueryPoll::Ready(Ok(())));
            };
            let value = self.store.read_scalar(cell)?;
            let value = match value.kind {
                4 => i128::from(value.payload as i64),
                5 => i128::from(value.payload),
                _ => return Err(call_fault("enum discriminant is not an integer")),
            };
            let table = schema.anchored_discriminants(anchor, value);
            self.store.attach_enum_discriminants(enumeration, &table)?;
        }
        Ok(QueryPoll::Ready(Ok(())))
    }
}
