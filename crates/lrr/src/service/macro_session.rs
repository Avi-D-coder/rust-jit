//! Prepared external macro execution. Pure query providers never call this module.
use super::expansion::{DemandContext, DemandToken, ExpansionCoordinator, Reply, StartDecision};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::poll::InputRequest,
    expand::{
        identity::{GrammarSlot, MacroIdentity, ProceduralKind},
        record::ExpansionRecord,
    },
    project::{
        cargo_units::UnitRole,
        tool_manifest::{ArtifactKind, ToolArtifact, ToolManifest},
    },
};
use lrr_expand_adapter::{
    AdapterError,
    proc_client::{
        MacroArtifact, ProcForm, ProcGrammar, ProcInvocation, ProcSession, ServerIdentity,
        SessionConfig,
    },
    token_wire::Key,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
mod failure;
pub use failure::{MacroFaultPhase, MacroFaultReport};

fn fault(code: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
fn adapter(error: AdapterError) -> Diagnostic {
    fault(error.code, error.message)
}
pub fn macro_artifact_identity(unit: Digest, artifact: &ToolArtifact) -> Digest {
    let mut encoder = StableEncoder::new("lrr/v2/procedural-artifact");
    encoder.digest(unit);
    encoder.digest(artifact.content);
    encoder.finish()
}
pub fn procedural_engine_version() -> Digest {
    let mut encoder = StableEncoder::new("lrr/v2/procedural-engine");
    encoder.string(env!("CARGO_PKG_VERSION"));
    encoder.string(lrr_expand_adapter::UPSTREAM_COMMIT);
    encoder.u32(6);
    encoder.string("api6/bidirectional-postcard/captured-spans-v1/context-labels-v1/sites-v2/producer-roots-v1");
    encoder.finish()
}
fn raw_hash(path: &Path, expected: Digest) -> Result<String, Diagnostic> {
    let mut file =
        File::open(path).map_err(|e| fault("LRR-E-TOOL-PREPARE-STALE", e.to_string()))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .read_to_end(&mut bytes)
        .map_err(|e| fault("LRR-E-TOOL-PREPARE-STALE", e.to_string()))?;
    if Digest::of_bytes(&bytes) != expected {
        return Err(fault(
            "LRR-E-TOOL-PREPARE-STALE",
            "prepared tool content changed",
        ));
    }
    Ok(blake3::hash(&bytes).to_hex().to_string())
}
/// Binds tokens, contexts and immutable span callback inputs; no ambient environment.
pub fn procedural_input_fingerprint(request: &ProcInvocation) -> Result<Digest, Diagnostic> {
    let mut encoder = StableEncoder::new("lrr/v2/procedural-input-tokens/sites-v1");
    for key in [
        request.scope,
        request.parent_origin,
        request.definition_site,
        request.call_site,
        request.mixed_site,
    ] {
        encoder.bytes(&key.0);
    }
    encoder.bytes(&request.input.encode().map_err(adapter)?);
    match &request.attributes {
        None => encoder.u8(0),
        Some(tokens) => {
            encoder.u8(1);
            encoder.bytes(&tokens.encode().map_err(adapter)?);
        }
    }
    encoder.u64(request.sources.len() as u64);
    for source in &request.sources {
        encoder.bytes(&source.source().unit.0);
        encoder.bytes(&source.source().content.0);
        encoder.string(&source.source().path);
        encoder.u8(match source.encoding() {
            lrr_expand_adapter::proc_client::SourceDigestEncoding::RawBlake3 => 0,
            lrr_expand_adapter::proc_client::SourceDigestEncoding::LrrSourceBytes => 1,
        });
        match source.local_path() {
            None => encoder.u8(0),
            Some(path) => {
                encoder.u8(1);
                encoder.string(path);
            }
        }
    }
    Ok(encoder.finish())
}

pub struct MacroSession {
    context: DemandContext,
    session: ProcSession,
    primary_failure: Option<MacroFaultReport>,
    preparation: Digest,
    macro_units: BTreeSet<Digest>,
    artifact_editions: BTreeMap<Digest, u16>,
    loaded_exports: BTreeSet<(Key, String, ProcForm)>,
    catalog: crate::frontend::procedural_exports::PreparedProcCatalog,
    source_derives: BTreeMap<
        (Digest, String),
        (
            Digest,
            crate::expand::derive_declaration::SourceDeriveDeclaration,
        ),
    >,
    definition_sites: BTreeMap<Digest, std::sync::Arc<SourceDefinitionSite>>,
    host_declaration_expansions: u64,
}
pub(crate) use crate::frontend::procedural_exports::PreparedDefinitionSite as SourceDefinitionSite;
impl MacroSession {
    /// Caller supplies an immutable schema4 preparation; this never runs builds.
    pub fn open(
        context: DemandContext,
        manifest: &ToolManifest,
        allow_macro_execution: bool,
        directory: PathBuf,
        timeout: Duration,
        output_limit: usize,
    ) -> Result<Self, Diagnostic> {
        Self::open_bounded(
            context,
            manifest,
            allow_macro_execution,
            directory,
            timeout,
            output_limit,
            None,
        )
    }
    pub(crate) fn open_with_deadline(
        context: DemandContext,
        manifest: &ToolManifest,
        allow_macro_execution: bool,
        directory: PathBuf,
        deadline: std::time::Instant,
        output_limit: usize,
    ) -> Result<Self, Diagnostic> {
        Self::open_bounded(
            context,
            manifest,
            allow_macro_execution,
            directory,
            lrr_expand_adapter::NO_TIME_LIMIT,
            output_limit,
            Some(deadline),
        )
    }
    fn open_bounded(
        context: DemandContext,
        manifest: &ToolManifest,
        allow_macro_execution: bool,
        directory: PathBuf,
        timeout: Duration,
        output_limit: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<Self, Diagnostic> {
        if !allow_macro_execution {
            return Err(fault(
                "LRR-E-MACRO-PERMISSION",
                "native macro execution requires separate authority",
            ));
        }
        manifest.validate()?;
        let unit = manifest
            .graph
            .units
            .get(&context.unit)
            .ok_or_else(|| fault("LRR-E-UNIT-GRAPH", "macro consuming unit absent"))?;
        if !matches!(
            unit.key.role,
            UnitRole::Live | UnitRole::Native | UnitRole::MacroExport
        ) {
            return Err(fault(
                "LRR-E-UNIT-GRAPH",
                "macro consumer must be a source runtime unit",
            ));
        }
        let environment = manifest
            .compile_env
            .get(&context.unit)
            .filter(|env| env.fingerprint() == context.compile_environment)
            .ok_or_else(|| {
                fault(
                    "LRR-E-TOOL-PREPARE-STALE",
                    "macro compile environment differs from frozen request",
                )
            })?;
        let mut visited = BTreeSet::from([context.unit]);
        let mut pending = vec![context.unit];
        let mut artifacts = vec![];
        let mut macro_units = BTreeSet::new();
        let mut artifact_editions = BTreeMap::new();
        let mut artifact_units = BTreeMap::new();
        while let Some(id) = pending.pop() {
            let unit = manifest
                .graph
                .units
                .get(&id)
                .ok_or_else(|| fault("LRR-E-UNIT-GRAPH", "macro dependency unit absent"))?;
            if unit.key.role == UnitRole::ProcMacroHost {
                macro_units.insert(id);
                let artifact = manifest
                    .artifacts
                    .get(&id)
                    .filter(|a| a.kind == ArtifactKind::ProcMacroLibrary)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-MACRO-SERVER-MISMATCH",
                            "exact Cargo proc artifact is not prepared",
                        )
                    })?;
                artifact_editions.insert(macro_artifact_identity(id, artifact), unit.key.edition);
                artifact_units.insert(macro_artifact_identity(id, artifact), id);
                artifacts.push(MacroArtifact {
                    identity: Key(macro_artifact_identity(id, artifact).0),
                    path: PathBuf::from(&artifact.path),
                    raw_blake3: raw_hash(Path::new(&artifact.path), artifact.content)?,
                });
            } else if matches!(
                unit.key.role,
                UnitRole::Live | UnitRole::Native | UnitRole::MacroExport
            ) {
                for dependency in &unit.dependencies {
                    if visited.insert(dependency.target) {
                        pending.push(dependency.target);
                    }
                }
            }
        }
        let mut env: BTreeMap<_, _> = environment.cargo.clone();
        env.extend(
            environment.declared.iter().filter_map(|(key, value)| {
                value.as_ref().map(|value| (key.clone(), value.clone()))
            }),
        );
        env.extend(environment.rustc_env.clone());
        if let Some(out_dir) = &environment.out_dir {
            env.insert("OUT_DIR".into(), out_dir.clone());
        }
        let server = &manifest.toolchain.macro_server;
        let config = SessionConfig {
            allow_macro_execution: true,
            required_span_operations:
                lrr_expand_adapter::proc_client::ProcSpanOperation::stable_set(),
            unit: Key(context.unit.0),
            epoch: context.epoch,
            server: ServerIdentity {
                path: PathBuf::from(&server.path),
                raw_blake3: raw_hash(Path::new(&server.path), server.content)?,
                compiler_commit: manifest.toolchain.compiler_commit.clone(),
                protocol: manifest.toolchain.macro_protocol,
            },
            artifacts,
            environment: env,
            directory,
            timeout,
            wire_limit: usize::MAX,
            output_limit,
        };
        let session = match deadline {
            Some(deadline) => ProcSession::new_with_deadline(config, deadline),
            None => ProcSession::new(config),
        }
        .map_err(adapter)?;
        let loaded_exports: BTreeSet<_> = session
            .exports()
            .map(|export| (export.artifact, export.name.to_owned(), export.form))
            .collect();
        let mut rows = Vec::new();
        for (artifact, name, form) in &loaded_exports {
            let identity = Digest(artifact.0);
            let producer = *artifact_units
                .get(&identity)
                .ok_or_else(|| fault("LRR-E-MACRO-SERVER-MISMATCH", "macro producer absent"))?;
            let edition = *artifact_editions
                .get(&identity)
                .ok_or_else(|| fault("LRR-E-MACRO-SERVER-MISMATCH", "macro edition absent"))?;
            let kind = match form {
                ProcForm::FunctionLike => ProceduralKind::Function,
                ProcForm::Attribute => ProceduralKind::Attribute,
                ProcForm::Derive => ProceduralKind::Derive,
            };
            rows.push((identity, producer, edition, name.as_str(), kind));
        }
        let mut aliases = BTreeMap::new();
        for edge in &manifest.graph.units[&context.unit].dependencies {
            if artifact_units.values().any(|unit| *unit == edge.target) {
                let alias = crate::expand::scopes::MacroName::parse(
                    &format!("r#{}", edge.alias),
                    manifest.graph.units[&context.unit].key.edition,
                )?;
                if aliases.insert(alias, edge.target).is_some() {
                    return Err(fault("LRR-E-MACRO-SCOPE", "duplicate prepared macro alias"));
                }
            }
        }
        let binding = crate::frontend::procedural_exports::PreparedProcBinding {
            consumer: context.unit,
            graph: manifest.graph.fingerprint(),
            epoch: context.epoch,
            environment: context.compile_environment,
            preparation: manifest.fingerprint(),
            aliases,
        };
        let catalog =
            crate::frontend::procedural_exports::PreparedProcCatalog::from_exports(rows, binding)?;
        let mut result = Self {
            context,
            session,
            primary_failure: None,
            preparation: manifest.fingerprint(),
            macro_units,
            artifact_editions,
            loaded_exports,
            catalog,
            source_derives: BTreeMap::new(),
            definition_sites: BTreeMap::new(),
            host_declaration_expansions: 0,
        };
        // Capture loaded producer roots in this explicit, authorized startup
        // path. Pure generated request construction receives immutable facts.
        let producers: Vec<_> = result.macro_units.iter().copied().collect();
        for producer in producers {
            if deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
                return Err(fault(
                    "LRR-E-MACRO-TIMEOUT",
                    "producer source capture startup deadline expired",
                ));
            }
            let blob = super::derive_declarations::captured_root(manifest, producer)?;
            result.retain_definition_site(manifest, producer, &blob)?;
        }
        result
            .catalog
            .bind_definition_sites(result.definition_sites.clone())?;
        Ok(result)
    }

    /// Owned compiled macro-export facets for pure source namespace construction.
    /// Reads frozen inventory only, including after a cached macro fault.
    pub fn export_catalog(
        &self,
        current: &DemandContext,
    ) -> Result<crate::frontend::procedural_exports::PreparedProcCatalog, Diagnostic> {
        if current != &self.context {
            return Err(fault(
                "LRR-E-MACRO-SCOPE",
                "catalog consumer differs from frozen session",
            ));
        }
        Ok(self.catalog.clone())
    }

    /// Bind a source-resolved request to the exact loaded export and frozen consumer.
    /// Construction reads retained facts only and grants no execution authority.
    pub fn request_key(
        &self,
        current: &DemandContext,
        request: &ProcInvocation,
    ) -> Result<crate::expand::identity::ExpansionKey, Diagnostic> {
        if current != &self.context
            || request.unit != Key(current.unit.0)
            || request.epoch != current.epoch
        {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "macro request differs from frozen consumer",
            ));
        }
        request
            .input
            .validate()
            .map_err(|e| fault("LRR-E-MACRO-OUTPUT", e.to_string()))?;
        if !request.form.accepts_grammar(request.grammar) {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "macro form does not support the captured grammar",
            ));
        }
        if !self.loaded_exports.iter().any(|(artifact, name, form)| {
            *artifact == request.artifact && name == &request.export && *form == request.form
        }) {
            return Err(fault(
                "LRR-E-MACRO-NOT-FOUND",
                "request export absent from exact loaded artifact",
            ));
        }
        let site = |key| {
            request
                .input
                .spans()
                .iter()
                .find(|s| s.key() == key)
                .ok_or_else(|| fault("LRR-E-MACRO-OUTPUT", "macro site span is not registered"))
        };
        let call = site(request.call_site)?;
        let definition = site(request.definition_site)?;
        site(request.mixed_site)?;
        let definition_edition = *self
            .artifact_editions
            .get(&Digest(request.artifact.0))
            .ok_or_else(|| fault("LRR-E-MACRO-NOT-FOUND", "macro producer edition absent"))?;
        if edition(definition.edition) != definition_edition {
            return Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                "definition site edition differs from compiled macro producer",
            ));
        }
        let kind = match request.form {
            ProcForm::FunctionLike => ProceduralKind::Function,
            ProcForm::Attribute => ProceduralKind::Attribute,
            ProcForm::Derive => ProceduralKind::Derive,
        };
        let grammar = match request.grammar {
            ProcGrammar::Items => GrammarSlot::Items,
            ProcGrammar::Expression => GrammarSlot::Expression,
            ProcGrammar::Pattern => GrammarSlot::Pattern,
            ProcGrammar::Type => GrammarSlot::Type,
            ProcGrammar::TraitItems => GrammarSlot::TraitItems,
            ProcGrammar::ImplItems => GrammarSlot::ImplItems,
            ProcGrammar::Statements => GrammarSlot::Statements,
        };
        let key = crate::expand::identity::ExpansionKey {
            macro_identity: MacroIdentity::Procedural {
                artifact: Digest(request.artifact.0),
                export: request.export.clone(),
                kind,
            },
            invocation: Digest(request.identity.0),
            input_tokens: procedural_input_fingerprint(request)?,
            consumer_compile_env: current.compile_environment,
            parent_context: Digest(call.context.0),
            definition_edition,
            call_edition: edition(call.edition),
            grammar,
            engine_version: procedural_engine_version(),
            procedural_epoch: Some(current.epoch),
        };
        key.validate()?;
        Ok(key)
    }
    /// Borrow retained exact export facts for this frozen consuming unit only.
    pub fn exports(
        &self,
        current: &DemandContext,
    ) -> Result<impl Iterator<Item = lrr_expand_adapter::proc_client::ProcExportRef<'_>>, Diagnostic>
    {
        if current != &self.context {
            return Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                "stale macro export inventory context",
            ));
        }
        Ok(self.session.exports())
    }
    /// Admit an original-source declaration from the exact immutable macro-host preparation.
    /// This is source-facet processing, not native body analysis or metadata decoding.
    pub fn register_source_derive(
        &mut self,
        current: &DemandContext,
        manifest: &ToolManifest,
        producer: Digest,
        source_path: &Path,
        blob: &crate::source::snapshot::SourceBlob,
        selected: crate::syntax::spans::SourceRange,
    ) -> Result<Digest, Diagnostic> {
        if current != &self.context || manifest.fingerprint() != self.preparation {
            return Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "source facet preparation/context differs",
            ));
        }
        if !self.macro_units.contains(&producer) {
            return Err(fault(
                "LRR-E-MACRO-NOT-FOUND",
                "macro producer was not loaded for this consuming unit",
            ));
        }
        let unit = manifest
            .graph
            .units
            .get(&producer)
            .filter(|u| u.key.role == UnitRole::ProcMacroHost)
            .ok_or_else(|| {
                fault(
                    "LRR-E-UNIT-GRAPH",
                    "source derive requires the exact proc macro host unit",
                )
            })?;
        let artifact = manifest
            .artifacts
            .get(&producer)
            .filter(|a| a.kind == ArtifactKind::ProcMacroLibrary)
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "source derive artifact absent",
                )
            })?;
        let environment = manifest.compile_env.get(&producer).ok_or_else(|| {
            fault(
                "LRR-E-UNIT-GRAPH",
                "source derive compile environment absent",
            )
        })?;
        let path = source_path
            .to_str()
            .ok_or_else(|| fault("LRR-E-TOOL-PREPARE-STALE", "source facet path is not UTF-8"))?;
        let package = environment
            .cargo
            .get("CARGO_MANIFEST_DIR")
            .ok_or_else(|| fault("LRR-E-UNIT-GRAPH", "macro host source root absent"))?;
        if !source_path.is_absolute()
            || path != unit.source_path
            || !source_path.starts_with(package)
            || manifest.inputs.source_files.get(path) != Some(&Some(blob.content()))
        {
            return Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "source facet bytes/path are not captured for this macro unit",
            ));
        }
        let parsed = crate::syntax::parse::ParsedFile::parse_macro_facet(blob, unit.key.edition)?;
        let declaration = parsed.source_derive_declaration_at(selected, &environment.cfg)?;
        let artifact_id = macro_artifact_identity(producer, artifact);
        if !self.session.exports().any(|e| {
            e.artifact == Key(artifact_id.0)
                && e.name == declaration.names().export()
                && e.form == ProcForm::Derive
        }) {
            return Err(fault(
                "LRR-E-MACRO-NOT-FOUND",
                "source declaration differs from exact loaded derive export",
            ));
        }
        let mut encoder = StableEncoder::new("lrr/v2/prepared-source-derive-binding");
        encoder.digest(self.preparation);
        encoder.digest(producer);
        encoder.digest(artifact_id);
        encoder.record(&declaration);
        let identity = encoder.finish();
        let entry = (artifact_id, declaration.names().export().to_owned());
        if let Some((existing, _)) = self.source_derives.get(&entry) {
            if *existing != identity {
                return Err(fault(
                    "LRR-E-MACRO-SCOPE",
                    "conflicting source declarations for loaded derive",
                ));
            }
            return Ok(identity);
        }
        let mut bytes = 0usize;
        for d in self
            .source_derives
            .values()
            .map(|(_, d)| d)
            .chain(std::iter::once(&declaration))
        {
            let storage = d.names().allocated_bytes()?;
            bytes = bytes
                .checked_add(512)
                .and_then(|n| n.checked_add(d.names().export().len()))
                .and_then(|n| n.checked_add(storage))
                .ok_or_else(|| {
                    fault("LRR-E-METADATA-LIMIT", "source derive accounting overflow")
                })?;
        }
        self.retain_definition_site(manifest, producer, blob)?;
        self.source_derives.insert(entry, (identity, declaration));
        Ok(identity)
    }
    /// Atomically admit a complete original-source catalog, or demand its producers.
    pub fn register_source_derive_catalog(
        &mut self,
        current: &DemandContext,
        manifest: &ToolManifest,
        producer: Digest,
        source_path: &Path,
        blob: &crate::source::snapshot::SourceBlob,
    ) -> Result<crate::expand::derive_declaration::SourceCatalogAdmission, Diagnostic> {
        use crate::expand::derive_declaration::SourceCatalogAdmission;
        if current != &self.context || manifest.fingerprint() != self.preparation {
            return Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "source catalog preparation/context differs",
            ));
        }
        if !self.macro_units.contains(&producer) {
            return Err(fault(
                "LRR-E-MACRO-NOT-FOUND",
                "macro producer was not loaded for this consuming unit",
            ));
        }
        let unit = manifest
            .graph
            .units
            .get(&producer)
            .filter(|u| u.key.role == UnitRole::ProcMacroHost)
            .ok_or_else(|| {
                fault(
                    "LRR-E-UNIT-GRAPH",
                    "source catalog requires exact macro host producer",
                )
            })?;
        let environment = manifest.compile_env.get(&producer).ok_or_else(|| {
            fault(
                "LRR-E-UNIT-GRAPH",
                "source catalog compile environment absent",
            )
        })?;
        let artifact = manifest
            .artifacts
            .get(&producer)
            .filter(|a| a.kind == ArtifactKind::ProcMacroLibrary)
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "source catalog artifact absent",
                )
            })?;
        let package = environment
            .cargo
            .get("CARGO_MANIFEST_DIR")
            .ok_or_else(|| fault("LRR-E-UNIT-GRAPH", "source catalog package root absent"))?;
        let path = source_path.to_str().ok_or_else(|| {
            fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "source catalog path is not UTF-8",
            )
        })?;
        if !source_path.is_absolute()
            || path != unit.source_path
            || !source_path.starts_with(package)
            || manifest.inputs.source_files.get(path) != Some(&Some(blob.content()))
        {
            return Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "source catalog bytes/path not captured for producer",
            ));
        }
        let parsed = crate::syntax::parse::ParsedFile::parse_macro_facet(blob, unit.key.edition)?;
        let catalog = parsed.source_derive_catalog(&environment.cfg)?;
        if !catalog.pending.is_empty() {
            return Ok(SourceCatalogAdmission::NeedExpansion(catalog.pending));
        }
        let artifact_id = macro_artifact_identity(producer, artifact);
        let declared: BTreeSet<_> = catalog
            .declarations
            .iter()
            .map(|(_, d)| d.names().export())
            .collect();
        let loaded: BTreeSet<_> = self
            .session
            .exports()
            .filter(|e| e.artifact == Key(artifact_id.0) && e.form == ProcForm::Derive)
            .map(|e| e.name)
            .collect();
        let missing: Vec<_> = loaded
            .difference(&declared)
            .map(|s| (*s).to_owned())
            .collect();
        if !missing.is_empty() {
            return Ok(SourceCatalogAdmission::NeedSource(missing));
        }
        let previous = self.source_derives.clone();
        let mut admitted = Vec::new();
        for (range, _) in catalog.declarations {
            match self.register_source_derive(current, manifest, producer, source_path, blob, range)
            {
                Ok(id) => admitted.push(id),
                Err(e) => {
                    self.source_derives = previous;
                    return Err(e);
                }
            }
        }
        Ok(SourceCatalogAdmission::Ready(admitted))
    }
    pub(crate) fn prepared_derive_declaration(
        &self,
        current: &DemandContext,
        request: &crate::frontend::generated_derive_exports::DeriveDeclarationRequest,
    ) -> Result<crate::service::derive_declarations::PreparedDeriveDeclaration, Diagnostic> {
        if current != &self.context || self.catalog.fingerprint() != request.catalog {
            return Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                "derive declaration request differs from frozen session",
            ));
        }
        let selected = self
            .catalog
            .select(&request.definition, ProceduralKind::Derive)?
            .filter(|export| export.identity() == &request.identity)
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-NOT-FOUND",
                    "derive declaration export differs from loaded catalog",
                )
            })?;
        let MacroIdentity::Procedural {
            artifact, export, ..
        } = selected.identity()
        else {
            return Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                "derive declaration requires a procedural export",
            ));
        };
        let (identity, declaration) = self
            .source_derives
            .get(&(*artifact, export.clone()))
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "exact helper source declaration unavailable (E-M010-001)",
                )
            })?;
        if declaration.edition != request.definition.definition_edition {
            return Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                "derive source declaration edition differs",
            ));
        }
        Ok(
            crate::service::derive_declarations::PreparedDeriveDeclaration::new(
                request,
                *identity,
                declaration.names().helpers().iter().cloned().collect(),
            ),
        )
    }
    pub fn derive_helper_binding(
        &self,
        current: &DemandContext,
        key: &crate::expand::identity::ExpansionKey,
    ) -> Result<crate::expand::derive::DeriveHelperBinding, Diagnostic> {
        let names = self.derive_helpers(current, key)?;
        let MacroIdentity::Procedural {
            artifact, export, ..
        } = &key.macro_identity
        else {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "helper binding requires procedural identity",
            ));
        };
        let (declaration, _) = self
            .source_derives
            .get(&(*artifact, export.clone()))
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "helper declaration source facet unavailable",
                )
            })?;
        crate::expand::derive::DeriveHelperBinding::new(key.clone(), *declaration, names)
    }
    /// A missing protocol field cannot certify an empty helper declaration.
    /// Required metadata fails before a procedural invocation; no source-spelling fallback.
    pub fn derive_helpers(
        &self,
        current: &DemandContext,
        key: &crate::expand::identity::ExpansionKey,
    ) -> Result<Vec<String>, Diagnostic> {
        key.validate()?;
        if current != &self.context
            || key.consumer_compile_env != current.compile_environment
            || key.procedural_epoch != Some(current.epoch)
            || key.grammar != GrammarSlot::Items
        {
            return Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                "derive helper demand differs from frozen context",
            ));
        }
        let MacroIdentity::Procedural {
            artifact,
            export,
            kind: ProceduralKind::Derive,
        } = &key.macro_identity
        else {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "helper metadata requires an exact derive export",
            ));
        };
        let entry = self
            .session
            .exports()
            .find(|e| {
                e.artifact == Key(artifact.0) && e.name == export && e.form == ProcForm::Derive
            })
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-NOT-FOUND",
                    "derive export absent from exact loaded artifact",
                )
            })?;
        if let Some((_, declaration)) = self.source_derives.get(&(*artifact, export.clone())) {
            if key.definition_edition != declaration.edition {
                return Err(fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "derive declaration edition differs from exact source facet",
                ));
            }
            return Ok(declaration.names().helpers().iter().cloned().collect());
        }
        match entry.helpers {
            lrr_expand_adapter::proc_client::DeriveHelperMetadata::Unavailable { protocol } => {
                Err(fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    format!(
                        "matched protocol {protocol} does not expose compiled derive helper metadata (E-M010-001)"
                    ),
                ))
            }
            lrr_expand_adapter::proc_client::DeriveHelperMetadata::NotDerive => Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                "derive export has inconsistent metadata kind",
            )),
        }
    }
    pub fn invocations(&self) -> u64 {
        self.session.invocations()
    }
    pub(crate) fn host_declaration_expansions(&self) -> u64 {
        self.host_declaration_expansions
    }
    pub(crate) fn source_definition_site(&self, artifact: Digest) -> Option<&SourceDefinitionSite> {
        self.definition_sites
            .get(&artifact)
            .map(std::sync::Arc::as_ref)
    }
    pub(crate) fn has_source_derive(&self, identity: &MacroIdentity) -> bool {
        matches!(identity,MacroIdentity::Procedural{artifact,export,kind:ProceduralKind::Derive} if self.source_derives.contains_key(&(*artifact,export.clone())))
    }
    pub(crate) fn retain_definition_site(
        &mut self,
        manifest: &ToolManifest,
        producer: Digest,
        blob: &crate::source::snapshot::SourceBlob,
    ) -> Result<(), Diagnostic> {
        use lrr_expand_adapter::token_wire::{
            self, ContextRecord, Edition, SourceIdentity, TokenTree, Transparency,
        };
        let unit = manifest
            .graph
            .units
            .get(&producer)
            .filter(|unit| unit.key.role == UnitRole::ProcMacroHost)
            .ok_or_else(|| {
                fault(
                    "LRR-E-UNIT-GRAPH",
                    "definition site requires prepared macro-host unit",
                )
            })?;
        let prepared = manifest
            .artifacts
            .get(&producer)
            .filter(|artifact| artifact.kind == ArtifactKind::ProcMacroLibrary)
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-NOT-FOUND",
                    "definition site macro-host artifact absent",
                )
            })?;
        let artifact = macro_artifact_identity(producer, prepared);
        if !self.macro_units.contains(&producer) {
            return Err(fault(
                "LRR-E-MACRO-NOT-FOUND",
                "definition site producer is not owned by this session",
            ));
        }
        if self.definition_sites.contains_key(&artifact) {
            return Ok(());
        }
        let package = manifest.compile_env[&producer]
            .cargo
            .get("CARGO_MANIFEST_DIR")
            .ok_or_else(|| fault("LRR-E-UNIT-GRAPH", "definition package root absent"))?;
        let path = Path::new(&unit.source_path)
            .strip_prefix(package)
            .ok()
            .and_then(|p| p.to_str())
            .ok_or_else(|| fault("LRR-E-TOOL-PREPARE-STALE", "definition source path differs"))?;
        let bytes = blob
            .bytes()
            .len()
            .checked_add(path.len())
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "definition source storage overflow"))?;
        let _retained = self
            .definition_sites
            .values()
            .try_fold(bytes, |n, s| n.checked_add(s.bytes))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "definition source storage limit"))?;
        let edition = match unit.key.edition {
            2015 => Edition::E2015,
            2018 => Edition::E2018,
            2021 => Edition::E2021,
            2024 => Edition::E2024,
            _ => {
                return Err(fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "definition source edition unsupported",
                ));
            }
        };
        let identity = SourceIdentity {
            unit: Key(producer.0),
            content: Key(blob.content().0),
            path: path.into(),
        };
        let text = blob.text()?.to_owned();
        let parsed = token_wire::parse_source(
            &text,
            identity,
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: Key(producer.0),
                edition,
            },
            Key(artifact.0),
        )
        .map_err(adapter)?;
        let [TokenTree::Group { open, .. }] = parsed.roots() else {
            return Err(fault("LRR-E-MACRO-OUTPUT", "definition source root absent"));
        };
        let span = parsed
            .spans()
            .iter()
            .find(|s| s.key() == *open)
            .ok_or_else(|| fault("LRR-E-MACRO-OUTPUT", "definition root span absent"))?
            .clone();
        let registry = token_wire::TokenForest::new(vec![], parsed.contexts().to_vec(), vec![span])
            .map_err(adapter)?;
        // The external server's exact loaded export list selects source-facet
        // headers; this does not inspect or execute any macro-host function body.
        let native = crate::syntax::parse::ParsedFile::parse_macro_facet(blob, unit.key.edition)?;
        let mut diagnostic_definitions: BTreeMap<
            ProceduralKind,
            BTreeMap<String, crate::syntax::spans::SourceRange>,
        > = BTreeMap::new();
        let mut diagnostic_bytes = 0usize;
        let mut diagnostic_count = 0usize;
        for (loaded_artifact, export, form) in &self.loaded_exports {
            if *loaded_artifact != Key(artifact.0) {
                continue;
            }
            let Some(at) = native.source_procedural_definition_origin(
                export,
                *form,
                &manifest.compile_env[&producer].cfg,
            )?
            else {
                continue;
            };
            diagnostic_bytes = diagnostic_bytes
                .checked_add(128)
                .and_then(|n| n.checked_add(export.len()))
                .ok_or_else(|| {
                    fault(
                        "LRR-E-METADATA-LIMIT",
                        "procedural definition origin storage limit",
                    )
                })?;
            let mut name = String::new();
            name.try_reserve_exact(export.len()).map_err(|_| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "procedural definition origin allocation",
                )
            })?;
            name.push_str(export);
            let kind = match form {
                ProcForm::FunctionLike => ProceduralKind::Function,
                ProcForm::Attribute => ProceduralKind::Attribute,
                ProcForm::Derive => ProceduralKind::Derive,
            };
            if !diagnostic_definitions.contains_key(&kind) {
                diagnostic_bytes = diagnostic_bytes
                    .checked_add(128)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-METADATA-LIMIT",
                            "procedural origin form accounting limit",
                        )
                    })?;
            }
            let definitions = diagnostic_definitions.entry(kind).or_default();
            if definitions.insert(name, at).is_some() {
                return Err(fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "duplicate loaded procedural definition origin",
                ));
            }
            diagnostic_count = diagnostic_count.checked_add(1).ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "procedural definition origin count overflow",
                )
            })?;
        }
        let mut proof = StableEncoder::new("lrr/v2/prepared-procedural-definition-site");
        proof.digest(artifact);
        proof.bytes(&open.0);
        proof.bytes(&registry.encode().map_err(adapter)?);
        proof.u64(
            u64::try_from(diagnostic_definitions.len())
                .expect("bounded procedural definition origins"),
        );
        for (kind, definitions) in &diagnostic_definitions {
            proof.u8(match kind {
                ProceduralKind::Function => 0,
                ProceduralKind::Attribute => 1,
                ProceduralKind::Derive => 2,
            });
            proof.u64(
                u64::try_from(definitions.len()).expect("bounded procedural definition origins"),
            );
            for (name, at) in definitions {
                proof.string(name);
                proof.u32(at.start);
                proof.u32(at.end);
            }
        }
        let fingerprint = proof.finish();
        let registry =
            crate::syntax::token_wire::TokenForest::decode(&registry.encode().map_err(adapter)?)
                .map_err(|e| fault("LRR-E-MACRO-OUTPUT", &e.message))?;
        let source = registry
            .spans()
            .first()
            .ok_or_else(|| fault("LRR-E-MACRO-OUTPUT", "owned definition span absent"))?
            .source
            .clone();
        let text: std::sync::Arc<str> = std::sync::Arc::from(text);
        let bytes = text
            .len()
            .checked_add(source.path.capacity())
            .and_then(|n| n.checked_add(diagnostic_bytes))
            .and_then(|n| {
                registry
                    .allocated_bytes()
                    .ok()
                    .and_then(|size| n.checked_add(size))
            })
            .and_then(|n| n.checked_add(std::mem::size_of::<SourceDefinitionSite>() + 160))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "definition site storage overflow"))?;
        self.definition_sites
            .values()
            .try_fold(bytes, |n, site| n.checked_add(site.bytes))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "definition site storage limit"))?;
        self.definition_sites.insert(
            artifact,
            std::sync::Arc::new(SourceDefinitionSite {
                site: crate::syntax::token_wire::Key(open.0),
                registry,
                source,
                text,
                bytes,
                fingerprint,
                diagnostic_definitions,
            }),
        );
        Ok(())
    }
    pub(crate) fn register_generated_host_derives(
        &mut self,
        current: &DemandContext,
        manifest: &ToolManifest,
        proof: super::generated_host_derives::GeneratedHostDerives,
    ) -> Result<(), Diagnostic> {
        if current != &self.context || manifest.fingerprint() != self.preparation {
            return Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "generated macro-host catalog context changed",
            ));
        }
        let unit = manifest
            .graph
            .units
            .get(&proof.producer)
            .filter(|u| u.key.role == UnitRole::ProcMacroHost)
            .ok_or_else(|| fault("LRR-E-UNIT-GRAPH", "generated macro-host unit absent"))?;
        let env = &manifest.compile_env[&proof.producer];
        if manifest.inputs.source_files.get(&unit.source_path) != Some(&Some(proof.source))
            || proof.cfg != env.cfg.fingerprint()
            || proof.edition != unit.key.edition
        {
            return Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "generated macro-host catalog inputs differ",
            ));
        }
        let artifact = manifest
            .artifacts
            .get(&proof.producer)
            .filter(|a| a.kind == ArtifactKind::ProcMacroLibrary)
            .ok_or_else(|| {
                fault(
                    "LRR-E-MACRO-SERVER-MISMATCH",
                    "generated catalog artifact absent",
                )
            })?;
        let artifact_id = macro_artifact_identity(proof.producer, artifact);
        let loaded: BTreeSet<_> = self
            .loaded_exports
            .iter()
            .filter(|(id, _, form)| *id == Key(artifact_id.0) && *form == ProcForm::Derive)
            .map(|(_, name, _)| name.as_str())
            .collect();
        let declared: BTreeSet<_> = proof
            .declarations
            .iter()
            .map(|d| d.names().export())
            .collect();
        if declared != loaded || declared.len() != proof.declarations.len() {
            return Err(fault(
                "LRR-E-MACRO-SERVER-MISMATCH",
                format!(
                    "generated helper declarations differ from exact loaded derive exports (declared={}, loaded={}; declared_sample={:?}; loaded_sample={:?})",
                    declared.len(),
                    loaded.len(),
                    declared
                        .iter()
                        .take(5)
                        .map(|n| n.chars().take(80).collect::<String>())
                        .collect::<Vec<_>>(),
                    loaded
                        .iter()
                        .take(5)
                        .map(|n| n.chars().take(80).collect::<String>())
                        .collect::<Vec<_>>()
                ),
            ));
        }
        let mut candidate = self.source_derives.clone();
        for declaration in proof.declarations {
            let key = (artifact_id, declaration.names().export().to_owned());
            if let Some((_, old)) = candidate.get(&key) {
                if old.fingerprint() != declaration.fingerprint() {
                    return Err(fault(
                        "LRR-E-MACRO-SCOPE",
                        "conflicting generated helper declaration",
                    ));
                }
                continue;
            }
            let mut e = StableEncoder::new("lrr/v2/generated-macro-host-declaration");
            e.digest(artifact_id);
            e.digest(proof.origin);
            e.record(&declaration);
            candidate.insert(key, (e.finish(), declaration));
        }
        let _bytes = candidate.values().try_fold(0usize, |n, (_, d)| {
            n.checked_add(d.names().allocated_bytes()?)
                .and_then(|n| n.checked_add(512))
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "generated helper storage overflow"))
        })?;
        self.retain_definition_site(manifest, proof.producer, &proof.definition_blob)?;
        let count = self
            .host_declaration_expansions
            .checked_add(proof.expansions)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "macro-host declaration expansion count limit",
                )
            })?;
        self.source_derives = candidate;
        self.host_declaration_expansions = count;
        Ok(())
    }
    pub(crate) fn diagnostic_snapshot(
        &self,
    ) -> Result<lrr_expand_adapter::proc_client::ProcDiagnosticSnapshot, Diagnostic> {
        self.session.diagnostic_snapshot().map_err(adapter)
    }
    /// First admitted expansion failure for this unit/epoch, retained on replay.
    pub fn primary_failure(&self) -> Option<&MacroFaultReport> {
        self.primary_failure.as_ref()
    }
    pub fn cancel(&mut self) {
        self.session.cancel();
    }
    pub(crate) fn cap_execution_deadline(
        &mut self,
        deadline: std::time::Instant,
    ) -> Result<(), Diagnostic> {
        self.session
            .cap_execution_deadline(deadline)
            .map_err(adapter)
    }
    /// Recorded inputs must stay unchanged; a failure closes the whole unit request.
    pub fn fulfil_verified(
        &mut self,
        context: &DemandContext,
        coordinator: &mut ExpansionCoordinator,
        token: &DemandToken,
        request: &ProcInvocation,
        cache: &mut crate::expand::cache::ExpansionOutputCache,
        inputs: &super::run::RecordedMacroInputs<'_>,
    ) -> Result<
        (
            ExpansionRecord,
            crate::expand::cache::ValidatedExpansionOutput,
        ),
        Diagnostic,
    > {
        let before = if inputs.preparation() == self.preparation {
            inputs.verify()
        } else {
            Err(fault(
                "LRR-E-TOOL-PREPARE-STALE",
                "macro input witness preparation mismatch",
            ))
        };
        if let Err(error) = before {
            self.cancel();
            coordinator.cancel();
            return Err(error);
        }
        let record = self.fulfil(context, coordinator, token, request)?;
        if let Err(error) = inputs.verify() {
            self.cancel();
            coordinator.cancel();
            return Err(error);
        }
        let output = cache.validate(context, &record)?;
        Ok((record, output))
    }
    /// Fresh session fulfilment precedes any reuse of pure output-derived facts.
    pub fn fulfil_with_output_cache(
        &mut self,
        context: &DemandContext,
        coordinator: &mut ExpansionCoordinator,
        token: &DemandToken,
        request: &ProcInvocation,
        cache: &mut crate::expand::cache::ExpansionOutputCache,
    ) -> Result<
        (
            ExpansionRecord,
            crate::expand::cache::ValidatedExpansionOutput,
        ),
        Diagnostic,
    > {
        let record = self.fulfil(context, coordinator, token, request)?;
        let output = cache.validate(context, &record)?;
        Ok((record, output))
    }
    /// Start/admit the exact operation and return a record for coordinator adoption.
    /// This never publishes into SourceDb; reuse cannot initiate a native call.
    pub fn fulfil(
        &mut self,
        context: &DemandContext,
        coordinator: &mut ExpansionCoordinator,
        token: &DemandToken,
        request: &ProcInvocation,
    ) -> Result<ExpansionRecord, Diagnostic> {
        if context != &self.context || token.context() != context {
            return Err(fault(
                "LRR-E-MACRO-CRASH",
                "stale or mismatched macro demand context",
            ));
        }
        let InputRequest::Expand(input) = token.input() else {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "macro helper cannot capture source inputs",
            ));
        };
        let key = &input.key;
        if key != &self.request_key(context, request)? {
            return Err(fault(
                "LRR-E-MACRO-OUTPUT",
                "procedural request differs from exact expansion demand",
            ));
        }
        let call = request
            .input
            .spans()
            .iter()
            .find(|span| span.key() == request.call_site)
            .ok_or_else(|| fault("LRR-E-MACRO-OUTPUT", "call site span is not registered"))?;
        let decision = coordinator.start(context, token)?;
        let completed = match decision {
            StartDecision::Start => {
                self.session
                    .expand(request)
                    .map_err(adapter)
                    .and_then(|tokens| {
                        ExpansionRecord::from_wire(key.clone(), tokens.encode().map_err(adapter)?)
                    })
            }
            StartDecision::Reuse(expected) => {
                let tokens = self
                    .session
                    .cached_result(request)
                    .map_err(adapter)?
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-MACRO-OUTPUT",
                            "coordinator reuse has no matching session record",
                        )
                    })?;
                let record =
                    ExpansionRecord::from_wire(key.clone(), tokens.encode().map_err(adapter)?)?;
                if record.output() != expected {
                    return Err(fault(
                        "LRR-E-MACRO-OUTPUT",
                        "coordinator/session output differs",
                    ));
                }
                return Ok(record);
            }
            StartDecision::InFlight => {
                return Err(fault(
                    "LRR-E-QUERY-REQUEST",
                    "procedural input is already in flight",
                ));
            }
        };
        let reply = Reply {
            context: context.clone(),
            token: token.fingerprint(),
            input: token.input().fingerprint(),
            output: completed
                .as_ref()
                .map(|record| record.output())
                .map_err(Clone::clone),
        };
        if let Err(error) = &completed
            && self.primary_failure.is_none()
        {
            self.primary_failure = Some(MacroFaultReport::capture(
                context,
                token,
                request,
                call.origin,
                error.clone(),
                &self.session,
            ));
        }
        coordinator.accept(context, token, reply)?;
        completed
    }
}
fn edition(edition: lrr_expand_adapter::token_wire::Edition) -> u16 {
    match edition {
        lrr_expand_adapter::token_wire::Edition::E2015 => 2015,
        lrr_expand_adapter::token_wire::Edition::E2018 => 2018,
        lrr_expand_adapter::token_wire::Edition::E2021 => 2021,
        lrr_expand_adapter::token_wire::Edition::E2024 => 2024,
    }
}
