//! Private discovery completion certificates; incomplete catalogs cannot mint one.
use crate::{
    core::{
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    source::manifest::Namespace,
};
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum SealDomain {
    Scope { id: Digest, namespace: Namespace },
    ImplCatalog { id: Digest },
    TestCatalog { id: Digest },
}
impl StableEncode for SealDomain {
    const DOMAIN: &'static str = "lrr/v2/seal-domain";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Scope { id, namespace } => {
                e.u32(0);
                e.digest(*id);
                e.u32(namespace.tag());
            }
            Self::ImplCatalog { id } => {
                e.u32(1);
                e.digest(*id);
            }
            Self::TestCatalog { id } => {
                e.u32(2);
                e.digest(*id);
            }
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct SealKey {
    pub domain: SealDomain,
    pub unit_graph: Digest,
    pub epoch: u64,
}
impl SealKey {
    pub fn validate(&self) -> Result<(), crate::core::diagnostic::Diagnostic> {
        ProfileIdentity::current().validate_live()?;
        if self.epoch == 0 {
            return Err(crate::core::diagnostic::Diagnostic::new(
                "LRR-E-QUERY-REQUEST",
                "discovery certificate epoch must be nonzero".into(),
            ));
        }
        Ok(())
    }
}
impl StableEncode for SealKey {
    const DOMAIN: &'static str = "lrr/v2/discovery-seal-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&ProfileIdentity::current());
        e.record(&self.domain);
        e.digest(self.unit_graph);
        e.u64(self.epoch);
    }
}
#[derive(Debug, Clone)]
pub struct CertifiedScope {
    pub(crate) scope: crate::syntax::outline::ScopeKey,
    pub(crate) namespace: Namespace,
    pub(crate) edition: u16,
    pub(crate) names: std::collections::BTreeMap<String, crate::frontend::names::NameLookup>,
}
impl CertifiedScope {
    pub(crate) fn lookup(
        &self,
        bucket: &crate::db::key::BucketKey,
    ) -> Result<crate::frontend::names::NameLookup, crate::core::diagnostic::Diagnostic> {
        if bucket.scope != self.scope
            || bucket.namespace != self.namespace
            || bucket.name.len() > 64 * 1024
        {
            return Err(crate::core::diagnostic::Diagnostic::new(
                "LRR-E-QUERY-REQUEST",
                "certified namespace bucket owner/name differs".into(),
            ));
        }
        let name = crate::frontend::names::owned_name(&bucket.name, self.edition)?;
        Ok(self
            .names
            .get(name.as_str())
            .cloned()
            .unwrap_or(crate::frontend::names::NameLookup::Absent))
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, crate::core::diagnostic::Diagnostic> {
        let fail = || {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "certified namespace size overflow".into(),
            )
        };
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.scope.heap_bytes())
            .ok_or_else(fail)?;
        for (name, lookup) in &self.names {
            bytes = bytes
                .checked_add(96)
                .and_then(|n| n.checked_add(name.capacity()))
                .and_then(|n| lookup.metadata_bytes().ok().and_then(|m| n.checked_add(m)))
                .ok_or_else(fail)?;
        }
        Ok(bytes)
    }
}
impl StableEncode for CertifiedScope {
    const DOMAIN: &'static str = "lrr/v2/certified-namespace";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.scope);
        e.u32(self.namespace.tag());
        e.u16(self.edition);
        e.u64(u64::try_from(self.names.len()).expect("bounded namespace"));
        for (name, lookup) in &self.names {
            e.string(name);
            e.record(lookup);
        }
    }
}
#[derive(Debug, Clone)]
pub struct DiscoveryInput {
    pub(crate) seal: Seal,
    // Scope-local declaration facts; these do not grant global impl completeness.
    pub(crate) declaration_inventory: std::sync::Arc<crate::frontend::impl_index::ImplHeaderIndex>,
    pub(crate) source_dependencies: Digest,
    pub(crate) compiler_inputs: std::collections::BTreeMap<crate::db::dependency::InputKey, Digest>,
    pub(crate) sources: Vec<crate::source::changes::SourceSelection>,
    pub(crate) records: Vec<crate::expand::record::ExpansionDescriptor>,
    pub(crate) procedural_catalogs:
        Vec<std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>>,
    pub(crate) helper_declarations: Vec<(
        std::sync::Arc<crate::frontend::generated_derive_exports::DeriveDeclarationRequest>,
        std::sync::Arc<crate::expand::derive_receipt::PreparedDeriveDeclaration>,
    )>,
    pub(crate) argument_summaries: Vec<(
        Digest,
        std::sync::Arc<crate::syntax::declaration_scan::DeclarationScan>,
        Vec<std::sync::Arc<crate::expand::argument_declarations::ArgumentDeclarations>>,
    )>,
    pub(crate) argument_indexes: Vec<crate::frontend::impl_index::ArgumentDeclarationMacroIndex>,
    pub(crate) argument_fragments:
        Vec<std::sync::Arc<crate::expand::declaration_fragment::CompletedDeclarationFragment>>,
    pub(crate) source_replacements:
        Vec<std::sync::Arc<crate::expand::source_replacement::SourceReplacementProof>>,
    pub(crate) declaration_fragments:
        Vec<std::sync::Arc<crate::expand::declaration_fragment::CompletedDeclarationFragment>>,
    pub(crate) nested_inert_proofs: Vec<crate::expand::nested_inert::NestedInertProof>,
    pub(crate) runtime_no_item_proofs: Vec<(
        Digest,
        crate::frontend::runtime_macro_plan::RuntimeNoItemProof,
    )>,
    pub(crate) scope: Option<CertifiedScope>,
}
impl DiscoveryInput {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, crate::core::diagnostic::Diagnostic> {
        let fail = || {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "discovery input size overflow".into(),
            )
        };
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.seal.metadata_bytes()?)
            .and_then(|n| n.checked_add(self.declaration_inventory.metadata_bytes()))
            .and_then(|n| {
                self.sources
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::source::changes::SourceSelection>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(fail)?;
        bytes = bytes
            .checked_add(
                self.source_replacements
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        std::sync::Arc<crate::expand::source_replacement::SourceReplacementProof>,
                    >())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        for proof in &self.source_replacements {
            bytes = bytes
                .checked_add(proof.metadata_bytes()?)
                .ok_or_else(fail)?;
        }
        for source in &self.sources {
            bytes = bytes
                .checked_add(source.metadata_bytes()?)
                .ok_or_else(fail)?;
        }
        bytes = bytes
            .checked_add(
                self.records
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        crate::expand::record::ExpansionDescriptor,
                    >())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        for record in &self.records {
            bytes = bytes
                .checked_add(crate::expand::record::key_heap_bytes(&record.key))
                .ok_or_else(fail)?;
        }
        bytes = bytes
            .checked_add(
                self.procedural_catalogs
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>,
                    >())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        for catalog in &self.procedural_catalogs {
            bytes = bytes
                .checked_add(catalog.metadata_bytes()?)
                .ok_or_else(fail)?;
        }
        bytes = bytes
            .checked_add(
                self.compiler_inputs
                    .len()
                    .checked_mul(192)
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        bytes = bytes
            .checked_add(
                self.helper_declarations
                    .capacity()
                    .checked_mul(std::mem::size_of::<(
                        std::sync::Arc<
                            crate::frontend::generated_derive_exports::DeriveDeclarationRequest,
                        >,
                        std::sync::Arc<crate::expand::derive_receipt::PreparedDeriveDeclaration>,
                    )>())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        for (request, receipt) in &self.helper_declarations {
            bytes = bytes
                .checked_add(request.metadata_bytes()?)
                .and_then(|n| n.checked_add(receipt.estimated_bytes().ok()?))
                .ok_or_else(fail)?;
        }
        bytes = bytes
            .checked_add(
                self.runtime_no_item_proofs
                    .capacity()
                    .checked_mul(std::mem::size_of::<(
                        Digest,
                        crate::frontend::runtime_macro_plan::RuntimeNoItemProof,
                    )>())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        bytes = bytes
            .checked_add(
                self.nested_inert_proofs
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        crate::expand::nested_inert::NestedInertProof,
                    >())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        for proof in &self.nested_inert_proofs {
            bytes = bytes
                .checked_add(proof.metadata_bytes()?)
                .ok_or_else(fail)?;
        }
        for (_, proof) in &self.runtime_no_item_proofs {
            bytes = bytes
                .checked_add(proof.allocated_bytes().ok_or_else(fail)?)
                .ok_or_else(fail)?;
        }
        bytes = bytes
            .checked_add(
                self.declaration_fragments
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        std::sync::Arc<
                            crate::expand::declaration_fragment::CompletedDeclarationFragment,
                        >,
                    >())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        for fragment in &self.declaration_fragments {
            bytes = bytes
                .checked_add(fragment.metadata_bytes())
                .ok_or_else(fail)?;
        }
        bytes = self
            .argument_summaries
            .capacity()
            .checked_mul(std::mem::size_of::<(
                Digest,
                std::sync::Arc<crate::syntax::declaration_scan::DeclarationScan>,
                Vec<std::sync::Arc<crate::expand::argument_declarations::ArgumentDeclarations>>,
            )>())
            .and_then(|n| bytes.checked_add(n))
            .ok_or_else(fail)?;
        for (_, source, arguments) in &self.argument_summaries {
            bytes = arguments
                .capacity()
                .checked_mul(std::mem::size_of::<
                    std::sync::Arc<crate::expand::argument_declarations::ArgumentDeclarations>,
                >())
                .and_then(|n| bytes.checked_add(n))
                .ok_or_else(fail)?;
            bytes = bytes
                .checked_add(source.retained_bytes())
                .ok_or_else(fail)?;
            for argument in arguments {
                bytes = bytes
                    .checked_add(argument.metadata_bytes())
                    .ok_or_else(fail)?;
            }
        }
        bytes = self
            .argument_indexes
            .capacity()
            .checked_mul(std::mem::size_of::<
                crate::frontend::impl_index::ArgumentDeclarationMacroIndex,
            >())
            .and_then(|n| bytes.checked_add(n))
            .ok_or_else(fail)?;
        bytes = self
            .argument_fragments
            .capacity()
            .checked_mul(std::mem::size_of::<
                std::sync::Arc<crate::expand::declaration_fragment::CompletedDeclarationFragment>,
            >())
            .and_then(|n| bytes.checked_add(n))
            .ok_or_else(fail)?;
        for index in &self.argument_indexes {
            bytes = bytes.checked_add(index.metadata_bytes()).ok_or_else(fail)?;
        }
        for fragment in &self.argument_fragments {
            bytes = bytes
                .checked_add(fragment.metadata_bytes())
                .ok_or_else(fail)?;
        }
        if let Some(scope) = &self.scope {
            bytes = bytes
                .checked_add(scope.metadata_bytes()?)
                .ok_or_else(fail)?;
        }
        Ok(bytes)
    }
}
impl StableEncode for DiscoveryInput {
    const DOMAIN: &'static str = "lrr/v2/discovery-input/procedural-catalogs-v1";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.seal);
        e.digest(self.source_dependencies);
        e.u64(u64::try_from(self.compiler_inputs.len()).expect("bounded compiler input count"));
        for (key, fingerprint) in &self.compiler_inputs {
            e.record(key);
            e.digest(*fingerprint);
        }
        e.digest(self.declaration_inventory.fingerprint());
        e.sequence(&self.sources, |e, s| {
            e.string(s.path.as_str());
            e.record(&crate::db::key::QueryKey::Outline(s.outline.clone()));
        });
        e.sequence(&self.records, |e, r| e.record(r));
        e.sequence(&self.helper_declarations, |e, (request, receipt)| {
            e.record(request.as_ref());
            e.record(receipt.as_ref());
        });
        e.sequence(&self.procedural_catalogs, |e, catalog| {
            e.record(catalog.as_ref())
        });
        e.sequence(&self.nested_inert_proofs, |e, proof| e.record(proof));
        e.sequence(&self.runtime_no_item_proofs, |e, (identity, proof)| {
            e.digest(*identity);
            e.record(proof);
        });
        e.sequence(&self.argument_indexes, |e, index| {
            e.digest(index.argument().fingerprint());
            e.digest(index.inert_fingerprint());
            e.bool(index.is_complete());
        });
        e.sequence(&self.argument_fragments, |e, fragment| {
            e.digest(fragment.index().output().fingerprint());
            e.digest(fragment.inventory().fingerprint());
            e.digest(fragment.intrinsic_fingerprint());
        });
        e.sequence(&self.source_replacements, |e, proof| {
            e.record(proof.as_ref())
        });
        e.sequence(&self.declaration_fragments, |e, fragment| {
            e.digest(fragment.index().output().record().key().invocation);
            e.digest(fragment.index().output().fingerprint());
            e.digest(fragment.inventory().fingerprint());
            e.digest(fragment.intrinsic_fingerprint());
        });
        e.sequence(
            &self.argument_summaries,
            |e, (identity, source, arguments)| {
                e.digest(*identity);
                e.digest(source.fingerprint());
                e.sequence(arguments, |e, argument| e.record(argument.as_ref()));
            },
        );
        e.option(&self.scope, |e, s| e.record(s));
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Seal {
    domain: SealDomain,
    unit_graph: Digest,
    epoch: u64,
    members: Digest,
    scanned: Digest,
    dependencies: Vec<(Digest, Digest)>,
}
impl Seal {
    pub(crate) fn completed(
        domain: SealDomain,
        unit_graph: Digest,
        epoch: u64,
        members: Digest,
        scanned: Digest,
        dependencies: Vec<(Digest, Digest)>,
    ) -> Self {
        Self {
            domain,
            unit_graph,
            epoch,
            members,
            scanned,
            dependencies,
        }
    }
    pub fn key(&self) -> SealKey {
        SealKey {
            domain: self.domain.clone(),
            unit_graph: self.unit_graph,
            epoch: self.epoch,
        }
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, crate::core::diagnostic::Diagnostic> {
        self.dependencies
            .capacity()
            .checked_mul(std::mem::size_of::<(Digest, Digest)>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or(crate::core::diagnostic::Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "discovery certificate size overflow".into(),
            ))
    }
    pub fn input_dependencies(&self) -> &[(Digest, Digest)] {
        &self.dependencies
    }
    pub fn unresolved_frontiers(&self) -> u32 {
        0
    }
    pub fn domain(&self) -> &SealDomain {
        &self.domain
    }
}
impl StableEncode for Seal {
    const DOMAIN: &'static str = "lrr/v2/discovery-seal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&ProfileIdentity::current());
        e.record(&self.domain);
        e.digest(self.unit_graph);
        e.u64(self.epoch);
        e.digest(self.members);
        e.digest(self.scanned);
        e.sequence(&self.dependencies, |e, (id, value)| {
            e.digest(*id);
            e.digest(*value);
        });
        e.u32(0);
    }
}
