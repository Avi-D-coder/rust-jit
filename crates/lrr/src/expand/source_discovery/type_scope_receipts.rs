//! Type facts from the actual discovery graph, nested in its real publication.
//! The enclosing seal keeps its selected namespace; this is never a Type seal.
use super::*;
use crate::{
    core::{digest::StableEncoder, profile::ProfileIdentity},
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        seals::{CertifiedScope, DiscoveryInput},
    },
    expand::record::ExpansionDescriptor,
    source::changes::SourceSelection,
};

#[derive(Debug, Clone)]
pub(crate) struct TypeScopeReceipt {
    parent: Digest,
    universe: Digest,
    profile: ProfileIdentity,
    // Actual published world membership, separate from consumed seal inputs.
    compiler_world: std::sync::Arc<[(Digest, Digest)]>,
    scope: CertifiedScope,
}
fn unavailable() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-CALL-CONTEXT",
        "current generated Type namespace receipt differs or is unavailable".into(),
    )
}
fn universe(
    sources: &[SourceSelection],
    records: &[ExpansionDescriptor],
    compiler: &BTreeMap<InputKey, Digest>,
    dependencies: Digest,
) -> Result<Digest, Diagnostic> {
    if sources.is_empty() {
        return Err(unavailable());
    }
    let mut bytes = 0usize;
    let mut encoder = StableEncoder::new("lrr/v1/discovery-type-scope-universe");
    encoder.digest(dependencies);
    encoder.u64(u64::try_from(sources.len()).map_err(|_| unavailable())?);
    for source in sources {
        bytes = bytes
            .checked_add(source.metadata_bytes()?)
            .ok_or_else(unavailable)?;
        encoder.string(source.path.as_str());
        encoder.record(&source.outline);
    }
    encoder.sequence(records, |e, record| e.record(record));
    encoder.u64(u64::try_from(compiler.len()).map_err(|_| unavailable())?);
    for (key, fingerprint) in compiler {
        bytes = bytes
            .checked_add(128)
            .and_then(|n| n.checked_add(key.heap_bytes()))
            .ok_or_else(unavailable)?;
        encoder.record(key);
        encoder.digest(*fingerprint);
    }
    Ok(encoder.finish())
}
impl TypeScopeReceipt {
    pub(crate) fn scope(&self) -> &CertifiedScope {
        &self.scope
    }
    /// Cold publication validation; querying a retained facet never re-encodes
    /// the parent/source universe or scans the namespace world.
    pub(crate) fn require_publication(&self, parent: Digest) -> Result<(), Diagnostic> {
        self.profile.validate_source()?;
        if self.parent != parent
            || !matches!(self.scope.namespace, Namespace::Type | Namespace::Value)
            || self.scope.scope.owner.is_some()
            || self.scope.edition != self.profile.edition
        {
            return Err(unavailable());
        }
        Ok(())
    }
    pub(crate) fn owner_profile(&self) -> (&ScopeKey, &ProfileIdentity) {
        (&self.scope.scope, &self.profile)
    }
    pub(crate) fn require_owner(
        &self,
        scope: &ScopeKey,
        profile: &ProfileIdentity,
    ) -> Result<(), Diagnostic> {
        if &self.scope.scope != scope || &self.profile != profile {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt owning scope/profile mismatch".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn require_current(
        &self,
        db: &Database,
        publication: &DiscoveryInput,
    ) -> Result<(), Diagnostic> {
        // This immutable searched receipt does not change across candidates.
        let fingerprint = self.fingerprint();
        let members = match self.scope.namespace {
            Namespace::Value => &publication.value_scopes,
            _ => &publication.type_scopes,
        };
        if !members
            .iter()
            .any(|proof| proof.fingerprint() == fingerprint)
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt is not a member of its publication".into(),
            ));
        }
        if self.parent != publication.seal.fingerprint()
            || self.universe
                != universe(
                    &publication.sources,
                    &publication.records,
                    &publication.compiler_inputs,
                    publication.source_dependencies,
                )?
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt parent/source/compiler/record universe differs".into(),
            ));
        }
        if publication.seal.unresolved_frontiers() != 0
            || !matches!(self.scope.namespace, Namespace::Type | Namespace::Value)
            || self.scope.edition != self.profile.edition
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt scope/profile/frontier differs".into(),
            ));
        }
        if !matches!(db.published_input(&InputKey::DiscoverySeal(publication.seal.key())), Some((InputValue::DiscoverySeal(current), _)) if std::ptr::eq(current.as_ref(),publication) || current.fingerprint() == publication.fingerprint())
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt enclosing publication is not current".into(),
            ));
        }
        if !matches!(db.published_input(&InputKey::ExpansionEpoch), Some((InputValue::ExpansionEpoch(epoch), _)) if *epoch == publication.seal.key().epoch)
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt requires its published matching ExpansionEpoch".into(),
            ));
        }
        if !matches!(db.published_input(&InputKey::SourceDependencies), Some((InputValue::SourceDependencies(graph), _)) if graph.fingerprint() == publication.source_dependencies)
        {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt current source dependency graph differs".into(),
            ));
        }
        self.profile.validate_source()?;
        // Map construction is pure and bounded by the existing 32MiB/100k API.
        // Count actual current rows, including additions not read at mint time.
        let stamps = db.lexical_namespace_stamps()?;
        let count = stamps
            .keys()
            .filter(|key| matches!(key, InputKey::CapturedSource(_)))
            .count();
        if count != publication.sources.len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt captured source membership count differs".into(),
            ));
        }
        for key in stamps.keys() {
            match key {
                InputKey::ModuleRecord(_) | InputKey::IncludeRecord(_)
                    if !publication.compiler_inputs.contains_key(key) =>
                {
                    return Err(Diagnostic::new(
                        "LRR-E-CALL-CONTEXT",
                        "Type receipt current compiler/module/include input is not retained".into(),
                    ));
                }
                InputKey::ExpansionRecord(id)
                    if !publication.compiler_inputs.contains_key(key)
                        && !publication
                            .records
                            .iter()
                            .any(|record| record.key.fingerprint() == *id) =>
                {
                    return Err(Diagnostic::new(
                        "LRR-E-CALL-CONTEXT",
                        "Type receipt current expansion record is not retained".into(),
                    ));
                }
                InputKey::GeneratedSyntax(_) => {
                    let Some((InputValue::GeneratedSyntax(unit), _)) = db.published_input(key)
                    else {
                        return Err(Diagnostic::new(
                            "LRR-E-CALL-CONTEXT",
                            "Type receipt current generated syntax input is absent".into(),
                        ));
                    };
                    let record = unit.items.record();
                    if !publication
                        .records
                        .iter()
                        .any(|owned| owned.fingerprint() == record.descriptor().fingerprint())
                        && !publication
                            .compiler_inputs
                            .contains_key(&InputKey::ExpansionRecord(record.key().fingerprint()))
                    {
                        return Err(Diagnostic::new(
                            "LRR-E-CALL-CONTEXT",
                            "Type receipt current generated syntax record is not retained".into(),
                        ));
                    }
                }
                _ => {}
            }
        }
        let environment_count = stamps
            .keys()
            .filter(|key| matches!(key, InputKey::UnitCompileEnv(_)))
            .count();
        if environment_count != self.compiler_world.len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                "Type receipt compiler world membership changed".into(),
            ));
        }
        for (unit, expected) in self.compiler_world.iter() {
            if stamps.get(&InputKey::UnitCompileEnv(*unit)) != Some(expected) {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    "Type receipt compiler world fingerprint changed".into(),
                ));
            }
        }
        let mut locations = BTreeSet::new();
        for source in &publication.sources {
            if !locations.insert(source.location_identity())
                || !matches!(db.published_input(&InputKey::CapturedSource(source.location_identity())), Some((InputValue::CapturedSource(current), _)) if current == source)
                || !matches!(db.published_input(&InputKey::CrateCfg(source.outline.crate_key)), Some((InputValue::CrateCfg(cfg), _)) if cfg.fingerprint() == source.outline.cfg)
            {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    "Type receipt captured source/cfg is no longer current".into(),
                ));
            }
        }
        for (key, expected) in &publication.compiler_inputs {
            if db
                .published_input(key)
                .is_none_or(|(_, current)| current != *expected)
            {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    "Type receipt retained compiler input fingerprint changed".into(),
                ));
            }
        }
        for descriptor in &publication.records {
            if !matches!(db.published_input(&InputKey::ExpansionRecord(descriptor.key.fingerprint())), Some((InputValue::ExpansionRecord(current), _)) if current.descriptor().fingerprint() == descriptor.fingerprint())
            {
                return Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    "Type receipt retained expansion descriptor is no longer current".into(),
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.profile.semantics.capacity())
            .and_then(|n| {
                self.compiler_world
                    .len()
                    .checked_mul(std::mem::size_of::<(Digest, Digest)>())
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_add(self.scope.metadata_bytes().ok()?))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "Type scope receipt metadata overflow".into(),
                )
            })
    }
}
impl StableEncode for TypeScopeReceipt {
    const DOMAIN: &'static str = "lrr/v2/discovery-type-scope-receipt/compiler-world-v1";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.parent);
        e.digest(self.universe);
        e.record(&self.profile);
        e.sequence(&self.compiler_world, |e, (unit, fingerprint)| {
            e.digest(*unit);
            e.digest(*fingerprint);
        });
        e.record(&self.scope);
    }
}
impl SourceDiscovery {
    pub(super) fn type_scope_receipts(
        &self,
        db: &Database,
        sources: &[SourceSelection],
        records: &[ExpansionDescriptor],
        derived: &BTreeMap<InputKey, InputValue>,
    ) -> Result<Vec<TypeScopeReceipt>, Diagnostic> {
        self.scope_receipts(db, sources, records, derived, Namespace::Type)
    }
    /// Value facets of the other module scopes this Value fixed point
    /// completed (rustc resolves `m::C` in module `m`'s own Value namespace).
    /// Only a Value discovery establishes Value facts; the primary scope keeps
    /// its own certificate. Unresolved scopes stay explicitly unavailable.
    pub(super) fn value_scope_receipts(
        &self,
        db: &Database,
        sources: &[SourceSelection],
        records: &[ExpansionDescriptor],
        derived: &BTreeMap<InputKey, InputValue>,
    ) -> Result<Vec<TypeScopeReceipt>, Diagnostic> {
        if self.namespace != Namespace::Value {
            return Ok(Vec::new());
        }
        self.scope_receipts(db, sources, records, derived, Namespace::Value)
    }
    fn scope_receipts(
        &self,
        db: &Database,
        sources: &[SourceSelection],
        records: &[ExpansionDescriptor],
        derived: &BTreeMap<InputKey, InputValue>,
        namespace: Namespace,
    ) -> Result<Vec<TypeScopeReceipt>, Diagnostic> {
        let _span = crate::core::timing::Span::enter(|| "discovery type_scope_receipts".into());
        let seal = self.seal.as_ref().ok_or_else(unavailable)?;
        // A configured impl catalog closes this same graph and can retain its
        // actual Type facets. Its private source-world completion is required
        // at publication; the parent remains an ImplCatalog, never a Type seal.
        let supported = matches!(seal.domain(), crate::db::seals::SealDomain::Scope { .. })
            || namespace == Namespace::Type
                && matches!(seal.domain(), crate::db::seals::SealDomain::ImplCatalog { .. })
                && self.impl_catalog_plan.is_some();
        if !supported {
            return Ok(Vec::new());
        }
        // A Value fixed point is not a Type fixed point. Retain no positive
        // receipt if the actual ordinary import frontier is unestablished.
        if !self.imports.pending.is_empty()
            || !self.imports.missing.is_empty()
            || !self.imports.inaccessible.is_empty()
            || !self.imports.ambiguous.is_empty()
        {
            return Ok(Vec::new());
        }
        if sources.is_empty() {
            // This optional bounded facet cannot establish that larger universe.
            // Preserve ordinary publication; reached consumers demand the facet.
            return Ok(Vec::new());
        }
        let universe = universe(
            sources,
            records,
            &self.compiler_inputs,
            self.dependencies.fingerprint(),
        )?;
        // Capture actual published compiler world rows at the explicit
        // coordinator boundary, including genuinely prepared but unused envs.
        db.require_coordinator()?;
        let stamps = db.lexical_namespace_stamps()?;
        let count = stamps
            .keys()
            .filter(|key| matches!(key, InputKey::UnitCompileEnv(_)))
            .count();
        let mut compiler_world = Vec::new();
        compiler_world
            .try_reserve_exact(count)
            .map_err(|_| crate::runtime::arena::resource())?;
        for (key, fingerprint) in stamps.iter().map(|(key, stamp)| (key.clone(), *stamp)) {
            if let InputKey::UnitCompileEnv(unit) = key {
                // A configured dependency world (for example a macro-export SDK
                // discovery) can coexist with its consumer's environment rows.
                // This optional facet then cannot describe that wider world.
                if !sources.iter().any(|source| {
                    source.outline.crate_key == unit && source.outline.module_path.is_empty()
                }) {
                    return Ok(Vec::new());
                }
                if !matches!(db.published_input(&InputKey::UnitCompileEnv(unit)), Some((InputValue::UnitCompileEnv(environment), current)) if environment.unit() == unit && current == fingerprint)
                {
                    return Err(Diagnostic::new(
                        "LRR-E-CALL-CONTEXT",
                        "Type receipt compiler world has no current captured owning root".into(),
                    ));
                }
                compiler_world.push((unit, fingerprint));
            }
        }
        let compiler_world: std::sync::Arc<[(Digest, Digest)]> = compiler_world.into();
        let mut owners = BTreeMap::<ScopeKey, BTreeSet<ProfileIdentity>>::new();
        let mut owner_count = 0usize;
        let mut owner_bytes = 0usize;
        let mut insert = |scope: &ScopeKey, profile: &ProfileIdentity| -> Result<(), Diagnostic> {
            if scope.owner.is_some() {
                return Ok(());
            }
            if owners
                .get(scope)
                .is_some_and(|profiles| profiles.contains(profile))
            {
                return Ok(());
            }
            owner_bytes = owner_bytes
                .checked_add(128)
                .and_then(|n| n.checked_add(scope.heap_bytes()))
                .and_then(|n| n.checked_add(profile.semantics.capacity()))
                .ok_or_else(unavailable)?;
            owners
                .entry(scope.clone())
                .or_default()
                .insert(profile.clone());
            owner_count += 1;
            Ok(())
        };
        for source in self.sources.iter().chain(
            self.staged_modules
                .values()
                .map(|module| module.syntax.prepared_source()),
        ) {
            for scope in source.outline().scopes.keys() {
                insert(scope, &source.outline().profile)?;
            }
        }
        for (key, value) in derived {
            if let (InputKey::DeclarationContext(id), InputValue::DeclarationContext(context)) =
                (key, value)
                && let Some(InputValue::DefinitionHeader(header)) =
                    derived.get(&InputKey::DefinitionHeader(*id))
            {
                insert(&context.scope, &header.profile)?;
            }
        }
        let mut receipts = Vec::new();
        let mut bytes = 0usize;
        // One exact immutable seal is the parent of every receipt in this batch.
        let parent = seal.fingerprint();
        // Receipts are a pure function of these facts and the (unchanged)
        // graph state; a republication of the same discovery reuses them.
        let memo_key = {
            let mut e = StableEncoder::new("lrr/v1/scope-receipt-batch");
            e.u32(namespace.tag());
            e.digest(parent);
            e.digest(universe);
            e.sequence(&compiler_world, |e, (unit, fingerprint)| {
                e.digest(*unit);
                e.digest(*fingerprint);
            });
            e.record(&self.scope);
            e.u64(self.graph.mutation_generation());
            e.sequence(&owners.iter().collect::<Vec<_>>(), |e, (scope, profiles)| {
                e.record(*scope);
                e.sequence(&profiles.iter().collect::<Vec<_>>(), |e, profile| e.record(*profile));
            });
            e.finish()
        };
        if let Ok(memo) = self.receipt_memo.lock()
            && let Some((key, receipts)) = memo.get(&namespace.tag())
            && *key == memo_key
        {
            return Ok(receipts.clone());
        }
        // Each observed profile is a separate lookup identity. Different
        // owning editions in one lexical module never erase one another.
        // One immutable graph certifies every scope of this batch.
        let mut certification = crate::frontend::names::CertificationBatch::default();
        for (scope, profiles) in owners {
            // The primary Value scope is certified by the seal itself.
            if namespace == Namespace::Value && scope == self.scope {
                continue;
            }
            for profile in profiles {
                let certified = match self.graph.certified_scope_in(
                    &mut certification,
                    &scope,
                    namespace,
                    profile.edition,
                ) {
                        Ok(scope) => scope,
                        // This is an explicit unavailable prerequisite, never absence.
                        Err(error) if error.code == "LRR-E-CATALOG-INCOMPLETE" => continue,
                        Err(error) => return Err(error),
                    };
                let proof = TypeScopeReceipt {
                    parent,
                    universe,
                    profile,
                    compiler_world: compiler_world.clone(),
                    scope: certified,
                };
                bytes = bytes
                    .checked_add(proof.metadata_bytes()?)
                    .and_then(|n| n.checked_add(128))
                    .ok_or_else(unavailable)?;
                receipts
                    .try_reserve(1)
                    .map_err(|_| crate::runtime::arena::resource())?;
                receipts.push(proof);
            }
        }
        if let Ok(mut memo) = self.receipt_memo.lock() {
            memo.insert(namespace.tag(), (memo_key, receipts.clone()));
        }
        Ok(receipts)
    }
}

#[cfg(test)]
#[path = "type_scope_receipt_encoding_tests.rs"]
mod encoding_tests;
