//! Immutable syntax inventories; pending producers never become completed impl seals.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    syntax::declaration_scan::{DeclarationProducer, DeclarationScan, ScannedImpl},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Actual immutable syntax owner of a captured scan. A raw scan ParseKey is
/// not necessarily a globally published file (notably for macro fragments).
#[derive(Clone)]
pub(crate) enum ImplSyntaxSource {
    Original {
        source: Arc<crate::source::changes::PreparedSourceView>,
        parsed: Arc<crate::syntax::parse::ParsedFile>,
    },
    Generated(Arc<crate::expand::generated_declarations::GeneratedDeclarations>),
    Argument(Arc<crate::expand::argument_declarations::ArgumentDeclarations>),
}
impl ImplSyntaxSource {
    pub(crate) fn original(source: &crate::source::changes::PreparedSourceView) -> Result<Self, Diagnostic> {
        Ok(Self::Original {
            source: Arc::new(source.share()),
            parsed: source.retained_declaration_parser()?.clone(),
        })
    }
    pub(crate) fn parsed(&self) -> &crate::syntax::parse::ParsedFile {
        match self {
            Self::Original { parsed, .. } => parsed,
            Self::Generated(output) => output.admitted_parsed(),
            Self::Argument(argument) => argument.admitted_parsed(),
        }
    }
}
impl std::fmt::Debug for ImplSyntaxSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ImplSyntaxSource")
            .field(&self.parsed().key())
            .finish()
    }
}
pub struct ImplHeaderIndex {
    sources: Vec<Arc<DeclarationScan>>,
    headers: Vec<ScannedImpl>,
    producers: Vec<DeclarationProducer>,
    retained_bytes: usize,
    syntax_sources: BTreeMap<crate::core::digest::Digest, ImplSyntaxSource>,
    environment: Option<Arc<crate::expand::source_discovery::CompletedSourceEnvironment>>,
}
impl std::fmt::Debug for ImplHeaderIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImplHeaderIndex")
            .field("sources", &self.sources.len())
            .field("headers", &self.headers.len())
            .field("producers", &self.producers.len())
            .finish()
    }
}
fn fault() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-IMPL-INVENTORY",
        "impl inventory source/count/metadata conflict or limit".into(),
    )
}
impl ImplHeaderIndex {
    pub fn from_scans(mut sources: Vec<Arc<DeclarationScan>>) -> Result<Self, Diagnostic> {
        sources.sort_by_cached_key(|s| s.fingerprint());
        let mut seen = BTreeSet::new();
        let mut headers = Vec::new();
        let mut producers = Vec::new();
        let mut bytes = 0usize;
        for source in &sources {
            if !seen.insert(source.fingerprint()) {
                return Err(fault());
            }
            bytes = bytes
                .checked_add(source.retained_bytes().checked_mul(2).ok_or_else(fault)?)
                .ok_or_else(fault)?;
            headers
                .try_reserve_exact(source.impls().len())
                .map_err(|_| fault())?;
            headers.extend_from_slice(source.impls());
            producers
                .try_reserve_exact(source.producers().len())
                .map_err(|_| fault())?;
            producers.extend_from_slice(source.producers());
        }
        headers.sort_by_key(|h| h.identity);
        producers.sort_by_key(|p| p.identity);
        Ok(Self {
            sources,
            headers,
            producers,
            retained_bytes: bytes,
            syntax_sources: BTreeMap::new(),
            environment: None,
        })
    }
    /// Called by actual discovery publication, retaining only owners joined
    /// to this inventory's exact scans. Missing syntax is an unfinished join.
    pub(crate) fn with_syntax_sources(
        mut self,
        mut sources: BTreeMap<crate::core::digest::Digest, ImplSyntaxSource>,
    ) -> Result<Self, Diagnostic> {
        for scan in &self.sources {
            if let Some(source) = sources.remove(&scan.fingerprint()) {
                if source.parsed().key() != *scan.parse() {
                    return Err(fault());
                }
                self.syntax_sources.insert(scan.fingerprint(), source);
            }
        }
        Ok(self)
    }
    pub(crate) fn with_environment(mut self, environment: Option<Arc<crate::expand::source_discovery::CompletedSourceEnvironment>>) -> Self {
        self.environment = environment;
        self
    }
    pub(crate) fn environment(&self) -> Option<&Arc<crate::expand::source_discovery::CompletedSourceEnvironment>> {
        self.environment.as_ref()
    }
    pub(crate) fn syntax_source(
        &self,
        scan: crate::core::digest::Digest,
    ) -> Option<&ImplSyntaxSource> {
        self.syntax_sources.get(&scan)
    }
    pub(crate) fn exclude_replaced_sources(
        mut self,
        proofs: &[Arc<crate::expand::source_replacement::SourceReplacementProof>],
    ) -> Result<Self, Diagnostic> {
        let mut ranges = std::collections::BTreeMap::<
            crate::core::digest::Digest,
            Vec<crate::syntax::spans::SourceRange>,
        >::new();
        proofs
            .len()
            .checked_mul(192)
            .ok_or_else(fault)?;
        for proof in proofs {
            let rows = ranges.entry(proof.attachment()).or_default();
            rows.try_reserve_exact(1).map_err(|_| fault())?;
            rows.push(proof.range());
        }
        for rows in ranges.values_mut() {
            rows.sort_by_key(|range| (range.start, std::cmp::Reverse(range.end)));
            let mut merged: Vec<crate::syntax::spans::SourceRange> = Vec::new();
            merged.try_reserve_exact(rows.len()).map_err(|_| fault())?;
            for range in rows.iter() {
                if let Some(last) = merged.last_mut()
                    && range.start <= last.end
                {
                    last.end = last.end.max(range.end);
                } else {
                    merged.push(*range);
                }
            }
            *rows = merged;
        }
        let mut headers = BTreeSet::new();
        let mut producers = BTreeSet::new();
        for scan in &self.sources {
            let mut e = StableEncoder::new("lrr/v2/source-replacement-attachment");
            e.string(scan.path().as_str());
            e.record(&crate::db::key::QueryKey::Outline(
                crate::db::key::OutlineKey {
                    parse: scan.parse().clone(),
                    crate_key: scan.scope().crate_key,
                    module_path: scan.scope().module_path.clone(),
                    cfg: scan.cfg(),
                },
            ));
            let Some(rows) = ranges.get(&e.finish()) else {
                continue;
            };
            let covered = |range: crate::syntax::spans::SourceRange| {
                let end = rows.partition_point(|row| row.start <= range.start);
                end > 0 && range.end <= rows[end - 1].end
            };
            headers.extend(
                scan.impls()
                    .iter()
                    .filter(|header| covered(header.range))
                    .map(|header| header.identity),
            );
            producers.extend(
                scan.producers()
                    .iter()
                    .filter(|producer| covered(producer.range))
                    .map(|producer| producer.identity),
            );
        }
        self.headers
            .retain(|header| !headers.contains(&header.identity));
        self.producers
            .retain(|producer| !producers.contains(&producer.identity));
        Ok(self)
    }
    pub(crate) fn exclude_generated_replacements(
        mut self,
        masks: &[(
            crate::core::digest::Digest,
            crate::syntax::spans::SourceRange,
        )],
    ) -> Result<Self, Diagnostic> {
        let mut ranges =
            std::collections::BTreeMap::<_, Vec<crate::syntax::spans::SourceRange>>::new();
        for (identity, range) in masks {
            if range.start >= range.end {
                return Err(fault());
            }
            let rows = ranges.entry(*identity).or_default();
            rows.try_reserve_exact(1).map_err(|_| fault())?;
            rows.push(*range);
        }
        for rows in ranges.values_mut() {
            rows.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
            let mut merged: Vec<crate::syntax::spans::SourceRange> = Vec::new();
            merged.try_reserve_exact(rows.len()).map_err(|_| fault())?;
            for row in rows.iter() {
                if let Some(last) = merged.last_mut()
                    && row.start <= last.end
                {
                    last.end = last.end.max(row.end);
                    continue;
                }
                merged.push(*row);
            }
            *rows = merged;
        }
        let mut headers = BTreeSet::new();
        let mut producers = BTreeSet::new();
        for scan in &self.sources {
            let Some(rows) = ranges.remove(&scan.fingerprint()) else {
                continue;
            };
            let covered = |range: crate::syntax::spans::SourceRange| {
                let end = rows.partition_point(|r| r.start <= range.start);
                end > 0 && range.end <= rows[end - 1].end
            };
            headers.extend(
                scan.impls()
                    .iter()
                    .filter(|h| covered(h.range))
                    .map(|h| h.identity),
            );
            producers.extend(
                scan.producers()
                    .iter()
                    .filter(|p| covered(p.range))
                    .map(|p| p.identity),
            );
        }
        if !ranges.is_empty() {
            return Err(fault());
        }
        self.headers.retain(|h| !headers.contains(&h.identity));
        self.producers.retain(|p| !producers.contains(&p.identity));
        Ok(self)
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn scan_handles(&self) -> &[Arc<DeclarationScan>] {
        &self.sources
    }
    pub fn headers(&self) -> &[ScannedImpl] {
        &self.headers
    }
    pub fn pending_producers(&self) -> &[DeclarationProducer] {
        &self.producers
    }
}
impl StableEncode for ImplHeaderIndex {
    const DOMAIN: &'static str = "lrr/v5/impl-header-inventory";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.option(&self.environment, |e, environment| {
            e.record(environment.dependency());
            e.digest(environment.language_item_identity());
        });
        e.sequence(&self.sources, |e, s| e.digest(s.fingerprint()));
        e.sequence(&self.headers, |e, h| {
            e.digest(h.identity);
            e.digest(h.header_identity);
        });
        e.sequence(&self.producers, |e, p| e.digest(p.identity));
        e.u64(self.syntax_sources.len() as u64);
        for (scan, source) in &self.syntax_sources {
            e.digest(*scan);
            e.record(&source.parsed().key());
        }
    }
}

/// Unsealed lexical candidates for cold declaration discovery. This is not a
/// runtime body-entry grant or an impl catalog completion certificate.
#[derive(Clone)]
pub struct DeclarationMacroIndex {
    imports: Arc<super::declaration_imports::DeclarationImportPlan>,
    scan: Arc<DeclarationScan>,
    scopes: crate::expand::scopes::TextualScopes,
    declarations: super::macro_scopes::MacroDeclarations,
    facts: super::hygiene::HygieneFacts,
    ancestry: super::macro_origins::ExpansionAncestry,
    context: crate::syntax::token_wire::Key,
    derive_prefix: Option<crate::core::digest::Digest>,
    // Actual admitted occurrence records, indexed by their original Meta owner.
    appended_outputs: std::collections::BTreeMap<crate::core::digest::Digest,
        std::collections::BTreeMap<crate::core::digest::Digest, crate::core::digest::Digest>>,
    producer_rows: std::collections::BTreeMap<crate::core::digest::Digest, usize>,
    definition_rows: std::collections::BTreeMap<crate::core::digest::Digest, usize>,
    retained_bytes: usize,
    published_bodies: std::collections::BTreeMap<
        crate::core::digest::Digest,
        Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
    >,
    pending: std::collections::BTreeSet<crate::core::digest::Digest>,
}
impl DeclarationMacroIndex {
    pub fn new(scan: Arc<DeclarationScan>) -> Result<Self, Diagnostic> {
        use crate::expand::scopes::{MacroDefinition, MacroName, TextualScopes};
        if scan.generated_origin().is_some() {
            return Err(fault()); // Generated names require their actual output hygiene facts.
        }
        scan.retained_bytes()
            .checked_add(
                scan.lexical_scopes()
                    .len()
                    .checked_mul(320)
                    .ok_or_else(fault)?,
            )
            .and_then(|n| n.checked_add(scan.macro_definitions().len().checked_mul(512)?))
            .and_then(|n| n.checked_add(scan.producers().len().checked_mul(256)?))
            .ok_or_else(fault)?;
        let root = crate::expand::source_hygiene::source_root(
            scan.scope().crate_key,
            scan.parse().profile.edition,
        )?;
        let facts = super::hygiene::HygieneFacts::new(root.contexts, root.rows)?
            .with_name_contexts(root.names)?;
        let mut scopes = TextualScopes::default();
        for lexical in scan.lexical_scopes() {
            if let Some(parent) = lexical.parent {
                let actual = scopes.add_child(parent, u64::from(lexical.range.start), 0)?;
                if actual != lexical.identity {
                    return Err(fault());
                }
            } else {
                scopes.add_root(lexical.identity)?;
            }
        }
        let mut declarations = super::macro_scopes::MacroDeclarations::default();
        let ancestry = super::macro_origins::ExpansionAncestry::default();
        let mut definition_rows = std::collections::BTreeMap::new();
        for (ordinal, header) in scan.macro_definitions().iter().enumerate() {
            if definition_rows.insert(header.identity, ordinal).is_some() {
                return Err(fault());
            }
            let definition = MacroDefinition {
                identity: header.identity,
                defining_unit: scan.scope().crate_key,
                definition_edition: scan.parse().profile.edition,
            };
            declarations.register(definition.clone(), root.context, None, &facts, &ancestry)?;
            scopes.define(
                header.lexical_scope,
                // Events use strict `before` cutoffs. End offsets can equal the
                // next call/scope start; the declaration's own event is earlier.
                u64::from(header.range.start),
                MacroName::parse(&header.name, definition.definition_edition)?,
                definition,
            )?;
        }
        let mut producer_rows = std::collections::BTreeMap::new();
        for (ordinal, producer) in scan.producers().iter().enumerate() {
            if producer_rows.insert(producer.identity, ordinal).is_some() {
                return Err(fault());
            }
            scopes.block(
                producer.lexical_scope,
                u64::from(producer.range.start),
                producer.identity,
            )?;
        }
        let imports = Arc::new(super::declaration_imports::DeclarationImportPlan::new(
            &scan,
        )?);
        let declaration_bytes = declarations.metadata_bytes()?;
        let hygiene_bytes = facts.metadata_bytes()?;
        let scope_bytes = scopes.metadata_bytes()?;
        let retained_bytes = scan
            .retained_bytes()
            .checked_add(imports.metadata_bytes())
            .and_then(|n| n.checked_add(scope_bytes))
            .and_then(|n| n.checked_add(declaration_bytes))
            .and_then(|n| n.checked_add(hygiene_bytes))
            .and_then(|n| n.checked_add(producer_rows.len().checked_mul(192)?))
            .and_then(|n| n.checked_add(definition_rows.len().checked_mul(128)?))
            .ok_or_else(fault)?;
        Ok(Self {
            imports,
            pending: producer_rows.keys().copied().collect(),
            published_bodies: Default::default(),
            ancestry,
            scan,
            scopes,
            declarations,
            facts,
            context: root.context,
            derive_prefix: None,
            appended_outputs: Default::default(),
            producer_rows,
            definition_rows,
            retained_bytes,
        })
    }

    /// Only this inventory's producer frontiers; this is not a graph or impl seal.
    pub(crate) fn retire_replaced_item(
        &mut self,
        proof: &crate::expand::source_replacement::SourceReplacementProof,
    ) -> Result<Vec<crate::core::digest::Digest>, Diagnostic> {
        if !proof.matches_scan(&self.scan) {
            return Err(fault());
        }
        let mut retired = Vec::new();
        retired
            .try_reserve_exact(self.pending.len())
            .map_err(|_| fault())?;
        for producer in self.scan.producers().iter().filter(|producer| {
            proof.covers(producer.range) && self.pending.contains(&producer.identity)
        }) {
            retired.push((
                producer.identity,
                producer.lexical_scope,
                producer.range.start,
            ));
        }
        for (identity, lexical, position) in &retired {
            self.scopes
                .publish(*lexical, u64::from(*position), *identity, Vec::new())?;
            self.pending.remove(identity);
        }
        let mut identities = Vec::new();
        identities
            .try_reserve_exact(retired.len())
            .map_err(|_| fault())?;
        identities.extend(retired.into_iter().map(|(identity, _, _)| identity));
        Ok(identities)
    }
    pub fn is_complete(&self) -> bool {
        self.pending.is_empty()
    }
    pub(crate) fn contains_definition(&self, definition: crate::core::digest::Digest) -> bool {
        self.definition_rows.contains_key(&definition)
            || self.published_bodies.contains_key(&definition)
    }
    pub(crate) fn body_owner(
        &self,
        definition: crate::core::digest::Digest,
    ) -> Result<DeclarationBodyOwner<'_>, Diagnostic> {
        if let Some(output) = self.published_bodies.get(&definition) {
            return Ok(DeclarationBodyOwner::Generated(output));
        }
        if self.definition_rows.contains_key(&definition) {
            return Ok(DeclarationBodyOwner::Source(self));
        }
        Err(fault())
    }
    pub fn adopt_completed_child(
        &mut self,
        expected: &crate::expand::identity::ExpansionKey,
        child: &GeneratedDeclarationMacroIndex,
    ) -> Result<(), Diagnostic> {
        self.adopt_completed_child_prefix(expected, child, true)
    }
    pub(crate) fn derive_prefix_view(
        &self,
        invocation: crate::core::digest::Digest,
    ) -> Result<Self, Diagnostic> {
        let producer = self.producer(invocation)?;
        if !self.pending.contains(&invocation) {
            return Err(fault());
        }
        let mut view = self.clone();
        view.scopes = self.scopes.prefix_view(
            producer.lexical_scope, u64::from(producer.range.start), invocation,
        )?;
        view.derive_prefix = Some(invocation);
        Ok(view)
    }
    pub(crate) fn adopt_completed_child_prefix(
        &mut self,
        expected: &crate::expand::identity::ExpansionKey,
        child: &GeneratedDeclarationMacroIndex,
        complete: bool,
    ) -> Result<(), Diagnostic> {
        let owner = child.output.parent_invocation();
        let occurrence = child.output.record().key().invocation;
        if self.appended_outputs.get(&owner).is_some_and(|outputs| outputs.contains_key(&occurrence)) {
            return Err(fault());
        }
        let mut publication = DeclarationPublication {
            scopes: self.scopes.clone(),
            declarations: self.declarations.clone(),
            facts: self.facts.clone(),
            ancestry: self.ancestry.clone(),
            bodies: self.published_bodies.clone(),
            pending: self.pending.clone(),
            retained_bytes: self.retained_bytes,
        };
        publication.adopt(&self.scan, expected, child, complete)?;
        self.appended_outputs.entry(owner).or_default().insert(
            occurrence, child.output.record().descriptor().fingerprint(),
        );
        self.scopes = publication.scopes;
        self.declarations = publication.declarations;
        self.facts = publication.facts;
        self.ancestry = publication.ancestry;
        self.published_bodies = publication.bodies;
        self.pending = publication.pending;
        self.retained_bytes = publication.retained_bytes;
        Ok(())
    }
    pub(crate) fn require_pending_original_group(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        group: &crate::source::original_items::OriginalAnnotationGroup,
    ) -> Result<(), Diagnostic> {
        group.require_source(source)?;
        if group.scan_identity() != self.scan.fingerprint() || group.annotations().is_empty()
            || group.annotations().iter().any(|id| !self.pending.contains(id)) {
            return Err(fault());
        }
        Ok(())
    }
    pub(crate) fn retain_original_derive_prefix(
        &self,
        group: &crate::source::original_items::OriginalAnnotationGroup,
    ) -> Result<super::macro_boundary::RetainedMacroScope, Diagnostic> {
        let invocation = *group.annotations().first().ok_or_else(fault)?;
        if self.scan.fingerprint() != group.scan_identity()
            || self.derive_prefix != Some(invocation) || !self.pending.contains(&invocation) {
            return Err(fault());
        }
        let producer = self.producer(invocation)?;
        super::macro_boundary::RetainedMacroScope::new(
            Arc::new(self.scopes.clone()), Arc::new(self.declarations.clone()),
            Arc::new(self.facts.clone()), Arc::new(self.ancestry.clone()),
            producer.lexical_scope, u64::from(producer.range.start).checked_add(1).ok_or_else(fault)?,
        )
    }
    /// Retire only the actual scan annotations owned by a fully replayed
    /// original item group. Appended definitions survive pending retirement.
    pub(crate) fn complete_original_annotation_group(
        &mut self,
        source: &crate::source::changes::PreparedSourceView,
        group: &crate::source::original_items::OriginalAnnotationGroup,
        proof: &crate::expand::source_derives::SourceDeriveCompletionProof,
    ) -> Result<(), Diagnostic> {
        proof.require_group(group, source)?;
        if group.scan_identity() != self.scan.fingerprint() || group.annotations().is_empty() {
            return Err(fault());
        }
        let occurrences = group.annotations().iter()
            .filter_map(|annotation| self.appended_outputs.get(annotation))
            .flat_map(|outputs| outputs.keys().copied()).collect();
        proof.require_occurrences(&occurrences)?;
        let mut scopes = self.scopes.clone();
        let mut pending = self.pending.clone();
        for annotation in group.annotations() {
            let producer = self.producer(*annotation)?;
            if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
                || !group.call().range.contains(producer.range)
                || !pending.remove(annotation) {
                return Err(fault());
            }
            scopes.complete_prefix(producer.lexical_scope, u64::from(producer.range.start), *annotation)?;
        }
        self.scopes = scopes;
        self.pending = pending;
        Ok(())
    }
    /// Transfer an original-item frontier to its existing discovery owner.
    /// Callers must retain that owner's readiness gate before nested lookup.
    pub(crate) fn accept_completed_argument_inventory(
        &mut self,
        invocation: crate::core::digest::Digest,
        indexes: &[ArgumentDeclarationMacroIndex],
        catalog: &super::runtime_macro_catalog::RuntimeMacroCatalog,
    ) -> Result<(), Diagnostic> {
        if !self.pending.contains(&invocation) || indexes.is_empty() {
            return Err(fault());
        }
        for index in indexes {
            let argument = index.argument();
            if !index.is_complete()
                || argument.parent_inventory() != self.scan.fingerprint()
                || argument.invocation() != invocation
                || catalog.select(argument.definition())?.is_none()
            {
                return Err(fault());
            }
        }
        let producer = self.producer(invocation)?;
        self.scopes.publish(
            producer.lexical_scope,
            u64::from(producer.range.start),
            invocation,
            Vec::new(),
        )?;
        self.pending.remove(&invocation);
        Ok(())
    }
    pub(crate) fn accept_inert_attribute(
        &mut self,
        identity: crate::core::digest::Digest,
        name: &str,
    ) -> Result<(), Diagnostic> {
        let producer = self.producer(identity)?;
        if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            || producer.unqualified_attribute.as_deref() != Some(name)
            || !self.pending.contains(&identity)
        {
            return Err(fault());
        }
        self.scopes.publish(
            producer.lexical_scope,
            u64::from(producer.range.start),
            identity,
            Vec::new(),
        )?;
        self.pending.remove(&identity);
        Ok(())
    }
    pub(crate) fn accept_runtime_no_item_proof(
        &mut self,
        invocation: crate::core::digest::Digest,
        proof: &super::runtime_macro_plan::RuntimeNoItemProof,
        catalog: &super::runtime_macro_catalog::RuntimeMacroCatalog,
    ) -> Result<(), Diagnostic> {
        let producer = self.producer(invocation)?;
        if proof.source() != self.scan.parse()
            || proof.invocation() != producer.range
            || producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            || catalog.select(proof.definition())?.is_none()
            || !self.pending.contains(&invocation)
        {
            return Err(fault());
        }
        self.scopes.publish(
            producer.lexical_scope,
            u64::from(producer.range.start),
            invocation,
            Vec::new(),
        )?;
        self.pending.remove(&invocation);
        Ok(())
    }
    pub(crate) fn delegate_original_producer(
        &mut self,
        identity: crate::core::digest::Digest,
    ) -> Result<(), Diagnostic> {
        let producer = self.producer(identity)?;
        let scope = producer.lexical_scope;
        let at = u64::from(producer.range.start);
        self.scopes.publish(scope, at, identity, Vec::new())?;
        if !self.pending.remove(&identity) {
            return Err(fault());
        }
        Ok(())
    }
    pub(crate) fn retained_metadata_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn scan_handle(&self) -> &Arc<DeclarationScan> {
        &self.scan
    }
    pub fn scan(&self) -> &DeclarationScan {
        &self.scan
    }
    pub(crate) fn producer(
        &self,
        identity: crate::core::digest::Digest,
    ) -> Result<&DeclarationProducer, Diagnostic> {
        self.scan
            .producers()
            .get(*self.producer_rows.get(&identity).ok_or_else(fault)?)
            .ok_or_else(fault)
    }
    pub(crate) fn definition(
        &self,
        identity: crate::core::digest::Digest,
    ) -> Result<&crate::syntax::declaration_scan::ScannedMacroDefinition, Diagnostic> {
        self.scan
            .macro_definitions()
            .get(*self.definition_rows.get(&identity).ok_or_else(fault)?)
            .ok_or_else(fault)
    }
    pub(crate) fn lookup_imports(
        &self,
        invocation: crate::core::digest::Digest,
        segments: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        names: &super::macro_names::MacroNamespaces,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        super::declaration_imports::DeclarationImportIndex::prepared(&self.scan, &self.imports)?
            .lookup_with_textual(
                invocation,
                segments,
                absolute,
                names,
                |_| Ok(true),
                |row, name| {
                    declaration_import_reference(
                        &self.scan,
                        row,
                        name,
                        Some(self.context),
                        None,
                        &self.scopes,
                        &self.declarations,
                        &[],
                        &self.facts,
                        &self.ancestry,
                    )
                },
            )
    }
    pub fn lookup(
        &self,
        invocation: crate::core::digest::Digest,
    ) -> Result<super::macro_scopes::HygienicLookup, Diagnostic> {
        use crate::syntax::declaration_scan::DeclarationProducerKind;
        let producer = self.producer(invocation)?;
        // Qualified and annotation namespaces require their separate captured
        // namespace inputs. An unresolved route must never certify absence.
        if producer.kind != DeclarationProducerKind::Macro || producer.path.len() != 1 {
            return Ok(super::macro_scopes::HygienicLookup::NeedInput(invocation));
        }
        let name = crate::expand::scopes::MacroName::parse(
            &producer.path[0].text,
            self.scan.parse().profile.edition,
        )?;
        self.declarations.resolve(
            &self.scopes,
            super::macro_scopes::MacroReference {
                scope: producer.lexical_scope,
                before: u64::from(producer.range.start),
                name: &name,
                context: self.context,
                origin: None,
            },
            &self.facts,
            &self.ancestry,
        )
    }
}

/// Original intrinsic argument lexical facts. Its syntax identity is not expansion ancestry.
#[derive(Clone)]
pub struct ArgumentDeclarationMacroIndex {
    argument: Arc<crate::expand::argument_declarations::ArgumentDeclarations>,
    inert: std::collections::BTreeMap<crate::core::digest::Digest, crate::core::digest::Digest>,
    source: Arc<DeclarationMacroIndex>,
    outer: DeclarationMacroParent,
    origin: Option<crate::syntax::token_wire::Key>,
    depth: usize,
    scopes: crate::expand::scopes::TextualScopes,
    declarations: super::macro_scopes::MacroDeclarations,
    facts: super::hygiene::HygieneFacts,
    ancestry: super::macro_origins::ExpansionAncestry,
    published_bodies: std::collections::BTreeMap<
        crate::core::digest::Digest,
        Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
    >,
    pending: std::collections::BTreeSet<crate::core::digest::Digest>,
    retained_bytes: usize,
}
impl std::fmt::Debug for ArgumentDeclarationMacroIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArgumentDeclarationMacroIndex")
            .field("argument", &self.argument.fingerprint())
            .field("pending", &self.pending.len())
            .finish()
    }
}
impl ArgumentDeclarationMacroIndex {
    pub fn new(
        argument: Arc<crate::expand::argument_declarations::ArgumentDeclarations>,
        source: Arc<DeclarationMacroIndex>,
    ) -> Result<Self, Diagnostic> {
        Self::with_parent(argument, DeclarationMacroParent::Source(source))
    }
    pub(crate) fn with_parent(
        argument: Arc<crate::expand::argument_declarations::ArgumentDeclarations>,
        outer: DeclarationMacroParent,
    ) -> Result<Self, Diagnostic> {
        let source = match &outer {
            DeclarationMacroParent::Source(source) => source.clone(),
            DeclarationMacroParent::Generated(index) => Arc::new(index.source_index()?.clone()),
            DeclarationMacroParent::Argument(index) => index.source.clone(),
        };
        let depth = match &outer {
            DeclarationMacroParent::Source(_) => 0,
            DeclarationMacroParent::Generated(index) => index.depth,
            DeclarationMacroParent::Argument(index) => index.depth,
        };
        let origin = match &outer {
            DeclarationMacroParent::Source(_) => None,
            DeclarationMacroParent::Generated(index) => Some(index.origin),
            DeclarationMacroParent::Argument(index) => index.origin,
        };
        use crate::expand::scopes::{MacroDefinition, MacroName, TextualScopes};
        if argument.parent_inventory() != outer.scan().fingerprint() {
            return Err(fault());
        }
        let facts = outer.facts().clone();
        let ancestry = match &outer {
            DeclarationMacroParent::Source(i) => i.ancestry.clone(),
            DeclarationMacroParent::Generated(i) => i.ancestry.clone(),
            DeclarationMacroParent::Argument(i) => i.ancestry.clone(),
        };
        let mut scopes = TextualScopes::default();
        for lexical in argument.scan().lexical_scopes() {
            if let Some(parent) = lexical.parent {
                if scopes.add_child(parent, u64::from(lexical.range.start), 0)? != lexical.identity
                {
                    return Err(fault());
                }
            } else {
                scopes.add_root(lexical.identity)?;
            }
        }
        let mut declarations = super::macro_scopes::MacroDeclarations::default();
        for header in argument.scan().macro_definitions() {
            let capture = argument.macro_definition(header.identity)?;
            let context = argument.token_context(capture.name_span)?;
            let definition = MacroDefinition {
                identity: header.identity,
                defining_unit: argument.scan().scope().crate_key,
                definition_edition: capture.edition,
            };
            declarations.register(definition.clone(), context, origin, &facts, &ancestry)?;
            scopes.define(
                header.lexical_scope,
                u64::from(header.range.start),
                MacroName::parse(&header.name, capture.edition)?,
                definition,
            )?;
        }
        let mut pending = std::collections::BTreeSet::new();
        for producer in argument.scan().producers() {
            scopes.block(
                producer.lexical_scope,
                u64::from(producer.range.start),
                producer.identity,
            )?;
            pending.insert(producer.identity);
        }
        let retained_bytes = argument
            .metadata_bytes()
            .checked_add(source.retained_bytes)
            .and_then(|n| n.checked_add(scopes.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(declarations.metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(pending.len().checked_mul(128)?))
            .ok_or_else(fault)?;
        Ok(Self {
            argument,
            inert: Default::default(),
            source,
            outer,
            origin,
            depth,
            scopes,
            declarations,
            facts,
            ancestry,
            published_bodies: Default::default(),
            pending,
            retained_bytes,
        })
    }
    pub fn argument(&self) -> &Arc<crate::expand::argument_declarations::ArgumentDeclarations> {
        &self.argument
    }
    pub(crate) fn retire_inert(
        &mut self,
        identity: crate::core::digest::Digest,
        definition: crate::core::digest::Digest,
    ) -> Result<(), Diagnostic> {
        let producer = self.producer(identity)?;
        if !self.pending.contains(&identity)
            || producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            || producer.unqualified_attribute.is_none()
            || self.inert.contains_key(&identity)
        {
            return Err(fault());
        }
        let bytes = self
            .retained_bytes
            .checked_add(128)
            .ok_or_else(fault)?;
        self.scopes.publish(
            producer.lexical_scope,
            u64::from(producer.range.start),
            identity,
            Vec::new(),
        )?;
        self.pending.remove(&identity);
        self.inert.insert(identity, definition);
        self.retained_bytes = bytes;
        Ok(())
    }
    pub(crate) fn validates_inert(
        &self,
        scan: &DeclarationScan,
        identity: crate::core::digest::Digest,
    ) -> Result<bool, Diagnostic> {
        let Some(definition) = self.inert.get(&identity) else {
            return Ok(false);
        };
        Ok(scan.fingerprint() == self.argument.scan().fingerprint()
            && crate::expand::nested_inert::selected_definition(scan, identity)?
                == Some(*definition))
    }
    pub(crate) fn inert_fingerprint(&self) -> crate::core::digest::Digest {
        let mut e = StableEncoder::new("lrr/v2/argument-inert-attributes");
        e.u64(u64::try_from(self.inert.len()).expect("bounded argument producers"));
        for (id, definition) in &self.inert {
            e.digest(*id);
            e.digest(*definition);
        }
        e.finish()
    }
    pub(crate) fn emitting_origin(&self) -> Option<crate::syntax::token_wire::Key> {
        self.origin
    }
    pub(crate) fn source_index(&self) -> &Arc<DeclarationMacroIndex> {
        &self.source
    }
    pub fn is_complete(&self) -> bool {
        self.pending.is_empty()
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub(crate) fn completed_for(
        &self,
        argument: &crate::expand::argument_declarations::ArgumentDeclarations,
    ) -> bool {
        self.is_complete() && self.argument.fingerprint() == argument.fingerprint()
    }
    pub fn next_pending_producer(&self) -> Option<crate::core::digest::Digest> {
        self.argument
            .scan()
            .producers()
            .iter()
            .filter(|p| self.pending.contains(&p.identity))
            .min_by_key(|p| (p.range.start, p.identity))
            .map(|p| p.identity)
    }
    fn producer(
        &self,
        id: crate::core::digest::Digest,
    ) -> Result<&crate::syntax::declaration_scan::DeclarationProducer, Diagnostic> {
        self.argument
            .scan()
            .producers()
            .iter()
            .find(|p| p.identity == id)
            .ok_or_else(fault)
    }
    fn source_outer(&self) -> Result<super::macro_scopes::OuterMacroScope<'_>, Diagnostic> {
        self.outer.outer(self.argument.invocation())
    }
    fn outer_scopes(&self) -> Result<Vec<super::macro_scopes::OuterMacroScope<'_>>, Diagnostic> {
        let mut result = Vec::new();
        result
            .try_reserve_exact(
                self.depth
                    .checked_add(1)
                    .ok_or_else(fault)?,
            )
            .map_err(|_| fault())?;
        result.push(self.source_outer()?);
        let mut parent = &self.outer;
        loop {
            match parent {
                DeclarationMacroParent::Source(_) => break,
                DeclarationMacroParent::Argument(index) => {
                    result.try_reserve(1).map_err(|_| fault())?;
                    result.push(index.source_outer()?);
                    parent = &index.outer;
                }
                DeclarationMacroParent::Generated(index) => {
                    result.try_reserve(1).map_err(|_| fault())?;
                    result.push(index.parent.outer(index.output.parent_invocation())?);
                    parent = &index.parent;
                }
            }
        }
        Ok(result)
    }
    pub fn lookup(
        &self,
        id: crate::core::digest::Digest,
    ) -> Result<super::macro_scopes::HygienicLookup, Diagnostic> {
        let p = self.producer(id)?;
        if p.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Macro
            || p.path.len() != 1
        {
            return Ok(super::macro_scopes::HygienicLookup::NeedInput(id));
        }
        let call = self.argument.macro_call(id)?;
        let (name, context) =
            crate::expand::source_hygiene::declaration_macro_name(&call.path, call.edition)?;
        self.declarations.resolve_with_outers(
            &self.scopes,
            super::macro_scopes::MacroReference {
                scope: p.lexical_scope,
                before: u64::from(p.range.start),
                name: &name,
                context,
                origin: self.origin,
            },
            &self.outer_scopes()?,
            &self.facts,
            &self.ancestry,
        )
    }
    pub(crate) fn body_owner(
        &self,
        id: crate::core::digest::Digest,
    ) -> Result<DeclarationBodyOwner<'_>, Diagnostic> {
        if let Some(output) = self.published_bodies.get(&id) {
            return Ok(DeclarationBodyOwner::Generated(output));
        }
        if self
            .argument
            .scan()
            .macro_definitions()
            .iter()
            .any(|h| h.identity == id)
        {
            return Ok(DeclarationBodyOwner::Argument(&self.argument));
        }
        match &self.outer {
            DeclarationMacroParent::Source(i) => i.body_owner(id),
            DeclarationMacroParent::Generated(i) => i.body_owner(id),
            DeclarationMacroParent::Argument(i) => i.body_owner(id),
        }
    }
    pub fn adopt_completed_child(
        &mut self,
        expected: &crate::expand::identity::ExpansionKey,
        child: &GeneratedDeclarationMacroIndex,
    ) -> Result<(), Diagnostic> {
        let mut publication = DeclarationPublication {
            scopes: self.scopes.clone(),
            declarations: self.declarations.clone(),
            facts: self.facts.clone(),
            ancestry: self.ancestry.clone(),
            bodies: self.published_bodies.clone(),
            pending: self.pending.clone(),
            retained_bytes: self.retained_bytes,
        };
        publication.adopt(self.argument.scan(), expected, child, true)?;
        self.scopes = publication.scopes;
        self.declarations = publication.declarations;
        self.facts = publication.facts;
        self.ancestry = publication.ancestry;
        self.published_bodies = publication.bodies;
        self.pending = publication.pending;
        self.retained_bytes = publication.retained_bytes;
        Ok(())
    }
}

#[derive(Clone)]
pub enum DeclarationMacroParent {
    Source(Arc<DeclarationMacroIndex>),
    Generated(Arc<GeneratedDeclarationMacroIndex>),
    Argument(Arc<ArgumentDeclarationMacroIndex>),
}
/// Exact physical invocation capture, independent of declaration lookup and
/// of diagnostic/structural expansion ancestry.
pub(crate) enum DeclarationCallSite<'a> {
    Source { scan: &'a DeclarationScan, range: crate::syntax::spans::SourceRange },
    Captured(&'a lrr_expand_adapter::token_wire::TokenForest),
}
impl DeclarationMacroParent {
    fn invocation_site(&self, invocation: crate::core::digest::Digest) -> Result<DeclarationCallSite<'_>, Diagnostic> {
        Ok(match self {
            Self::Source(index) => {
                let producer = index.producer(invocation)?;
                DeclarationCallSite::Source { scan: &index.scan, range: producer.range }
            }
            Self::Generated(index) => DeclarationCallSite::Captured(&index.output.macro_call(invocation)?.path),
            Self::Argument(index) => DeclarationCallSite::Captured(&index.argument.macro_call(invocation)?.path),
        })
    }
    fn scan(&self) -> &DeclarationScan {
        match self {
            Self::Source(index) => index.scan(),
            Self::Generated(index) => index.output.scan(),
            Self::Argument(index) => index.argument.scan(),
        }
    }
    pub(crate) fn facts(&self) -> &super::hygiene::HygieneFacts {
        match self {
            Self::Source(index) => &index.facts,
            Self::Generated(index) => &index.facts,
            Self::Argument(index) => &index.facts,
        }
    }
    pub(crate) fn outer(
        &self,
        invocation: crate::core::digest::Digest,
    ) -> Result<super::macro_scopes::OuterMacroScope<'_>, Diagnostic> {
        match self {
            Self::Source(index) => {
                let producer = index.producer(invocation)?;
                Ok(super::macro_scopes::OuterMacroScope {
                    scopes: &index.scopes,
                    declarations: &index.declarations,
                    scope: producer.lexical_scope,
                    before: u64::from(producer.range.start)
                        .checked_add(u64::from(index.derive_prefix == Some(invocation)))
                        .ok_or_else(fault)?,
                })
            }
            Self::Argument(index) => {
                let producer = index.producer(invocation)?;
                Ok(super::macro_scopes::OuterMacroScope {
                    scopes: &index.scopes,
                    declarations: &index.declarations,
                    scope: producer.lexical_scope,
                    before: u64::from(producer.range.start),
                })
            }
            Self::Generated(index) => {
                let producer = index.producer(invocation)?;
                Ok(super::macro_scopes::OuterMacroScope {
                    scopes: &index.scopes,
                    declarations: &index.declarations,
                    scope: producer.lexical_scope,
                    before: u64::from(producer.range.start)
                        .checked_add(u64::from(index.derive_prefix == Some(invocation)))
                        .ok_or_else(fault)?,
                })
            }
        }
    }
    pub(crate) fn body_owner(
        &self,
        definition: crate::core::digest::Digest,
    ) -> Result<DeclarationBodyOwner<'_>, Diagnostic> {
        match self {
            Self::Source(index) => index.body_owner(definition),
            Self::Argument(index) => index.body_owner(definition),
            Self::Generated(index) => index.body_owner(definition),
        }
    }
    pub(crate) fn compiler_ancestry(&self) -> &super::macro_origins::ExpansionAncestry {
        match self {
            Self::Source(index) => &index.ancestry,
            Self::Generated(index) => &index.ancestry,
            Self::Argument(index) => &index.ancestry,
        }
    }
    /// Argument lookup cannot depend on bindings produced by its own invocation.
    /// Retain the live index unchanged; omit only that position in this private view.
    pub(crate) fn compiler_argument_snapshot(
        mut self,
        invocation: crate::core::digest::Digest,
    ) -> Result<Self, Diagnostic> {
        match &mut self {
            Self::Source(index) => {
                let producer = index.producer(invocation)?;
                let site = (producer.lexical_scope, u64::from(producer.range.start));
                Arc::make_mut(index).scopes.omit_invocation_position(site.0, site.1)?;
            }
            Self::Argument(index) => {
                let producer = index.producer(invocation)?;
                let site = (producer.lexical_scope, u64::from(producer.range.start));
                Arc::make_mut(index).scopes.omit_invocation_position(site.0, site.1)?;
            }
            Self::Generated(index) => {
                let producer = index.producer(invocation)?;
                let site = (producer.lexical_scope, u64::from(producer.range.start));
                Arc::make_mut(index).scopes.omit_invocation_position(site.0, site.1)?;
            }
        }
        Ok(self)
    }
    pub(crate) fn compiler_origin(&self) -> Option<crate::syntax::token_wire::Key> {
        match self {
            Self::Source(_) => None,
            Self::Generated(index) => Some(index.origin),
            Self::Argument(index) => index.origin,
        }
    }
    /// The same emitting prefixes used by hygienic lookup, retained in order.
    pub(crate) fn compiler_outers(
        &self,
    ) -> Result<Vec<super::macro_scopes::OuterMacroScope<'_>>, Diagnostic> {
        match self {
            Self::Source(_) => Ok(Vec::new()),
            Self::Argument(index) => index.outer_scopes(),
            Self::Generated(index) => {
                let mut result = Vec::new();
                result.try_reserve_exact(257).map_err(|_| fault())?;
                let mut child = index.as_ref();
                loop {
                    result.push(child.parent.outer(child.output.parent_invocation())?);
                    match &child.parent {
                        Self::Source(_) => break,
                        Self::Argument(parent) => {
                            let outers = parent.outer_scopes()?;
                            result.extend(outers);
                            break;
                        }
                        Self::Generated(parent) => child = parent,
                    }
                }
                Ok(result)
            }
        }
    }
}
pub(crate) enum DeclarationBodyOwner<'a> {
    Source(&'a DeclarationMacroIndex),
    Argument(&'a crate::expand::argument_declarations::ArgumentDeclarations),
    Generated(&'a crate::expand::generated_declarations::GeneratedDeclarations),
}
/// Unsealed generated lexical candidates joined to the actual emitting prefix.
/// A candidate is not an outward publication or a completed impl catalog.
#[derive(Clone)]
pub struct GeneratedDeclarationMacroIndex {
    imports: Arc<super::declaration_imports::DeclarationImportPlan>,
    import_contexts: Arc<
        std::collections::BTreeMap<
            crate::core::digest::Digest,
            Option<(
                crate::syntax::token_wire::NameContextKey,
                crate::syntax::token_wire::Key,
            )>,
        >,
    >,
    output: Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
    parent: DeclarationMacroParent,
    depth: usize,
    scopes: crate::expand::scopes::TextualScopes,
    declarations: super::macro_scopes::MacroDeclarations,
    facts: super::hygiene::HygieneFacts,
    ancestry: super::macro_origins::ExpansionAncestry,
    origin: crate::syntax::token_wire::Key,
    definitions: std::collections::BTreeSet<crate::core::digest::Digest>,
    outward: Vec<DeclarationOutwardMacro>,
    producers: std::collections::BTreeMap<crate::core::digest::Digest, usize>,
    retained_bytes: usize,
    published_bodies: std::collections::BTreeMap<
        crate::core::digest::Digest,
        Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
    >,
    pending: std::collections::BTreeSet<crate::core::digest::Digest>,
    derive_prefix: Option<crate::core::digest::Digest>,
    pending_order: std::collections::BTreeSet<(u32, crate::core::digest::Digest)>,
}
impl GeneratedDeclarationMacroIndex {
    pub fn new(
        output: Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
        parent: DeclarationMacroParent,
    ) -> Result<Self, Diagnostic> {
        use crate::{
            core::digest::Digest,
            expand::scopes::{MacroDefinition, MacroName, TextualScopes},
            syntax::token_wire::Key,
        };
        if output.parent_inventory() != parent.scan().fingerprint()
            || output.scan().generated_origin() != Some(output.fingerprint())
            || matches!(&parent, DeclarationMacroParent::Generated(index) if index.output.epoch() != output.epoch())
        {
            return Err(fault());
        }
        let depth = match &parent {
            DeclarationMacroParent::Source(_) => 0,
            DeclarationMacroParent::Argument(index) => index.depth,
            DeclarationMacroParent::Generated(index) => index.depth,
        };
        let depth = depth
            .checked_add(1)            .ok_or_else(fault)?;
        output
            .metadata_bytes()?
            .checked_add(
                output
                    .scan()
                    .lexical_scopes()
                    .len()
                    .checked_mul(320)
                    .ok_or_else(fault)?,
            )
            .and_then(|n| n.checked_add(output.scan().macro_definitions().len().checked_mul(512)?))
            .and_then(|n| n.checked_add(output.scan().producers().len().checked_mul(256)?))
            .ok_or_else(fault)?;
        let mut facts = parent.facts().clone();
        facts.extend(output.hygiene_facts()?)?;
        let mut ancestry = match &parent {
            DeclarationMacroParent::Source(index) => index.ancestry.clone(),
            DeclarationMacroParent::Argument(index) => index.ancestry.clone(),
            DeclarationMacroParent::Generated(index) => index.ancestry.clone(),
        };
        let origin = Key(output.record().key().fingerprint().0);
        ancestry.register(
            origin,
            match &parent {
                DeclarationMacroParent::Source(_) => None,
                DeclarationMacroParent::Argument(index) => index.origin,
                DeclarationMacroParent::Generated(index) => Some(index.origin),
            },
        )?;
        let mut scopes = TextualScopes::default();
        for lexical in output.scan().lexical_scopes() {
            if let Some(parent) = lexical.parent {
                if scopes.add_child(parent, u64::from(lexical.range.start), 0)? != lexical.identity
                {
                    return Err(fault());
                }
            } else {
                scopes.add_root(lexical.identity)?;
            }
        }
        let mut declarations = super::macro_scopes::MacroDeclarations::default();
        let spans = output
            .tokens()
            .spans()
            .iter()
            .map(|s| (s.key().0, s))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut definitions = std::collections::BTreeSet::new();
        let mut outward = Vec::new();
        let root = output
            .scan()
            .lexical_scopes()
            .first()
            .ok_or_else(fault)?
            .identity;
        for header in output.scan().macro_definitions() {
            let capture = output.macro_definition(header.identity)?;
            let context = spans.get(&capture.name_span.0).ok_or_else(fault)?.context;
            let definition = MacroDefinition {
                identity: header.identity,
                defining_unit: output.scan().scope().crate_key,
                definition_edition: capture.edition,
            };
            declarations.register(definition.clone(), context, Some(origin), &facts, &ancestry)?;
            if header.lexical_scope == root {
                outward.push(DeclarationOutwardMacro {
                    order: vec![header.range.start],
                    name: MacroName::parse(&header.name, capture.edition)?,
                    definition: definition.clone(),
                    context: crate::syntax::token_wire::Key(context.0),
                    origin: Some(origin),
                    body: Arc::clone(&output),
                });
            }

            scopes.define(
                header.lexical_scope,
                u64::from(header.range.start),
                MacroName::parse(&header.name, capture.edition)?,
                definition,
            )?;
            if !definitions.insert(header.identity) {
                return Err(fault());
            }
        }
        let mut producers = std::collections::BTreeMap::<Digest, usize>::new();
        for (ordinal, producer) in output.scan().producers().iter().enumerate() {
            scopes.block(
                producer.lexical_scope,
                u64::from(producer.range.start),
                producer.identity,
            )?;
            if producers.insert(producer.identity, ordinal).is_some() {
                return Err(fault());
            }
        }
        let outward_bytes = outward.iter().try_fold(0usize, |n, row| {
            n.checked_add(row.metadata_bytes()?).ok_or_else(fault)
        })?;
        let imports = Arc::new(super::declaration_imports::DeclarationImportPlan::new(
            output.scan(),
        )?);
        let import_contexts = Arc::new(declaration_import_contexts(&output, &facts)?);
        let import_context_bytes = import_contexts.len().checked_mul(192).ok_or_else(fault)?;
        let declaration_bytes = declarations.metadata_bytes()?;
        let fact_bytes = facts.metadata_bytes()?;
        let ancestry_bytes = ancestry.metadata_bytes()?;
        let scope_bytes = scopes.metadata_bytes()?;
        let local_bytes = output
            .metadata_bytes()?
            .checked_add(imports.metadata_bytes())
            .and_then(|n| n.checked_add(import_context_bytes))
            .and_then(|n| n.checked_add(scope_bytes))
            .and_then(|n| n.checked_add(declaration_bytes))
            .and_then(|n| n.checked_add(fact_bytes))
            .and_then(|n| n.checked_add(ancestry_bytes))
            .and_then(|n| n.checked_add(definitions.len().checked_mul(128)?))
            .and_then(|n| n.checked_add(producers.len().checked_mul(256)?))
            .and_then(|n| n.checked_add(outward_bytes))
            .ok_or_else(fault)?;
        let parent_bytes = match &parent {
            DeclarationMacroParent::Source(index) => index.retained_bytes,
            DeclarationMacroParent::Argument(index) => index.retained_bytes,
            DeclarationMacroParent::Generated(index) => index.retained_bytes,
        };
        let retained_bytes = local_bytes
            .checked_add(parent_bytes)
            .ok_or_else(fault)?;
        let pending_order = output
            .scan()
            .producers()
            .iter()
            .map(|p| (p.range.start, p.identity))
            .collect();
        Ok(Self {
            imports,
            import_contexts,
            derive_prefix: None,
            pending_order,
            pending: producers.keys().copied().collect(),
            published_bodies: Default::default(),
            outward,
            output,
            parent,
            depth,
            scopes,
            declarations,
            facts,
            ancestry,
            origin,
            definitions,
            producers,
            retained_bytes,
        })
    }

    /// Only this inventory's producer frontiers; this is not a graph or impl seal.
    pub fn is_complete(&self) -> bool {
        self.pending.is_empty()
    }
    pub fn adopt_completed_child(
        &mut self,
        expected: &crate::expand::identity::ExpansionKey,
        child: &GeneratedDeclarationMacroIndex,
    ) -> Result<(), Diagnostic> {
        self.adopt_completed_child_prefix(expected, child, true, None)
    }
    pub(crate) fn derive_prefix_view(
        &self,
        invocation: crate::core::digest::Digest,
    ) -> Result<Self, Diagnostic> {
        let p = self.producer(invocation)?;
        let mut view = self.clone();
        view.scopes =
            self.scopes
                .prefix_view(p.lexical_scope, u64::from(p.range.start), invocation)?;
        view.derive_prefix = Some(invocation);
        Ok(view)
    }
    pub(crate) fn adopt_completed_child_prefix(
        &mut self,
        expected: &crate::expand::identity::ExpansionKey,
        child: &GeneratedDeclarationMacroIndex,
        complete: bool,
        append_ordinal: Option<u32>,
    ) -> Result<(), Diagnostic> {
        let mut publication = DeclarationPublication {
            scopes: self.scopes.clone(),
            declarations: self.declarations.clone(),
            facts: self.facts.clone(),
            ancestry: self.ancestry.clone(),
            bodies: self.published_bodies.clone(),
            pending: self.pending.clone(),
            retained_bytes: self.retained_bytes,
        };
        publication.adopt(self.output.scan(), expected, child, complete)?;
        let producer = self.producer(child.output.parent_invocation())?;
        let mut outward = self.outward.clone();
        if producer.lexical_scope
            == self
                .output
                .scan()
                .lexical_scopes()
                .first()
                .ok_or_else(fault)?
                .identity
        {
            outward
                .len()
                .checked_add(child.outward.len())
                .ok_or_else(fault)?;
            outward
                .try_reserve_exact(child.outward.len())
                .map_err(|_| fault())?;
            for row in &child.outward {
                let mut row = row.clone();
                let mut order = Vec::new();
                order
                    .try_reserve_exact(
                        row.order
                            .len()
                            .checked_add(1 + usize::from(append_ordinal.is_some()))                            .ok_or_else(fault)?,
                    )
                    .map_err(|_| fault())?;
                order.push(producer.range.start);
                if let Some(ordinal) = append_ordinal {
                    order.push(ordinal);
                }
                order.extend_from_slice(&row.order);
                row.order = order;
                outward.push(row);
            }
            outward.sort_by(|a, b| a.order.cmp(&b.order));
        }
        let outward_bytes = outward.iter().try_fold(0usize, |n, row| {
            n.checked_add(row.metadata_bytes()?).ok_or_else(fault)
        })?;
        publication.retained_bytes = publication
            .retained_bytes
            .checked_add(outward_bytes)
            .ok_or_else(fault)?;
        if complete {
            self.pending_order
                .remove(&(producer.range.start, producer.identity));
        }
        self.scopes = publication.scopes;
        self.declarations = publication.declarations;
        self.facts = publication.facts;
        self.ancestry = publication.ancestry;
        self.published_bodies = publication.bodies;
        self.pending = publication.pending;
        self.retained_bytes = publication.retained_bytes;
        self.outward = outward;
        Ok(())
    }
    pub(crate) fn next_pending_producer(&self) -> Option<crate::core::digest::Digest> {
        self.pending_order.first().map(|(_, id)| *id)
    }
    /// Admit physical generated module facts against this owned declaration
    /// index. The caller supplies the current epoch; this grants no route/seal.
    /// Annotation frontiers remain pending until separately proved inert.
    pub fn module_bindings(
        &self,
        expected_epoch: u64,
    ) -> Result<Vec<super::macro_names::DeclarationModuleBinding>, Diagnostic> {
        if expected_epoch == 0 || self.output.epoch() != expected_epoch {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-EPOCH",
                "generated module binding epoch differs".into(),
            ));
        }
        let scan = self.output.scan();
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(scan.modules().len())
            .map_err(|_| fault())?;
        let mut bytes = 0usize;
        let tokens = self.output.module_name_token_forests()?;
        if tokens.len() != scan.modules().len() {
            return Err(fault());
        }
        for (index, (row, tokens)) in scan.modules().iter().zip(tokens).enumerate() {
            let (name, physical) =
                crate::expand::source_hygiene::declaration_macro_name(&tokens, row.name_edition)?;
            // Normalize through this index's pinned owned facts, not through a
            // spelling or a cross-unit comparison class supplied by the caller.
            let context = self.facts.item(physical)?;
            let binding =
                super::macro_names::DeclarationModuleBinding::from_generated_scan_in_grammar(
                    self.parent.scan(),
                    self.output.parent_invocation(),
                    scan,
                    index,
                    context,
                    self.output.record().key().grammar,
                )?;
            if binding.name != name {
                return Err(fault());
            }
            bytes = bytes
                .checked_add(binding.metadata_bytes()?)
                .ok_or_else(fault)?;
            bindings.push(binding);
        }
        Ok(bindings)
    }
    /// Select an actual expansion's invocation, following only the retained
    /// emitting source graph. The caller chooses the expansion from a raw
    /// token context, rather than assuming structural parents are call sites.
    pub(crate) fn expansion_call_site(&self, expansion: crate::core::digest::Digest) -> Result<DeclarationCallSite<'_>, Diagnostic> {
        let mut current = self;
        loop {
            if current.output.record().key().fingerprint() == expansion {
                return current.parent.invocation_site(current.output.parent_invocation());
            }
            let mut parent = &current.parent;
            loop {
                match parent {
                    DeclarationMacroParent::Generated(index) => { current = index; break; }
                    DeclarationMacroParent::Argument(index) => parent = &index.outer,
                    DeclarationMacroParent::Source(_) => return Err(fault()),
                }
            }
        }
    }
    pub(crate) fn parent(&self) -> &DeclarationMacroParent {
        &self.parent
    }
    pub(crate) fn retained_metadata_bytes(&self) -> usize {
        self.retained_bytes
    }
    pub fn output(&self) -> &Arc<crate::expand::generated_declarations::GeneratedDeclarations> {
        &self.output
    }
    pub(crate) fn producer(
        &self,
        identity: crate::core::digest::Digest,
    ) -> Result<&DeclarationProducer, Diagnostic> {
        self.output
            .scan()
            .producers()
            .get(*self.producers.get(&identity).ok_or_else(fault)?)
            .ok_or_else(fault)
    }
    pub(crate) fn source_index(&self) -> Result<&DeclarationMacroIndex, Diagnostic> {
        let mut parent = &self.parent;
        for _ in 0usize.. {
            match parent {
                DeclarationMacroParent::Source(index) => return Ok(index),
                DeclarationMacroParent::Argument(index) => return Ok(&index.source),
                DeclarationMacroParent::Generated(index) => parent = &index.parent,
            }
        }
        Err(fault())
    }
    pub(crate) fn body_owner(
        &self,
        definition: crate::core::digest::Digest,
    ) -> Result<DeclarationBodyOwner<'_>, Diagnostic> {
        if let Some(output) = self.published_bodies.get(&definition) {
            return Ok(DeclarationBodyOwner::Generated(output));
        }
        if self.definitions.contains(&definition) {
            return Ok(DeclarationBodyOwner::Generated(&self.output));
        }
        let mut parent = &self.parent;
        for _ in 0usize.. {
            match parent {
                DeclarationMacroParent::Source(index) => {
                    return index.body_owner(definition);
                }
                DeclarationMacroParent::Argument(index) => {
                    return index.body_owner(definition);
                }
                DeclarationMacroParent::Generated(index) => {
                    if let Some(output) = index.published_bodies.get(&definition) {
                        return Ok(DeclarationBodyOwner::Generated(output));
                    }
                    if index.definitions.contains(&definition) {
                        return Ok(DeclarationBodyOwner::Generated(&index.output));
                    }
                    parent = &index.parent;
                }
            }
        }
        Err(fault())
    }
    pub(crate) fn complete_resolved_intrinsic_arguments(
        &mut self,
        identity: crate::core::digest::Digest,
        indexes: &[ArgumentDeclarationMacroIndex],
    ) -> Result<(), Diagnostic> {
        if !self.pending.contains(&identity)
            || indexes.iter().any(|i| {
                !i.is_complete()
                    || i.argument().parent_inventory() != self.output.scan().fingerprint()
                    || i.argument().invocation() != identity
            })
        {
            return Err(fault());
        }
        let p = self.producer(identity)?;
        let lexical = p.lexical_scope;
        let position = p.range.start;
        self.scopes
            .publish(lexical, u64::from(position), identity, Vec::new())?;
        self.pending.remove(&identity);
        self.pending_order.remove(&(position, identity));
        Ok(())
    }
    pub(crate) fn retire_replaced_generated_body(
        &mut self,
        invocation: crate::core::digest::Digest,
        range: crate::syntax::spans::SourceRange,
    ) -> Result<(), Diagnostic> {
        let producer = self.producer(invocation)?;
        if !self.pending.contains(&invocation)
            || producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            || self.output.attribute_item_range(invocation)? != range
        {
            return Err(fault());
        }
        let mut retired = Vec::new();
        retired
            .try_reserve_exact(self.pending.len())
            .map_err(|_| fault())?;
        retired.extend(
            self.output
                .scan()
                .producers()
                .iter()
                .filter(|p| {
                    p.identity != invocation
                        && self.pending.contains(&p.identity)
                        && range.start <= p.range.start
                        && p.range.end <= range.end
                })
                .map(|p| (p.identity, p.lexical_scope, p.range.start)),
        );
        for (identity, scope, position) in retired {
            self.scopes
                .publish(scope, u64::from(position), identity, Vec::new())?;
            self.pending.remove(&identity);
            self.pending_order.remove(&(position, identity));
        }
        Ok(())
    }
    pub(crate) fn retire_completed_item_attribute(
        &mut self,
        identity: crate::core::digest::Digest,
        catalog: &super::generated_macros::GeneratedMacroCatalog,
    ) -> Result<crate::core::digest::Digest, Diagnostic> {
        let definition = catalog
            .completed_helper_declaration(&self.output, identity)?
            .ok_or_else(fault)?;
        let producer = self.producer(identity)?;
        if !self.pending.contains(&identity) {
            return Err(fault());
        }
        let lexical = producer.lexical_scope;
        let position = producer.range.start;
        self.scopes
            .publish(lexical, u64::from(position), identity, Vec::new())?;
        self.pending.remove(&identity);
        self.pending_order.remove(&(position, identity));
        Ok(definition)
    }
    pub(crate) fn retire_inert_attribute(
        &mut self,
        identity: crate::core::digest::Digest,
    ) -> Result<(), Diagnostic> {
        let producer = self.producer(identity)?;
        if !self.pending.contains(&identity)
            || producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            || producer.unqualified_attribute.is_none()
        {
            return Err(fault());
        }
        let lexical = producer.lexical_scope;
        let position = producer.range.start;
        self.scopes
            .publish(lexical, u64::from(position), identity, Vec::new())?;
        self.pending.remove(&identity);
        self.pending_order.remove(&(position, identity));
        Ok(())
    }
    pub(crate) fn complete_intrinsic_arguments(
        &mut self,
        identity: crate::core::digest::Digest,
        scans: &[Arc<DeclarationScan>],
    ) -> Result<(), Diagnostic> {
        if !self.pending.contains(&identity) || scans.iter().any(|scan| !scan.producers().is_empty()) {
            return Err(fault());
        }
        let producer = self.producer(identity)?;
        let lexical = producer.lexical_scope;
        let position = producer.range.start;
        self.scopes
            .publish(lexical, u64::from(position), identity, Vec::new())?;
        self.pending.remove(&identity);
        self.pending_order.remove(&(position, identity));
        Ok(())
    }
    fn outer_scopes(&self) -> Result<Vec<super::macro_scopes::OuterMacroScope<'_>>, Diagnostic> {
        let mut outers = Vec::new();
        outers.try_reserve_exact(self.depth).map_err(|_| fault())?;
        let mut child = self;
        loop {
            outers.push(child.parent.outer(child.output.parent_invocation())?);
            match &child.parent {
                DeclarationMacroParent::Source(_) => break,
                DeclarationMacroParent::Argument(index) => {
                    outers.extend(index.outer_scopes()?);
                    break;
                }
                DeclarationMacroParent::Generated(index) => child = index,
            }
        }
        Ok(outers)
    }
    pub(crate) fn lookup_imports(
        &self,
        invocation: crate::core::digest::Digest,
        segments: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        names: &super::macro_names::MacroNamespaces,
    ) -> Result<super::macro_names::PathLookup, Diagnostic> {
        let capture = self.output.macro_call(invocation)?;
        let mut expected = None;
        for span in capture.path.spans() {
            let context = crate::syntax::token_wire::Key(span.context.0);
            let class = (self.facts.name_item(context)?, self.facts.item(context)?);
            if expected.is_some_and(|prior| prior != class) {
                return Ok(super::macro_names::PathLookup::NeedInput(invocation));
            }
            expected = Some(class);
        }
        let Some(expected) = expected else {
            return Ok(super::macro_names::PathLookup::NeedInput(invocation));
        };
        let caller = self.producer(invocation)?;
        let consumer_context =
            self.facts.context(expected.1)?.defining_unit.0 == caller.scope.crate_key.0;
        let outers = self.outer_scopes()?;
        super::declaration_imports::DeclarationImportIndex::prepared(
            self.output.scan(),
            &self.imports,
        )?
        .lookup_with_textual(
            invocation,
            segments,
            absolute,
            names,
            |row| {
                Ok(self
                    .import_contexts
                    .get(&row.source.fingerprint())
                    .is_some_and(|class| *class == Some(expected))
                    && (consumer_context || row.target.is_some()))
            },
            |row, name| {
                let found = declaration_import_reference(
                    self.output.scan(),
                    row,
                    name,
                    None,
                    Some(self.origin),
                    &self.scopes,
                    &self.declarations,
                    &outers,
                    &self.facts,
                    &self.ancestry,
                )?;
                if !consumer_context
                    && matches!(
                        found,
                        super::macro_scopes::HygienicLookup::Selection(
                            super::macro_origins::MacroSelection::Absent
                        )
                    )
                {
                    Ok(super::macro_scopes::HygienicLookup::NeedInput(
                        row.source.identity,
                    ))
                } else {
                    Ok(found)
                }
            },
        )
    }
    pub fn lookup(
        &self,
        invocation: crate::core::digest::Digest,
    ) -> Result<super::macro_scopes::HygienicLookup, Diagnostic> {
        use crate::syntax::declaration_scan::DeclarationProducerKind;
        let producer = self.producer(invocation)?;
        if producer.kind != DeclarationProducerKind::Macro || producer.path.len() != 1 {
            return Ok(super::macro_scopes::HygienicLookup::NeedInput(invocation));
        }
        let capture = self.output.macro_call(invocation)?;
        let (name, context) =
            crate::expand::source_hygiene::declaration_macro_name(&capture.path, capture.edition)?;
        let outers = self.outer_scopes()?;
        self.declarations.resolve_with_outers(
            &self.scopes,
            super::macro_scopes::MacroReference {
                scope: producer.lexical_scope,
                before: u64::from(producer.range.start),
                name: &name,
                context,
                origin: Some(self.origin),
            },
            &outers,
            &self.facts,
            &self.ancestry,
        )
    }
}

/// Import target lookup uses the captured identifier occurrence, not the alias or call token.
fn declaration_import_reference(
    scan: &DeclarationScan,
    row: &crate::syntax::declaration_scan::ScannedImport,
    spelling: &str,
    original_context: Option<crate::syntax::token_wire::Key>,
    origin: Option<crate::syntax::token_wire::Key>,
    scopes: &crate::expand::scopes::TextualScopes,
    declarations: &super::macro_scopes::MacroDeclarations,
    outers: &[super::macro_scopes::OuterMacroScope<'_>],
    facts: &super::hygiene::HygieneFacts,
    ancestry: &super::macro_origins::ExpansionAncestry,
) -> Result<super::macro_scopes::HygienicLookup, Diagnostic> {
    use crate::syntax::declaration_scan::DeclarationNameContext;
    let pending = || super::macro_scopes::HygienicLookup::NeedInput(row.source.identity);
    let Some(target) = &row.target else {
        return Ok(pending());
    };
    let name = crate::expand::scopes::MacroName::parse(&target.spelling, target.edition)?;
    if name != crate::expand::scopes::MacroName::parse(spelling, target.edition)? {
        return Err(fault());
    }
    let context = match target.context {
        DeclarationNameContext::Captured(context) => context,
        DeclarationNameContext::Original if scan.generated_origin().is_none() => {
            let Some(context) = original_context else {
                return Ok(pending());
            };
            context
        }
        _ => return Ok(pending()),
    };
    declarations.resolve_with_outers(
        scopes,
        super::macro_scopes::MacroReference {
            scope: row.lexical_scope,
            before: u64::from(row.source.range.start),
            name: &name,
            context,
            origin,
        },
        outers,
        facts,
        ancestry,
    )
}
/// Cold, immutable pinned comparison receipts from the admitted tree's actual use leaves.
/// Reused on retries; no import syntax extraction is performed by lookup.
fn declaration_import_contexts(
    output: &crate::expand::generated_declarations::GeneratedDeclarations,
    facts: &super::hygiene::HygieneFacts,
) -> Result<
    std::collections::BTreeMap<
        crate::core::digest::Digest,
        Option<(
            crate::syntax::token_wire::NameContextKey,
            crate::syntax::token_wire::Key,
        )>,
    >,
    Diagnostic,
> {
    let mut result = std::collections::BTreeMap::new();
    let parsed = output.admitted_parsed();
    for row in output.scan().imports() {
        let ranges = crate::syntax::imports::name_ranges(parsed, &row.source)?;
        let mut class = None;
        let mut uniform = true;
        for range in ranges {
            let Some(context) = parsed.identifier_context(range)? else {
                uniform = false;
                continue;
            };
            let current = (facts.name_item(context)?, facts.item(context)?);
            if class.is_some_and(|old| old != current) {
                uniform = false;
            }
            class = Some(current);
        }
        if result
            .insert(row.source.fingerprint(), class.filter(|_| uniform))
            .is_some()
        {
            return Err(fault());
        }
    }
    Ok(result)
}

#[derive(Clone)]
struct DeclarationOutwardMacro {
    order: Vec<u32>,
    name: crate::expand::scopes::MacroName,
    definition: crate::expand::scopes::MacroDefinition,
    context: crate::syntax::token_wire::Key,
    origin: Option<crate::syntax::token_wire::Key>,
    body: Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
}
impl DeclarationOutwardMacro {
    fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        self.name
            .as_str()
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(self.order.capacity().checked_mul(4)?))
            .and_then(|n| n.checked_add(512))
            .ok_or_else(fault)
    }
}
struct DeclarationPublication {
    scopes: crate::expand::scopes::TextualScopes,
    declarations: super::macro_scopes::MacroDeclarations,
    facts: super::hygiene::HygieneFacts,
    ancestry: super::macro_origins::ExpansionAncestry,
    bodies: std::collections::BTreeMap<
        crate::core::digest::Digest,
        Arc<crate::expand::generated_declarations::GeneratedDeclarations>,
    >,
    pending: std::collections::BTreeSet<crate::core::digest::Digest>,
    retained_bytes: usize,
}
impl DeclarationPublication {
    fn adopt(
        &mut self,
        parent: &DeclarationScan,
        expected: &crate::expand::identity::ExpansionKey,
        child: &GeneratedDeclarationMacroIndex,
        complete: bool,
    ) -> Result<(), Diagnostic> {
        if !child.is_complete()
            || child.output.parent_inventory() != parent.fingerprint()
            || child.output.record().key() != expected
            || !self.pending.contains(&child.output.parent_invocation())
        {
            return Err(fault());
        }
        let producer = parent
            .producers()
            .iter()
            .find(|p| p.identity == child.output.parent_invocation())
            .ok_or_else(fault)?;
        self.facts.extend(&child.facts)?;
        self.ancestry.extend(&child.ancestry)?;
        let mut bindings = Vec::new();
        bindings
            .try_reserve_exact(child.outward.len())
            .map_err(|_| fault())?;
        for row in &child.outward {
            self.declarations.register(
                row.definition.clone(),
                row.context,
                row.origin,
                &self.facts,
                &self.ancestry,
            )?;
            if self
                .bodies
                .insert(row.definition.identity, Arc::clone(&row.body))
                .is_some()
            {
                return Err(fault());
            }
            bindings.push((row.name.clone(), row.definition.clone()));
        }
        self.scopes.publish_prefix(
            producer.lexical_scope,
            u64::from(producer.range.start),
            producer.identity,
            bindings,
        )?;
        if complete {
            self.scopes.complete_prefix(
                producer.lexical_scope,
                u64::from(producer.range.start),
                producer.identity,
            )?;
            self.pending.remove(&producer.identity);
        }
        let declaration_bytes = self.declarations.metadata_bytes()?;
        let fact_bytes = self.facts.metadata_bytes()?;
        let ancestry_bytes = self.ancestry.metadata_bytes()?;
        self.retained_bytes = self
            .retained_bytes
            .checked_add(child.output.metadata_bytes()?)
            .and_then(|n| n.checked_add(child.outward.len().checked_mul(512)?))
            .and_then(|n| n.checked_add(declaration_bytes))
            .and_then(|n| n.checked_add(fact_bytes))
            .and_then(|n| n.checked_add(ancestry_bytes))
            .ok_or_else(fault)?;
        Ok(())
    }
}
