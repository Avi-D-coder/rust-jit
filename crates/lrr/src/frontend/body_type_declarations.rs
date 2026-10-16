//! Final direct declaration join for an actual entered Type occurrence. Module
//! bucket candidates, header metadata and runtime descriptors are not use proof.
use super::{LocalTypePrefix, LocalTypePrefixSelection, PreparedSourceLocalPlan, error};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        key::{CurrentModuleNameKey, QueryKey},
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        current_module_names::ModuleNameCompletion,
        names::{NameDefinition, NameLookup},
        source_types::{NamedTypePreparation, SourceType},
    },
    source::manifest::{DefKey, ItemKind, Namespace},
    syntax::{
        ast_adapter::{range, tokens},
        parse::ParseKey,
        spans::SourceRange,
        token_wire::{Key, NameContextKey},
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
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
pub(crate) enum BodyTypeDemand {
    Import(Digest),
    Producer(Digest),
    DeclarationPublication(Digest),
    BindingOccurrence(Digest),
    DeclarationOccurrence(Digest),
    PinnedAdjustment(Digest),
    DeclarationSource(Digest),
    RetainedDeclaration(Digest),
    AliasContext(Digest),
    TypeRecipe(Digest),
    GenericArguments(Digest),
    CachedSchema(Digest),
    OwnerGeneric,
    QualifiedPath,
    Parameter(crate::traits::term::ParamKey),
    Source(crate::frontend::source_body_context::type_occurrences::BodyTypeAuthorityDemand),
    ExpansionDefinition(Key),
    DefinitionModule(Key),
    ModulePublication,
    ModuleCompletion(ModuleNameCompletion),
    ModuleCandidate(Digest),
}
pub(crate) enum BodyTypeOutcome {
    Ready(PreparedBodyType),
    Pending(BodyTypeDemand),
}
pub(crate) struct PreparedBodyType {
    declaration: BodyTypeDeclaration,
    source: SourceType,
}
impl PreparedBodyType {
    pub(crate) fn declaration(&self) -> &BodyTypeDeclaration {
        &self.declaration
    }
    pub(crate) fn source_type(&self) -> &SourceType {
        &self.source
    }
    pub(crate) fn exclusive_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(self.declaration.extra_bytes()?)
            .and_then(|n| n.checked_add(self.source.heap_bytes().ok()?))
            .ok_or_else(crate::runtime::arena::resource)
    }
}
/// Constructed only after the actual opaque prefix/current module/declaration
/// occurrence join. No constructor accepts a caller-selected lookup or schema.
pub(crate) struct BodyTypeDeclaration {
    plan: Arc<PreparedSourceLocalPlan>,
    prefix: Digest,
    selected: Digest,
    definition: DefKey,
    parse: ParseKey,
    range: SourceRange,
    raw: Key,
    comparison: NameContextKey,
    edition: u16,
    header: Digest,
    dependencies: Vec<DependencyStamp>,
    fingerprint: Digest,
}
impl BodyTypeDeclaration {
    pub(crate) fn definition_identity(&self) -> Digest {
        self.selected
    }
    pub(crate) fn definition(&self) -> &DefKey {
        &self.definition
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn declaration_range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn token_edition(&self) -> u16 {
        self.edition
    }
    pub(crate) fn comparison_context(&self) -> NameContextKey {
        self.comparison
    }
    fn extra_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .definition
            .declared_name
            .capacity()
            .checked_add(self.parse.profile.semantics.capacity())
            .and_then(|n| {
                n.checked_add(
                    self.definition
                        .module_path
                        .capacity()
                        .checked_mul(std::mem::size_of::<String>())?,
                )
            })
            .and_then(|n| {
                self.definition
                    .module_path
                    .iter()
                    .try_fold(n, |n, s| n.checked_add(s.capacity()))
            })
            .and_then(|n| {
                n.checked_add(
                    self.dependencies
                        .capacity()
                        .checked_mul(std::mem::size_of::<DependencyStamp>())?,
                )
            })
            .ok_or_else(crate::runtime::arena::resource)?;
        for dependency in &self.dependencies {
            bytes = bytes
                .checked_add(dependency.heap_bytes())
                .ok_or_else(crate::runtime::arena::resource)?;
        }
        if bytes > 32 * 1024 * 1024 {
            return Err(crate::runtime::arena::resource());
        }
        Ok(bytes)
    }
    /// Cold query proof validation only. Installed operations consume retained
    /// relevant versions; they must not call source preparation here.
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        polled!(self.plan.context().require_current(db));
        for dependency in &self.dependencies {
            match dependency {
                DependencyStamp::Input { key, fingerprint } => {
                    if db.input(key).is_none()
                        || !matches!(db.published_input(key), Some((_, actual)) if actual==*fingerprint)
                    {
                        return QueryPoll::Ready(Err(error("body Type declaration input changed")));
                    }
                }
                DependencyStamp::Query { key, fingerprint } => {
                    let value = polled!(db.fetch(&SyntaxProvider, key.clone()));
                    if value.fingerprint != *fingerprint {
                        return QueryPoll::Ready(Err(error("body Type declaration query changed")));
                    }
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }
}
impl StableEncode for BodyTypeDeclaration {
    const DOMAIN: &'static str = "lrr/v1/body-type-declaration";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.plan.source_identity());
        e.digest(self.prefix);
        e.digest(self.selected);
        e.record(&self.definition);
        e.record(&self.parse);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.digest(Digest(self.raw.0));
        e.digest(Digest(self.comparison.0.0));
        e.u16(self.edition);
        e.digest(self.header);
        e.sequence(&self.dependencies, |e, stamp| e.record(stamp));
    }
}
struct Trace {
    rows: Vec<DependencyStamp>,
}
impl Trace {
    fn new() -> Result<Self, Diagnostic> {
        let mut rows = Vec::new();
        rows.try_reserve_exact(16)
            .map_err(|_| crate::runtime::arena::resource())?;
        Ok(Self { rows })
    }
    fn push(&mut self, stamp: DependencyStamp) -> Result<(), Diagnostic> {
        if self.rows.iter().any(|old| old == &stamp) {
            return Ok(());
        }
        if self.rows.len() >= 16 || stamp.heap_bytes() > 1024 * 1024 {
            return Err(crate::runtime::arena::resource());
        }
        self.rows.push(stamp);
        Ok(())
    }
    fn input(
        &mut self,
        db: &mut Database,
        key: InputKey,
    ) -> Result<Option<Arc<InputValue>>, Diagnostic> {
        let Some(value) = db.input(&key) else {
            return Ok(None);
        };
        let fingerprint = db
            .published_input(&key)
            .ok_or_else(|| error("body Type input disappeared"))?
            .1;
        self.push(DependencyStamp::Input { key, fingerprint })?;
        Ok(Some(value))
    }
    fn query(&mut self, db: &mut Database, key: QueryKey) -> QueryPoll<Arc<QueryValue>> {
        let value = polled!(db.fetch(&SyntaxProvider, key.clone()));
        checked!(self.push(DependencyStamp::Query {
            key,
            fingerprint: value.fingerprint
        }));
        QueryPoll::Ready(Ok(value.value))
    }
}
fn pending(demand: BodyTypeDemand) -> QueryPoll<BodyTypeOutcome> {
    QueryPoll::Ready(Ok(BodyTypeOutcome::Pending(demand)))
}

pub(super) fn prepare(db: &mut Database, prefix: LocalTypePrefix) -> QueryPoll<BodyTypeOutcome> {
    let mut trace = checked!(Trace::new());
    let mut module = None;
    let selected: NameDefinition = match prefix.selection() {
        LocalTypePrefixSelection::ModuleTail => {
            let Some(observation) = polled!(prefix.module_name(db)) else {
                return QueryPoll::Ready(Err(error("body Type module observation absent")));
            };
            if observation.completion() != ModuleNameCompletion::Complete {
                return pending(BodyTypeDemand::ModuleCompletion(observation.completion()));
            }
            let selected = match observation.lookup() {
                Some(NameLookup::Defined(selected)) => selected.clone(),
                Some(NameLookup::NeedInput(producer)) => {
                    return pending(BodyTypeDemand::Producer(*producer));
                }
                _ => {
                    return pending(BodyTypeDemand::ModuleCandidate(
                        observation.semantic_identity(),
                    ));
                }
            };
            let request = CurrentModuleNameKey {
                source: observation.source().outline.clone(),
                path: observation.source().path.as_str().to_owned(),
                bucket: crate::db::key::BucketKey {
                    scope: observation.scope().clone(),
                    namespace: Namespace::Type,
                    name: observation.name().to_owned(),
                },
            };
            checked!(trace.push(DependencyStamp::Query {
                key: QueryKey::CurrentModuleName(Box::new(request)),
                fingerprint: QueryValue::CurrentModuleName(Arc::clone(&observation)).fingerprint()
            }));
            module = Some(observation);
            selected
        }
        LocalTypePrefixSelection::Lookup(NameLookup::Defined(selected)) => selected.clone(),
        LocalTypePrefixSelection::Lookup(NameLookup::NeedInput(producer)) => {
            return pending(BodyTypeDemand::Producer(*producer));
        }
        LocalTypePrefixSelection::Lookup(_) => {
            return pending(BodyTypeDemand::ModuleCandidate(prefix.fingerprint()));
        }
        LocalTypePrefixSelection::Parameter(parameter) => {
            return pending(BodyTypeDemand::Parameter(*parameter));
        }
        LocalTypePrefixSelection::NeedImport(id) => return pending(BodyTypeDemand::Import(*id)),
        LocalTypePrefixSelection::NeedDeclarationOccurrence(id) => {
            return pending(BodyTypeDemand::DeclarationOccurrence(*id));
        }
        LocalTypePrefixSelection::NeedOwnerGenericOccurrence => {
            return pending(BodyTypeDemand::OwnerGeneric);
        }
        LocalTypePrefixSelection::NeedSource(demand) => {
            return pending(BodyTypeDemand::Source(*demand));
        }
        LocalTypePrefixSelection::NeedUnqualifiedPath => {
            return pending(BodyTypeDemand::QualifiedPath);
        }
        LocalTypePrefixSelection::NeedExpansionDefinition(key) => {
            return pending(BodyTypeDemand::ExpansionDefinition(*key));
        }
        LocalTypePrefixSelection::NeedDefinitionModule(key) => {
            return pending(BodyTypeDemand::DefinitionModule(*key));
        }
        LocalTypePrefixSelection::NeedModulePublication => {
            return pending(BodyTypeDemand::ModulePublication);
        }
    };
    let identity = selected.identity;
    let source = checked!(trace.input(db, InputKey::DefinitionSource(identity)));
    let header = checked!(trace.input(db, InputKey::DefinitionHeader(identity)));
    let context = checked!(trace.input(db, InputKey::DeclarationContext(identity)));
    let (Some(source), Some(header), Some(context)) = (source, header, context) else {
        return pending(BodyTypeDemand::DeclarationPublication(identity));
    };
    let (
        InputValue::DefinitionSource(parse),
        InputValue::DefinitionHeader(header),
        InputValue::DeclarationContext(context),
    ) = (source.as_ref(), header.as_ref(), context.as_ref())
    else {
        return QueryPoll::Ready(Err(error("body Type declaration input variants differ")));
    };
    let parsed = polled!(trace.query(db, QueryKey::ParseFile(parse.clone())));
    let QueryValue::ParseFile(parsed) = parsed.as_ref() else {
        return QueryPoll::Ready(Err(error("body Type declaration parse differs")));
    };
    if parsed.key() != *parse || header.profile != parse.profile {
        return QueryPoll::Ready(Err(error("body Type declaration source/profile differs")));
    }
    // The opaque prefix already selected this owning-body declaration. Its
    // canonical key is the exact admitted local facts row, even when the body
    // parse also has a module-level GeneratedSyntax/RetainedSyntax publication.
    // Module maps cannot stand in for function-local declaration authority.
    let canonical = if module.is_none() {
        let Some(item) = prefix
            .retained_plan()
            .context()
            .facts()
            .items
            .iter()
            .find(|row| row.item.def_key.fingerprint() == identity)
        else {
            return pending(BodyTypeDemand::DeclarationOccurrence(identity));
        };
        if *parse != prefix.retained_plan().context().source().parsed().key()
            || item.item.scope != context.scope
            || item.item.declaration_span != context.range
            || item.item.header != *header
        {
            return QueryPoll::Ready(Err(error("body Type local declaration rows differ")));
        }
        item.item.def_key.clone()
    } else if let Some(input) = checked!(trace.input(db, InputKey::GeneratedSyntax(parse.content)))
    {
        let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
            return QueryPoll::Ready(Err(error("body Type generated source differs")));
        };
        if unit.key != *parse {
            return QueryPoll::Ready(Err(error("body Type generated parse differs")));
        }
        let Some(definition) = unit.definitions.get(&identity) else {
            return pending(BodyTypeDemand::DeclarationSource(identity));
        };
        if context.scope.crate_key != unit.scope.crate_key
            || !context
                .scope
                .module_path
                .starts_with(&unit.scope.module_path)
            || !unit.items.items().iter().any(|item| {
                item.key.fingerprint() == identity
                    && item.start == context.range.start
                    && item.end == context.range.end
            })
        {
            return QueryPoll::Ready(Err(error(
                "body Type generated canonical declaration differs",
            )));
        }
        definition.clone()
    } else if db.input(&InputKey::RetainedSyntax(parse.content)).is_some() {
        return pending(BodyTypeDemand::RetainedDeclaration(identity));
    } else if let Some(observation) = &module {
        if observation.source().outline.parse != *parse {
            return pending(BodyTypeDemand::BindingOccurrence(identity));
        }
        let outlined =
            polled!(trace.query(db, QueryKey::Outline(observation.source().outline.clone())));
        let QueryValue::Outline(outline) = outlined.as_ref() else {
            return QueryPoll::Ready(Err(error("body Type original outline differs")));
        };
        let Some(item) = outline
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == identity)
        else {
            return pending(BodyTypeDemand::DeclarationOccurrence(identity));
        };
        if item.scope != context.scope
            || item.declaration_span != context.range
            || item.header != *header
        {
            return QueryPoll::Ready(Err(error("body Type original declaration rows differ")));
        }
        item.def_key.clone()
    } else {
        return QueryPoll::Ready(Err(error(
            "body Type module declaration observation absent",
        )));
    };
    if canonical.namespace != Namespace::Type
        || selected.namespace != Namespace::Type
        || canonical.crate_key != context.scope.crate_key
        || canonical.module_path != context.scope.module_path
        || selected.defining_unit != canonical.crate_key
    {
        return QueryPoll::Ready(Err(error(
            "body Type canonical source/member identity differs",
        )));
    }
    if let Some(observation) = &module {
        if context.scope != *observation.scope() {
            return pending(BodyTypeDemand::BindingOccurrence(identity));
        }
    }
    let Some(use_name) = prefix.use_name() else {
        return pending(BodyTypeDemand::DeclarationOccurrence(identity));
    };
    if checked!(crate::frontend::names::owned_name(
        use_name.name(),
        use_name.edition()
    ))
    .as_str()
        != canonical.declared_name
    {
        return pending(BodyTypeDemand::BindingOccurrence(identity));
    }
    if canonical.item_kind == ItemKind::TypeAlias {
        return pending(BodyTypeDemand::AliasContext(identity));
    }
    if !matches!(canonical.item_kind, ItemKind::Struct | ItemKind::Enum) {
        return pending(BodyTypeDemand::TypeRecipe(identity));
    }
    let syntax = checked!(parsed.syntax_at(context.range, |node| ast::Item::can_cast(node.kind())));
    let Some(syntax) = syntax.and_then(ast::Item::cast) else {
        return pending(BodyTypeDemand::DeclarationOccurrence(identity));
    };
    let name = match &syntax {
        ast::Item::Struct(item) => item.name(),
        ast::Item::Enum(item) => item.name(),
        _ => None,
    };
    let Some(name) = name else {
        return pending(BodyTypeDemand::DeclarationOccurrence(identity));
    };
    if tokens(syntax.syntax(), None) != header.tokens {
        return QueryPoll::Ready(Err(error(
            "body Type actual declaration/header tokens differ",
        )));
    }
    let name_range = range(name.syntax().text_range());
    let edition = checked!(parsed.token_edition(name_range));
    if checked!(crate::frontend::names::owned_name(name.text(), edition)).as_str()
        != canonical.declared_name
        || selected.definition_edition != edition
    {
        return QueryPoll::Ready(Err(error("body Type declaration name/edition differs")));
    }
    let (original, original_facts) = prefix.retained_plan().entered.local_hygiene();
    let (raw, comparison, normalized) = match checked!(parsed.identifier_context(name_range)) {
        Some(raw) => {
            let Some(facts) = parsed.hygiene_facts() else {
                return pending(BodyTypeDemand::DeclarationOccurrence(identity));
            };
            (
                raw,
                checked!(facts.name_item(raw)),
                checked!(facts.item(raw)),
            )
        }
        None => {
            if prefix.retained_plan().entered.parent.source().outline.parse != *parse {
                return pending(BodyTypeDemand::DeclarationOccurrence(identity));
            }
            let root = checked!(original_facts.context(original));
            let root_edition = match root.edition {
                crate::syntax::token_wire::Edition::E2015 => 2015,
                crate::syntax::token_wire::Edition::E2018 => 2018,
                crate::syntax::token_wire::Edition::E2021 => 2021,
                crate::syntax::token_wire::Edition::E2024 => 2024,
            };
            if root.parent.is_some()
                || root.defining_unit.0 != canonical.crate_key.0
                || root_edition != edition
            {
                return pending(BodyTypeDemand::DeclarationOccurrence(identity));
            }
            (
                original,
                checked!(original_facts.name_item(original)),
                checked!(original_facts.item(original)),
            )
        }
    };
    let use_normalized = checked!(original_facts.item(use_name.context()));
    let declaration_facts = parsed
        .hygiene_facts()
        .map_or(original_facts.as_ref(), |facts| facts.as_ref());
    if checked!(declaration_facts.context(normalized))
        .parent
        .is_some()
        || checked!(original_facts.context(use_normalized))
            .parent
            .is_some()
        || comparison != checked!(original_facts.name_item(use_name.context()))
    {
        return pending(BodyTypeDemand::PinnedAdjustment(identity));
    }
    let mut declaration = BodyTypeDeclaration {
        plan: Arc::clone(prefix.retained_plan()),
        prefix: prefix.fingerprint(),
        selected: identity,
        definition: canonical,
        parse: parse.clone(),
        range: name_range,
        raw,
        comparison,
        edition,
        header: header.fingerprint(),
        dependencies: trace.rows,
        fingerprint: Digest([0; 32]),
    };
    checked!(declaration.extra_bytes());
    declaration.fingerprint = StableEncode::fingerprint(&declaration);
    let prepared = polled!(crate::frontend::source_types::prepare_body_declaration(
        db,
        &declaration
    ));
    let source = match prepared {
        NamedTypePreparation::Ready(source) => source,
        _ => return pending(BodyTypeDemand::GenericArguments(identity)),
    };
    let key = QueryKey::TypeSchema(identity);
    let Some(memo) = db.memo(&key) else {
        return pending(BodyTypeDemand::CachedSchema(identity));
    };
    declaration.dependencies.push(DependencyStamp::Query {
        key,
        fingerprint: memo.fingerprint,
    });
    checked!(declaration.extra_bytes());
    declaration.fingerprint = StableEncode::fingerprint(&declaration);
    let prepared = PreparedBodyType {
        declaration,
        source,
    };
    checked!(prepared.exclusive_metadata_bytes());
    QueryPoll::Ready(Ok(BodyTypeOutcome::Ready(prepared)))
}

#[cfg(test)]
#[path = "body_type_declaration_tests.rs"]
mod tests;
