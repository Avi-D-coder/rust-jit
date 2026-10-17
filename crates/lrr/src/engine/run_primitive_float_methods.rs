//! Run-owned method receipts; cached recipes do not bypass receiver admission.
use super::*;
use crate::{
    core::digest::Digest,
    db::dependency::{InputKey, InputValue},
    frontend::runtime_macro_plan::PrimitiveFloatAdmission,
};
impl RunMachine {
    pub(super) fn retain_primitive_method(
        &mut self,
        key: &PcKey,
        method: PrimitiveFloatAdmission,
        recipe: Digest,
    ) -> Result<(), Diagnostic> {
        if let Some(old) = self.primitive_methods.get(key) {
            if old != &(method, recipe) {
                return Err(call_fault("primitive method retained admission differs"));
            }
            return Ok(());
        }
        self.primitive_methods.insert(key.clone(), (method, recipe));
        Ok(())
    }
    pub(super) fn revalidate_primitive_method(
        &self,
        db: &mut Database,
        contexts: &[super::super::runtime_macros::RuntimeMacroContext<'_>],
        id: super::super::install::RecipeId,
    ) -> Result<(), Diagnostic> {
        let key = self.frame.pc();
        let Some((admission, fingerprint)) = self.primitive_methods.get(key) else {
            return Ok(());
        };
        let recipe = self.installed.get(id)?;
        if recipe.recipe().fingerprint() != *fingerprint {
            return Err(call_fault("primitive method installed recipe differs"));
        }
        for context in contexts {
            let source = context.source();
            let Some(handle) = source
                .outline()
                .items
                .iter()
                .filter_map(|item| item.body.as_ref())
                .find(|handle| handle.key() == key.body)
            else {
                continue;
            };
            if !matches!(db.input(&InputKey::DefinitionSource(key.body.definition)).as_deref(), Some(InputValue::DefinitionSource(parse)) if parse == &source.selection().outline.parse)
                || !matches!(db.input(&InputKey::DefinitionBody(key.body.definition)).as_deref(), Some(InputValue::DefinitionBody(body)) if body == handle)
                || !matches!(db.input(&InputKey::CapturedSource(source.selection().location_identity())).as_deref(), Some(InputValue::CapturedSource(selected)) if selected == source.selection())
            {
                return Err(call_fault("primitive method source publication changed"));
            }
            let selected = context
                .resolve_at(
                    &self.frame.body,
                    key,
                    self.runtime_no_item_proofs.values(),
                    Some((&self.frame, &self.store)),
                )?
                .ok_or_else(|| call_fault("primitive method current source selection absent"))?;
            if selected.primitive_method.as_ref() != Some(admission)
                || selected.recipe.recipe().fingerprint() != *fingerprint
            {
                return Err(call_fault(
                    "primitive method current member/receiver differs",
                ));
            }
            return Ok(());
        }
        Err(call_fault("primitive method current source context absent"))
    }
}
