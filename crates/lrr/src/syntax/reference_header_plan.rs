//! Immutable source-only reference header plan retained by the pure query memo.
use super::*;
use crate::core::digest::StableEncoder;
use std::sync::Arc;

fn metadata(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-METADATA-LIMIT", message.into())
}

/// Opaque source facts. This plan grants no Type namespace or caller authority.
#[derive(Debug)]
pub struct ReferenceHeaderPlan {
    definition: Digest,
    parse: ParseKey,
    signature: Digest,
    declaration: SourceRange,
    body: Option<SourceRange>,
    parameters: Box<[Option<Arc<ClosedReferenceSyntax>>]>,
    result: Option<Arc<ClosedReferenceTypeSyntax>>,
    input_references: u32,
    bytes: usize,
}
impl ReferenceHeaderPlan {
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub(crate) fn parse(&self) -> &ParseKey {
        &self.parse
    }
    pub(crate) fn parameter(&self, ordinal: usize) -> Option<&Arc<ClosedReferenceSyntax>> {
        self.parameters.get(ordinal).and_then(Option::as_ref)
    }
    pub(crate) fn result(&self) -> Option<&Arc<ClosedReferenceTypeSyntax>> {
        self.result.as_ref()
    }
    pub(crate) fn input_references(&self) -> u32 {
        self.input_references
    }
    pub fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn require(
        &self,
        signature: &FunctionSignature,
        parse: &ParseKey,
    ) -> Result<(), Diagnostic> {
        if self.definition != signature.definition
            || self.signature != signature.fingerprint()
            || self.parse != *parse
            || self.parse.profile != signature.header.profile
            || self.parameters.len() != signature.parameters.len()
        {
            return Err(fault("reference header plan source/signature differs"));
        }
        Ok(())
    }
}
fn encode_reference(e: &mut StableEncoder, reference: &ClosedReferenceTypeSyntax) {
    e.record(&reference.parse);
    e.digest(reference.signature);
    e.u32(match reference.role {
        ReferenceRole::Parameter => 0,
        ReferenceRole::Result => 1,
    });
    e.u32(reference.range.start);
    e.u32(reference.range.end);
    e.bool(reference.mutable);
    e.option(&reference.lifetime, |e, name| e.string(name));
    e.string(&reference.target);
    e.u32(reference.target_range.start);
    e.u32(reference.target_range.end);
}
impl StableEncode for ReferenceHeaderPlan {
    const DOMAIN: &'static str = "lrr/v1/reference-header-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.record(&self.parse);
        e.digest(self.signature);
        e.u32(self.declaration.start);
        e.u32(self.declaration.end);
        e.option(&self.body, |e, range| {
            e.u32(range.start);
            e.u32(range.end);
        });
        e.u32(self.input_references);
        e.sequence(&self.parameters, |e, parameter| {
            e.option(parameter, |e, parameter| {
                e.u32(parameter.ordinal);
                encode_reference(e, &parameter.reference);
            })
        });
        e.option(&self.result, |e, result| encode_reference(e, result));
    }
}
impl ParsedFile {
    pub(crate) fn reference_header_plan(
        &self,
        signature: &FunctionSignature,
        context: &DeclarationContext,
        body: Option<&BodyHandle>,
    ) -> Result<ReferenceHeaderPlan, Diagnostic> {
        let function = self.closed_reference_function(signature, context, body)?;
        let list = function
            .param_list()
            .ok_or_else(|| fault("closed reference function parameters absent"))?;
        if signature.parameters.len() > 100_000 {
            return Err(metadata("reference header parameter limit"));
        }
        let admitted_owner = context.scope.owner.is_none() && list.self_param().is_none();
        let mut parameters = Vec::new();
        parameters
            .try_reserve_exact(signature.parameters.len())
            .map_err(|_| metadata("reference header allocation"))?;
        let mut input_references = 0u32;
        let signature_fingerprint = signature.fingerprint();
        let mut bytes = std::mem::size_of::<ReferenceHeaderPlan>()
            .checked_add(self.key().profile.semantics.capacity())
            .and_then(|n| {
                signature
                    .parameters
                    .len()
                    .checked_mul(std::mem::size_of::<Option<Arc<ClosedReferenceSyntax>>>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(|| metadata("reference header metadata size"))?;
        let mut syntax = list.params();
        for (ordinal, parameter) in signature.parameters.iter().enumerate() {
            let ty = if parameter.receiver {
                None
            } else {
                syntax.next().and_then(|p| p.ty())
            };
            if matches!(ty, Some(ast::Type::RefType(_))) {
                input_references = input_references
                    .checked_add(1)
                    .ok_or_else(|| metadata("reference header count overflow"))?;
            }
            let ordinal = u32::try_from(ordinal)
                .map_err(|_| metadata("reference header ordinal overflow"))?;
            let captured = if admitted_owner && !parameter.receiver && !parameter.variadic {
                self.closed_reference_type(ty, signature_fingerprint, ReferenceRole::Parameter)?
                    .map(|reference| ClosedReferenceSyntax { reference, ordinal })
            } else {
                None
            };
            if let Some(captured) = &captured {
                bytes = bytes
                    .checked_add(
                        captured
                            .allocated_bytes()
                            .ok_or_else(|| metadata("reference header metadata size"))?,
                    )
                    .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
                    .ok_or_else(|| metadata("reference header metadata size"))?;
            }
            if bytes > 32 * 1024 * 1024 {
                return Err(metadata("reference header metadata limit"));
            }
            parameters.push(captured.map(Arc::new));
        }
        let result = if admitted_owner {
            self.closed_reference_type(
                function.ret_type().and_then(|ret| ret.ty()),
                signature_fingerprint,
                ReferenceRole::Result,
            )?
        } else {
            None
        };
        if let Some(result) = &result {
            bytes = bytes
                .checked_add(
                    result
                        .allocated_bytes()
                        .ok_or_else(|| metadata("reference header metadata size"))?,
                )
                .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
                .ok_or_else(|| metadata("reference header metadata size"))?;
        }
        if bytes > 32 * 1024 * 1024 {
            return Err(metadata("reference header metadata limit"));
        }
        Ok(ReferenceHeaderPlan {
            definition: signature.definition,
            parse: self.key(),
            signature: signature_fingerprint,
            declaration: context.range,
            body: body.map(|body| body.range),
            parameters: parameters.into_boxed_slice(),
            result: result.map(Arc::new),
            input_references,
            bytes,
        })
    }
}
