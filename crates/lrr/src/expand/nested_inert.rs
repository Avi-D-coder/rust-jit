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
        let row = scan
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(|| fault("nested inert attribute source producer absent"))?;
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
            .iter()
            .find(|row| row.identity == self.producer)
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
        let mut actual = Self::select(scan, self.producer)?
            .ok_or_else(|| fault("nested inert attribute no longer selected"))?;
        // This pure source replay verifies syntax and the retained certificate.
        // SourceDiscovery separately re-resolves the namespace facet at seal.
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
        e.digest(self.scan.fingerprint());
        e.digest(self.producer);
        e.digest(self.definition);
        e.option(&self.attribute_facet, |e, binding| e.digest(*binding));
    }
}
