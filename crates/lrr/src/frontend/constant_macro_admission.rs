//! T054 reached constant macro admission from current published selection evidence.
//! NoItem certifies declarations; operand admission below certifies only the closed
//! empty/static-string panic argument lane, never arbitrary const/format semantics.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        seals::{SealDomain, SealKey},
    },
    facade::macros::RuntimeMacroKind,
    frontend::{
        constant_source_admission::ConstantSourceAdmission,
        macro_names::PathLookup,
        runtime_macro_catalog::RuntimeMacroCatalog,
        runtime_macro_plan::{RuntimeMacroPreparation, RuntimeNoItemProof, StaticPanicLowering},
    },
    source::{
        changes::{PreparedSourceView, SourceSelection},
        manifest::{BodyKey, Namespace},
    },
    syntax::{
        outline::{BodyHandle, DeclarationContext},
        spans::SourceRange,
    },
};
use std::{collections::BTreeMap, rc::Rc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-MACRO-SOURCE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "constant macro admission budget".into(),
    )
}
#[derive(Debug)]
pub(crate) struct CurrentConstantMacroProof {
    selection: SourceSelection,
    context: DeclarationContext,
    body: BodyHandle,
    range: SourceRange,
    no_item: RuntimeNoItemProof,
    arguments: Digest,
    message: String,
    seal: SealKey,
    seal_fingerprint: Digest,
    stamps: BTreeMap<InputKey, Digest>,
    bytes: usize,
    byte_limit: usize,
}
/// A cheap immutable syntax-node handle. Its private proof is not a runtime
/// Macro PC and cannot be applied at another source/body/range or edition.
#[derive(Debug, Clone)]
pub(crate) struct AdmittedConstantPanic(Rc<CurrentConstantMacroProof>);
impl AdmittedConstantPanic {
    pub(crate) fn message(&self) -> &str {
        &self.0.message
    }
    pub(crate) fn definition(&self) -> Digest {
        self.0.no_item.definition().identity
    }
    pub(crate) fn identity(&self) -> Digest {
        self.0.identity()
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.0.bytes
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.0.range
    }
    pub(crate) fn matches(
        &self,
        selection: &SourceSelection,
        body: &BodyHandle,
        range: SourceRange,
    ) -> bool {
        &self.0.selection == selection && &self.0.body == body && self.0.range == range
    }
    pub(crate) fn revalidate(
        &self,
        db: &Database,
        source: &PreparedSourceView,
    ) -> Result<(), Diagnostic> {
        self.0.revalidate(db, source)
    }
}
impl CurrentConstantMacroProof {
    pub(crate) fn capture_current(
        db: &Database,
        source: &PreparedSourceView,
        constant: &ConstantSourceAdmission,
        body: &BodyKey,
        range: SourceRange,
        byte_limit: usize,
    ) -> Result<AdmittedConstantPanic, Diagnostic> {
        if byte_limit < std::mem::size_of::<Self>() + 256 {
            return Err(limit());
        }
        constant.revalidate(db, source)?;
        let proof = Self::mint(db, source, body, range, byte_limit)?;
        if proof.context.scope != constant.plan().owner_context().scope {
            return Err(fault("constant macro belongs to another captured module"));
        }
        Ok(AdmittedConstantPanic(Rc::new(proof)))
    }
    fn mint(
        db: &Database,
        source: &PreparedSourceView,
        body: &BodyKey,
        range: SourceRange,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if byte_limit < std::mem::size_of::<Self>() + 256 {
            return Err(limit());
        }
        source.require_original_hygiene()?;
        source.require_declaration_syntax()?;
        let item = source
            .outline()
            .items
            .iter()
            .find(|i| i.body.as_ref().is_some_and(|h| h.key() == *body))
            .ok_or_else(|| fault("constant macro exact owning body is absent"))?;
        if item.scope.owner.is_some() {
            return Err(fault(
                "lexical/associated constant macro requires separate admission",
            ));
        }
        let handle = item
            .body
            .as_ref()
            .ok_or_else(|| fault("constant macro body handle absent"))?;
        handle.require_entry()?;
        if range.is_empty() || !handle.range.contains(range) {
            return Err(fault("constant macro range is outside exact body"));
        }
        let context = DeclarationContext {
            scope: item.scope.clone(),
            range: item.declaration_span,
        };
        let definition = item.def_key.fingerprint();
        if !matches!(db.published_input(&InputKey::DefinitionSource(definition)),Some((InputValue::DefinitionSource(parse),_)) if parse==&source.selection().outline.parse)
            || !matches!(db.published_input(&InputKey::DefinitionHeader(definition)),Some((InputValue::DefinitionHeader(header),_)) if header==&item.header)
            || !matches!(db.published_input(&InputKey::DefinitionBody(definition)),Some((InputValue::DefinitionBody(actual),_)) if actual==handle)
            || !matches!(db.published_input(&InputKey::DeclarationContext(definition)),Some((InputValue::DeclarationContext(actual),_)) if actual==&context)
        {
            return Err(fault(
                "constant macro declaration/header/body context changed",
            ));
        }
        for (key, value) in source.prepared_inputs() {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != value.fingerprint())
            {
                return Err(fault(
                    "constant macro captured source/compiler input changed",
                ));
            }
        }
        let stamps = db.source_view_namespace_stamps()?;
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(256)
            .and_then(|n| n.checked_add(source.selection().metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(context.scope.heap_bytes()))
            .ok_or_else(limit)?;
        for key in stamps.keys() {
            bytes = bytes
                .checked_add(std::mem::size_of::<(InputKey, Digest)>() + 128)
                .and_then(|n| n.checked_add(key.heap_bytes()))
                .filter(|n| *n <= byte_limit)
                .ok_or_else(limit)?;
        }
        let published = db.source_view_scope_certificate(source.selection(), &item.scope)?;
        let InputValue::DiscoverySeal(input) = published.as_ref() else {
            return Err(fault("constant macro Value seal variant differs"));
        };
        let certified = input
            .scope
            .as_ref()
            .ok_or_else(|| fault("constant macro scope certificate absent"))?;
        let seal = input.seal.key();
        seal.validate()?;
        if certified.scope != item.scope
            || certified.namespace != Namespace::Value
            || certified.edition != source.selection().outline.parse.profile.edition
            || !matches!(input.seal.domain(),SealDomain::Scope{id,namespace} if *id==item.scope.fingerprint() && *namespace==Namespace::Value)
            || input.sources.len() != 1
            || input.sources[0] != *source.selection()
            || input.seal.unresolved_frontiers() != 0
        {
            return Err(fault(
                "constant macro scope/world/edition/source is incomplete or differs",
            ));
        }
        if !matches!(db.published_input(&InputKey::ExpansionEpoch),Some((InputValue::ExpansionEpoch(epoch),_)) if *epoch==seal.epoch)
            || !matches!(db.published_input(&InputKey::SourceDependencies),Some((InputValue::SourceDependencies(graph),_)) if graph.fingerprint()==input.source_dependencies)
        {
            return Err(fault("constant macro source graph/epoch differs"));
        }
        for (key, fingerprint) in &input.compiler_inputs {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != *fingerprint)
            {
                return Err(fault("constant macro compiler publication changed"));
            }
        }
        if !input.records.is_empty()
            || !input.procedural_catalogs.is_empty()
            || !input.helper_declarations.is_empty()
            || !input.argument_summaries.is_empty()
            || !input.argument_indexes.is_empty()
            || !input.argument_fragments.is_empty()
            || !input.source_replacements.is_empty()
            || !input.declaration_fragments.is_empty()
        {
            return Err(fault(
                "generated/procedural constant macro needs external source admission",
            ));
        }
        let mut matching = input.runtime_no_item_proofs.iter().filter(|(_, proof)| {
            proof.source() == &source.selection().outline.parse
                && proof.body() == body
                && proof.invocation() == range
        });
        let (_, proof) = matching
            .next()
            .ok_or_else(|| fault("exact published selected macro proof prerequisite"))?;
        if matching.next().is_some() {
            return Err(fault("constant macro selected proof is ambiguous"));
        }
        let catalog = RuntimeMacroCatalog::standard_for_definition(
            &source.selection().outline.parse.profile,
            proof.definition(),
        )?;
        if catalog.select(proof.definition())? != Some(RuntimeMacroKind::Panic) {
            return Err(fault(
                "selected macro requires a different constant operand/effect contract",
            ));
        }
        // Definition is not selected from spelling or supplied by the caller: it
        // came from the current published genuine SourceDiscovery proof above.
        let RuntimeMacroPreparation::Intrinsic(plan) = source.prepare_runtime_macro(
            range,
            PathLookup::Defined(proof.definition().clone()),
            &catalog,
        )?
        else {
            return Err(fault(
                "constant macro selected identity is not its compiler facet",
            ));
        };
        let replay = plan
            .no_item_proof(body)
            .ok_or_else(|| fault("constant panic operands contain declaration/nested producers"))?;
        if &replay != proof
            || plan.arguments().kind() != RuntimeMacroKind::Panic
            || plan.arguments().source() != &source.selection().outline.parse
            || plan.arguments().invocation() != range
        {
            return Err(fault(
                "constant macro source/argument/no-item replay differs",
            ));
        }
        // Only empty arguments and one actual string literal are pure here.
        // Nonstring values, captures, formatting and operand expressions remain
        // precise integration frontiers; they never become a canned panic.
        let StaticPanicLowering::Operation(operation) = plan.lower_static_panic()? else {
            return Err(Diagnostic::new(
                "LRR-E-CONST-MACRO-OPERANDS",
                "constant panic needs formatting or nonliteral argument integration".into(),
            ));
        };
        let crate::ir::op::Op::Panic { message } = *operation else {
            return Err(fault("constant static panic lowering kind differs"));
        };
        bytes = bytes
            .checked_add(proof.allocated_bytes().ok_or_else(limit)?)
            .and_then(|n| n.checked_add(message.capacity()))
            .and_then(|n| n.checked_add(plan.allocated_bytes().ok()?))
            .filter(|n| *n <= byte_limit)
            .ok_or_else(limit)?;
        Ok(Self {
            selection: source.selection().clone(),
            context,
            body: handle.clone(),
            range,
            no_item: proof.clone(),
            arguments: plan.arguments().fingerprint(),
            message,
            seal,
            seal_fingerprint: published.fingerprint(),
            stamps,
            bytes,
            byte_limit,
        })
    }
    fn revalidate(&self, db: &Database, source: &PreparedSourceView) -> Result<(), Diagnostic> {
        if source.selection() != &self.selection
            || db.source_view_namespace_census()?.stamps() != &self.stamps
        {
            return Err(fault("constant macro source/input membership changed"));
        }
        let fresh = Self::mint(db, source, &self.body.key(), self.range, self.byte_limit)?;
        if self.identity() != fresh.identity() {
            return Err(fault("constant macro current selection/operands changed"));
        }
        Ok(())
    }
    fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v1/current-constant-static-panic-source");
        e.string(self.selection.path.as_str());
        e.digest(self.selection.location_identity());
        e.record(&self.selection.outline.parse);
        e.digest(self.selection.outline.cfg);
        e.record(&self.context);
        e.record(&self.body);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.record(&self.no_item);
        e.digest(self.arguments);
        e.string(&self.message);
        e.record(&self.seal);
        e.digest(self.seal_fingerprint);
        e.u64(self.stamps.len() as u64);
        for (key, value) in &self.stamps {
            e.record(key);
            e.digest(*value);
        }
        e.finish()
    }
}
