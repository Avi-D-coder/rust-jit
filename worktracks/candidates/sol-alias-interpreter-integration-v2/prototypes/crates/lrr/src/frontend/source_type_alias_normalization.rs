//! Current source-owned primitive alias chains. No symbolic recipe or descriptor grants authority.
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
    expand::source_declarative::SourceDeclarative,
    frontend::{
        generated_constant_provenance::CurrentGeneratedConstant,
        generated_type_admission::{
            GeneratedTypePrerequisite, SelectedGeneratedType, SelectedSourceType, primitive_origin,
        },
        names::{NameDefinition, NameLookup},
        type_prelude_admission::{
            CurrentTypePreludeAbsence, TypePreludeAbsenceOutcome, TypePreludePrerequisite,
        },
    },
    ir::op::PrimitiveType,
    source::manifest::{DefKey, ItemKind, Namespace},
    syntax::{
        generated_type_site::{
            GeneratedTypeRole, GeneratedTypeSegmentKind, GeneratedTypeSyntax, WrittenGeneratedType,
        },
        outline::{DeclarationContext, ScopeKey},
        parse::{ParseKey, ParsedFile},
        type_aliases::TypeAliasRecipe,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams},
};
use std::collections::BTreeMap;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_STEPS: usize = 256;
fn fault(s: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-ALIAS", s.into())
}
fn limit() -> Diagnostic {
    Diagnostic::new("LRR-E-METADATA-LIMIT", "source alias proof budget".into())
}
fn charge(bytes: &mut usize, n: usize, max: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(n)
        .filter(|n| *n <= max)
        .ok_or_else(limit)?;
    Ok(())
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum SourceAliasPrerequisite {
    TypeNamespace(ScopeKey),
    Name(NameLookup),
    RetainedDeclaration(Digest),
    GenericSubstitution(Digest),
    OwnerContext(ScopeKey),
    WrittenTarget(Digest),
    GeneratedOwner(Digest),
    GeneratedAliasHop(Digest),
    Origin(GeneratedTypePrerequisite),
    Prelude(TypePreludePrerequisite),
    CompilerPrimitiveFacet,
    UnknownPrimitive,
}
pub(crate) enum SourceAliasOutcome {
    Ready(CurrentSourcePrimitiveAlias),
    Need(SourceAliasPrerequisite),
}
/// Each step names its actual immutable source member, never an inferred runtime identity.
struct AliasStep {
    member: Digest,
    definition: DefKey,
    parse: ParseKey,
    context: DeclarationContext,
    header: Digest,
    recipe: Digest,
    written: Option<Digest>,
}
pub(crate) struct CurrentSourcePrimitiveAlias {
    seed: Digest,
    integrity: Digest,
    steps: Vec<AliasStep>,
    primitive: PrimitiveType,
    compiler_member: Digest,
    terminal_scope: ScopeKey,
    prelude: Digest,
    facet: Digest,
    stamps: BTreeMap<InputKey, Digest>,
    revision: u64,
    bytes: usize,
}
struct Capture {
    steps: Vec<AliasStep>,
    stamps: BTreeMap<InputKey, Digest>,
    bytes: usize,
    max: usize,
}
impl Capture {
    fn new(max: usize) -> Result<Self, Diagnostic> {
        if max > MAX_BYTES || max < std::mem::size_of::<CurrentSourcePrimitiveAlias>() {
            return Err(limit());
        }
        Ok(Self {
            steps: Vec::new(),
            stamps: BTreeMap::new(),
            bytes: std::mem::size_of::<CurrentSourcePrimitiveAlias>(),
            max,
        })
    }
    fn residual(&self) -> Result<usize, Diagnostic> {
        self.max.checked_sub(self.bytes).ok_or_else(limit)
    }
    fn stamp(&mut self, db: &Database, key: InputKey) -> Result<(), Diagnostic> {
        let (_, value) = db
            .published_input(&key)
            .ok_or_else(|| fault("alias current published input absent"))?;
        if let Some(old) = self.stamps.get(&key) {
            return if *old == value {
                Ok(())
            } else {
                Err(fault("alias input changed during capture"))
            };
        };
        if self.stamps.len() >= 100_000 {
            return Err(limit());
        };
        charge(
            &mut self.bytes,
            std::mem::size_of::<(InputKey, Digest)>()
                .checked_add(128)
                .and_then(|n| n.checked_add(key.heap_bytes()))
                .ok_or_else(limit)?,
            self.max,
        )?;
        self.stamps.insert(key, value);
        Ok(())
    }
    fn scope<'a>(
        &mut self,
        db: &'a Database,
        owner: &SourceDeclarative,
        scope: &ScopeKey,
        key: &SealKey,
    ) -> Result<Option<&'a DiscoveryInput>, Diagnostic> {
        if scope.owner.is_some() {
            return Ok(None);
        };
        let source = owner.module_source();
        source.require_original_hygiene()?;
        if scope.crate_key != source.selection().outline.crate_key
            || !scope
                .module_path
                .starts_with(&source.selection().outline.module_path)
        {
            return Err(fault("alias scope/source owner differs"));
        };
        if key.domain
            != (SealDomain::Scope {
                id: scope.fingerprint(),
                namespace: Namespace::Type,
            })
        {
            return Err(fault("alias Type seal domain differs"));
        };
        let Some((InputValue::DiscoverySeal(input), _)) =
            db.published_input(&InputKey::DiscoverySeal(key.clone()))
        else {
            return Ok(None);
        };
        let Some(cert) = &input.scope else {
            return Err(fault("alias Type scope certificate absent"));
        };
        if input.seal.key() != *key
            || input.seal.unresolved_frontiers() != 0
            || cert.scope != *scope
            || cert.namespace != Namespace::Type
            || cert.edition != source.outline().profile.edition
            || !input.sources.contains(source.selection())
            || !matches!(db.published_input(&InputKey::ExpansionEpoch),Some((InputValue::ExpansionEpoch(e),_))if *e==key.epoch)
            || !matches!(db.published_input(&InputKey::SourceDependencies),Some((InputValue::SourceDependencies(g),_))if g.fingerprint()==input.source_dependencies)
            || !matches!(db.published_input(&InputKey::CapturedSource(source.selection().location_identity())),Some((InputValue::CapturedSource(s),_))if s==source.selection())
            || !matches!(db.published_input(&InputKey::SourceBlob(source.selection().outline.parse.content)),Some((InputValue::SourceBlob(b),_))if b.content()==source.selection().outline.parse.content)
            || !matches!(db.published_input(&InputKey::CrateCfg(scope.crate_key)),Some((InputValue::CrateCfg(c),_))if c.fingerprint()==source.selection().outline.cfg)
        {
            return Err(fault("alias current source/edition/cfg/world differs"));
        };
        if input.sources.is_empty()
            || input.sources.len() > 100_000
            || input.compiler_inputs.len() > 100_000
            || input.records.len() > 100_000
        {
            return Err(limit());
        };
        for retained in &input.sources {
            charge(&mut self.bytes, retained.metadata_bytes()?, self.max)?;
            if !matches!(db.published_input(&InputKey::CapturedSource(retained.location_identity())), Some((InputValue::CapturedSource(actual), _)) if actual == retained)
                || !matches!(db.published_input(&InputKey::SourceBlob(retained.outline.parse.content)), Some((InputValue::SourceBlob(actual), _)) if actual.content() == retained.outline.parse.content)
                || !matches!(db.published_input(&InputKey::CrateCfg(retained.outline.crate_key)), Some((InputValue::CrateCfg(actual), _)) if actual.fingerprint() == retained.outline.cfg)
            {
                return Err(fault("alias Type retained source/content/cfg changed"));
            }
            for k in [
                InputKey::CapturedSource(retained.location_identity()),
                InputKey::SourceBlob(retained.outline.parse.content),
                InputKey::CrateCfg(retained.outline.crate_key),
            ] {
                self.stamp(db, k)?;
            }
        }
        for (k, v) in &input.compiler_inputs {
            if db.published_input(k).is_none_or(|(_, a)| a != *v) {
                return Err(fault("alias compiler selection changed"));
            };
            self.stamp(db, k.clone())?;
        }
        for d in &input.records {
            let k = InputKey::ExpansionRecord(d.key.fingerprint());
            if !matches!(db.published_input(&k),Some((InputValue::ExpansionRecord(r),_))if r.descriptor().fingerprint()==d.fingerprint())
            {
                return Err(fault("alias Type record changed"));
            };
            self.stamp(db, k)?;
        }
        for k in [
            InputKey::DiscoverySeal(key.clone()),
            InputKey::ExpansionEpoch,
            InputKey::SourceDependencies,
            InputKey::CapturedSource(source.selection().location_identity()),
            InputKey::CrateCfg(scope.crate_key),
            InputKey::SourceBlob(source.selection().outline.parse.content),
        ] {
            self.stamp(db, k)?;
        }
        Ok(Some(input))
    }
    fn push(
        &mut self,
        member: Digest,
        definition: &DefKey,
        parse: &ParseKey,
        context: &DeclarationContext,
        recipe: &TypeAliasRecipe,
        written: Option<Digest>,
    ) -> Result<(), Diagnostic> {
        if self.steps.iter().any(|s| s.member == member) {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-ALIAS-CYCLE",
                "reached source alias identity repeats".into(),
            ));
        };
        if self.steps.len() >= MAX_STEPS {
            return Err(limit());
        };
        let mut n = std::mem::size_of::<AliasStep>()
            .checked_add(recipe.estimated_bytes())
            .and_then(|n| n.checked_add(context.scope.heap_bytes()))
            .and_then(|n| n.checked_add(parse.profile.semantics.capacity()))
            .and_then(|n| n.checked_add(definition.declared_name.capacity()))
            .and_then(|n| {
                n.checked_add(
                    definition
                        .module_path
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())?,
                )
            })
            .ok_or_else(limit)?;
        for p in &definition.module_path {
            n = n.checked_add(p.capacity()).ok_or_else(limit)?;
        }
        charge(&mut self.bytes, n, self.max)?;
        self.steps
            .try_reserve_exact(1)
            .map_err(|_| crate::runtime::arena::resource())?;
        self.steps.push(AliasStep {
            member,
            definition: definition.clone(),
            parse: parse.clone(),
            context: context.clone(),
            header: recipe.header.fingerprint(),
            recipe: recipe.fingerprint(),
            written,
        });
        Ok(())
    }
    fn finish(
        mut self,
        db: &Database,
        seed: Digest,
        owner: &SourceDeclarative,
        scope: &ScopeKey,
        key: &SealKey,
        name: &str,
    ) -> Result<SourceAliasOutcome, Diagnostic> {
        let input = match self.scope(db, owner, scope, key)? {
            Some(i) => i,
            None => {
                return Ok(SourceAliasOutcome::Need(
                    SourceAliasPrerequisite::TypeNamespace(scope.clone()),
                ));
            }
        };
        let proof = match CurrentTypePreludeAbsence::capture_current(
            db,
            owner,
            scope,
            key,
            name,
            self.residual()?.checked_div(2).ok_or_else(limit)?,
        )? {
            TypePreludeAbsenceOutcome::Absent(p) => p,
            TypePreludeAbsenceOutcome::Need(n) => {
                return Ok(SourceAliasOutcome::Need(SourceAliasPrerequisite::Prelude(
                    n,
                )));
            }
        };
        charge(
            &mut self.bytes,
            proof.metadata_bytes().checked_mul(2).ok_or_else(limit)?,
            self.max,
        )?;
        for k in proof.dependencies().keys() {
            self.stamp(db, k.clone())?;
        }
        let Some(catalog) = &owner.compiler else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::CompilerPrimitiveFacet,
            ));
        };
        charge(
            &mut self.bytes,
            catalog
                .owning_profile_metadata_bytes()?
                .checked_mul(2)
                .ok_or_else(limit)?,
            self.max,
        )?;
        input.validate_compiler_primitive_receipts()?;
        let Some(receipt) = input.compiler_primitive_receipt(owner.module_source().selection())?
        else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::CompilerPrimitiveFacet,
            ));
        };
        if !receipt.matches(owner.module_source().selection(), catalog)? {
            return Err(fault("alias actual compiler primitive receipt differs"));
        };
        let Some(facet) = catalog.primitive_facet(owner.profile())? else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::CompilerPrimitiveFacet,
            ));
        };
        let Some(primitive) = facet.select(name) else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::UnknownPrimitive,
            ));
        };
        charge(&mut self.bytes, scope.heap_bytes(), self.max)?;
        let mut admitted = CurrentSourcePrimitiveAlias {
            seed,
            integrity: Digest([0; 32]),
            steps: self.steps,
            primitive,
            compiler_member: facet.member_identity(primitive),
            terminal_scope: scope.clone(),
            prelude: proof.identity(),
            facet: facet.identity(),
            stamps: self.stamps,
            revision: db.revision(),
            bytes: self.bytes,
        };
        admitted.integrity = admitted.identity();
        Ok(SourceAliasOutcome::Ready(admitted))
    }
}
// This classifies an exact retained header only; no spelling supplies type authority.
fn requires_substitution(
    parsed: &ParsedFile,
    context: &DeclarationContext,
) -> Result<bool, Diagnostic> {
    let alias = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::TypeAlias::cast)
        .find(|a| crate::syntax::ast_adapter::range(a.syntax().text_range()) == context.range)
        .ok_or_else(|| fault("alias exact retained syntax absent"))?;
    Ok(alias.generic_param_list().is_some()
        || alias.where_clause().is_some()
        || alias.ty().is_some_and(|ty| {
            ty.syntax()
                .descendants()
                .any(|n| ast::GenericArgList::can_cast(n.kind()))
        }))
}
fn seal_for(scope: &ScopeKey, world: &SealKey) -> SealKey {
    SealKey {
        domain: SealDomain::Scope {
            id: scope.fingerprint(),
            namespace: Namespace::Type,
        },
        unit_graph: world.unit_graph,
        epoch: world.epoch,
    }
}
fn provider_for<'a>(
    db: &Database,
    definition: Digest,
    owners: &[&'a SourceDeclarative],
) -> Result<Option<&'a SourceDeclarative>, Diagnostic> {
    let Some((InputValue::DefinitionSource(parse), _)) =
        db.published_input(&InputKey::DefinitionSource(definition))
    else {
        return Ok(None);
    };
    let mut actual: Option<&SourceDeclarative> = None;
    for owner in owners {
        let source = owner.module_source();
        if source.selection().outline.parse != *parse
            || !source
                .outline()
                .items
                .iter()
                .any(|i| i.def_key.fingerprint() == definition)
        {
            continue;
        };
        if let Some(old) = actual {
            if !old.module_source().same_publication(source) {
                return Err(fault("alias original source owner ambiguous"));
            }
        } else {
            actual = Some(owner)
        };
    }
    Ok(actual)
}
impl CurrentSourcePrimitiveAlias {
    /// Exact original header classification; this boolean never grants type authority.
    pub(crate) fn has_qualified_original_target(
        db: &Database,
        member: Digest,
        sources: &[&crate::source::changes::PreparedSourceView],
    ) -> Result<bool, Diagnostic> {
        db.require_coordinator()?;
        if sources.len() > 100_000 {
            return Err(limit());
        }
        let mut found = None;
        for source in sources {
            if let Some(item) = source
                .outline()
                .items
                .iter()
                .find(|i| i.def_key.fingerprint() == member)
            {
                if found.is_some() {
                    return Err(fault("qualified alias original member owner ambiguous"));
                }
                found = Some((*source, item));
            }
        }
        let Some((source, item)) = found else {
            return Ok(false);
        };
        if item.def_key.item_kind != ItemKind::TypeAlias {
            return Ok(false);
        }
        item.scope
            .heap_bytes()
            .checked_add(
                item.header
                    .estimated_bytes()
                    .checked_mul(4)
                    .ok_or_else(limit)?,
            )
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        let context = DeclarationContext {
            scope: item.scope.clone(),
            range: item.declaration_span,
        };
        if !matches!(db.published_input(&InputKey::DefinitionSource(member)),Some((InputValue::DefinitionSource(parse),_)) if parse == &source.selection().outline.parse)
            || !matches!(db.published_input(&InputKey::DefinitionHeader(member)),Some((InputValue::DefinitionHeader(header),_)) if header == &item.header)
            || !matches!(db.published_input(&InputKey::DeclarationContext(member)),Some((InputValue::DeclarationContext(actual),_)) if actual == &context)
        {
            return Err(fault(
                "qualified alias structural source/header/context differs",
            ));
        }
        let Some(parsed) = db.prepared_parse(&source.selection().outline.parse) else {
            return Ok(false);
        };
        Ok(qualified::written_path(&parsed, &item.header, &context, MAX_BYTES)?.is_some())
    }

    /// Readonly owner data for a genuinely retained original module invocation.
    /// Positive admission still requires capture_original_current and its current Type proof.
    pub(crate) fn original_owner_for_generated(
        db: &Database,
        identity: Digest,
        sources: &[&crate::source::changes::PreparedSourceView],
        max: usize,
    ) -> Result<Option<(usize, ScopeKey, String)>, Diagnostic> {
        generated::original_owner_data(db, identity, sources, max)
    }
    /// The caller name is a lookup demand, never a selected definition or primitive identity.
    pub(crate) fn capture_original_current(
        db: &Database,
        owner: &SourceDeclarative,
        scope: &ScopeKey,
        key: &SealKey,
        name: &str,
        owners: &[&SourceDeclarative],
        max: usize,
    ) -> Result<SourceAliasOutcome, Diagnostic> {
        db.require_coordinator()?;
        if owners.len() > 100_000 || name.len() > 65536 {
            return Err(limit());
        };
        let mut capture = Capture::new(max)?;
        charge(
            &mut capture.bytes,
            name.len()
                .checked_add(scope.heap_bytes())
                .ok_or_else(limit)?,
            max,
        )?;
        let Some(input) = capture.scope(db, owner, scope, key)? else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::TypeNamespace(scope.clone()),
            ));
        };
        let lookup = input
            .scope
            .as_ref()
            .ok_or_else(|| fault("alias scope absent"))?
            .lookup(&BucketKey {
                scope: scope.clone(),
                namespace: Namespace::Type,
                name: name.into(),
            })?;
        let NameLookup::Defined(selected) = lookup else {
            return Ok(SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(
                lookup,
            )));
        };
        let mut e = StableEncoder::new("lrr/v1/original-selected-source-alias");
        e.record(&selected);
        e.record(scope);
        e.record(key);
        e.string(name);
        charge(
            &mut capture.bytes,
            owners
                .len()
                .checked_add(1)
                .and_then(|n| n.checked_mul(std::mem::size_of::<&SourceDeclarative>()))
                .ok_or_else(limit)?,
            max,
        )?;
        let all = std::iter::once(owner)
            .chain(owners.iter().copied())
            .collect::<Vec<_>>();
        Self::walk_original(db, selected, e.finish(), key, &all, capture)
    }
    fn walk_original(
        db: &Database,
        mut selected: NameDefinition,
        seed: Digest,
        world: &SealKey,
        owners: &[&SourceDeclarative],
        mut capture: Capture,
    ) -> Result<SourceAliasOutcome, Diagnostic> {
        loop {
            let Some(owner) = provider_for(db, selected.identity, owners)? else {
                return generated::walk_generated(db, selected, seed, world, owners, capture);
            };
            let source = owner.module_source();
            let item = source
                .outline()
                .items
                .iter()
                .find(|i| i.def_key.fingerprint() == selected.identity)
                .ok_or_else(|| fault("alias exact source member absent"))?;
            if selected.namespace != Namespace::Type
                || selected.defining_unit != item.scope.crate_key
                || selected.definition_edition != item.header.profile.edition
            {
                return Err(fault("alias selected unit/edition differs"));
            };
            if item.scope.owner.is_some() {
                return Ok(SourceAliasOutcome::Need(
                    SourceAliasPrerequisite::OwnerContext(item.scope.clone()),
                ));
            };
            if item.def_key.item_kind != ItemKind::TypeAlias {
                return Ok(SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(
                    NameLookup::Defined(selected),
                )));
            };
            let context = DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            };
            let key = seal_for(&context.scope, world);
            let Some(_) = capture.scope(db, owner, &context.scope, &key)? else {
                return Ok(SourceAliasOutcome::Need(
                    SourceAliasPrerequisite::TypeNamespace(context.scope),
                ));
            };
            if !matches!(db.published_input(&InputKey::DefinitionHeader(selected.identity)),Some((InputValue::DefinitionHeader(h),_))if h==&item.header)
                || !matches!(db.published_input(&InputKey::DeclarationContext(selected.identity)),Some((InputValue::DeclarationContext(c),_))if c==&context)
            {
                return Err(fault("alias published exact header/context differs"));
            };
            let Some(parsed) = db.prepared_parse(&source.selection().outline.parse) else {
                return Ok(SourceAliasOutcome::Need(
                    SourceAliasPrerequisite::RetainedDeclaration(selected.identity),
                ));
            };
            charge(
                &mut capture.bytes,
                item.header
                    .estimated_bytes()
                    .checked_mul(4)
                    .ok_or_else(limit)?,
                capture.max,
            )?;
            let recipe = parsed.type_alias_recipe(selected.identity, &item.header, &context)?;
            for k in [
                InputKey::DefinitionSource(selected.identity),
                InputKey::DefinitionHeader(selected.identity),
                InputKey::DeclarationContext(selected.identity),
            ] {
                capture.stamp(db, k)?;
            }
            capture.push(
                selected.identity,
                &item.def_key,
                &source.selection().outline.parse,
                &context,
                &recipe,
                None,
            )?;
            if !recipe.generics.is_empty() || requires_substitution(&parsed, &context)? {
                return Ok(SourceAliasOutcome::Need(
                    SourceAliasPrerequisite::GenericSubstitution(selected.identity),
                ));
            };
            // Never accept recipe.symbolic: lower_type sees spelling before namespace/prelude authority.
            if let Some(path) =
                qualified::written_path(&parsed, &item.header, &context, capture.residual()?)?
            {
                charge(&mut capture.bytes, path.metadata_bytes()?, capture.max)?;
                let Some(lookup) = qualified::lookup(
                    db,
                    owner,
                    &context.scope,
                    &key,
                    &path,
                    owners,
                    &mut capture,
                )?
                else {
                    return Ok(SourceAliasOutcome::Need(
                        SourceAliasPrerequisite::WrittenTarget(selected.identity),
                    ));
                };
                match lookup {
                    NameLookup::Defined(next) => {
                        selected = next;
                        continue;
                    }
                    // A qualified absence is an unresolved export/associated/compiler route,
                    // never primitive fallback in the caller or terminal module.
                    NameLookup::Absent => {
                        return Ok(SourceAliasOutcome::Need(
                            SourceAliasPrerequisite::WrittenTarget(selected.identity),
                        ));
                    }
                    other => {
                        return Ok(SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(
                            other,
                        )));
                    }
                }
            }
            let [token] = recipe.target.as_slice() else {
                return Ok(SourceAliasOutcome::Need(
                    SourceAliasPrerequisite::WrittenTarget(selected.identity),
                ));
            };
            let input = capture
                .scope(db, owner, &context.scope, &key)?
                .ok_or_else(|| fault("alias scope changed"))?;
            let lookup = input
                .scope
                .as_ref()
                .ok_or_else(|| fault("alias scope absent"))?
                .lookup(&BucketKey {
                    scope: context.scope.clone(),
                    namespace: Namespace::Type,
                    name: token.text.clone(),
                })?;
            match lookup {
                NameLookup::Defined(next) => selected = next,
                NameLookup::Absent => {
                    return capture.finish(db, seed, owner, &context.scope, &key, &token.text);
                }
                other => {
                    return Ok(SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(
                        other,
                    )));
                }
            }
        }
    }
    /// A genuine current generated type selection supplies the first alias identity.
    /// General generated alias hops still demand a hygienic selected-member producer.
    pub(crate) fn capture_generated_current(
        db: &Database,
        owner: &SourceDeclarative,
        parent: &CurrentGeneratedConstant,
        selected: &SelectedGeneratedType,
        owners: &[&SourceDeclarative],
        max: usize,
    ) -> Result<SourceAliasOutcome, Diagnostic> {
        db.require_coordinator()?;
        if owners.len() > 100_000 {
            return Err(limit());
        };
        let mut capture = Capture::new(max)?;
        charge(
            &mut capture.bytes,
            selected
                .metadata_bytes()
                .checked_add(parent.metadata_bytes())
                .and_then(|n| n.checked_mul(2))
                .ok_or_else(limit)?,
            max,
        )?;
        selected.revalidate(db, owner, parent, owners)?;
        for k in selected.dependencies().keys() {
            capture.stamp(db, k.clone())?;
        }
        let found = match selected.result() {
            SelectedSourceType::AliasHeader(n) | SelectedSourceType::AliasUnit(n) => n,
            _ => {
                return Ok(SourceAliasOutcome::Need(
                    SourceAliasPrerequisite::WrittenTarget(
                        parent.retained().member().fingerprint(),
                    ),
                ));
            }
        };
        let Some((InputValue::DefinitionSource(parse), _)) =
            db.published_input(&InputKey::DefinitionSource(found.identity))
        else {
            return Err(fault("selected alias source absent"));
        };
        if db
            .published_input(&InputKey::GeneratedSyntax(parse.content))
            .is_none()
        {
            charge(
                &mut capture.bytes,
                owners
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(std::mem::size_of::<&SourceDeclarative>()))
                    .ok_or_else(limit)?,
                max,
            )?;
            let all = std::iter::once(owner)
                .chain(owners.iter().copied())
                .collect::<Vec<_>>();
            return Self::walk_original(
                db,
                found.clone(),
                selected.identity(),
                parent.seal_keys().1,
                &all,
                capture,
            );
        }
        let Some((InputValue::GeneratedSyntax(unit), _)) =
            db.published_input(&InputKey::GeneratedSyntax(parse.content))
        else {
            return Err(fault("alias generated syntax variant differs"));
        };
        if unit.key != *parse {
            return Err(fault("alias generated parse differs"));
        };
        if !unit.items.record().same_record(parent.retained().record())
            || unit.scope != *parent.retained().scope()
        {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::GeneratedOwner(found.identity),
            ));
        };
        let definition = unit
            .definitions
            .get(&found.identity)
            .ok_or_else(|| fault("alias generated canonical member absent"))?;
        let header = unit
            .items
            .items()
            .iter()
            .find(|h| h.key.fingerprint() == found.identity)
            .ok_or_else(|| fault("alias generated retained header absent"))?;
        if definition.item_kind != ItemKind::TypeAlias
            || found.defining_unit != unit.scope.crate_key
            || found.definition_edition != unit.key.profile.edition
        {
            return Err(fault("alias generated selected owner/edition differs"));
        };
        let Some((InputValue::DefinitionHeader(h), _)) =
            db.published_input(&InputKey::DefinitionHeader(found.identity))
        else {
            return Err(fault("alias generated header absent"));
        };
        let Some((InputValue::DeclarationContext(context), _)) =
            db.published_input(&InputKey::DeclarationContext(found.identity))
        else {
            return Err(fault("alias generated context absent"));
        };
        let site = WrittenGeneratedType::capture(
            &unit.items,
            &unit.scope,
            &unit.cfg,
            &header.key,
            GeneratedTypeRole::AliasTarget,
            capture.residual()?,
        )?;
        if site.declaration_fingerprint() != definition.fingerprint()
            || site.header_fingerprint() != h.fingerprint()
            || site.declaration() != context.range
            || site.scope() != &context.scope
            || context.scope.owner.is_some()
        {
            return Err(fault("alias generated exact target/header/context differs"));
        };
        charge(
            &mut capture.bytes,
            site.metadata_bytes().checked_mul(2).ok_or_else(limit)?,
            max,
        )?;
        charge(
            &mut capture.bytes,
            unit.items.text().len().checked_mul(8).ok_or_else(limit)?,
            max,
        )?;
        let parsed = ParsedFile::from_generated_items(&unit.items)?;
        let recipe = parsed.type_alias_recipe(found.identity, h, context)?;
        capture.push(
            found.identity,
            definition,
            parse,
            context,
            &recipe,
            Some(site.identity()),
        )?;
        for k in [
            InputKey::GeneratedSyntax(parse.content),
            InputKey::DefinitionSource(found.identity),
            InputKey::DefinitionHeader(found.identity),
            InputKey::DeclarationContext(found.identity),
            InputKey::ExpansionRecord(unit.items.record().key().fingerprint()),
        ] {
            capture.stamp(db, k)?;
        }
        if !recipe.generics.is_empty() || requires_substitution(&parsed, context)? {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::GenericSubstitution(found.identity),
            ));
        };
        let GeneratedTypeSyntax::Path(parts) = site.shape() else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::WrittenTarget(found.identity),
            ));
        };
        let [part] = parts.as_slice() else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::WrittenTarget(found.identity),
            ));
        };
        let GeneratedTypeSegmentKind::Name(name) = part.kind() else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::WrittenTarget(found.identity),
            ));
        };
        let origin = match primitive_origin(
            db,
            owner,
            owners,
            unit,
            &site,
            part,
            &mut capture.stamps,
            &mut capture.bytes,
            max,
        )? {
            Ok(o) => o,
            Err(n) => return Ok(SourceAliasOutcome::Need(SourceAliasPrerequisite::Origin(n))),
        };
        let key = seal_for(&origin.scope, &unit.seal);
        let Some(input) = capture.scope(db, origin.provider, &origin.scope, &key)? else {
            return Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::TypeNamespace(origin.scope),
            ));
        };
        match input
            .scope
            .as_ref()
            .ok_or_else(|| fault("alias generated target scope absent"))?
            .lookup(&BucketKey {
                scope: origin.scope.clone(),
                namespace: Namespace::Type,
                name: name.clone(),
            })? {
            NameLookup::Absent => capture.finish(
                db,
                selected.identity(),
                origin.provider,
                &origin.scope,
                &key,
                name,
            ),
            NameLookup::Defined(n) => Ok(SourceAliasOutcome::Need(
                SourceAliasPrerequisite::GeneratedAliasHop(n.identity),
            )),
            other => Ok(SourceAliasOutcome::Need(SourceAliasPrerequisite::Name(
                other,
            ))),
        }
    }
    pub(crate) fn primitive(&self) -> PrimitiveType {
        self.primitive
    }
    pub(crate) fn selected_definition(&self) -> Digest {
        self.steps
            .first()
            .expect("private nonempty admitted alias chain")
            .member
    }
    pub(crate) fn compiler_member(&self) -> Digest {
        self.compiler_member
    }
    pub(crate) fn alias_members(&self) -> impl Iterator<Item = Digest> {
        self.steps.iter().map(|s| s.member)
    }
    pub(crate) fn alias_definitions(&self) -> impl Iterator<Item = &DefKey> {
        self.steps.iter().map(|s| &s.definition)
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn dependencies(&self) -> &BTreeMap<InputKey, Digest> {
        &self.stamps
    }
    /// Root's actual TypeAlias query consumer can compare its immutable recipe without guessing an alias row.
    pub(crate) fn matches_query_recipe(&self, recipe: &TypeAliasRecipe) -> bool {
        self.steps.iter().any(|s| {
            s.member == recipe.definition
                && s.recipe == recipe.fingerprint()
                && s.header == recipe.header.fingerprint()
                && s.context.scope == recipe.scope
        })
    }
    pub(crate) fn revalidate_inputs(&self, db: &Database) -> Result<(), Diagnostic> {
        db.require_coordinator()?;
        if self.integrity != self.identity() {
            return Err(fault("alias private admitted payload differs"));
        }
        if self.revision != db.revision() {
            return Err(fault("alias publication changed; remint required"));
        };
        for (k, v) in &self.stamps {
            if db.published_input(k).is_none_or(|(_, a)| a != *v) {
                return Err(fault("alias retained source dependency changed"));
            }
        }
        Ok(())
    }
    pub(crate) fn identity(&self) -> Digest {
        let mut e = StableEncoder::new("lrr/v2/current-source-primitive-alias/canonical-member-v1");
        e.digest(self.seed);
        e.sequence(&self.steps, |e, s| {
            e.digest(s.member);
            e.record(&s.definition);
            e.record(&s.parse);
            e.record(&s.context.scope);
            e.u32(s.context.range.start);
            e.u32(s.context.range.end);
            e.digest(s.header);
            e.digest(s.recipe);
            e.option(&s.written, |e, d| e.digest(*d));
        });
        e.u32(self.primitive.tag());
        e.digest(self.compiler_member);
        e.record(&self.terminal_scope);
        e.digest(self.prelude);
        e.digest(self.facet);
        e.u64(u64::try_from(self.stamps.len()).expect("bounded alias input count"));
        for (key, value) in &self.stamps {
            e.record(key);
            e.digest(*value);
        }
        e.finish()
    }
}

#[path = "source_type_alias_normalization/generated.rs"]
mod generated;

#[cfg(test)]
#[path = "source_type_alias_normalization_tests.rs"]
mod tests;

#[path = "source_type_alias_normalization/qualified.rs"]
mod qualified;
