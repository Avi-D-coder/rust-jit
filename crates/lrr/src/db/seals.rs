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
    /// The spellings in `names` whose answer comes from the crate's standard
    /// prelude (no module item, import or extern-prelude crate supplied one).
    pub(crate) prelude_names: std::collections::BTreeSet<String>,
}
impl CertifiedScope {
    /// The fingerprint of the module-only answers. A replay graph without the
    /// compiler-injected standard prelude reproduces exactly this part.
    pub(crate) fn module_fingerprint(&self) -> crate::core::digest::Digest {
        if self.prelude_names.is_empty() {
            return self.fingerprint();
        }
        let mut module = self.clone();
        module
            .names
            .retain(|name, _| !self.prelude_names.contains(name));
        module.prelude_names.clear();
        module.fingerprint()
    }
    pub(crate) fn lookup(
        &self,
        bucket: &crate::db::key::BucketKey,
    ) -> Result<crate::frontend::names::NameLookup, crate::core::diagnostic::Diagnostic> {
        if bucket.scope != self.scope || bucket.namespace != self.namespace {
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
        for name in &self.prelude_names {
            bytes = bytes
                .checked_add(64)
                .and_then(|n| n.checked_add(name.capacity()))
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
        // Absent for prelude-free scopes, keeping their encoding unchanged.
        if !self.prelude_names.is_empty() {
            e.sequence(
                &self.prelude_names.iter().collect::<Vec<_>>(),
                |e, name| e.string(name),
            );
        }
    }
}
#[derive(Debug, Clone)]
pub struct DiscoveryInput {
    pub(crate) impl_catalog: Option<crate::expand::source_discovery::impl_catalog::ImplCatalogCompletion>,
    pub(crate) type_scopes: Vec<crate::expand::source_discovery::TypeScopeReceipt>,
    /// Value facets of the other module scopes a Value fixed point completed,
    /// ordered like `type_scopes`; contained in this seal, never another seal.
    pub(crate) value_scopes: Vec<crate::expand::source_discovery::TypeScopeReceipt>,
    pub(crate) value_binding_preludes:
        Vec<crate::facade::value_binding_prelude::ValueBindingPrelude>,
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
    /// Exact privately minted primary or contained facet. A contained Type
    /// receipt keeps the enclosing seal identity; it is not another seal.
    /// Sorted ownership is checked once before the locator is published.
    pub(crate) fn scope_certificate(
        &self,
        scope: &crate::syntax::outline::ScopeKey,
        namespace: Namespace,
        profile: &ProfileIdentity,
    ) -> Result<Option<&CertifiedScope>, crate::core::diagnostic::Diagnostic> {
        profile.validate_source()?;
        if let Some(primary) = &self.scope
            && primary.scope == *scope
            && primary.namespace == namespace
            && primary.edition == profile.edition
        {
            return Ok(Some(primary));
        }
        let receipts = match namespace {
            Namespace::Type => &self.type_scopes,
            Namespace::Value => &self.value_scopes,
            _ => return Ok(None),
        };
        let key = (scope, profile);
        let Ok(index) = receipts.binary_search_by(|proof| proof.owner_profile().cmp(&key)) else {
            return Ok(None);
        };
        let proof = &receipts[index];
        proof.require_owner(scope, profile)?;
        Ok(Some(proof.scope()))
    }
    /// The contained facet (never the primary scope) certifying `scope`.
    pub(crate) fn scope_receipt(
        &self,
        scope: &crate::syntax::outline::ScopeKey,
        namespace: Namespace,
        profile: &ProfileIdentity,
    ) -> Option<&crate::expand::source_discovery::TypeScopeReceipt> {
        let receipts = match namespace {
            Namespace::Type => &self.type_scopes,
            Namespace::Value => &self.value_scopes,
            _ => return None,
        };
        let key = (scope, profile);
        receipts
            .binary_search_by(|proof| proof.owner_profile().cmp(&key))
            .ok()
            .map(|index| &receipts[index])
    }
    /// SealedName predates profile-specific facets. Select only an unambiguous
    /// owning edition; callers cannot project one profile through another key.
    pub(crate) fn bucket_certificate(
        &self,
        bucket: &crate::db::key::BucketKey,
    ) -> Result<Option<&CertifiedScope>, crate::core::diagnostic::Diagnostic> {
        let mut selected = self
            .scope
            .as_ref()
            .filter(|scope| scope.scope == bucket.scope && scope.namespace == bucket.namespace);
        if matches!(bucket.namespace, Namespace::Type | Namespace::Value) {
            let receipts = if bucket.namespace == Namespace::Type {
                &self.type_scopes
            } else {
                &self.value_scopes
            };
            let start = receipts.partition_point(|proof| proof.scope().scope < bucket.scope);
            for proof in receipts[start..]
                .iter()
                .take_while(|proof| proof.scope().scope == bucket.scope)
            {
                let candidate = proof.scope();
                if selected.is_some_and(|old| old.edition != candidate.edition) {
                    return Err(crate::core::diagnostic::Diagnostic::new(
                        "LRR-E-QUERY-REQUEST",
                        "sealed bucket requires profile-specific query identity".into(),
                    ));
                }
                selected = Some(candidate);
            }
        }
        Ok(selected)
    }
    /// Coordinator-only structural admission, before any selector/input commit.
    pub(crate) fn require_scope_certificate_layout(
        &self,
    ) -> Result<(), crate::core::diagnostic::Diagnostic> {
        let fail = |detail: &str| {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                format!("discovery scope facet parent/order differs: {detail}"),
            )
        };
        match (&self.seal.domain(), &self.impl_catalog) {
            (SealDomain::ImplCatalog { .. }, Some(completion)) => completion.require_publication(self)?,
            (SealDomain::ImplCatalog { .. }, None) => return Err(fail("ImplCatalog has no actual source-world completion")),
            (_, Some(_)) => return Err(fail("ImplCatalog completion attached to another domain")),
            (_, None) => {},
        }
        if let Some(scope) = &self.scope
            && (!matches!(self.seal.domain(), SealDomain::Scope { id, namespace }
                if *id == scope.scope.fingerprint() && *namespace == scope.namespace)
                || scope.scope.owner.is_some())
        {
            return Err(fail("primary scope differs from its seal domain"));
        }
        if (!self.type_scopes.is_empty() || !self.value_scopes.is_empty())
            && !matches!(self.seal.domain(), SealDomain::Scope { .. })
        {
            return Err(fail("contained type scopes outside a scope seal"));
        }
        if !self.value_scopes.is_empty()
            && !matches!(self.seal.domain(), SealDomain::Scope { namespace: Namespace::Value, .. })
        {
            return Err(fail("contained value scopes outside a Value seal"));
        }
        let parent = self.seal.fingerprint();
        for (namespace, receipts) in [
            (Namespace::Type, &self.type_scopes),
            (Namespace::Value, &self.value_scopes),
        ] {
            let mut previous = None;
            for proof in receipts {
                proof.require_publication(parent)?;
                if proof.scope().namespace != namespace {
                    return Err(fail("contained scope namespace differs from its facet list"));
                }
                let key = proof.owner_profile();
                if previous.is_some_and(|old| old >= key) {
                    return Err(fail("contained type scopes are not strictly ordered"));
                }
                previous = Some(key);
                if let Some(primary) = &self.scope
                    && primary.scope == proof.scope().scope
                    && primary.namespace == namespace
                    && primary.edition == proof.scope().edition
                    && primary.fingerprint() != proof.scope().fingerprint()
                {
                    return Err(fail("contained type scope differs from primary"));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn scope_certificates(&self) -> impl Iterator<Item = &CertifiedScope> {
        self.scope
            .iter()
            .chain(self.type_scopes.iter().map(|proof| proof.scope()))
            .chain(self.value_scopes.iter().map(|proof| proof.scope()))
    }
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
        bytes = bytes
            .checked_add(
                self.type_scopes
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        crate::expand::source_discovery::TypeScopeReceipt,
                    >())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        bytes = bytes
            .checked_add(
                self.value_scopes
                    .capacity()
                    .checked_mul(std::mem::size_of::<
                        crate::expand::source_discovery::TypeScopeReceipt,
                    >())
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        for proof in self.type_scopes.iter().chain(&self.value_scopes) {
            bytes = bytes
                .checked_add(proof.metadata_bytes()?)
                .ok_or_else(fail)?;
        }
        for proof in &self.value_binding_preludes {
            bytes = bytes
                .checked_add(proof.metadata_bytes()?)
                .and_then(|n| n.checked_add(128))
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
impl DiscoveryInput {
    /// Entire existing publication payload, excluding only the new completion
    /// receipt itself. Used only at cold source-catalog admission/reader setup.
    pub(crate) fn catalog_payload_fingerprint(&self) -> Digest {
        let mut e = StableEncoder::new(<Self as StableEncode>::DOMAIN);
        self.encode_catalog_payload(&mut e);
        e.finish()
    }
    fn encode_catalog_payload(&self, e: &mut StableEncoder) {
        e.record(&self.seal);
        e.sequence(&self.type_scopes, |e, proof| e.record(proof));
        e.sequence(&self.value_scopes, |e, proof| e.record(proof));
        e.sequence(&self.value_binding_preludes, |e, proof| e.record(proof));
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
impl StableEncode for DiscoveryInput {
    const DOMAIN: &'static str = "lrr/v4/discovery-input/type-scope-receipts-v1";
    fn encode_fields(&self, e: &mut StableEncoder) {
        self.encode_catalog_payload(e);
        // Preserve exact existing Scope/TestCatalog encodings. Only the new
        // genuine source-catalog publication carries this typed extension.
        if let Some(completion) = &self.impl_catalog {
            e.string("impl-catalog-completion-v1");
            e.record(completion);
        }
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
