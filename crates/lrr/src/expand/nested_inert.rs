//! Source-attached proof for a pinned compiler inert attribute in cold syntax.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    syntax::declaration_scan::{DeclarationProducerKind, DeclarationScan},
};
use std::sync::Arc;
#[derive(Clone, Debug)]
pub(crate) struct NestedInertProof {
    scan: Arc<DeclarationScan>,
    scan_identity: Digest,
    row: usize,
    producer: Digest,
    definition: Digest,
    attribute_facet: Option<Digest>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-SNAPSHOT", message.into())
}
pub(crate) fn selected_definition(
    scan: &DeclarationScan,
    producer: Digest,
) -> Result<Option<Digest>, Diagnostic> {
    let row = scan
        .producers()
        .iter()
        .find(|row| row.identity == producer)
        .ok_or_else(|| fault("inert attribute producer absent"))?;
    if row.kind != DeclarationProducerKind::Attribute {
        return Ok(None);
    }
    let Some(name) = row.unqualified_attribute.as_deref() else {
        return Ok(None);
    };
    Ok(super::builtin_attributes::retained_descriptor(name)?
        .map(|descriptor| descriptor.definition_identity()))
}
impl NestedInertProof {
    pub(crate) fn select(
        scan: Arc<DeclarationScan>,
        producer: Digest,
    ) -> Result<Option<Self>, Diagnostic> {
        let ordinal = scan
            .producers()
            .iter()
            .position(|row| row.identity == producer)
            .ok_or_else(|| fault("nested inert attribute source producer absent"))?;
        Self::select_indexed(scan, ordinal)
    }
    /// Exact row selected by a once-prepared source annotation plan.
    pub(crate) fn select_indexed(
        scan: Arc<DeclarationScan>,
        ordinal: usize,
    ) -> Result<Option<Self>, Diagnostic> {
        let identity = scan.fingerprint();
        Self::select_row(scan, ordinal, identity)
    }
    pub(crate) fn select_original_plan(
        plan: &super::source_declarative::original_annotations::OriginalAnnotationPlan,
        ordinal: usize,
    ) -> Result<Option<Self>, Diagnostic> {
        Self::select_row(Arc::clone(plan.scan()), ordinal, plan.scan_identity())
    }
    fn select_row(
        scan: Arc<DeclarationScan>,
        ordinal: usize,
        scan_identity: Digest,
    ) -> Result<Option<Self>, Diagnostic> {
        let row = scan
            .producers()
            .get(ordinal)
            .ok_or_else(|| fault("nested inert attribute source row absent"))?;
        let producer = row.identity;
        if row.kind != DeclarationProducerKind::Attribute {
            return Ok(None);
        }
        let Some(name) = row.unqualified_attribute.as_deref() else {
            return Ok(None);
        };
        let Some(descriptor) = super::builtin_attributes::retained_descriptor(name)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            scan,
            scan_identity,
            row: ordinal,
            producer,
            definition: descriptor.definition_identity(),
            attribute_facet: None,
        }))
    }
    pub(crate) fn with_attribute_facet(mut self, binding: Digest) -> Self {
        self.attribute_facet = Some(binding);
        self
    }
    pub(crate) fn attribute_facet(&self) -> Option<Digest> {
        self.attribute_facet
    }
    pub(crate) fn scan(&self) -> &Arc<DeclarationScan> {
        &self.scan
    }
    pub(crate) fn producer(&self) -> Digest {
        self.producer
    }
    pub(crate) fn name(&self) -> Result<&str, Diagnostic> {
        self.scan
            .producers()
            .get(self.row)
            .filter(|row| row.identity == self.producer)
            .and_then(|row| row.unqualified_attribute.as_deref())
            .ok_or_else(|| fault("nested inert attribute name absent"))
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.scan
            .retained_bytes()
            .checked_add(std::mem::size_of::<Self>())
            .ok_or_else(|| fault("nested inert attribute proof metadata overflow"))
    }
    pub(crate) fn revalidate(
        &self,
        blob: &crate::source::snapshot::SourceBlob,
        cfg: &crate::source::snapshot::CfgSet,
    ) -> Result<(), Diagnostic> {
        let parsed =
            crate::syntax::parse::ParsedFile::parse(blob, self.scan.parse().profile.clone())?;
        let scan = Arc::new(parsed.scan_declarations(
            self.scan.path().clone(),
            self.scan.scope().clone(),
            cfg,
            Default::default(),
        )?);
        let identity = scan.fingerprint();
        self.revalidate_scan(&scan, identity)
    }
    /// Reuse independently retained current preparation syntax. Current source
    /// receipts/epoch and namespace replay are separate coordinator obligations.
    pub(crate) fn revalidate_prepared(
        &self,
        source: &crate::source::compiler_replay::CompilerReplaySource<'_>,
    ) -> Result<(), Diagnostic> {
        self.revalidate_scan(source.scan(), source.scan_identity())
    }
    fn revalidate_scan(
        &self,
        scan: &Arc<DeclarationScan>,
        identity: Digest,
    ) -> Result<(), Diagnostic> {
        if identity != self.scan_identity {
            return Err(fault("nested inert replay source scan differs"));
        }
        let mut actual = Self::select_row(Arc::clone(scan), self.row, identity)?
            .ok_or_else(|| fault("nested inert attribute no longer selected"))?;
        // Syntax/compiler selection shares one kernel. Actual competing
        // Attribute namespace selection is still replayed by SourceDiscovery.
        actual.attribute_facet = self.attribute_facet;
        if actual.fingerprint() != self.fingerprint() {
            return Err(fault(
                "nested inert attribute source or compiler identity differs",
            ));
        }
        Ok(())
    }
}
impl StableEncode for NestedInertProof {
    const DOMAIN: &'static str = "lrr/v2/nested-inert-attribute-and-facet-proof";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.scan_identity);
        e.digest(self.producer);
        e.digest(self.definition);
        e.option(&self.attribute_facet, |e, binding| e.digest(*binding));
    }
}
