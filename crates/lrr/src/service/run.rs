//! Explicit macro run input verification; no build or runtime compilation occurs here.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    project::tool_manifest::{
        InputCapture, InputPlan, PreparedFiles, ToolManifest, verify_adoption,
    },
    source::generated::GeneratedRoot,
};

/// Retained capability plan and before-witness for one immutable preparation.
/// Undeclared native reads are outside this proof and remain non-hermetic.
pub struct RecordedMacroInputs<'a> {
    manifest: &'a ToolManifest,
    plan: &'a InputPlan,
    before: InputCapture,
    files: PreparedFiles,
}
impl<'a> RecordedMacroInputs<'a> {
    pub fn begin(manifest: &'a ToolManifest, plan: &'a InputPlan) -> Result<Self, Diagnostic> {
        let before = plan.capture()?;
        let files = PreparedFiles::capture(manifest)?;
        verify_adoption(manifest, &manifest.toolchain, &before, &files)?;
        let witness = Self {
            manifest,
            plan,
            before,
            files,
        };
        witness.verify()?;
        Ok(witness)
    }
    pub(crate) fn preparation(&self) -> Digest {
        self.manifest.fingerprint()
    }
    /// Call before/after native expansion and again before publishing a catalog seal.
    pub fn verify(&self) -> Result<(), Diagnostic> {
        let after = self.plan.capture()?;
        let files = PreparedFiles::capture(self.manifest)?;
        verify_adoption(self.manifest, &self.manifest.toolchain, &after, &files)?;
        self.before.verify_unchanged(&after)?;
        self.files.verify_unchanged(&files)?;
        for (producer, output) in &self.manifest.build_outputs {
            let unit = self.manifest.graph.units.get(producer).ok_or_else(stale)?;
            let tree = GeneratedRoot::open(&unit.key, std::path::Path::new(output.tree.out_dir()))?
                .capture()?;
            if tree.fingerprint() != output.tree.fingerprint() {
                return Err(stale());
            }
        }
        Ok(())
    }
}
fn stale() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-TOOL-PREPARE-STALE",
        "recorded macro inputs changed; repeat explicit preparation".into(),
    )
}

struct UnitMacroRun {
    context: super::expansion::DemandContext,
    coordinator: super::expansion::ExpansionCoordinator,
    session: super::macro_session::MacroSession,
}
/// Constructed only by successful final input verification; fields are private.
#[derive(Debug, Clone)]
pub struct VerifiedMacroRunInputs {
    context: Digest,
    preparation: Digest,
    identity: Digest,
    units: usize,
    base: super::expansion::DemandContext,
    graph: Digest,
    catalogs: std::collections::BTreeMap<Digest, Digest>,
    outputs: std::collections::BTreeMap<Digest, Digest>,
}
impl VerifiedMacroRunInputs {
    pub(crate) fn validate_discovery(
        &self,
        db: &mut crate::db::memo::Database,
        key: &crate::db::seals::SealKey,
        catalogs: &[std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>],
        records: &[crate::expand::record::ExpansionDescriptor],
    ) -> Result<(), Diagnostic> {
        self.validate(db, &self.base, self.preparation)?;
        if key.unit_graph != self.graph || key.epoch != self.base.epoch {
            return Err(stale());
        }
        for catalog in catalogs {
            if self.catalogs.get(&catalog.consumer()) != Some(&catalog.fingerprint()) {
                return Err(stale());
            }
        }
        for record in records
            .iter()
            .filter(|record| record.key.procedural_epoch.is_some())
        {
            if self.outputs.get(&record.key.fingerprint()) != Some(&record.fingerprint()) {
                return Err(stale());
            }
        }
        Ok(())
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn unit_count(&self) -> usize {
        self.units
    }
    /// Eligibility is tied to the exact frozen run, not an arbitrary retained hash.
    pub fn validate(
        &self,
        db: &mut crate::db::memo::Database,
        context: &super::expansion::DemandContext,
        preparation: Digest,
    ) -> Result<(), Diagnostic> {
        use crate::db::dependency::{InputKey, InputValue};
        if self.context != context.fingerprint()
            || self.preparation != preparation
            || db.source_revision() != context.revision
            || !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch == context.epoch)
        {
            return Err(Diagnostic::new(
                "LRR-E-QUERY-REQUEST",
                "macro input certificate belongs to another run, revision, epoch or preparation"
                    .into(),
            ));
        }
        Ok(())
    }
}
/// A request owns its per-unit sessions. Only pure output facts may outlive this owner.
pub struct MacroRun<'a> {
    base: super::expansion::DemandContext,
    inputs: RecordedMacroInputs<'a>,
    units: std::collections::BTreeMap<Digest, UnitMacroRun>,
    allow_macro_execution: bool,
    closed: bool,
    expansion_budget: crate::expand::budget::SharedExpansionBudget,
    outputs: std::collections::BTreeMap<Digest, Digest>,
}
impl<'a> MacroRun<'a> {
    pub fn begin(
        db: &mut crate::db::memo::Database,
        base: super::expansion::DemandContext,
        manifest: &'a ToolManifest,
        plan: &'a InputPlan,
        allow_macro_execution: bool,
    ) -> Result<Self, Diagnostic> {
        db.require_coordinator()?;
        let inputs = RecordedMacroInputs::begin(manifest, plan)?;
        let expansion_budget = crate::expand::budget::SharedExpansionBudget::new(
            base.request,
            manifest.graph.fingerprint(),
            base.epoch,
            2_000_000,
        )?;
        let run = Self {
            base,
            inputs,
            units: Default::default(),
            allow_macro_execution,
            closed: false,
            expansion_budget,
            outputs: Default::default(),
        };
        run.check_database(db)?;
        if run.base.source_generation != manifest.inputs.fingerprint() {
            return Err(stale());
        }
        Ok(run)
    }
    fn check_database(&self, db: &mut crate::db::memo::Database) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        use crate::db::dependency::{InputKey, InputValue};
        if self.closed
            || db.source_revision() != self.base.revision
            || !matches!(db.input(&InputKey::ExpansionEpoch).as_deref(),Some(InputValue::ExpansionEpoch(epoch)) if *epoch==self.base.epoch)
        {
            return Err(Diagnostic::new(
                "LRR-E-QUERY-REQUEST",
                "macro run is closed or its frozen revision/epoch changed".into(),
            ));
        }
        Ok(())
    }
    pub fn unit_invocations(&self, unit: Digest) -> u64 {
        self.units.get(&unit).map_or(0, |u| u.session.invocations())
    }
    pub fn unit_declaration_expansions(&self, unit: Digest) -> u64 {
        self.units
            .get(&unit)
            .map_or(0, |u| u.session.host_declaration_expansions())
    }
    /// Bounded diagnostics from an existing helper, including after run closure.
    /// Reading never starts a session; query providers cannot inspect coordinator channels.
    pub fn unit_diagnostics(
        &self,
        db: &mut crate::db::memo::Database,
        unit: Digest,
    ) -> Result<Option<lrr_expand_adapter::proc_client::ProcDiagnosticSnapshot>, Diagnostic> {
        db.require_coordinator()?;
        self.units
            .get(&unit)
            .map(|u| u.session.diagnostic_snapshot())
            .transpose()
    }
    pub fn expansion_budget(&self) -> crate::expand::budget::SharedExpansionBudget {
        self.expansion_budget.clone()
    }
    pub fn with_expansion_budget(
        mut self,
        budget: crate::expand::budget::SharedExpansionBudget,
    ) -> Result<Self, Diagnostic> {
        if self.closed
            || !self.units.is_empty()
            || !budget.matches(
                self.base.request,
                self.inputs.manifest.graph.fingerprint(),
                self.base.epoch,
            )
        {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-INPUT",
                "macro run expansion budget owner differs or attachment is late".into(),
            ));
        }
        self.expansion_budget = budget;
        Ok(self)
    }
    pub fn cancel(&mut self) {
        self.closed = true;
        for unit in self.units.values_mut() {
            unit.session.cancel();
            unit.coordinator.cancel();
        }
    }
    fn ensure_unit(
        &mut self,
        db: &mut crate::db::memo::Database,
        unit: Digest,
        deadline: std::time::Instant,
    ) -> Result<(), Diagnostic> {
        self.check_database(db)?;
        if !self.allow_macro_execution {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-PERMISSION",
                "procedural macro session requires macro execution authority".into(),
            ));
        }
        if !self.units.contains_key(&unit) {
            if self.units.len() >= 4096 {
                return Err(Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "macro run consumer session count limit".into(),
                ));
            }
            self.inputs.verify()?;
            self.expansion_budget.check_deadline()?;
            let env = self
                .inputs
                .manifest
                .compile_env
                .get(&unit)
                .ok_or_else(stale)?;
            let mut context = self.base.clone();
            context.unit = unit;
            context.compile_environment = env.fingerprint();
            let directory = env.cargo.get("CARGO_MANIFEST_DIR").ok_or_else(stale)?;
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-TIMEOUT",
                    "macro invocation deadline expired before session startup".into(),
                ));
            }
            let session = super::macro_session::MacroSession::open_with_deadline(
                context.clone(),
                self.inputs.manifest,
                self.allow_macro_execution,
                std::path::PathBuf::from(directory),
                deadline,
                4 * 1024 * 1024,
            )?;
            self.units.insert(
                unit,
                UnitMacroRun {
                    coordinator: super::expansion::ExpansionCoordinator::new(context.clone()),
                    context,
                    session,
                },
            );
        }
        Ok(())
    }
    /// Start or reuse this run's authorized per-consumer session and retain its catalog.
    pub fn catalog(
        &mut self,
        db: &mut crate::db::memo::Database,
        unit: Digest,
    ) -> Result<std::sync::Arc<crate::frontend::procedural_exports::PreparedProcCatalog>, Diagnostic>
    {
        let result = (|| {
            self.check_database(db)?;
            self.expansion_budget.freeze()?;
            let deadline = self.expansion_budget.invocation_deadline()?;
            self.ensure_unit(db, unit, deadline)?;
            let active = self.units.get(&unit).ok_or_else(stale)?;
            Ok(std::sync::Arc::new(
                active.session.export_catalog(&active.context)?,
            ))
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    /// Resolve captured source metadata outside queries, without invoking a derive.
    pub fn resolve_derive_declaration(
        &mut self,
        db: &mut crate::db::memo::Database,
        request: &crate::frontend::generated_derive_exports::DeriveDeclarationRequest,
    ) -> Result<super::derive_declarations::DeriveDeclarationPoll, Diagnostic> {
        let result = (|| {
            use super::derive_declarations::DeriveDeclarationPoll;
            self.check_database(db)?;
            if !self.allow_macro_execution {
                return Err(Diagnostic::new(
                    "LRR-E-MACRO-PERMISSION",
                    "derive source facet requires this run's macro authority".into(),
                ));
            }
            if request.graph != self.inputs.manifest.graph.fingerprint()
                || request.preparation != self.inputs.preparation()
                || request.epoch != self.base.epoch
                || self
                    .inputs
                    .manifest
                    .compile_env
                    .get(&request.consumer)
                    .is_none_or(|env| env.fingerprint() != request.environment)
            {
                return Err(stale());
            }
            request.origin.validate_database(db)?;
            match &request.origin {
                crate::expand::derive_origin::DeriveDeclarationOrigin::Generated(origin) => {
                    if origin.key.procedural_epoch != Some(self.base.epoch)
                        || self.outputs.get(&origin.key.fingerprint())
                            != Some(&origin.fingerprint())
                    {
                        return Err(Diagnostic::new(
                            "LRR-E-CATALOG-INCOMPLETE",
                            "derive declaration parent is not an adopted output owned by this run"
                                .into(),
                        ));
                    }
                }
                crate::expand::derive_origin::DeriveDeclarationOrigin::Source {
                    selection, ..
                } => {
                    if selection.outline.crate_key != request.consumer {
                        return Err(stale());
                    }
                }
            }
            self.inputs.verify()?;
            let deadline = self.expansion_budget.invocation_deadline()?;
            self.ensure_unit(db, request.consumer, deadline)?;
            let active = self.units.get_mut(&request.consumer).ok_or_else(stale)?;
            let catalog = active.session.export_catalog(&active.context)?;
            if catalog.fingerprint() != request.catalog
                || catalog
                    .select(
                        &request.definition,
                        crate::expand::identity::ProceduralKind::Derive,
                    )?
                    .is_none_or(|export| export.identity() != &request.identity)
            {
                return Err(stale());
            }
            if active.session.has_source_derive(&request.identity) {
                let receipt = active
                    .session
                    .prepared_derive_declaration(&active.context, request)?;
                self.inputs.verify()?;
                self.expansion_budget.check_deadline()?;
                return Ok(DeriveDeclarationPoll::Ready(receipt));
            }
            let producer = request.definition.defining_unit;
            let unit = self
                .inputs
                .manifest
                .graph
                .units
                .get(&producer)
                .ok_or_else(stale)?;
            let env = self
                .inputs
                .manifest
                .compile_env
                .get(&producer)
                .ok_or_else(stale)?;
            let blob = super::derive_declarations::captured_root(self.inputs.manifest, producer)?;
            let parsed =
                crate::syntax::parse::ParsedFile::parse_macro_facet(&blob, unit.key.edition)?;
            let facts = parsed.source_derive_catalog(&env.cfg)?;
            let crate::expand::identity::MacroIdentity::Procedural { export, .. } =
                &request.identity
            else {
                return Err(stale());
            };
            let mut selected = facts
                .declarations()
                .iter()
                .filter(|(_, decl)| decl.names().export() == export);
            let range = selected.next().map(|(range, _)| *range);
            if selected.next().is_some() {
                return Err(stale());
            }
            let poll = match range {
                Some(range) => {
                    active.session.register_source_derive(
                        &active.context,
                        self.inputs.manifest,
                        producer,
                        std::path::Path::new(&unit.source_path),
                        &blob,
                        range,
                    )?;
                    DeriveDeclarationPoll::Ready(
                        active
                            .session
                            .prepared_derive_declaration(&active.context, request)?,
                    )
                }
                None if !facts.pending().is_empty() => {
                    match super::generated_host_derives::prepare(
                        self.inputs.manifest,
                        request.consumer,
                        producer,
                        &blob,
                    )? {
                        Some(proof) => {
                            active.session.register_generated_host_derives(
                                &active.context,
                                self.inputs.manifest,
                                proof,
                            )?;
                            DeriveDeclarationPoll::Ready(
                                active
                                    .session
                                    .prepared_derive_declaration(&active.context, request)?,
                            )
                        }
                        None => DeriveDeclarationPoll::NeedExpansion {
                            producer,
                            ranges: facts.pending().to_vec(),
                        },
                    }
                }
                None => DeriveDeclarationPoll::NeedSource {
                    producer,
                    export: export.clone(),
                },
            };
            self.inputs.verify()?;
            self.expansion_budget.check_deadline()?;
            Ok(poll)
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    pub fn prepare_source_selection(
        &mut self,
        db: &mut crate::db::memo::Database,
        selection: &crate::expand::source_discovery::ProceduralSourceSelection,
        target: super::item_macro_requests::ResolvedItemTarget<'_>,
    ) -> Result<std::sync::Arc<super::function_macro_requests::PreparedFunctionRequest>, Diagnostic>
    {
        let result = (|| {
            self.check_database(db)?;
            self.expansion_budget.freeze()?;
            let deadline = self.expansion_budget.invocation_deadline()?;
            let unit = selection.input().scope().crate_key;
            self.ensure_unit(db, unit, deadline)?;
            let active = self.units.get(&unit).ok_or_else(stale)?;
            Ok(std::sync::Arc::new(selection.prepare(
                &active.session,
                &active.context,
                target,
            )?))
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    pub fn prepare_attribute_selection(
        &mut self,
        db: &mut crate::db::memo::Database,
        selection: &super::source_attribute_requests::SourceAttributeSelection,
    ) -> Result<std::sync::Arc<super::function_macro_requests::PreparedFunctionRequest>, Diagnostic>
    {
        let result = (|| {
            self.check_database(db)?;
            self.expansion_budget.freeze()?;
            let deadline = self.expansion_budget.invocation_deadline()?;
            let unit = selection.scope().crate_key;
            self.ensure_unit(db, unit, deadline)?;
            let active = self.units.get_mut(&unit).ok_or_else(stale)?;
            let catalog = active.session.export_catalog(&active.context)?;
            let selected = catalog
                .select(
                    selection.definition(),
                    crate::expand::identity::ProceduralKind::Attribute,
                )?
                .ok_or_else(stale)?;
            let crate::expand::identity::MacroIdentity::Procedural { artifact, .. } =
                selected.identity()
            else {
                return Err(stale());
            };
            if active.session.source_definition_site(*artifact).is_none() {
                self.inputs.verify()?;
                let producer = selection.definition().defining_unit;
                let blob =
                    super::derive_declarations::captured_root(self.inputs.manifest, producer)?;
                active
                    .session
                    .retain_definition_site(self.inputs.manifest, producer, &blob)?;
                self.inputs.verify()?;
            }
            Ok(std::sync::Arc::new(
                selection.prepare(&active.session, &active.context)?,
            ))
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    pub fn prepare_source_derive_selection(
        &mut self,
        db: &mut crate::db::memo::Database,
        selection: &super::source_derive_requests::SourceDeriveSelection,
    ) -> Result<std::sync::Arc<super::function_macro_requests::PreparedFunctionRequest>, Diagnostic>
    {
        let result = (|| {
            self.check_database(db)?;
            self.expansion_budget.freeze()?;
            let deadline = self.expansion_budget.invocation_deadline()?;
            selection.validate_database(db)?;
            let unit = selection.scope().crate_key;
            self.ensure_unit(db, unit, deadline)?;
            let active = self.units.get(&unit).ok_or_else(stale)?;
            Ok(std::sync::Arc::new(
                selection.prepare(&active.session, &active.context)?,
            ))
        })();
        if result.is_err() {
            self.cancel();
        }
        result
    }
    pub fn fulfil_source_derive(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
        phase: super::expansion::PhaseIdentity,
        request: &super::function_macro_requests::PreparedFunctionRequest,
        cache: &mut crate::expand::cache::ExpansionOutputCache,
    ) -> Result<crate::expand::cache::ValidatedExpansionOutput, Diagnostic> {
        let mut origin = Ok(None);
        let mut result = (|| {
            self.check_database(db)?;
            discovery.require_run_budget(&self.expansion_budget)?;
            discovery.require_source_derive_request(request)?;
            origin = discovery.source_derive_failure_origin(request).map(Some);
            discovery.validate_source_derive_database(db, request)?;
            let output = self.fulfil_inner(
                db,
                request.source().outline.crate_key,
                phase,
                request.request(),
                cache,
            )?;
            use crate::db::dependency::{InputKey, InputValue};
            let adopted = db
                .input(&InputKey::ExpansionRecord(request.key().fingerprint()))
                .ok_or_else(stale)?;
            let InputValue::ExpansionRecord(record) = adopted.as_ref() else {
                return Err(stale());
            };
            if self.outputs.get(&request.key().fingerprint())
                != Some(&record.descriptor().fingerprint())
            {
                return Err(stale());
            }
            discovery.retain_source_derive_output(
                db,
                request,
                std::sync::Arc::new(record.clone()),
            )?;
            Ok(output)
        })();
        if let Err(error) = &mut result {
            self.cancel();
            match origin {
                Ok(Some(origin)) if error.macro_origin.is_none() => {
                    error.macro_origin = Some(Box::new(origin));
                }
                Err(capture) if error.origin_attachment_failure.is_none() => {
                    error.origin_attachment_failure = Some(capture.code);
                }
                _ => {}
            }
        }
        result
    }
    /// Execute and integrate an exactly bound source request through the same budget ledger.
    pub fn fulfil_discovery(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
        phase: super::expansion::PhaseIdentity,
        request: &super::function_macro_requests::PreparedFunctionRequest,
        cache: &mut crate::expand::cache::ExpansionOutputCache,
    ) -> Result<crate::expand::cache::ValidatedExpansionOutput, Diagnostic> {
        let mut origin = Ok(None);
        let mut result = (|| {
            self.check_database(db)?;
            discovery.require_run_budget(&self.expansion_budget)?;
            discovery.require_procedural_request(request)?;
            origin = discovery.source_request_diagnostic_origin(request);
            let unit = request.source().outline.crate_key;
            let output = self.fulfil_inner(db, unit, phase, request.request(), cache)?;
            use crate::db::dependency::{InputKey, InputValue};
            let adopted = db
                .input(&InputKey::ExpansionRecord(request.key().fingerprint()))
                .ok_or_else(stale)?;
            let InputValue::ExpansionRecord(record) = adopted.as_ref() else {
                return Err(stale());
            };
            discovery.admit_procedural_expansion(db, std::sync::Arc::new(record.clone()))?;
            Ok(output)
        })();
        if let Err(error) = &mut result {
            self.cancel();
            match origin {
                Ok(Some(origin)) if error.macro_origin.is_none() => {
                    error.macro_origin = Some(Box::new(origin));
                }
                Err(capture) if error.origin_attachment_failure.is_none() => {
                    error.origin_attachment_failure = Some(capture.code);
                }
                _ => {}
            }
        }
        result
    }
    /// Generated calls execute only through this run's exact catalog and adopted demand.
    pub fn fulfil_generated_discovery(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
        phase: super::expansion::PhaseIdentity,
        request: &crate::expand::generated_call_request::GeneratedCallRequest,
        cache: &mut crate::expand::cache::ExpansionOutputCache,
    ) -> Result<crate::expand::cache::ValidatedExpansionOutput, Diagnostic> {
        let mut origin = Ok(None);
        let mut result = (|| {
            self.check_database(db)?;
            discovery.require_run_budget(&self.expansion_budget)?;
            discovery.require_generated_procedural_request(request)?;
            origin = discovery.generated_request_diagnostic_origin(request);
            let native = request.procedural_request().ok_or_else(stale)?;
            let unit = Digest(native.unit.0);
            self.expansion_budget.freeze()?;
            let deadline = self.expansion_budget.invocation_deadline()?;
            self.ensure_unit(db, unit, deadline)?;
            let active = self.units.get(&unit).ok_or_else(stale)?;
            if Some(
                active
                    .session
                    .export_catalog(&active.context)?
                    .fingerprint(),
            ) != request.procedural_catalog()
                || &active.session.request_key(&active.context, native)? != request.key()
            {
                return Err(stale());
            }
            let output = self.fulfil_inner(db, unit, phase, native, cache)?;
            use crate::db::dependency::{InputKey, InputValue};
            let adopted = db
                .input(&InputKey::ExpansionRecord(request.key().fingerprint()))
                .ok_or_else(stale)?;
            let InputValue::ExpansionRecord(record) = adopted.as_ref() else {
                return Err(stale());
            };
            discovery.admit_procedural_expansion(db, std::sync::Arc::new(record.clone()))?;
            Ok(output)
        })();
        if let Err(error) = &mut result {
            self.cancel();
            match origin {
                Ok(Some(origin)) if error.macro_origin.is_none() => {
                    error.macro_origin = Some(Box::new(origin));
                }
                Err(capture) if error.origin_attachment_failure.is_none() => {
                    error.origin_attachment_failure = Some(capture.code);
                }
                _ => {}
            }
        }
        result
    }
    pub fn integrate_source_derive_append(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
        request: &crate::service::function_macro_requests::PreparedFunctionRequest,
        record: std::sync::Arc<crate::expand::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        self.check_database(db)?;
        discovery.require_run_budget(&self.expansion_budget)?;
        discovery.require_source_derive_append(request, &record)?;
        if self.outputs.get(&record.key().fingerprint()) != Some(&record.descriptor().fingerprint())
        {
            return Err(stale());
        }
        self.inputs.verify()?;
        discovery.integrate_source_derive_append(db, request, record)
    }
    pub fn integrate_derive_append(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
        request: &crate::expand::generated_call_request::GeneratedCallRequest,
        record: std::sync::Arc<crate::expand::record::ExpansionRecord>,
    ) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        self.check_database(db)?;
        discovery.require_run_budget(&self.expansion_budget)?;
        discovery.require_derive_append(request, &record)?;
        if self.outputs.get(&record.key().fingerprint()) != Some(&record.descriptor().fingerprint())
        {
            return Err(stale());
        }
        self.inputs.verify()?;
        discovery.integrate_derive_append(db, request, record)
    }
    /// Verify actual files immediately before publication; retained certificates cannot publish.
    pub fn publish_discovery(
        &mut self,
        db: &mut crate::db::memo::Database,
        discovery: &mut crate::expand::source_discovery::SourceDiscovery,
    ) -> Result<VerifiedMacroRunInputs, Diagnostic> {
        let result = (|| {
            self.check_database(db)?;
            discovery.require_run_budget(&self.expansion_budget)?;
            let _record_count = discovery.sealed_expansions()?.len();
            let certificate = self.verify_for_seal(db)?;
            discovery.publish_macro_verified(db, &certificate)?;
            Ok(certificate)
        })();
        self.cancel();
        result
    }
    pub fn fulfil(
        &mut self,
        db: &mut crate::db::memo::Database,
        unit: Digest,
        phase: super::expansion::PhaseIdentity,
        request: &lrr_expand_adapter::proc_client::ProcInvocation,
        cache: &mut crate::expand::cache::ExpansionOutputCache,
    ) -> Result<crate::expand::cache::ValidatedExpansionOutput, Diagnostic> {
        let result = self.fulfil_inner(db, unit, phase, request, cache);
        if result.is_err() {
            self.cancel();
        }
        result
    }
    fn fulfil_inner(
        &mut self,
        db: &mut crate::db::memo::Database,
        unit: Digest,
        phase: super::expansion::PhaseIdentity,
        request: &lrr_expand_adapter::proc_client::ProcInvocation,
        cache: &mut crate::expand::cache::ExpansionOutputCache,
    ) -> Result<crate::expand::cache::ValidatedExpansionOutput, Diagnostic> {
        self.check_database(db)?;
        self.expansion_budget.freeze()?;
        let deadline = self.expansion_budget.invocation_deadline()?;
        self.ensure_unit(db, unit, deadline)?;
        let active = self.units.get_mut(&unit).ok_or_else(stale)?;
        active.session.cap_execution_deadline(deadline)?;
        let key = active.session.request_key(&active.context, request)?;
        let key_identity = key.fingerprint();
        if !self.outputs.contains_key(&key_identity) && self.outputs.len() >= 100_000 {
            return Err(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "macro run output receipt count limit".into(),
            ));
        }
        let token = active.coordinator.issue(
            crate::db::key::QueryKey::CurrentExpansionInput(Box::new(key.clone())),
            phase,
            crate::db::poll::InputRequest::Expand(Box::new(crate::db::poll::ExpansionRequest {
                key,
            })),
        )?;
        let (record, output) = active.session.fulfil_verified(
            &active.context,
            &mut active.coordinator,
            &token,
            request,
            cache,
            &self.inputs,
        )?;
        let descriptor = record.descriptor().fingerprint();
        if self
            .outputs
            .get(&key_identity)
            .is_some_and(|old| *old != descriptor)
        {
            return Err(Diagnostic::new(
                "LRR-E-EXPANSION-CONFLICT",
                "macro run output receipt changed".into(),
            ));
        }
        self.expansion_budget.record_output(&record)?;
        active
            .coordinator
            .adopt(&active.context, &token, db, record)?;
        self.outputs.insert(key_identity, descriptor);
        Ok(output)
    }
    /// Input verification certificate only; declaration/catalog seals belong to M013.
    pub fn verify_for_seal(
        &mut self,
        db: &mut crate::db::memo::Database,
    ) -> Result<VerifiedMacroRunInputs, Diagnostic> {
        let result = (|| {
            self.check_database(db)?;
            self.inputs.verify()?;
            self.expansion_budget.check_deadline()?;
            let mut e = crate::core::digest::StableEncoder::new(
                "lrr/v2/verified-macro-run-inputs/discovery-v1",
            );
            e.digest(self.base.fingerprint());
            e.digest(self.inputs.preparation());
            e.u64(u64::try_from(self.units.len()).map_err(|_| stale())?);
            if self.units.len() > 4096 || self.outputs.len() > 100_000 {
                return Err(stale());
            }
            let mut catalogs = std::collections::BTreeMap::new();
            for (unit, active) in &self.units {
                let catalog = active.session.export_catalog(&active.context)?;
                catalogs.insert(*unit, catalog.fingerprint());
                e.digest(catalog.fingerprint());
                e.digest(*unit);
                e.digest(active.context.fingerprint());
                e.u64(active.session.invocations());
            }
            e.u64(u64::try_from(self.outputs.len()).map_err(|_| stale())?);
            for (key, descriptor) in &self.outputs {
                e.digest(*key);
                e.digest(*descriptor);
            }
            Ok(VerifiedMacroRunInputs {
                context: self.base.fingerprint(),
                preparation: self.inputs.preparation(),
                identity: e.finish(),
                units: self.units.len(),
                base: self.base.clone(),
                graph: self.inputs.manifest.graph.fingerprint(),
                catalogs,
                outputs: self.outputs.clone(),
            })
        })();
        self.cancel();
        result
    }
}
