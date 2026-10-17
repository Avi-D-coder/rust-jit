//! Retained original-item annotation attachment facts. Namespace decisions still
//! belong to the ordinary compiler/procedural selection services.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::changes::{PreparedSourceView, SourceSelection},
    syntax::{
        declaration_scan::{DeclarationProducerKind, DeclarationScan},
        imports::ImportSegment,
    },
};
use std::{collections::BTreeMap, sync::Arc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-ORIGINAL-ANNOTATION", message.into())
}
pub(crate) struct OriginalAttributeAttachment {
    pub(crate) scan_row: usize,
    pub(crate) segments: Vec<ImportSegment>,
    pub(crate) absolute: bool,
}
struct Item {
    ordinal: usize,
    identity: Digest,
    rows: Vec<usize>,
}
pub(crate) struct OriginalAnnotationPlan {
    source: SourceSelection,
    scan: Arc<DeclarationScan>,
    scan_identity: Digest,
    attributes: Vec<OriginalAttributeAttachment>,
    items: BTreeMap<Digest, Item>,
    bytes: usize,
}
impl OriginalAnnotationPlan {
    pub(crate) fn capture(
        source: &PreparedSourceView,
        scan: Arc<DeclarationScan>,
    ) -> Result<Self, Diagnostic> {
        source.require_original_hygiene()?;
        if scan.parse() != &source.selection().outline.parse
            || scan.cfg() != source.selection().outline.cfg
            || scan.path() != &source.selection().path
            || scan.scope() != &source.outline().root_scope
        {
            return Err(fault("original annotation scan/source differs"));
        }
        let mut attributes = Vec::new();
        let mut ranges = BTreeMap::<(u32, u32), usize>::new();
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(source.selection().metadata_bytes()?)
            .and_then(|n| n.checked_add(scan.retained_bytes()))
            .ok_or_else(|| fault("original annotation accounting overflow"))?;
        for (scan_row, row) in scan.producers().iter().enumerate() {
            if row.kind != DeclarationProducerKind::Attribute {
                continue;
            }
            bytes = bytes
                .checked_add(128)
                .ok_or_else(|| fault("original annotation attachment budget"))?;
            if ranges
                .insert((row.range.start, row.range.end), scan_row)
                .is_some()
            {
                return Err(fault("original annotation range is ambiguous"));
            }
        }
        let mut items = BTreeMap::new();
        for (item_ordinal, item) in source.outline().items.iter().enumerate() {
            let Some(call) = crate::syntax::outline::potential_attribute_producer(item) else {
                continue;
            };
            if item.scope.owner.is_some() {
                continue;
            }
            let mut rows = Vec::new();
            rows.try_reserve_exact(item.header.attributes.len())
                .map_err(|_| fault("original annotation item allocation"))?;
            for attribute in item
                .header
                .attributes
                .iter()
                .filter(|attribute| crate::syntax::outline::attribute_may_name_macro(attribute))
            {
                let selected = attribute
                    .source_meta
                    .ok_or_else(|| fault("original annotation lacks its Meta range"))?;
                let scan_row = *ranges
                    .get(&(selected.start, selected.end))
                    .ok_or_else(|| fault("original annotation Meta is absent from scan"))?;
                let row = &scan.producers()[scan_row];
                if row.scope != item.scope {
                    return Err(fault("original annotation item/scope differs"));
                }
                let (segments, absolute) = source.attribute_lookup_path_at(selected)?;
                bytes = bytes
                    .checked_add(128)
                    .and_then(|n| {
                        n.checked_add(
                            segments
                                .capacity()
                                .checked_mul(std::mem::size_of::<ImportSegment>())?,
                        )
                    })
                    .ok_or_else(|| fault("original annotation path budget"))?;
                for segment in &segments {
                    if let ImportSegment::Name(name) = segment {
                        bytes = bytes
                            .checked_add(name.capacity())
                            .ok_or_else(|| fault("original annotation name budget"))?;
                    }
                }
                attributes
                    .try_reserve(1)
                    .map_err(|_| fault("original annotation row allocation"))?;
                rows.push(attributes.len());
                attributes.push(OriginalAttributeAttachment {
                    scan_row,
                    segments,
                    absolute,
                });
            }
            bytes = bytes
                .checked_add(160)
                .and_then(|n| {
                    n.checked_add(rows.capacity().checked_mul(std::mem::size_of::<usize>())?)
                })
                .ok_or_else(|| fault("original annotation item budget"))?;
            if items
                .insert(
                    call.item_input,
                    Item {
                        ordinal: item_ordinal,
                        identity: item.def_key.fingerprint(),
                        rows,
                    },
                )
                .is_some()
            {
                return Err(fault("original annotation item producer is duplicated"));
            }
        }
        Ok(Self {
            source: source.selection().clone(),
            scan_identity: scan.fingerprint(),
            scan,
            attributes,
            items,
            bytes,
        })
    }
    pub(super) fn require_source(&self, source: &PreparedSourceView) -> Result<(), Diagnostic> {
        if source.selection() != &self.source {
            return Err(fault("original annotation plan source changed"));
        }
        Ok(())
    }
    pub(crate) fn rows<'a>(
        &'a self,
        source: &PreparedSourceView,
        producer: Digest,
        item: Digest,
    ) -> Result<Option<impl Iterator<Item = &'a OriginalAttributeAttachment>>, Diagnostic> {
        self.require_source(source)?;
        let Some(row) = self.items.get(&producer) else {
            return Ok(None);
        };
        if row.identity != item {
            return Err(fault("original annotation plan item changed"));
        }
        Ok(Some(row.rows.iter().map(|index| &self.attributes[*index])))
    }
    pub(super) fn items(&self) -> impl Iterator<Item = (Digest, usize)> + '_ {
        self.items
            .iter()
            .map(|(producer, item)| (*producer, item.ordinal))
    }
    pub(crate) fn item_ordinal(&self, producer: Digest) -> Result<usize, Diagnostic> {
        self.items
            .get(&producer)
            .map(|item| item.ordinal)
            .ok_or_else(|| fault("original annotation item ordinal is absent"))
    }
    pub(crate) fn scan(&self) -> &Arc<DeclarationScan> {
        &self.scan
    }
    pub(crate) fn scan_identity(&self) -> Digest {
        self.scan_identity
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
}
