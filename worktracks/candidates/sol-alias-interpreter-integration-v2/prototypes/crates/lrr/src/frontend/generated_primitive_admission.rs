//! Current generated primitive Type admission. Pure source facts only.
//! No source namespace/prelude absence is inferred from printed type spelling.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    db::{
        dependency::{InputKey, InputValue},
        key::BucketKey,
        memo::Database,
        seals::{SealDomain, SealKey},
    },
    expand::source_declarative::SourceDeclarative,
    frontend::{
        generated_constant_provenance::{
            CurrentGeneratedConstant, GeneratedConstantForm, GeneratedConstantPrerequisite,
        },
        generated_type_admission::{GeneratedTypePrerequisite, primitive_origin},
        names::NameLookup,
        type_prelude_admission::{
            CurrentTypePreludeAbsence, TypePreludeAbsenceOutcome, TypePreludePrerequisite,
        },
    },
    ir::op::PrimitiveType,
    source::{changes::crate_policy::PreparedCratePreludePolicy, manifest::Namespace},
    syntax::{
        generated_type_site::{
            GeneratedTypeRole, GeneratedTypeSegmentKind, GeneratedTypeSyntax, WrittenGeneratedType,
        },
        outline::ScopeKey,
        token_wire::Key,
    },
};
use std::collections::BTreeMap;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_ROWS: usize = 100_000;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERATED-PRIMITIVE", message.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "generated primitive admission budget".into(),
    )
}
fn charge(bytes: &mut usize, n: usize, maximum: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(n)
        .filter(|n| *n <= maximum)
        .ok_or_else(limit)?;
    Ok(())
}
fn stamp(
    db: &Database,
    stamps: &mut BTreeMap<InputKey, Digest>,
    key: &InputKey,
    bytes: &mut usize,
    maximum: usize,
) -> Result<(), Diagnostic> {
    let (_, value) = db
        .published_input(key)
        .ok_or_else(|| fault("generated primitive published input absent"))?;
    if let Some(old) = stamps.get(key) {
        return if *old == value {
            Ok(())
        } else {
            Err(fault("generated primitive input changed"))
        };
    };
    if stamps.len() >= MAX_ROWS {
        return Err(limit());
    };
    charge(
        bytes,
        std::mem::size_of::<(InputKey, Digest)>()
            .checked_add(128)
            .and_then(|n| n.checked_add(key.heap_bytes()))
            .ok_or_else(limit)?,
        maximum,
    )?;
    stamps.insert(key.clone(), value);
    Ok(())
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum GeneratedPrimitivePrerequisite {
    WrittenType,
    GeneratedScopePolicy,
    Origin(GeneratedTypePrerequisite),
    TypeNamespace(ScopeKey),
    SourceType(NameLookup),
    CompilerPrimitiveFacet,
    // Existing CertifiedScope is a module/import scope, not a Type prelude.
    TypePrelude,
    TypePreludeSelection(TypePreludePrerequisite),
    UnknownPrimitive,
}
pub(crate) enum GeneratedPrimitiveOutcome {
    Ready(CurrentGeneratedPrimitive),
    Need(GeneratedPrimitivePrerequisite),
}
/// Private immutable source proof; grants no layout, evaluation or runtime access.
pub(crate) struct CurrentGeneratedPrimitive {
    parent: Digest,
    site: WrittenGeneratedType,
    scope: ScopeKey,
    profile: ProfileIdentity,
    context: Key,
    facet: Digest,
    member: Digest,
    primitive: PrimitiveType,
    policy: Digest,
    prelude: Digest,
    stamps: BTreeMap<InputKey, Digest>,
    revision: u64,
    bytes: usize,
}
impl CurrentGeneratedPrimitive {
    pub(crate) fn capture_current(
        db: &Database,
        owner: &SourceDeclarative,
        parent: &CurrentGeneratedConstant,
        providers: &[&SourceDeclarative],
        maximum: usize,
    ) -> Result<GeneratedPrimitiveOutcome, Diagnostic> {
        if maximum > MAX_BYTES
            || providers.len() > MAX_ROWS
            || parent
                .metadata_bytes()
                .checked_mul(2)
                .is_none_or(|n| n > maximum)
        {
            return Err(limit());
        };
        parent.revalidate(db, owner)?;
        if parent
            .prerequisites()
            .contains(&GeneratedConstantPrerequisite::GeneratedScopePolicy)
        {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::GeneratedScopePolicy,
            ));
        }

        let definition = parent.retained().member().fingerprint();
        let Some((InputValue::DefinitionSource(parse), _)) =
            db.published_input(&InputKey::DefinitionSource(definition))
        else {
            return Err(fault("generated primitive definition source absent"));
        };
        let Some((InputValue::GeneratedSyntax(unit), _)) =
            db.published_input(&InputKey::GeneratedSyntax(parse.content))
        else {
            return Err(fault("generated primitive exact syntax absent"));
        };
        let role = match parent.retained().form() {
            GeneratedConstantForm::Scalar { .. } => GeneratedTypeRole::Declaration,
            GeneratedConstantForm::Array { .. } => GeneratedTypeRole::ArrayElement,
        };
        let site = WrittenGeneratedType::capture(
            &unit.items,
            &unit.scope,
            &unit.cfg,
            parent.retained().member(),
            role,
            maximum
                .checked_sub(parent.metadata_bytes())
                .ok_or_else(limit)?,
        )?;
        if site.scope().owner.is_some() {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::WrittenType,
            ));
        };
        if !unit
            .definitions
            .get(&definition)
            .is_some_and(|d| d.fingerprint() == site.declaration_fingerprint())
            || !matches!(db.published_input(&InputKey::DefinitionHeader(definition)),Some((InputValue::DefinitionHeader(h),_))if h.fingerprint()==site.header_fingerprint())
            || !matches!(db.published_input(&InputKey::DeclarationContext(definition)),Some((InputValue::DeclarationContext(c),_))if c.scope==*site.scope()&&c.range==site.declaration())
        {
            return Err(fault(
                "generated primitive registered declaration/header/context differs",
            ));
        }
        let GeneratedTypeSyntax::Path(parts) = site.shape() else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::WrittenType,
            ));
        };
        let [part] = parts.as_slice() else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::WrittenType,
            ));
        };
        let GeneratedTypeSegmentKind::Name(name) = part.kind() else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::WrittenType,
            ));
        };
        let mut bytes = parent
            .metadata_bytes()
            .checked_add(site.metadata_bytes())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .filter(|n| *n <= maximum)
            .ok_or_else(limit)?;
        let mut stamps = BTreeMap::new();
        let origin = match primitive_origin(
            db,
            owner,
            providers,
            unit,
            &site,
            part,
            &mut stamps,
            &mut bytes,
            maximum,
        )? {
            Ok(origin) => origin,
            Err(need) => {
                return Ok(GeneratedPrimitiveOutcome::Need(
                    GeneratedPrimitivePrerequisite::Origin(need),
                ));
            }
        };
        let source = origin.provider.module_source();
        let profile = &source.selection().outline.parse.profile;
        profile.validate_source()?;
        source.require_original_hygiene()?;
        let seal_key = SealKey {
            domain: SealDomain::Scope {
                id: origin.scope.fingerprint(),
                namespace: Namespace::Type,
            },
            unit_graph: unit.seal.unit_graph,
            epoch: unit.seal.epoch,
        };
        let Some((InputValue::DiscoverySeal(input), _)) =
            db.published_input(&InputKey::DiscoverySeal(seal_key.clone()))
        else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::TypeNamespace(origin.scope),
            ));
        };
        input.validate_compiler_primitive_receipts()?;
        let certified = input
            .scope
            .as_ref()
            .ok_or_else(|| fault("generated primitive Type certificate absent"))?;
        if input.seal.key() != seal_key
            || input.seal.unresolved_frontiers() != 0
            || certified.scope != origin.scope
            || certified.namespace != Namespace::Type
            || certified.edition != profile.edition
            || !input.sources.contains(source.selection())
            || !matches!(db.published_input(&InputKey::ExpansionEpoch),Some((InputValue::ExpansionEpoch(epoch),_))if *epoch==seal_key.epoch)
            || !matches!(db.published_input(&InputKey::SourceDependencies),Some((InputValue::SourceDependencies(graph),_))if graph.fingerprint()==input.source_dependencies)
            || !matches!(db.published_input(&InputKey::CapturedSource(source.selection().location_identity())),Some((InputValue::CapturedSource(actual),_))if actual==source.selection())
            || !matches!(db.published_input(&InputKey::CrateCfg(source.selection().outline.crate_key)),Some((InputValue::CrateCfg(actual),_))if actual.fingerprint()==source.selection().outline.cfg)
        {
            return Err(fault(
                "generated primitive owning source/profile/cfg/Type world differs",
            ));
        }
        if input.compiler_inputs.len() > MAX_ROWS || input.records.len() > MAX_ROWS {
            return Err(limit());
        };
        for (key, expected) in &input.compiler_inputs {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != *expected)
            {
                return Err(fault("generated primitive compiler inputs changed"));
            };
            stamp(db, &mut stamps, key, &mut bytes, maximum)?;
        }
        for descriptor in &input.records {
            let key = InputKey::ExpansionRecord(descriptor.key.fingerprint());
            if !matches!(db.published_input(&key),Some((InputValue::ExpansionRecord(actual),_))if actual.descriptor().fingerprint()==descriptor.fingerprint())
            {
                return Err(fault("generated primitive Type expansion record differs"));
            };
            stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
        }
        charge(
            &mut bytes,
            name.capacity()
                .checked_add(origin.scope.heap_bytes())
                .ok_or_else(limit)?,
            maximum,
        )?;
        let bucket = BucketKey {
            scope: origin.scope.clone(),
            namespace: Namespace::Type,
            name: name.clone(),
        };
        let lookup = certified.lookup(&bucket)?;
        if lookup != NameLookup::Absent {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::SourceType(lookup),
            ));
        };
        let Some(catalog) = &origin.provider.compiler else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::CompilerPrimitiveFacet,
            ));
        };
        charge(
            &mut bytes,
            catalog
                .owning_profile_metadata_bytes()?
                .checked_mul(2)
                .ok_or_else(limit)?,
            maximum,
        )?;
        let Some(receipt) = input.compiler_primitive_receipt(source.selection())? else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::CompilerPrimitiveFacet,
            ));
        };
        if !receipt.matches(source.selection(), catalog)? {
            return Err(fault(
                "generated primitive actual published owning compiler facet differs",
            ));
        }
        let Some(facet) = catalog.primitive_facet(profile)? else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::CompilerPrimitiveFacet,
            ));
        };
        // Reserve captured policy plus its immutable replay and returned policy
        // clone before allocating it. This is owning-crate policy, not a guessed
        // token attribute or a caller-selected standard/prelude boolean.
        let residual = maximum.checked_sub(bytes).ok_or_else(limit)?;
        let policy = PreparedCratePreludePolicy::capture_current(
            source,
            seal_key.epoch,
            residual.checked_div(2).ok_or_else(limit)?,
        )?;
        charge(
            &mut bytes,
            policy.metadata_bytes().checked_mul(2).ok_or_else(limit)?,
            maximum,
        )?;
        if !matches!(db.published_input(&InputKey::SourceDependencies), Some((InputValue::SourceDependencies(graph),_)) if graph.root_for(origin.scope.crate_key)==Some(policy.owning_root_selection()))
        {
            return Err(fault(
                "generated primitive owning crate root selection changed",
            ));
        }
        for retained in policy.source_selections() {
            if !matches!(db.published_input(&InputKey::CapturedSource(retained.location_identity())),Some((InputValue::CapturedSource(actual),_))if actual==retained)
                || !matches!(db.published_input(&InputKey::SourceBlob(retained.outline.parse.content)),Some((InputValue::SourceBlob(blob),_))if blob.content()==retained.outline.parse.content)
                || !matches!(db.published_input(&InputKey::CrateCfg(retained.outline.crate_key)),Some((InputValue::CrateCfg(cfg),_))if cfg.fingerprint()==retained.outline.cfg)
            {
                return Err(fault(
                    "generated primitive owning policy retained source publication differs",
                ));
            }
            for key in [
                InputKey::CapturedSource(retained.location_identity()),
                InputKey::SourceBlob(retained.outline.parse.content),
                InputKey::CrateCfg(retained.outline.crate_key),
            ] {
                stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
            }
        }
        let actual_policy = policy.policy();
        let local = origin
            .scope
            .module_path
            .strip_prefix(source.selection().outline.module_path.as_slice())
            .ok_or_else(|| fault("generated primitive owning policy scope differs"))?;
        let _owning_policy_is_retained = (&actual_policy, local);
        let prelude = match CurrentTypePreludeAbsence::capture_current(
            db,
            origin.provider,
            &origin.scope,
            &seal_key,
            name,
            maximum.checked_sub(bytes).ok_or_else(limit)?,
        )? {
            TypePreludeAbsenceOutcome::Absent(proof) => proof,
            TypePreludeAbsenceOutcome::Need(need) => {
                return Ok(GeneratedPrimitiveOutcome::Need(
                    GeneratedPrimitivePrerequisite::TypePreludeSelection(need),
                ));
            }
        };
        charge(&mut bytes, prelude.metadata_bytes(), maximum)?;
        for key in prelude.dependencies().keys() {
            stamp(db, &mut stamps, key, &mut bytes, maximum)?;
        }

        let Some(primitive) = facet.select(name) else {
            return Ok(GeneratedPrimitiveOutcome::Need(
                GeneratedPrimitivePrerequisite::UnknownPrimitive,
            ));
        };
        charge(
            &mut bytes,
            profile
                .semantics
                .capacity()
                .checked_add(origin.scope.heap_bytes())
                .ok_or_else(limit)?,
            maximum,
        )?;
        for key in [
            InputKey::DefinitionSource(definition),
            InputKey::DefinitionHeader(definition),
            InputKey::DeclarationContext(definition),
            InputKey::GeneratedSyntax(parse.content),
            InputKey::ExpansionEpoch,
            InputKey::SourceDependencies,
            InputKey::DiscoverySeal(seal_key),
            InputKey::CapturedSource(source.selection().location_identity()),
            InputKey::SourceBlob(source.selection().outline.parse.content),
            InputKey::CrateCfg(source.selection().outline.crate_key),
        ] {
            stamp(db, &mut stamps, &key, &mut bytes, maximum)?;
        }
        let context = part.span().context;
        let facet = facet.identity();
        let member = catalog
            .primitive_facet(profile)?
            .ok_or_else(|| fault("primitive facet disappeared"))?
            .member_identity(primitive);
        Ok(GeneratedPrimitiveOutcome::Ready(Self {
            parent: parent.identity(),
            site,
            scope: origin.scope,
            profile: profile.clone(),
            context,
            facet,
            member,
            primitive,
            policy: policy.identity(),
            prelude: prelude.identity(),
            stamps,
            revision: db.revision(),
            bytes,
        }))
    }
    pub(crate) fn primitive(&self) -> PrimitiveType {
        self.primitive
    }
    pub(crate) fn profile(&self) -> &ProfileIdentity {
        &self.profile
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn member_identity(&self) -> Digest {
        self.member
    }
    pub(crate) fn site(&self) -> &WrittenGeneratedType {
        &self.site
    }
    pub(crate) fn dependencies(&self) -> &BTreeMap<InputKey, Digest> {
        &self.stamps
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/current-generated-primitive-source/type-prelude-v1");
        e.digest(self.parent);
        e.digest(self.site.identity());
        e.record(&self.scope);
        e.record(&self.profile);
        e.digest(Digest(self.context.0));
        e.digest(self.facet);
        e.digest(self.member);
        e.u32(self.primitive.tag());
        e.digest(self.policy);
        e.digest(self.prelude);
        e.u64(u64::try_from(self.stamps.len()).expect("bounded primitive input count"));
        for (key, value) in &self.stamps {
            e.record(key);
            e.digest(*value)
        }
        e.finish()
    }
    pub(crate) fn revalidate(
        &self,
        db: &Database,
        owner: &SourceDeclarative,
        parent: &CurrentGeneratedConstant,
        providers: &[&SourceDeclarative],
        maximum: usize,
    ) -> Result<(), Diagnostic> {
        if db.revision() != self.revision || self.parent != parent.identity() || maximum > MAX_BYTES
        {
            return Err(fault(
                "generated primitive parent/current publication differs",
            ));
        };
        for (key, value) in &self.stamps {
            if db
                .published_input(key)
                .is_none_or(|(_, actual)| actual != *value)
            {
                return Err(fault("generated primitive dependency differs"));
            }
        }
        let residual = maximum.checked_sub(self.bytes).ok_or_else(limit)?;
        match Self::capture_current(db, owner, parent, providers, residual)? {
            GeneratedPrimitiveOutcome::Ready(current) if current.identity() == self.identity() => {
                Ok(())
            }
            _ => Err(fault(
                "generated primitive actual source/mark/Type route differs",
            )),
        }
    }
}
#[cfg(test)]
#[path = "generated_primitive_admission_tests.rs"]
mod tests;
