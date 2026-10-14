//! An actual adopted attribute output removes its original source item.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        identity::{MacroIdentity, ProceduralKind},
        record::{ExpansionDescriptor, ExpansionRecord},
    },
    source::changes::{PreparedSourceView, SourceSelection},
    syntax::{outline::ItemInvocation, spans::SourceRange},
};
#[derive(Clone, Debug)]
pub(crate) struct SourceReplacementProof {
    source: SourceSelection,
    range: SourceRange,
    producer: Digest,
    record: ExpansionDescriptor,
    cold_scan: Option<std::sync::Arc<crate::syntax::declaration_scan::DeclarationScan>>,
}
fn fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-INTERNAL-SNAPSHOT",
        "source replacement item/record attachment differs".into(),
    )
}
impl SourceReplacementProof {
    pub(crate) fn from_adopted(
        source: &PreparedSourceView,
        call: &ItemInvocation,
        record: &ExpansionRecord,
    ) -> Result<Self, Diagnostic> {
        if call.attribute_input.is_none()
            || call.item_input != record.key().invocation
            || !matches!(
                &record.key().macro_identity,
                MacroIdentity::Procedural {
                    kind: ProceduralKind::Attribute,
                    ..
                }
            )
            || !source
                .outline()
                .items
                .iter()
                .filter_map(crate::syntax::outline::potential_attribute_producer)
                .any(|actual| actual.item_input == call.item_input && actual.range == call.range)
        {
            return Err(fault());
        }
        Ok(Self {
            source: source.selection().clone(),
            range: call.range,
            producer: call.item_input,
            record: record.descriptor(),
            cold_scan: None,
        })
    }
    pub(crate) fn from_cold_adopted(
        source: &PreparedSourceView,
        scan: &std::sync::Arc<crate::syntax::declaration_scan::DeclarationScan>,
        producer: Digest,
        range: SourceRange,
        record: &ExpansionRecord,
    ) -> Result<Self, Diagnostic> {
        if scan.parse() != &source.selection().outline.parse
            || scan.path() != &source.selection().path
            || scan.cfg() != source.selection().outline.cfg
            || record.key().invocation != producer
            || !matches!(
                record.key().macro_identity,
                MacroIdentity::Procedural {
                    kind: ProceduralKind::Attribute,
                    ..
                }
            )
            || !scan.producers().iter().any(|row| {
                row.identity == producer
                    && row.kind
                        == crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                    && range.contains(row.range)
            })
        {
            return Err(fault());
        }
        let row = scan
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(fault)?;
        let parsed = crate::syntax::parse::ParsedFile::parse(
            source.captured_macro_blob()?,
            source.outline().profile.clone(),
        )?;
        if parsed.attribute_item_range_at(row.range)? != Some(range) {
            return Err(fault());
        }
        Ok(Self {
            source: source.selection().clone(),
            range,
            producer,
            record: record.descriptor(),
            cold_scan: Some(scan.clone()),
        })
    }
    pub(crate) fn source(&self) -> &SourceSelection {
        &self.source
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn attachment(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/source-replacement-attachment");
        e.string(self.source.path.as_str());
        e.record(&crate::db::key::QueryKey::Outline(
            self.source.outline.clone(),
        ));
        e.finish()
    }
    pub(crate) fn descriptor(&self) -> &ExpansionDescriptor {
        &self.record
    }
    pub(crate) fn matches_scan(
        &self,
        scan: &crate::syntax::declaration_scan::DeclarationScan,
    ) -> bool {
        scan.parse() == &self.source.outline.parse
            && scan.path() == &self.source.path
            && scan.scope().crate_key == self.source.outline.crate_key
            && scan.scope().module_path == self.source.outline.module_path
            && scan.cfg() == self.source.outline.cfg
    }
    pub(crate) fn covers(&self, range: SourceRange) -> bool {
        self.range.start <= range.start && range.end <= self.range.end
    }
    pub(crate) fn validate_outline(
        &self,
        outline: &crate::syntax::outline::Outline,
    ) -> Result<(), Diagnostic> {
        if let Some(scan) = &self.cold_scan {
            if outline.content != scan.parse().content
                || outline.profile != scan.parse().profile
                || outline.cfg != scan.cfg()
                || outline.root_scope != *scan.scope()
                || !self.matches_scan(scan)
                || !matches!(
                    self.record.key.macro_identity,
                    MacroIdentity::Procedural {
                        kind: ProceduralKind::Attribute,
                        ..
                    }
                )
                || self.record.key.invocation != self.producer
                || !scan
                    .producers()
                    .iter()
                    .any(|row| row.identity == self.producer && self.range.contains(row.range))
            {
                return Err(fault());
            }
            return Ok(());
        }
        if !matches!(
            &self.record.key.macro_identity,
            MacroIdentity::Procedural {
                kind: ProceduralKind::Attribute,
                ..
            }
        ) || self.record.key.invocation != self.producer
            || !outline
                .items
                .iter()
                .filter_map(crate::syntax::outline::potential_attribute_producer)
                .any(|call| call.item_input == self.producer && call.range == self.range)
        {
            return Err(fault());
        }
        Ok(())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.source
            .metadata_bytes()?
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|n| n.checked_add(crate::expand::record::key_heap_bytes(&self.record.key)))
            .and_then(|n| {
                n.checked_add(
                    self.cold_scan
                        .as_ref()
                        .map_or(0, |scan| scan.retained_bytes()),
                )
            })
            .ok_or_else(fault)
    }
}
impl StableEncode for SourceReplacementProof {
    const DOMAIN: &'static str = "lrr/v2/source-replacement-proof";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.string(self.source.path.as_str());
        e.record(&crate::db::key::QueryKey::Outline(
            self.source.outline.clone(),
        ));
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.digest(self.producer);
        e.record(&self.record);
        e.option(&self.cold_scan, |e, scan| e.digest(scan.fingerprint()));
    }
}
