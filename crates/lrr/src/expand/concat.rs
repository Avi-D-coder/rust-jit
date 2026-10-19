//! Owned literal concat result; nested eager expansion remains a separate demand.
use super::compile_input_identity::{CompileInputDescriptor, CompileInputKind};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    syntax::token_wire::{Key, TokenForest},
};
pub struct ConcatLiteralValue {
    value: String,
    input: Digest,
    origin: Key,
    descriptor: Digest,
    tokens: Digest,
}
impl ConcatLiteralValue {
    /// Certified compiler-facet emission of this already computed concat value.
    pub fn expansion_record(
        &self,
        catalog: &crate::frontend::compile_input_catalog::CompileInputCatalog,
        selected: &crate::expand::scopes::MacroDefinition,
        key: &crate::expand::identity::ExpansionKey,
        original: &TokenForest,
        max_wire_bytes: usize,
    ) -> Result<crate::expand::record::ExpansionRecord, Diagnostic> {
        let fault = |message: &str| Diagnostic::new("LRR-E-COMPILE-INPUT", message.into());
        key.validate()?;
        let descriptor = catalog
            .select(selected)?
            .ok_or_else(|| fault("concat output requires a compiler-facet definition"))?;
        let bytes = original.encode().map_err(|e| fault(&e.to_string()))?;
        if descriptor.kind() != CompileInputKind::Concat
            || descriptor.definition_identity() != self.descriptor
            || Digest::of_bytes(&bytes) != self.tokens
            || key.input_tokens != self.tokens
            || key.definition_edition != selected.definition_edition
            || key.macro_identity
                != (crate::expand::identity::MacroIdentity::Builtin {
                    definition: selected.identity,
                })
        {
            return Err(fault("concat output identity/input/grammar mismatch"));
        }
        let input = lrr_expand_adapter::token_wire::TokenForest::decode(&bytes)
            .map_err(|e| fault(&e.to_string()))?;
        let output = lrr_expand_adapter::string_output::emit(
            &input,
            lrr_expand_adapter::token_wire::Key(self.origin.0),
            &self.value,
            max_wire_bytes,
        )
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let wire = output.encode().map_err(|e| fault(&e.to_string()))?;
        crate::expand::record::ExpansionRecord::from_wire(key.clone(), wire)
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn fingerprint(&self) -> Digest {
        self.input
    }
    pub fn origin(&self) -> Key {
        self.origin
    }
}
pub fn literal_value(
    descriptor: &CompileInputDescriptor,
    input: &TokenForest,
    call_site: Key,
    max_input_bytes: usize,
    max_output_bytes: usize,
) -> Result<ConcatLiteralValue, Diagnostic> {
    let fault = |message: String| Diagnostic::new("LRR-E-COMPILE-INPUT", message);
    if descriptor.kind() != CompileInputKind::Concat {
        return Err(fault("selected builtin is not concat".into()));
    }
    let bytes = input.encode().map_err(|e| fault(e.to_string()))?;
    let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(&bytes)
        .map_err(|e| fault(e.to_string()))?;
    let value = lrr_expand_adapter::concat::literal_value(
        &tokens,
        lrr_expand_adapter::token_wire::Key(call_site.0),
        max_input_bytes,
        max_output_bytes,
    )
    .map_err(|e| Diagnostic::new(e.code, e.message))?;
    let mut e = StableEncoder::new("lrr/v2/literal-concat-input/numeric-v2/sites-v1");
    e.digest(descriptor.definition_identity());
    e.bytes(&bytes);
    e.bytes(&call_site.0);
    Ok(ConcatLiteralValue {
        value,
        input: e.finish(),
        origin: call_site,
        descriptor: descriptor.definition_identity(),
        tokens: Digest::of_bytes(&bytes),
    })
}
