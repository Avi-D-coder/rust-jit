//! Pure generated derive discovery facts. No macro execution or completeness grant.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        builtin_attributes::{BuiltinAttributeAction, BuiltinAttributeDescriptor},
        generated_items::GeneratedItems,
    },
    source::snapshot::CfgSet,
    syntax::{
        attributes::DeriveItemInputs,
        parse::ParsedFile,
        spans::SourceRange,
        token_wire::{Key, TokenForest},
    },
};
use std::collections::BTreeMap;

fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
fn binding(items: &GeneratedItems, index: usize, cfg: &CfgSet) -> Result<Digest, Diagnostic> {
    let header = items
        .items()
        .get(index)
        .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated derive header absent"))?;
    let mut e = StableEncoder::new("lrr/v2/generated-derive-annotation-owner");
    e.record(&header.key);
    e.record(items.record().key());
    e.digest(Digest::of_bytes(items.record().wire()));
    e.u64(items.epoch());
    e.digest(cfg.fingerprint());
    Ok(e.finish())
}
fn facts(
    items: &GeneratedItems,
    index: usize,
    cfg: &CfgSet,
) -> Result<DeriveItemInputs, Diagnostic> {
    let header = items
        .items()
        .get(index)
        .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "generated derive header absent"))?;
    let parsed = ParsedFile::from_generated_items(items)?;
    let facts = parsed.derive_item_inputs_at(
        SourceRange {
            start: header.start,
            end: header.end,
        },
        cfg,
    )?;
    if !facts.item.enabled {
        return Err(fault(
            "LRR-E-EXPANSION-INPUT",
            "generated derive item is inactive or attribute count exceeds limit",
        ));
    }
    Ok(facts)
}
pub(crate) fn annotation_path(
    items: &GeneratedItems,
    index: usize,
    selected: SourceRange,
) -> Result<TokenForest, Diagnostic> {
    let header = items.items().get(index).ok_or_else(|| {
        fault(
            "LRR-E-EXPANSION-INPUT",
            "generated annotation path header absent",
        )
    })?;
    let input = lrr_expand_adapter::token_wire::TokenForest::decode(items.record().wire())
        .map_err(|e| fault(e.code, &e.message))?;
    let path = lrr_expand_adapter::generated_attribute::path_forest(
        &input,
        header.start,
        header.end,
        selected.start,
        selected.end,
    )
    .map_err(|e| fault(e.code, &e.message))?;
    TokenForest::decode(&path.encode().map_err(|e| fault(e.code, &e.message))?)
        .map_err(|e| fault(e.code, &e.message))
}
pub struct GeneratedDeriveAnnotations {
    binding: Digest,
    identities: BTreeMap<usize, Digest>,
}
impl GeneratedDeriveAnnotations {
    /// The source namespace resolver supplies exact compiler-owned descriptors.
    /// A spelling, helper name or procedural export is not a derive-annotation proof.
    pub fn new(
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        resolved: impl IntoIterator<Item = (usize, BuiltinAttributeDescriptor)>,
    ) -> Result<Self, Diagnostic> {
        let facts = facts(items, index, cfg)?;
        let mut identities = BTreeMap::new();
        for (at, descriptor) in resolved {
            if at >= facts.item.attributes.len() || descriptor.action() != BuiltinAttributeAction::Derive || identities
                    .insert(at, descriptor.definition_identity())
                    .is_some() {
                return Err(fault(
                    "LRR-E-EXPANSION-INPUT",
                    "invalid generated derive annotation proof",
                ));
            }
        }
        Ok(Self {
            binding: binding(items, index, cfg)?,
            identities,
        })
    }
}
#[derive(Debug)]
pub struct GeneratedDerivePath {
    pub identity: Digest,
    pub absolute: bool,
    pub segments: Vec<String>,
    pub first_span: Key,
    pub last_span: Key,
    tokens: TokenForest,
}
impl GeneratedDerivePath {
    pub fn tokens(&self) -> &TokenForest {
        &self.tokens
    }
}
pub struct GeneratedDeriveAnnotation {
    index: usize,
    definition: Digest,
    arguments: TokenForest,
    paths: Vec<GeneratedDerivePath>,
}
impl GeneratedDeriveAnnotation {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn definition_identity(&self) -> Digest {
        self.definition
    }
    pub fn arguments(&self) -> &TokenForest {
        &self.arguments
    }
    pub fn paths(&self) -> &[GeneratedDerivePath] {
        &self.paths
    }
}
pub struct GeneratedDerivePlan {
    binding: Digest,
    item: Digest,
    facts: DeriveItemInputs,
    annotations: Vec<GeneratedDeriveAnnotation>,
    unresolved_attributes: Vec<usize>,
    retained_bytes: usize,
}
impl GeneratedDerivePlan {
    pub fn prepare(
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        proof: &GeneratedDeriveAnnotations,
    ) -> Result<Self, Diagnostic> {
        let owner = binding(items, index, cfg)?;
        if proof.binding != owner {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated derive proof differs from output/header/cfg/epoch",
            ));
        }
        let facts = facts(items, index, cfg)?;
        let adapter = lrr_expand_adapter::token_wire::TokenForest::decode(items.record().wire())
            .map_err(|e| fault(e.code, &e.message))?;
        let mut annotations = Vec::new();
        annotations
            .try_reserve_exact(proof.identities.len())
            .map_err(|_| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated derive annotation allocation",
                )
            })?;
        let mut bytes =
            facts
                .members
                .iter()
                .try_fold(facts.item.allocated_bytes()?, |n, member| {
                    n.checked_add(member.allocated_bytes()?).ok_or_else(|| {
                        fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "generated derive fact retention overflow",
                        )
                    })
                })?;
        bytes = bytes
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| {
                proof
                    .identities
                    .len()
                    .checked_mul(std::mem::size_of::<GeneratedDeriveAnnotation>())
                    .and_then(|size| n.checked_add(size))
            })
            .and_then(|n| {
                facts
                    .item
                    .attributes
                    .len()
                    .checked_mul(std::mem::size_of::<usize>())
                    .and_then(|size| n.checked_add(size))
            })
            .and_then(|n| {
                facts
                    .members
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        crate::syntax::attributes::ItemAttributeInputs,
                    >())
                    .and_then(|size| n.checked_add(size))
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated derive plan retention limit",
                )
            })?;
        let mut path_count = 0usize;
        for (&at, &definition) in &proof.identities {
            let attribute = &facts.item.attributes[at];
            let arguments = lrr_expand_adapter::generated_attribute::argument_forest(
                &adapter,
                facts.item.item.start,
                facts.item.item.end,
                attribute.range.start,
                attribute.range.end,
            )
            .map_err(|e| fault(e.code, &e.message))?;
            let selected = lrr_expand_adapter::derive_paths::parse(&arguments)
                .map_err(|e| fault(e.code, &e.message))?;
            path_count = path_count
                .checked_add(selected.len())
                .ok_or_else(|| {
                    fault("LRR-E-DISCOVERY-LIMIT", "generated derive path count limit")
                })?;
            bytes = bytes
                .checked_add(
                    arguments
                        .allocated_bytes()
                        .map_err(|e| fault(e.code, &e.message))?,
                )
                .ok_or_else(|| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated derive argument retention limit",
                    )
                })?;
            let digest =
                Digest::of_bytes(&arguments.encode().map_err(|e| fault(e.code, &e.message))?);
            bytes = selected
                .len()
                .checked_mul(std::mem::size_of::<GeneratedDerivePath>())
                .and_then(|size| bytes.checked_add(size))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated derive path storage limit",
                    )
                })?;
            let mut paths = Vec::new();
            paths
                .try_reserve_exact(selected.len())
                .map_err(|_| fault("LRR-E-DISCOVERY-LIMIT", "generated derive path allocation"))?;
            for (position, path) in selected.into_iter().enumerate() {
                bytes = bytes
                    .checked_add(
                        path.tokens
                            .allocated_bytes()
                            .map_err(|e| fault(e.code, &e.message))?,
                    )
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "generated derive path token retention limit",
                        )
                    })?;
                let tokens = TokenForest::decode(
                    &path
                        .tokens
                        .encode()
                        .map_err(|e| fault(e.code, &e.message))?,
                )
                .map_err(|e| fault(e.code, &e.message))?;
                bytes = path
                    .segments
                    .iter()
                    .try_fold(bytes, |n, s| {
                        n.checked_add(s.capacity()).and_then(|n| n.checked_add(64))
                    })
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "generated derive path retention limit",
                        )
                    })?;
                let mut e = StableEncoder::new("lrr/v2/generated-derive-occurrence");
                e.digest(owner);
                e.digest(definition);
                e.digest(digest);
                e.u64(u64::try_from(at).map_err(|_| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated derive annotation index overflow",
                    )
                })?);
                e.u64(u64::try_from(position).map_err(|_| {
                    fault(
                        "LRR-E-DISCOVERY-LIMIT",
                        "generated derive path index overflow",
                    )
                })?);
                paths.push(GeneratedDerivePath {
                    identity: e.finish(),
                    absolute: path.absolute,
                    segments: path.segments,
                    first_span: Key(path.first_span.0),
                    last_span: Key(path.last_span.0),
                    tokens,
                });
            }
            let arguments =
                TokenForest::decode(&arguments.encode().map_err(|e| fault(e.code, &e.message))?)
                    .map_err(|e| fault(e.code, &e.message))?;
            annotations.push(GeneratedDeriveAnnotation {
                index: at,
                definition,
                arguments,
                paths,
            });
        }
        let mut unresolved_attributes = Vec::new();
        unresolved_attributes
            .try_reserve_exact(facts.item.attributes.len())
            .map_err(|_| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated unresolved attribute allocation",
                )
            })?;
        for (at, attribute) in facts.item.attributes.iter().enumerate() {
            if !proof.identities.contains_key(&at)
                && attribute.name != "cfg"
                && super::builtin_attributes::retained_descriptor(&attribute.name)?.is_none()
            {
                unresolved_attributes.push(at);
            }
        }
        Ok(Self {
            binding: owner,
            item: items.items()[index].key.fingerprint(),
            facts,
            annotations,
            unresolved_attributes,
            retained_bytes: bytes,
        })
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn binding_digest(&self) -> Digest {
        self.binding
    }
    pub fn item_identity(&self) -> Digest {
        self.item
    }
    /// Pure token materialization. Unresolved attribute/helper demands still block
    /// execution and publication; these tokens are not an execution certificate.
    pub fn materialize_input(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
    ) -> Result<TokenForest, Diagnostic> {
        self.materialize_through(items, index, cfg, None)
    }
    pub(crate) fn annotation_index(&self, row: usize) -> Result<usize, Diagnostic> {
        self.annotations
            .iter()
            .flat_map(|a| a.paths.iter().map(move |_| a.index))
            .nth(row)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "derive occurrence absent"))
    }
    /// All occurrences in one annotation consume that annotation together;
    /// annotations later in source remain in this derive's input.
    pub fn materialize_occurrence_input(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        row: usize,
    ) -> Result<TokenForest, Diagnostic> {
        self.materialize_through(items, index, cfg, Some(self.annotation_index(row)?))
    }
    fn materialize_through(
        &self,
        items: &GeneratedItems,
        index: usize,
        cfg: &CfgSet,
        through: Option<usize>,
    ) -> Result<TokenForest, Diagnostic> {
        if binding(items, index, cfg)? != self.binding {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated derive materialization differs from selected output/header/cfg/epoch",
            ));
        }
        let header = &items.items()[index];
        let facts = ParsedFile::from_generated_items(items)?.derive_cfg_inputs_at(
            SourceRange {
                start: header.start,
                end: header.end,
            },
            cfg,
        )?;
        if facts.item != self.facts.item {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated derive materialization facts changed",
            ));
        }
        let ranges = |attributes: &[crate::syntax::attributes::AttributeInput]| -> Result<
            Vec<lrr_expand_adapter::generated_attribute::AttributeRange>,
            Diagnostic,
        > {
            let mut ranges = Vec::new();
            ranges.try_reserve_exact(attributes.len()).map_err(|_| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated derive attribute range allocation",
                )
            })?;
            ranges.extend(attributes.iter().map(|a| {
                lrr_expand_adapter::generated_attribute::AttributeRange {
                    start: a.range.start,
                    end: a.range.end,
                    wrapper_start: a.enclosing_attribute_range.start,
                    wrapper_end: a.enclosing_attribute_range.end,
                }
            }));
            Ok(ranges)
        };
        let outer = ranges(&facts.item.attributes)?;
        let removed = facts
            .item
            .attributes
            .iter()
            .enumerate()
            .filter_map(|(at, a)| {
                (a.name == "cfg"
                    || self
                        .annotations
                        .iter()
                        .any(|a| a.index == at && through.is_none_or(|end| at <= end)))
                .then_some(at)
            })
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(facts.descendants.len())
            .map_err(|_| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated derive cfg range allocation",
                )
            })?;
        for node in &facts.descendants {
            nodes.push(lrr_expand_adapter::generated_derive::CfgNode {
                range: lrr_expand_adapter::generated_derive::Range {
                    start: node.item.start,
                    end: node.item.end,
                },
                enabled: node.enabled,
                attributes: ranges(&node.attributes)?,
            });
        }
        let input = lrr_expand_adapter::token_wire::TokenForest::decode(items.record().wire())
            .map_err(|e| fault(e.code, &e.message))?;
        let output = lrr_expand_adapter::generated_derive::materialize(
            &input,
            lrr_expand_adapter::generated_derive::Range {
                start: header.start,
                end: header.end,
            },
            &outer,
            &removed,
            &nodes,
        )
        .map_err(|e| fault(e.code, &e.message))?;
        TokenForest::decode(&output.encode().map_err(|e| fault(e.code, &e.message))?)
            .map_err(|e| fault(e.code, &e.message))
    }
    pub fn facts(&self) -> &DeriveItemInputs {
        &self.facts
    }
    pub fn annotations(&self) -> &[GeneratedDeriveAnnotation] {
        &self.annotations
    }
    /// These may be replacing attributes or derive helpers; classification is pending.
    pub fn unresolved_attributes(&self) -> &[usize] {
        &self.unresolved_attributes
    }
}

#[cfg(test)]
mod tests;
