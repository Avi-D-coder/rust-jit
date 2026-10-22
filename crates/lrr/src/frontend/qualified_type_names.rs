//! Source-bound qualified Type preparation through the ordinary resolver.
//! Read observations demand current scope/bucket evidence; they are not proof.
use super::{ModuleRoute, PreparedSourceLocalPlan, error};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::{CurrentModuleNameKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        names::{NameLookup, reads::NamespaceReadCertification},
        source_body_context::type_occurrences::{BodyTypeOrigin, BodyTypeSegment, BodyTypeShape},
        source_locals::{OrdinaryNameSite, PrefixResolution},
    },
    ir::pc::{NodeId, PcKey},
    source::manifest::Namespace,
    syntax::{imports::ImportSegment, token_wire::Key},
};
use std::sync::Arc;
macro_rules! polled {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
macro_rules! checked {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
    };
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum QualifiedTypeDemand {
    SourceOccurrence,
    PathAdjustment,
    LocalPrefix,
    AssociatedType,
    ExpansionDefinition(Key),
    DefinitionModule(Key),
    SourceOwner(Digest),
    ScopeCertificate(Digest),
    ModuleCompletion(Digest),
    DeclarationPublication(Digest),
    DeclarationAdjustment(Digest),
}
/// Private construction joins an actual owning plan, indexed occurrence,
/// ordinary hygienic resolver result and all its observed namespace demands.
#[derive(Clone)]
pub struct QualifiedTypeObservation {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    lookup: Option<NameLookup>,
    module: Option<CurrentModuleNameKey>,
    demand: Option<QualifiedTypeDemand>,
    reads: Digest,
    fingerprint: Digest,
    bytes: usize,
}
impl std::fmt::Debug for QualifiedTypeObservation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QualifiedTypeObservation")
            .field("key", &self.key)
            .field("demand", &self.demand)
            .finish_non_exhaustive()
    }
}
impl QualifiedTypeObservation {
    pub fn key(&self) -> &PcKey {
        &self.key
    }
    pub fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    /// Exclusive owned key/outcome storage. The actual shared plan is accounted
    /// separately through its existing source-plan roots, not copied here.
    pub fn estimated_bytes(&self) -> usize {
        self.bytes
    }
    pub fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn demand(&self) -> Option<QualifiedTypeDemand> {
        self.demand
    }
    pub(crate) fn module_request(&self) -> Option<&CurrentModuleNameKey> {
        self.module.as_ref()
    }
    pub(crate) fn lookup(&self) -> Option<&NameLookup> {
        self.lookup.as_ref()
    }
    fn finish(mut self) -> Result<Self, Diagnostic> {
        self.bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.lookup
                    .as_ref()
                    .map_or(Ok(0), |answer| answer.metadata_bytes())?,
            )
            .and_then(|n| n.checked_add(self.module.as_ref().map_or(0, |key| key.heap_bytes())))
            .ok_or_else(crate::runtime::arena::resource)?;
        self.fingerprint = StableEncode::fingerprint(&self);
        Ok(self)
    }
    fn pending(mut self, demand: QualifiedTypeDemand) -> QueryPoll<Self> {
        self.demand = Some(demand);
        QueryPoll::Ready(self.finish())
    }
}
impl StableEncode for QualifiedTypeObservation {
    const DOMAIN: &'static str = "lrr/v1/qualified-body-type-observation";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        // The actual source/body/context is relevant. The whole completed graph
        // or its seal issuance identity is not a semantic result fingerprint.
        e.digest(self.plan.context().source().identity());
        e.digest(self.reads);
        e.option(&self.lookup, |e, answer| e.record(answer));
        e.option(&self.module, |e, module| e.record(module));
        e.option(&self.demand, |e, demand| match demand {
            QualifiedTypeDemand::SourceOccurrence => e.u8(0),
            QualifiedTypeDemand::PathAdjustment => e.u8(1),
            QualifiedTypeDemand::LocalPrefix => e.u8(2),
            QualifiedTypeDemand::AssociatedType => e.u8(3),
            QualifiedTypeDemand::ExpansionDefinition(key) => {
                e.u8(4);
                e.digest(Digest(key.0));
            }
            QualifiedTypeDemand::DefinitionModule(key) => {
                e.u8(5);
                e.digest(Digest(key.0));
            }
            QualifiedTypeDemand::SourceOwner(key) => {
                e.u8(6);
                e.digest(*key);
            }
            QualifiedTypeDemand::ScopeCertificate(key) => {
                e.u8(7);
                e.digest(*key);
            }
            QualifiedTypeDemand::ModuleCompletion(key) => {
                e.u8(8);
                e.digest(*key);
            }
            QualifiedTypeDemand::DeclarationPublication(key) => {
                e.u8(9);
                e.digest(*key);
            }
            QualifiedTypeDemand::DeclarationAdjustment(key) => {
                e.u8(10);
                e.digest(*key);
            }
        });
    }
}
/// Pure query provider entry. No source capture, graph publication or runtime
/// execution occurs here, and every path uses the existing ordinary kernel.
pub fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<QualifiedTypeObservation> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(error("qualified Type query requires Entry")));
    }
    let value = polled!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(key.body.clone())));
    let QueryValue::SourceLocalPlan(plan) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(error("qualified Type source plan variant differs")));
    };
    let plan = Arc::clone(plan);
    if plan.context().source().index().body() != &key.body {
        return QueryPoll::Ready(Err(error("qualified Type owning body differs")));
    }
    polled!(plan.require_current(db));
    let mut proof = QualifiedTypeObservation {
        key: key.clone(),
        plan: Arc::clone(&plan),
        lookup: None,
        module: None,
        demand: None,
        reads: Digest([0; 32]),
        fingerprint: Digest([0; 32]),
        bytes: 0,
    };
    let Some(occurrence) = polled!(plan.context().body_type_occurrence(db, NodeId(key.node)))
    else {
        return proof.pending(QualifiedTypeDemand::SourceOccurrence);
    };
    // A generic application's owner (`m::W<u8>`) is selected like any
    // qualified Type; its written arguments are separate occurrences, and only
    // the generic-base kernel reaches this query for that shape.
    if !matches!(
        occurrence.shape(),
        BodyTypeShape::Named | BodyTypeShape::NeedGenericArguments
    ) || matches!(
            occurrence.origin(),
            BodyTypeOrigin::NeedGeneratedBoundary | BodyTypeOrigin::NeedRetainedOccurrence
        )
    {
        return proof.pending(QualifiedTypeDemand::SourceOccurrence);
    }
    let Some(environment) = plan.environment() else {
        return proof.pending(QualifiedTypeDemand::LocalPrefix);
    };
    let environment = environment.macros().source_environment();
    if let Some(missing) = plan.missing_expansions.first() {
        return proof.pending(QualifiedTypeDemand::ExpansionDefinition(*missing));
    }
    let (root, hygiene) = plan.entered.local_hygiene();
    let mut parts = Vec::new();
    let mut first = None;
    let mut last = None;
    let mut normalized = None;
    let mut defining_root = None;
    let mut path_bytes = 0usize;
    let mut path_identity = StableEncoder::new("lrr/v1/qualified-type-occurrence");
    path_identity.bool(occurrence.absolute());
    for (ordinal, segment) in occurrence.segments().iter().enumerate() {
        let (identifier, part) = match segment {
            BodyTypeSegment::Name(name) => (name, ImportSegment::Name(name.name().to_owned())),
            BodyTypeSegment::Crate(name) => (name, ImportSegment::Crate),
            BodyTypeSegment::SelfModule(name) => (name, ImportSegment::SelfModule),
            BodyTypeSegment::Super(name) => (name, ImportSegment::Super),
            BodyTypeSegment::DollarCrate(name, unit) => {
                if ordinal != 0 || occurrence.absolute() { return proof.pending(QualifiedTypeDemand::PathAdjustment); }
                let Some(unit) = unit else { return proof.pending(QualifiedTypeDemand::SourceOccurrence); };
                defining_root = Some(*unit);
                (name, ImportSegment::Crate)
            }
            BodyTypeSegment::SelfType(_) | BodyTypeSegment::NeedQualifiedType => {
                return proof.pending(QualifiedTypeDemand::PathAdjustment);
            }
        };
        path_bytes = checked!(
            path_bytes
                .checked_add(identifier.name().len())
                .ok_or_else(crate::runtime::arena::resource)
        );
        let raw = checked!(
            plan.context()
                .source()
                .identifier_context_or_original(identifier.range(), root)
        );
        if identifier.raw_context().is_some_and(|actual| actual != raw) {
            return QueryPoll::Ready(Err(error("qualified Type retained token context differs")));
        }
        // A path rooted at a `$crate` token resolves from that definition crate's
        // root through ordinary module members, whatever edition spelled its
        // segments (a library macro's output in a source of another edition).
        if defining_root.is_none()
            && identifier.edition() != plan.entered.parent.source().outline.parse.profile.edition
        {
            // The legacy ordinary path kernel takes one owning source edition;
            // mixed-origin path segment editions need its pinned adjustment join.
            return proof.pending(QualifiedTypeDemand::PathAdjustment);
        }
        let item = checked!(hygiene.name_item(raw));
        if matches!(segment, BodyTypeSegment::DollarCrate(..)) {
            if Some(Digest(checked!(hygiene.dollar_crate(raw)).0)) != defining_root {
                return QueryPoll::Ready(Err(error("Type dollar-crate root differs from actual token context")));
            }
        } else {
            if normalized.is_some_and(|prior| prior != item) {
                return proof.pending(QualifiedTypeDemand::PathAdjustment);
            }
            normalized = Some(item);
        }
        path_identity.string(identifier.name());
        path_identity.u16(identifier.edition());
        path_identity.u32(identifier.range().start);
        path_identity.u32(identifier.range().end);
        path_identity.digest(Digest(raw.0));
        path_identity.digest(Digest(item.0.0));
        first.get_or_insert((identifier, raw));
        last = Some(identifier);
        checked!(
            parts
                .try_reserve(1)
                .map_err(|_| crate::runtime::arena::resource())
        );
        parts.push(part);
    }
    let (Some((first, context)), Some(last)) = (first, last) else {
        return proof.pending(QualifiedTypeDemand::SourceOccurrence);
    };
    if defining_root.is_some() && parts.len() == 1 {
        // The crate root alone names a module, not a completed source Type.
        return proof.pending(QualifiedTypeDemand::PathAdjustment);
    }
    // A path of only keywords (`crate`, `self`, `super`, `super::super`) names
    // a module, never a declared Type: the owner of `crate::f()` or
    // `super::f()` is the module route's, with no Type declaration to certify.
    if parts
        .iter()
        .all(|part| !matches!(part, ImportSegment::Name(_)))
    {
        proof.lookup = Some(NameLookup::Absent);
        proof.reads = path_identity.finish();
        return QueryPoll::Ready(proof.finish());
    }
    let last_name = checked!(crate::frontend::names::owned_name(
        last.name(),
        last.edition()
    ));
    if defining_root.is_none() {
        match checked!(plan.lexical_module.route(&hygiene, context)) {
            ModuleRoute::Definition(expansion) => {
                return proof.pending(QualifiedTypeDemand::DefinitionModule(expansion));
            }
            ModuleRoute::Lexical(_) => {}
        }
    }
    let site = OrdinaryNameSite {
        source: plan.entered.parent.source(),
        body: occurrence.body(),
        at: u64::from(first.range().start),
        name: first.name(),
        context,
        namespace: Namespace::Type,
    };
    if matches!(
        occurrence.segments().first(),
        Some(BodyTypeSegment::Name(_))
    ) && !occurrence.absolute()
    {
        match checked!(plan.adapter().lookup_prepared_type_prefix(
            &site,
            &plan.type_names,
            first.edition()
        )) {
            PrefixResolution::Parameter(_) => {
                return proof.pending(QualifiedTypeDemand::AssociatedType);
            }
            PrefixResolution::NeedDeclaration(_) | PrefixResolution::NeedOwnerGeneric => {
                return proof.pending(QualifiedTypeDemand::LocalPrefix);
            }
            // Import resolution is supplied by the SAME qualified kernel below.
            PrefixResolution::NeedImport(_) | PrefixResolution::ModuleTail => {}
            PrefixResolution::Name(_) => return proof.pending(QualifiedTypeDemand::AssociatedType),
        }
    }
    let (lookup, reads) = checked!(plan.adapter().lookup_qualified_module_rooted_observed(
        site,
        &parts,
        occurrence.absolute(),
        environment.names(),
        defining_root,
    ));
    proof.lookup = Some(lookup);
    let selected = match proof.lookup.as_ref() {
        Some(NameLookup::Defined(selected)) => Some(selected.clone()),
        _ => None,
    };
    let declaration = if let Some(selected) = &selected {
        match db.input(&InputKey::DeclarationContext(selected.identity)) {
            Some(value) => match value.as_ref() {
                InputValue::DeclarationContext(context) => Some(context.scope.clone()),
                _ => {
                    return QueryPoll::Ready(Err(error(
                        "qualified declaration context variant differs",
                    )));
                }
            },
            None => {
                return proof.pending(QualifiedTypeDemand::DeclarationPublication(
                    selected.identity,
                ));
            }
        }
    } else {
        None
    };
    match polled!(reads.certify(
        db,
        environment,
        path_identity.finish(),
        |request, answer| {
            if selected.as_ref().is_some_and(|selected| {
                matches!(answer.lookup(),
                Some(NameLookup::Defined(found)) if found == selected)
            }) && declaration.as_ref() == Some(&request.bucket.scope)
                && request.bucket.name == last_name.as_str()
            {
                proof.module = Some(request.clone());
            }
            Ok(())
        }
    )) {
        NamespaceReadCertification::Complete(identity) => proof.reads = identity,
        NamespaceReadCertification::PendingOwner(identity) => {
            return proof.pending(QualifiedTypeDemand::SourceOwner(identity));
        }
        NamespaceReadCertification::PendingScope(identity) => {
            return proof.pending(QualifiedTypeDemand::ScopeCertificate(identity));
        }
        NamespaceReadCertification::PendingName(identity) => {
            return proof.pending(QualifiedTypeDemand::ModuleCompletion(identity));
        }
    }
    if let Some(selected) = selected
        && proof.module.is_none()
    {
        return proof.pending(QualifiedTypeDemand::DeclarationAdjustment(
            selected.identity,
        ));
    }
    QueryPoll::Ready(proof.finish())
}
