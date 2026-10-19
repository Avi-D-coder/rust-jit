//! Compiler-owned derive metadata shared by original and generated inputs.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::scopes::MacroDefinition,
    frontend::compile_input_catalog::CompileInputCatalog,
};
use lrr_expand_adapter::builtin_macros::{BuiltinDeriveKind, DeriveContainerFacts};

#[derive(Debug)]
pub(crate) struct BuiltinDeriveExport {
    pub kind: BuiltinDeriveKind,
    pub demand: Digest,
    pub receipt: Digest,
    pub helpers: &'static [&'static str],
}
impl BuiltinDeriveExport {
    pub(crate) fn select(
        compiler: &CompileInputCatalog,
        definition: &MacroDefinition,
        occurrence: Digest,
    ) -> Result<Option<Self>, Diagnostic> {
        let Some(kind) = compiler.builtin_derive(definition) else {
            return Ok(None);
        };
        let descriptor = lrr_expand_adapter::builtin_derives::descriptor(kind.name())
            .map_err(|e| Diagnostic::new(e.code, e.message))?
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "builtin derive descriptor absent".into(),
                )
            })?;
        let mut demand = StableEncoder::new("lrr/v2/source-builtin-derive-helper-demand");
        demand.digest(occurrence);
        demand.digest(definition.identity);
        demand.digest(definition.defining_unit);
        let demand = demand.finish();
        let mut receipt = StableEncoder::new("lrr/v2/builtin-derive-helper-receipt");
        receipt.digest(demand);
        receipt.bytes(&descriptor.definition_identity().0);
        receipt.sequence(descriptor.helpers(), |e, name| e.string(name));
        Ok(Some(Self {
            kind,
            demand,
            receipt: receipt.finish(),
            helpers: descriptor.helpers(),
        }))
    }
}

pub(crate) fn input_identity(
    input: &[u8],
    facts: DeriveContainerFacts,
) -> Result<Digest, Diagnostic> {
    let mut tokens = StableEncoder::new("lrr/v2/builtin-derive-input");
    tokens.bytes(input);
    tokens.u8(u8::from(facts.derives_copy));
    tokens.u8(u8::from(facts.derives_ord));
    Ok(tokens.finish())
}
