//! T054 current original-source constant admission. Pure published facts only.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::BucketKey,
        memo::Database,
        seals::{DiscoveryInput, SealDomain, SealKey},
    },
    frontend::{constants::ConstValue, names::NameLookup},
    source::{
        changes::{PreparedSourceView, SourceSelection},
        manifest::Namespace,
    },
    syntax::{
        pure_constants::{ConstantPlan, ConstantSource},
        spans::SourceRange,
    },
};
use std::collections::{BTreeMap, BTreeSet};
#[path = "constant_source_admission_sources.rs"]
mod retained_sources;
use retained_sources::RetainedConstantSources;
#[path = "constant_child_admission.rs"]
mod child_request;
#[path = "constant_empty_expansions.rs"]
mod empty_expansions;
const MAX_BYTES: usize = 32 * 1024 * 1024;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-SOURCE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "constant source admission budget".into(),
    )
}
/// No public constructor or mutable field exposes primitive-name authority.
/// Minted only from the checked actually published Type scope. Present names
/// remain deferred until the evaluator reaches that type spelling.
#[derive(Debug, Clone)]
pub(crate) struct CurrentConstantTypeCertificate {
    selection: SourceSelection,
    scope: crate::syntax::outline::ScopeKey,
    absent: BTreeSet<String>,
    inert_attributes: BTreeMap<(u32, u32), Digest>,
}
impl CurrentConstantTypeCertificate {
    pub(crate) fn matches(
        &self,
        selection: &SourceSelection,
        scope: &crate::syntax::outline::ScopeKey,
    ) -> bool {
        &self.selection == selection && &self.scope == scope
    }
    pub(crate) fn permits(&self, name: &str) -> bool {
        self.absent.contains(name)
    }
    pub(crate) fn permits_inert_attribute(&self, range: SourceRange) -> bool {
        self.inert_attributes
            .contains_key(&(range.start, range.end))
    }
    pub(crate) fn encode_inert_attributes(&self, e: &mut StableEncoder) {
        e.u64(self.inert_attributes.len() as u64);
        for ((start, end), proof) in &self.inert_attributes {
            e.u32(*start);
            e.u32(*end);
            e.digest(*proof);
        }
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.selection.metadata_bytes()?)
            .and_then(|n| n.checked_add(self.scope.heap_bytes()))
            .ok_or_else(limit)?;
        for spelling in &self.absent {
            bytes = bytes
                .checked_add(128 + spelling.capacity())
                .ok_or_else(limit)?;
        }
        bytes = bytes
            .checked_add(
                self.inert_attributes
                    .len()
                    .checked_mul(128)
                    .ok_or_else(limit)?,
            )
            .ok_or_else(limit)?;
        if bytes > 64 * 1024 {
            return Err(limit());
        }
        Ok(bytes)
    }
}
/// A requested entry is not a source/seal authority and cannot forge a plan.
#[derive(Debug, Clone)]
pub(crate) enum ConstantRequest {
    Constant,
    ConstCall(Vec<ConstValue>),
    ArrayLength(SourceRange),
}
/// Immutable privately minted publication certificate. The plan carries no guest
/// state. Revalidate this packet before publishing/reusing a query result.
#[derive(Debug, Clone)]
pub(crate) struct ConstantSourceAdmission {
    selection: SourceSelection,
    definition: Digest,
    request: ConstantRequest,
    plan: ConstantPlan,
    value_seal: SealKey,
    type_seal: SealKey,
    value_fingerprint: Digest,
    type_fingerprint: Digest,
    stamps: BTreeMap<InputKey, Digest>,
    bytes: usize,
    byte_limit: usize,
    macro_proofs: Vec<crate::frontend::constant_macro_admission::AdmittedConstantPanic>,
    sources: RetainedConstantSources,
    // Conservative bounded child replay guard, never semantic source authority.
    publication_revision: u64,
}
fn value_bytes(value: &ConstValue, depth: usize, nodes: &mut usize) -> Result<usize, Diagnostic> {
    *nodes = nodes
        .checked_add(1)
        .filter(|n| *n <= 100_000)
        .ok_or_else(limit)?;
    if depth > 64 {
        return Err(limit());
    }
    let mut bytes = std::mem::size_of::<ConstValue>() + 128;
    if let Some(elements) = value.elements() {
        for value in elements {
            bytes = bytes
                .checked_add(value_bytes(value, depth + 1, nodes)?)
                .ok_or_else(limit)?;
        }
    } else if value.scalar().is_none() {
        return Err(fault("constant argument is not an immutable value"));
    }
    Ok(bytes)
}
fn request_bytes(request: &ConstantRequest) -> Result<usize, Diagnostic> {
    let mut bytes = std::mem::size_of::<ConstantRequest>() + 128;
    if let ConstantRequest::ConstCall(arguments) = request {
        if arguments.len() > 256 {
            return Err(limit());
        }
        let mut nodes = 0;
        for value in arguments {
            bytes = bytes
                .checked_add(value_bytes(value, 0, &mut nodes)?)
                .ok_or_else(limit)?;
        }
    }
    Ok(bytes)
}
fn validate_seal(
    db: &Database,
    source: &PreparedSourceView,
    input: &DiscoveryInput,
    key: &SealKey,
    scope: &crate::syntax::outline::ScopeKey,
    namespace: Namespace,
    sources: &RetainedConstantSources,
) -> Result<(), Diagnostic> {
    key.validate()?;
    let certified = input
        .scope
        .as_ref()
        .ok_or_else(|| fault("constant namespace scope absent"))?;
    if input.seal.key() != *key
        || certified.scope != *scope
        || certified.namespace != namespace
        || certified.edition != source.selection().outline.parse.profile.edition
        || !matches!(input.seal.domain(), SealDomain::Scope {id, namespace: actual}
            if *id == scope.fingerprint() && *actual == namespace)
        || !sources.matches_seal_sources(&input.sources)
        || input.seal.unresolved_frontiers() != 0
    {
        return Err(fault(
            "constant namespace source/scope/world differs or is incomplete",
        ));
    }
    if !matches!(db.published_input(&InputKey::ExpansionEpoch), Some((InputValue::ExpansionEpoch(epoch),_)) if *epoch == key.epoch)
        || !matches!(db.published_input(&InputKey::SourceDependencies), Some((InputValue::SourceDependencies(graph),_)) if graph.fingerprint() == input.source_dependencies)
    {
        return Err(fault("constant namespace epoch/source graph changed"));
    }
    if input.compiler_inputs.len() > 100_000 {
        return Err(limit());
    }
    for (key, fingerprint) in &input.compiler_inputs {
        if db
            .published_input(key)
            .is_none_or(|(_, actual)| actual != *fingerprint)
        {
            return Err(fault("constant namespace compiler input changed"));
        }
    }
    empty_expansions::validate(db, input, key, sources)?;
    if !input.procedural_catalogs.is_empty()
        || !input.helper_declarations.is_empty()
        || !input.argument_fragments.is_empty()
        || !input.argument_indexes.is_empty()
        || !input.argument_summaries.is_empty()
        || !input.source_replacements.is_empty()
    {
        return Err(fault(
            "generated/native constant namespace requires separate admission",
        ));
    }
    Ok(())
}
impl ConstantSourceAdmission {
    pub(crate) fn capture_current(
        db: &Database,
        source: &PreparedSourceView,
        definition: Digest,
        request: ConstantRequest,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        Self::capture_current_impl(db, source, definition, request, byte_limit, &[], &[source])
    }
    #[cfg(test)]
    pub(crate) fn capture_current_with_macro_proofs(
        db: &Database,
        source: &PreparedSourceView,
        definition: Digest,
        request: ConstantRequest,
        byte_limit: usize,
        proofs: &[crate::frontend::constant_macro_admission::AdmittedConstantPanic],
    ) -> Result<Self, Diagnostic> {
        Self::capture_current_impl(
            db,
            source,
            definition,
            request,
            byte_limit,
            proofs,
            &[source],
        )
    }
    /// Views are evidence, not authority: both actual current namespace seals
    /// must disclose exactly this original source set.
    pub(crate) fn capture_current_in_sources(
        db: &Database,
        source: &PreparedSourceView,
        definition: Digest,
        request: ConstantRequest,
        byte_limit: usize,
        views: &[&PreparedSourceView],
    ) -> Result<Self, Diagnostic> {
        Self::capture_current_impl(db, source, definition, request, byte_limit, &[], views)
    }
    fn capture_current_impl(
        db: &Database,
        source: &PreparedSourceView,
        definition: Digest,
        request: ConstantRequest,
        byte_limit: usize,
        proofs: &[crate::frontend::constant_macro_admission::AdmittedConstantPanic],
        views: &[&PreparedSourceView],
    ) -> Result<Self, Diagnostic> {
        if proofs.len() > 256
            || byte_limit < std::mem::size_of::<Self>() + 256
            || byte_limit > MAX_BYTES
        {
            return Err(limit());
        }
        let stamps = db.source_view_namespace_stamps()?;
        let sources = RetainedConstantSources::capture(db, source, views, &stamps, byte_limit)?;
        let mut proof_bytes = 0usize;
        for proof in proofs {
            proof_bytes = proof_bytes
                .checked_add(proof.metadata_bytes())
                .and_then(|n| n.checked_add(128))
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
        }
        for proof in proofs {
            proof.revalidate(db, source)?;
        }
        source.require_original_hygiene()?;
        source.require_declaration_syntax()?;
        let item = source
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.fingerprint() == definition)
            .ok_or_else(|| fault("constant owning declaration absent"))?;
        if item.scope.owner.is_some() {
            return Err(fault("lexical/associated constant admission prerequisite"));
        }
        let body = item
            .body
            .as_ref()
            .ok_or_else(|| fault("constant declaration body absent"))?;
        body.require_entry()?;
        let context = crate::syntax::outline::DeclarationContext {
            scope: item.scope.clone(),
            range: item.declaration_span,
        };
        if !matches!(db.published_input(&InputKey::DefinitionSource(definition)), Some((InputValue::DefinitionSource(parse),_)) if parse == &source.selection().outline.parse)
            || !matches!(db.published_input(&InputKey::DefinitionHeader(definition)), Some((InputValue::DefinitionHeader(header),_)) if header == &item.header)
            || !matches!(db.published_input(&InputKey::DefinitionBody(definition)), Some((InputValue::DefinitionBody(actual),_)) if actual == body)
            || !matches!(db.published_input(&InputKey::DeclarationContext(definition)), Some((InputValue::DeclarationContext(actual),_)) if actual == &context)
        {
            return Err(fault(
                "constant published declaration/member/body identity differs",
            ));
        }
        for (key, value) in source.prepared_inputs() {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != value.fingerprint())
            {
                return Err(fault(
                    "constant original source/compile publication changed",
                ));
            }
        }
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(256)
            .and_then(|n| {
                n.checked_add(proofs.len().checked_mul(std::mem::size_of::<
                    crate::frontend::constant_macro_admission::AdmittedConstantPanic,
                >())?)
            })
            .ok_or_else(limit)?;
        bytes = bytes
            .checked_add(sources.metadata_bytes())
            .and_then(|n| n.checked_add(request_bytes(&request).ok()?.checked_mul(2)?))
            .ok_or_else(limit)?;
        for key in stamps.keys() {
            bytes = bytes
                .checked_add(std::mem::size_of::<(InputKey, Digest)>() + 128)
                .and_then(|n| n.checked_add(key.heap_bytes()))
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
        }
        // Bound new parser/owned-plan allocation before it happens. This source
        // lane deliberately uses a conservative byte-per-token upper reservation.
        let parser_reservation = sources.parser_reservation()?;
        if bytes
            .checked_add(parser_reservation)
            .is_none_or(|n| n > byte_limit)
        {
            return Err(limit());
        }
        let value = db.source_view_scope_certificate(source.selection(), &item.scope)?;
        let InputValue::DiscoverySeal(value_input) = value.as_ref() else {
            return Err(fault("constant Value seal variant differs"));
        };
        let value_seal = value_input.seal.key();
        validate_seal(
            db,
            source,
            value_input,
            &value_seal,
            &item.scope,
            Namespace::Value,
            &sources,
        )?;
        let type_seal = SealKey {
            domain: SealDomain::Scope {
                id: item.scope.fingerprint(),
                namespace: Namespace::Type,
            },
            unit_graph: value_seal.unit_graph,
            epoch: value_seal.epoch,
        };
        let (type_value, type_fingerprint) = db
            .published_input(&InputKey::DiscoverySeal(type_seal.clone()))
            .ok_or_else(|| fault("constant current Type namespace certificate prerequisite"))?;
        let InputValue::DiscoverySeal(type_input) = type_value else {
            return Err(fault("constant Type seal variant differs"));
        };
        validate_seal(
            db,
            source,
            type_input,
            &type_seal,
            &item.scope,
            Namespace::Type,
            &sources,
        )?;
        sources.validate_names(source, value_input, type_input)?;
        let captured = ConstantSource::capture_prepared(source, definition)?;
        if captured.selection() != Some(source.selection()) || captured.scope() != &item.scope {
            return Err(fault("constant prepared capture source owner differs"));
        }
        // Bind every structurally captured direct member used by the immutable
        // name map, including forward references. This inspects no callee body
        // semantics and never prepares a thunk or guesses a missing name.
        for declaration in captured.declarations() {
            // Anonymous constants retain source/header/body identities in the
            // captured declaration inventory, but declare no Value name bucket.
            if declaration.is_anonymous() {
                continue;
            }
            let bucket = BucketKey {
                scope: item.scope.clone(),
                namespace: Namespace::Value,
                name: declaration.name().into(),
            };
            if !matches!(value_input.scope.as_ref().ok_or_else(|| fault("constant Value scope absent"))?.lookup(&bucket)?,
                NameLookup::Defined(found) if found.identity == declaration.definition()
                    && found.namespace == Namespace::Value && found.defining_unit == item.scope.crate_key
                    && found.definition_edition == source.selection().outline.parse.profile.edition)
            {
                return Err(fault(
                    "constant Value member does not select the exact declaration",
                ));
            }
        }
        let mut absent = BTreeSet::new();
        let certified = type_input
            .scope
            .as_ref()
            .ok_or_else(|| fault("constant Type scope absent"))?;
        for spelling in [
            "bool", "char", "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize",
            "f32", "f64",
        ] {
            let bucket = BucketKey {
                scope: item.scope.clone(),
                namespace: Namespace::Type,
                name: spelling.into(),
            };
            if matches!(certified.lookup(&bucket)?, NameLookup::Absent) {
                absent.insert(spelling.into());
            }
        }
        // Only the actually published, source-replayed inert attribute certificate
        // can retire an original attribute. A builtin spelling alone grants nothing.
        let mut inert_attributes = BTreeMap::new();
        for proof in &type_input.nested_inert_proofs {
            let scan = proof.scan();
            if scan.parse() != &source.selection().outline.parse
                || scan.path() != &source.selection().path
                || scan.cfg() != source.selection().outline.cfg
                || scan.scope().crate_key != source.selection().outline.crate_key
                || scan.scope().module_path != source.selection().outline.module_path
            {
                continue;
            }
            if proof.attribute_facet().is_none() {
                continue;
            }
            let producer = scan
                .producers()
                .iter()
                .find(|row| row.identity == proof.producer())
                .ok_or_else(|| fault("current inert producer is absent"))?;
            if producer.kind != crate::syntax::declaration_scan::DeclarationProducerKind::Attribute
            {
                return Err(fault("current inert producer is not an attribute"));
            }
            if inert_attributes.len() >= 256 {
                return Err(limit());
            }
            if inert_attributes
                .insert(
                    (producer.range.start, producer.range.end),
                    proof.fingerprint(),
                )
                .is_some()
            {
                return Err(fault("duplicate current inert attribute proof"));
            }
        }
        let captured = captured.with_type_certificate(CurrentConstantTypeCertificate {
            selection: source.selection().clone(),
            scope: item.scope.clone(),
            absent,
            inert_attributes,
        })?;
        let mut captured = captured;
        for proof in proofs {
            captured = captured.with_macro_proof(proof.clone())?;
        }
        bytes = bytes
            .checked_add(captured.metadata_bytes()?)
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        let plan = match &request {
            ConstantRequest::Constant => captured.constant(item.declaration_span)?,
            ConstantRequest::ConstCall(arguments) => {
                captured.const_call(item.declaration_span, arguments.clone())?
            }
            ConstantRequest::ArrayLength(at) => captured.array_length(*at)?,
        };
        if plan.owner_definition() != definition
            || plan.owner_context() != &context
            || plan.owner_body() != Some(body)
        {
            return Err(fault("constant selected plan owner/body differs"));
        }
        Ok(Self {
            selection: source.selection().clone(),
            definition,
            request,
            plan,
            value_seal,
            type_seal,
            value_fingerprint: value.fingerprint(),
            type_fingerprint,
            stamps,
            bytes,
            byte_limit,
            macro_proofs: proofs.to_vec(),
            sources,
            publication_revision: db.revision(),
        })
    }
    /// The demand names an actually reached syntax site. Current published
    /// NoItem/selection and operand proofs remain separately required here.
    pub(crate) fn prepare_reached_macro(
        &self,
        db: &Database,
        source: &PreparedSourceView,
        demand: &crate::frontend::constants::query_macro_demand::ConstantMacroDemand,
    ) -> Result<Self, Diagnostic> {
        demand.validate_current(db, source, self)?;
        if self.macro_proofs.len() >= 256
            || self
                .macro_proofs
                .iter()
                .any(|proof| proof.range() == demand.range())
        {
            return Err(fault(
                "constant macro demand repeated or exceeded its proof budget",
            ));
        }
        self.bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(256))
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        let proof =
            crate::frontend::constant_macro_admission::CurrentConstantMacroProof::capture_current(
                db,
                source,
                self,
                demand.body(),
                demand.range(),
                self.byte_limit,
            )?;
        let mut proofs = self.macro_proofs.clone();
        proofs.try_reserve_exact(1).map_err(|_| limit())?;
        proofs.push(proof);
        Self::capture_current_impl(
            db,
            source,
            self.definition,
            self.request.clone(),
            self.byte_limit,
            &proofs,
            &self.sources.views(),
        )
    }
    pub(crate) fn plan(&self) -> &ConstantPlan {
        &self.plan
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn dependencies(&self) -> &BTreeMap<InputKey, Digest> {
        &self.stamps
    }
    pub(crate) fn revalidate(
        &self,
        db: &Database,
        source: &PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        if source.selection() != &self.selection
            || db.source_view_namespace_stamps()? != self.stamps
        {
            return Err(fault(
                "constant admitted publication or input membership changed",
            ));
        }
        let fresh = Self::capture_current_impl(
            db,
            source,
            self.definition,
            self.request.clone(),
            self.byte_limit,
            &self.macro_proofs,
            &self.sources.views(),
        )?;
        if self.identity() != fresh.identity() {
            return Err(fault("constant source plan/certificate changed"));
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/current-constant-source-admission");
        e.string(self.selection.path.as_str());
        e.digest(self.selection.location_identity());
        e.digest(self.plan.identity());
        self.sources.encode(&mut e);
        e.record(&self.value_seal);
        e.record(&self.type_seal);
        e.digest(self.value_fingerprint);
        e.digest(self.type_fingerprint);
        e.u64(self.stamps.len() as u64);
        for (key, fingerprint) in &self.stamps {
            e.record(key);
            e.digest(*fingerprint);
        }
        e.finish()
    }
}

#[cfg(test)]
#[path = "constant_source_admission_tests.rs"]
mod tests;
