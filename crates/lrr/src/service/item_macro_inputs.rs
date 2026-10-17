//! Original-source invocation input preparation. No macro resolution or execution.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    expand::{attribute_materialize, attributes, member_cfg},
    syntax::{
        attributes::{DeriveItemInputs, ItemAttributeInputs},
        token_wire::{Key, SourceIdentity, TokenForest},
    },
};
#[derive(Debug)]
pub struct AttributeInvocationInput {
    pub(crate) source_selection: OriginalAttributeSelection,
    pub identity: Digest,
    pub item: TokenForest,
    pub arguments: Option<TokenForest>,
}
#[derive(Debug, Clone)]
pub(crate) struct OriginalAttributeSelection {
    pub parse: crate::syntax::parse::ParseKey,
    pub owner: Digest,
    pub cfg: Digest,
    pub item: crate::syntax::spans::SourceRange,
    pub meta: crate::syntax::spans::SourceRange,
    pub index: usize,
    pub identity: Digest,
}
#[derive(Debug, Clone)]
pub struct DeriveSelection {
    pub absolute: bool,
    pub segments: Vec<String>,
    pub first_span: Key,
    pub last_span: Key,
}
#[derive(Debug)]
pub struct DeriveInvocationInput {
    pub identity: Digest,
    pub item: TokenForest,
    pub selections: Vec<DeriveSelection>,
}
/// One immutable original-source occurrence. Constructing it executes no macro
/// and does not certify helper scope or discovery completion.
pub struct DeriveOccurrenceInput {
    input: DeriveInvocationInput,
    ordinal: usize,
    identity: Digest,
    retained_bytes: usize,
}
impl DeriveOccurrenceInput {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn item(&self) -> &TokenForest {
        &self.input.item
    }
    pub fn selected(&self) -> &DeriveSelection {
        &self.input.selections[self.ordinal]
    }
    pub fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}
pub struct ResolvedDeriveAnnotations {
    owner: Digest,
    parse: crate::syntax::parse::ParseKey,
    cfg: Digest,
    item: crate::syntax::spans::SourceRange,
    facts: Digest,
    identities: std::collections::BTreeMap<usize, Digest>,
}
impl ResolvedDeriveAnnotations {
    fn validate(
        &self,
        owner: Digest,
        inputs: &DeriveItemInputs,
        index: usize,
    ) -> Result<(), Diagnostic> {
        if self.owner != owner
            || self.parse != inputs.item.parse
            || self.cfg != inputs.item.cfg
            || self.item != inputs.item.item
            || self.facts != derive_facts_fingerprint(inputs)?
            || !self.identities.contains_key(&index)
        {
            return Err(fault(
                "derive annotation proof differs from selected source item",
            ));
        }
        Ok(())
    }
    /// Only namespace-selected compiler descriptors certify these source indexes.
    pub fn new(
        owner: Digest,
        inputs: &DeriveItemInputs,
        resolved: impl IntoIterator<
            Item = (
                usize,
                crate::expand::builtin_attributes::BuiltinAttributeDescriptor,
            ),
        >,
    ) -> Result<Self, Diagnostic> {
        let mut identities = std::collections::BTreeMap::new();
        for (index, descriptor) in resolved {
            if index >= inputs.item.attributes.len()
                || descriptor.action()
                    != crate::expand::builtin_attributes::BuiltinAttributeAction::Derive
                || identities
                    .insert(index, descriptor.definition_identity())
                    .is_some()
            {
                return Err(fault("invalid resolved derive annotation proof"));
            }
        }
        Ok(Self {
            owner,
            parse: inputs.item.parse.clone(),
            cfg: inputs.item.cfg,
            item: inputs.item.item,
            facts: derive_facts_fingerprint(inputs)?,
            identities,
        })
    }
}
/// Bind the complete preprocessing facts, not only their source/cfg labels.
pub(crate) fn derive_facts_fingerprint(inputs: &DeriveItemInputs) -> Result<Digest, Diagnostic> {
    let mut bytes = 0usize;
    let mut charge = |amount: usize| -> Result<(), Diagnostic> {
        bytes = bytes
            .checked_add(amount)
            .ok_or_else(|| fault("derive fact metadata limit"))?;
        Ok(())
    };
    let mut e = StableEncoder::new("lrr/v2/complete-derive-input-facts");
    e.u64(u64::try_from(inputs.members.len()).map_err(|_| fault("derive member count overflow"))?);
    for item in std::iter::once(&inputs.item).chain(&inputs.members) {
        charge(std::mem::size_of::<ItemAttributeInputs>())?;
        e.record(&item.parse);
        e.digest(item.cfg);
        e.u32(item.item.start);
        e.u32(item.item.end);
        e.u8(u8::from(item.enabled));
        e.u8(u8::from(item.requires_cfg_materialization));
        charge(
            item.original_attribute_ranges
                .len()
                .checked_mul(std::mem::size_of::<crate::syntax::spans::SourceRange>())
                .ok_or_else(|| fault("derive fact size overflow"))?,
        )?;
        e.sequence(&item.original_attribute_ranges, |e, r| {
            e.u32(r.start);
            e.u32(r.end);
        });
        e.u64(u64::try_from(item.attributes.len()).map_err(|_| fault("attribute count overflow"))?);
        for attribute in &item.attributes {
            charge(std::mem::size_of::<crate::syntax::attributes::AttributeInput>())?;
            charge(attribute.name.len())?;
            e.string(&attribute.name);
            e.u32(attribute.range.start);
            e.u32(attribute.range.end);
            e.u32(attribute.enclosing_attribute_range.start);
            e.u32(attribute.enclosing_attribute_range.end);
            e.option(&attribute.original_attribute_range, |e, r| {
                e.u32(r.start);
                e.u32(r.end);
            });
            e.option(&attribute.argument_range, |e, r| {
                e.u32(r.start);
                e.u32(r.end);
            });
            charge(
                attribute
                    .tokens
                    .len()
                    .checked_mul(std::mem::size_of::<crate::syntax::ast_adapter::TokenRecipe>())
                    .ok_or_else(|| fault("derive fact token size overflow"))?,
            )?;
            for token in &attribute.tokens {
                charge(token.text.len())?;
            }
            e.sequence(&attribute.tokens, |e, t| e.record(t));
        }
    }
    Ok(e.finish())
}
impl DeriveInvocationInput {
    /// Each source occurrence has an identity, including repeated paths.
    pub fn selection_identity(&self, index: usize) -> Result<Digest, Diagnostic> {
        let selection = self
            .selections
            .get(index)
            .ok_or_else(|| fault("derive selection index out of range"))?;
        let mut encoder = StableEncoder::new("lrr/v2/original-derive-selection");
        encoder.digest(self.identity);
        encoder.u64(u64::try_from(index).map_err(|_| fault("derive index overflow"))?);
        encoder.u8(u8::from(selection.absolute));
        encoder.sequence(&selection.segments, |e, segment| e.string(segment));
        encoder.bytes(&selection.first_span.0);
        encoder.bytes(&selection.last_span.0);
        Ok(encoder.finish())
    }
}
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-OUTPUT", message.into())
}
fn identity(
    owner: Digest,
    inputs: &ItemAttributeInputs,
    index: usize,
    kind: u8,
    item: &TokenForest,
    arguments: Option<&TokenForest>,
) -> Result<Digest, Diagnostic> {
    let attr = inputs
        .attributes
        .get(index)
        .ok_or_else(|| fault("original macro attribute index out of range"))?;
    let mut e = StableEncoder::new("lrr/v2/original-item-macro-input");
    e.digest(owner);
    e.record(&inputs.parse);
    e.digest(inputs.cfg);
    e.u32(inputs.item.start);
    e.u32(inputs.item.end);
    e.u64(u64::try_from(index).map_err(|_| fault("macro attribute index overflow"))?);
    e.u8(kind);
    e.sequence(&attr.tokens, |e, t| e.record(t));
    e.bytes(&item.encode().map_err(|error| fault(error.to_string()))?);
    if let Some(arguments) = arguments {
        e.u8(1);
        e.bytes(
            &arguments
                .encode()
                .map_err(|error| fault(error.to_string()))?,
        );
    } else {
        e.u8(0);
    }

    Ok(e.finish())
}
/// Called only after the resolver selects this exact attribute as a macro.
pub fn attribute(
    owner: Digest,
    inputs: &ItemAttributeInputs,
    index: usize,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<AttributeInvocationInput, Diagnostic> {
    let item = attribute_materialize::materialize(inputs, index, source, forest)?;
    attribute_with_item(owner, inputs, index, item, source, forest)
}
pub(crate) fn attribute_after_compiler_prefix(
    owner: Digest,
    inputs: &ItemAttributeInputs,
    index: usize,
    consumed: &[usize],
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<AttributeInvocationInput, Diagnostic> {
    let item =
        attribute_materialize::after_compiler_prefix(inputs, index, consumed, source, forest)?;
    attribute_with_item(owner, inputs, index, item, source, forest)
}
pub(crate) fn attribute_after_derives(
    owner: Digest,
    inputs: &DeriveItemInputs,
    index: usize,
    proof: &ResolvedDeriveAnnotations,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<AttributeInvocationInput, Diagnostic> {
    // Bind every preprocessing fact as for derive occurrence selection; the
    // selected attribute itself must not be certified as a derive.
    let first = *proof
        .identities
        .keys()
        .next()
        .ok_or_else(|| fault("preceding derive proof absent"))?;
    proof.validate(owner, inputs, first)?;
    let derives: Vec<_> = proof.identities.keys().copied().collect();
    let item = attribute_materialize::after_derives(&inputs.item, index, &derives, source, forest)?;
    attribute_with_item(owner, &inputs.item, index, item, source, forest)
}
fn attribute_with_item(
    owner: Digest,
    inputs: &ItemAttributeInputs,
    index: usize,
    item: TokenForest,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<AttributeInvocationInput, Diagnostic> {
    let arguments = attributes::argument_forest(inputs, index, source, forest)?
        .map(|tokens| attributes::macro_argument_contents(&tokens))
        .transpose()?;
    let identity = identity(owner, inputs, index, 0, &item, arguments.as_ref())?;
    Ok(AttributeInvocationInput {
        source_selection: OriginalAttributeSelection {
            parse: inputs.parse.clone(),
            owner,
            cfg: inputs.cfg,
            item: inputs.item,
            meta: inputs.attributes[index].range,
            index,
            identity,
        },
        identity,
        item,
        arguments,
    })
}
/// The caller certifies the selected annotation is the builtin derive by identity.
/// Paths remain unresolved source facts, in lexical order with original span keys.
pub fn derive(
    owner: Digest,
    inputs: &DeriveItemInputs,
    index: usize,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<DeriveInvocationInput, Diagnostic> {
    if inputs
        .item
        .attributes
        .get(index)
        .is_none_or(|a| a.name != "derive")
    {
        return Err(fault("selected annotation is not builtin derive syntax"));
    }
    let indices: Vec<_> = inputs
        .item
        .attributes
        .iter()
        .enumerate()
        .filter_map(|(i, a)| (a.name == "derive").then_some(i))
        .collect();
    prepare_derive(owner, inputs, index, &indices, source, forest)
}
pub fn derive_resolved(
    owner: Digest,
    inputs: &DeriveItemInputs,
    index: usize,
    annotations: &ResolvedDeriveAnnotations,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<DeriveInvocationInput, Diagnostic> {
    annotations.validate(owner, inputs, index)?;
    let indices: Vec<_> = annotations.identities.keys().copied().collect();
    let mut prepared = prepare_derive(owner, inputs, index, &indices, source, forest)?;
    let mut e = StableEncoder::new("lrr/v2/resolved-derive-input");
    e.digest(prepared.identity);
    e.digest(annotations.facts);
    e.u64(
        u64::try_from(annotations.identities.len())
            .map_err(|_| fault("derive annotation count overflow"))?,
    );
    for (index, identity) in &annotations.identities {
        e.u64(u64::try_from(*index).map_err(|_| fault("derive index overflow"))?);
        e.bytes(&identity.0);
    }
    prepared.identity = e.finish();
    Ok(prepared)
}
/// Remove certified groups through the selected annotation, retaining later
/// groups/attributes. Paths in the same group share a payload but have distinct
/// occurrence identities. The complete proof is replayed before materialization.
pub fn derive_occurrence_resolved(
    owner: Digest,
    inputs: &DeriveItemInputs,
    occurrence: (usize, usize),
    annotations: &ResolvedDeriveAnnotations,
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<DeriveOccurrenceInput, Diagnostic> {
    let (index, ordinal) = occurrence;
    annotations.validate(owner, inputs, index)?;
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(annotations.identities.len())
        .map_err(|_| fault("derive prefix allocation"))?;
    indices.extend(annotations.identities.range(..=index).map(|(at, _)| *at));
    let mut input = prepare_derive(owner, inputs, index, &indices, source, forest)?;
    if ordinal >= input.selections.len() {
        return Err(fault("derive occurrence index out of range"));
    }
    let mut encoder = StableEncoder::new("lrr/v2/resolved-derive-occurrence-input");
    encoder.digest(input.identity);
    encoder.digest(annotations.facts);
    encoder.u64(
        u64::try_from(annotations.identities.len())
            .map_err(|_| fault("derive annotation count overflow"))?,
    );
    for (at, definition) in &annotations.identities {
        encoder.u64(u64::try_from(*at).map_err(|_| fault("derive index overflow"))?);
        encoder.digest(*definition);
    }
    input.identity = encoder.finish();
    let identity = input.selection_identity(ordinal)?;
    let mut retained_bytes = input
        .item
        .allocated_bytes()
        .map_err(|e| fault(e.to_string()))?
        .checked_add(std::mem::size_of::<DeriveOccurrenceInput>())
        .and_then(|n| {
            input
                .selections
                .capacity()
                .checked_mul(std::mem::size_of::<DeriveSelection>())
                .and_then(|size| n.checked_add(size))
        })
        .ok_or_else(|| fault("derive occurrence retention overflow"))?;
    for path in &input.selections {
        retained_bytes = retained_bytes
            .checked_add(
                path.segments
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(|| fault("derive segment size overflow"))?,
            )
            .ok_or_else(|| fault("derive occurrence retention overflow"))?;
        for segment in &path.segments {
            retained_bytes = retained_bytes
                .checked_add(segment.capacity())
                .ok_or_else(|| fault("derive occurrence retention overflow"))?;
        }
    }
    Ok(DeriveOccurrenceInput {
        input,
        ordinal,
        identity,
        retained_bytes,
    })
}
fn prepare_derive(
    owner: Digest,
    inputs: &DeriveItemInputs,
    index: usize,
    indices: &[usize],
    source: &SourceIdentity,
    forest: &TokenForest,
) -> Result<DeriveInvocationInput, Diagnostic> {
    let args = attributes::argument_forest(&inputs.item, index, source, forest)?
        .ok_or_else(|| fault("derive annotation arguments absent"))?;
    let adapter = lrr_expand_adapter::token_wire::TokenForest::decode(
        &args.encode().map_err(|e| fault(e.to_string()))?,
    )
    .map_err(|e| fault(e.to_string()))?;
    let paths = lrr_expand_adapter::derive_paths::parse(&adapter)
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
    let mut selections = Vec::new();
    selections
        .try_reserve_exact(paths.len())
        .map_err(|_| fault("derive selection allocation"))?;
    selections.extend(paths.into_iter().map(|p| DeriveSelection {
        absolute: p.absolute,
        segments: p.segments,
        first_span: Key(p.first_span.0),
        last_span: Key(p.last_span.0),
    }));
    let item = member_cfg::derive_input_resolved(inputs, indices, source, forest)?;
    Ok(DeriveInvocationInput {
        identity: identity(owner, &inputs.item, index, 1, &item, Some(&args))?,
        item,
        selections,
    })
}
