//! Entered Value source goals. These select metadata through the ordinary
//! hygienic prefix; they grant neither callable entry nor a runtime value.
use super::{ModuleRoute, PreparedSourceLocalPlan, error};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        current_module_names::ModuleNameCompletion,
        names::{
            NameDefinition, NameLookup,
            reads::{NamespaceReadCertification, NamespaceReads},
        },
        source_locals::{OrdinaryNameSite, PrefixResolution},
    },
    ir::pc::{NodeId, PcKey},
    source::manifest::{BodyKey, Namespace},
    syntax::{
        ast_adapter::range,
        body_index::{ExpressionKind, NodeRole},
        local_bindings::PatternNameKind,
        outline::DeclarationContext,
        parse::ParseKey,
        spans::SourceRange,
        token_wire::Key,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};
use std::sync::Arc;
const LIMIT: usize = 32 * 1024 * 1024;
macro_rules! polled {
    ($v:expr) => {
        match $v {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
macro_rules! checked {
    ($v:expr) => {
        match $v {
            Ok(v) => v,
            Err(e) => return QueryPoll::Ready(Err(e)),
        }
    };
}
/// A missing join, never a negative Rust namespace answer.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ValueGoalDemand {
    Occurrence,
    PathAdjustment,
    BindingClassification(Digest),
    Prefix(Digest),
    ModulePublication,
    ModuleCompletion(Digest),
    SourceOwner(Digest),
    ScopeCertificate(Digest),
    DeclarationPublication(Digest),
    DeclarationOwner(Digest),
    DeclarationOccurrence(Digest),
    DefinitionModule(Key),
    ExpansionDefinition(Key),
    Prelude,
}
/// Syntax-only target metadata. In particular `BindingOrConstant` does not
/// authorize treating a same-spelling use as this binding.
#[derive(Debug, Clone)]
pub(crate) struct BindingTarget {
    identity: Digest,
    name: String,
    raw_name: String,
    name_range: SourceRange,
    classification: PatternNameKind,
}
impl BindingTarget {
    pub(crate) fn identity(&self) -> Digest {
        self.identity
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn raw_name(&self) -> &str {
        &self.raw_name
    }
    pub(crate) fn name_range(&self) -> SourceRange {
        self.name_range
    }
    pub(crate) fn classification(&self) -> PatternNameKind {
        self.classification
    }
}
/// Actual selected source header projection. No callee body or callable proof
/// is retained or inferred from this result.
#[derive(Debug, Clone)]
pub(crate) struct SelectedValueDeclaration {
    definition: NameDefinition,
    source: ParseKey,
    header: Digest,
    context: DeclarationContext,
    owner: Option<BodyKey>,
}
impl SelectedValueDeclaration {
    pub(crate) fn identity(&self) -> Digest {
        self.definition.identity
    }
    pub(crate) fn definition(&self) -> &NameDefinition {
        &self.definition
    }
    pub(crate) fn source(&self) -> &ParseKey {
        &self.source
    }
    pub(crate) fn header(&self) -> Digest {
        self.header
    }
    pub(crate) fn context(&self) -> &DeclarationContext {
        &self.context
    }
    pub(crate) fn owner(&self) -> Option<&BodyKey> {
        self.owner.as_ref()
    }
}
#[derive(Clone)]
pub struct EnteredValueGoal {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    local: Option<Digest>,
    target: Option<BindingTarget>,
    declaration: Option<SelectedValueDeclaration>,
    demand: Option<ValueGoalDemand>,
    occurrence: Digest,
    reads: Digest,
    fingerprint: Digest,
    bytes: usize,
}
impl std::fmt::Debug for EnteredValueGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnteredValueGoal")
            .field("key", &self.key)
            .field("local", &self.local)
            .field("demand", &self.demand)
            .finish_non_exhaustive()
    }
}
impl EnteredValueGoal {
    pub fn key(&self) -> &PcKey {
        &self.key
    }
    pub fn body(&self) -> &BodyKey {
        &self.key.body
    }
    pub fn node(&self) -> NodeId {
        NodeId(self.key.node)
    }
    pub fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub fn estimated_bytes(&self) -> usize {
        self.bytes
    }
    pub fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn local_binding(&self) -> Option<Digest> {
        self.local
    }
    pub(crate) fn binding_target(&self) -> Option<&BindingTarget> {
        self.target.as_ref()
    }
    pub(crate) fn declaration(&self) -> Option<&SelectedValueDeclaration> {
        self.declaration.as_ref()
    }
    pub(crate) fn pending(&self) -> Option<ValueGoalDemand> {
        self.demand
    }
    fn finish(mut self) -> Result<Self, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(n)
                .filter(|n| *n <= LIMIT)
                .ok_or_else(crate::runtime::arena::resource)?;
            Ok(())
        };
        if let Some(target) = &self.target {
            add(target.name.capacity())?;
            add(target.raw_name.capacity())?;
        }
        if let Some(declaration) = &self.declaration {
            add(declaration.source.profile.semantics.capacity())?;
            add(declaration.context.scope.heap_bytes())?;
        }
        self.bytes = bytes;
        self.fingerprint = StableEncode::fingerprint(&self);
        Ok(self)
    }
    fn needs(mut self, demand: ValueGoalDemand) -> QueryPoll<Self> {
        self.demand = Some(demand);
        QueryPoll::Ready(self.finish())
    }
}
impl StableEncode for EnteredValueGoal {
    const DOMAIN: &'static str = "lrr/v1/entered-value-source-goal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.context().source().identity());
        e.digest(self.occurrence);
        e.digest(self.reads);
        e.option(&self.local, |e, id| e.digest(*id));
        e.option(&self.target, |e, target| {
            e.digest(target.identity);
            e.string(&target.name);
            e.string(&target.raw_name);
            e.u32(target.name_range.start);
            e.u32(target.name_range.end);
            e.u8(match target.classification {
                PatternNameKind::ExplicitBinding => 0,
                PatternNameKind::BindingOrConstant => 1,
            });
        });
        e.option(&self.declaration, |e, declaration| {
            e.record(&declaration.definition);
            e.record(&declaration.source);
            e.digest(declaration.header);
            e.record(&declaration.context);
            e.option(&declaration.owner, |e, owner| e.record(owner));
        });
        e.option(&self.demand, |e, demand| match demand {
            ValueGoalDemand::Occurrence => e.u8(0),
            ValueGoalDemand::PathAdjustment => e.u8(1),
            ValueGoalDemand::BindingClassification(id) => {
                e.u8(2);
                e.digest(*id);
            }
            ValueGoalDemand::Prefix(id) => {
                e.u8(3);
                e.digest(*id);
            }
            ValueGoalDemand::ModulePublication => e.u8(4),
            ValueGoalDemand::ModuleCompletion(id) => {
                e.u8(5);
                e.digest(*id);
            }
            ValueGoalDemand::SourceOwner(id) => {
                e.u8(6);
                e.digest(*id);
            }
            ValueGoalDemand::ScopeCertificate(id) => {
                e.u8(7);
                e.digest(*id);
            }
            ValueGoalDemand::DeclarationPublication(id) => {
                e.u8(8);
                e.digest(*id);
            }
            ValueGoalDemand::DeclarationOwner(id) => {
                e.u8(9);
                e.digest(*id);
            }
            ValueGoalDemand::DeclarationOccurrence(id) => {
                e.u8(10);
                e.digest(*id);
            }
            ValueGoalDemand::DefinitionModule(id) => {
                e.u8(11);
                e.digest(Digest(id.0));
            }
            ValueGoalDemand::ExpansionDefinition(id) => {
                e.u8(12);
                e.digest(Digest(id.0));
            }
            ValueGoalDemand::Prelude => e.u8(13),
        });
    }
}
/// Pure metadata preparation over an actual entered source plan. This function
/// never captures source, publishes names, prepares a child body or executes it.
pub fn prepare(db: &mut Database, body: &BodyKey, node: NodeId) -> QueryPoll<EnteredValueGoal> {
    let actual = polled!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(body.clone())));
    let QueryValue::SourceLocalPlan(plan) = actual.value.as_ref() else {
        return QueryPoll::Ready(Err(error("Value goal source-plan variant differs")));
    };
    let plan = Arc::clone(plan);
    if plan.context().source().index().body() != body {
        return QueryPoll::Ready(Err(error("Value goal owning body differs")));
    }
    polled!(plan.require_current(db));
    let mut goal = EnteredValueGoal {
        key: PcKey {
            body: body.clone(),
            node: node.0,
            phase: 0,
        },
        plan: Arc::clone(&plan),
        local: None,
        target: None,
        declaration: None,
        demand: None,
        occurrence: Digest([0; 32]),
        reads: Digest([0; 32]),
        fingerprint: Digest([0; 32]),
        bytes: 0,
    };
    let Some(indexed) = plan.context().source().index().nodes().get(node.0 as usize) else {
        return QueryPoll::Ready(Err(error("Value goal node is outside actual body index")));
    };
    if indexed.id != node {
        return QueryPoll::Ready(Err(error("Value goal indexed ID differs")));
    }
    let source = plan.context().source();
    let syntax = checked!(
        source
            .parsed()
            .entered_node_syntax(source.body(), source.index(), node)
    );
    let Some(syntax) = syntax else {
        return goal.needs(ValueGoalDemand::Occurrence);
    };
    let (root, hygiene) = plan.entered.local_hygiene();
    let adapter = plan.adapter();
    let (name, name_range, target) = match indexed.role {
        NodeRole::Pattern => {
            let Some(pattern) = ast::IdentPat::cast(syntax) else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            // @/structural targets require the retained canonical group join.
            if pattern.pat().is_some() {
                return goal.needs(ValueGoalDemand::Occurrence);
            }
            let Some(name) = pattern.name() else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            let name_range = range(name.syntax().text_range());
            let recipe = checked!(adapter.binding_recipe(name_range));
            let Some(recipe) = recipe else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            if recipe.pattern_range != range(pattern.syntax().text_range()) {
                return goal.needs(ValueGoalDemand::Occurrence);
            }
            checked!(
                recipe
                    .name
                    .len()
                    .checked_add(recipe.raw_name.len())
                    .filter(|bytes| *bytes <= 2 * 1024 * 1024)
                    .ok_or_else(crate::runtime::arena::resource)
            );
            let target = BindingTarget {
                identity: recipe.identity,
                name: recipe.name.clone(),
                raw_name: recipe.raw_name.clone(),
                name_range,
                classification: recipe.classification,
            };
            (
                checked!(owned_spelling(name.text())),
                name_range,
                Some(target),
            )
        }
        NodeRole::Expression(ExpressionKind::Path) => {
            let Some(expression) = ast::PathExpr::cast(syntax) else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            let Some(path) = expression.path() else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            if path.qualifier().is_some() || path.coloncolon_token().is_some() {
                return goal.needs(ValueGoalDemand::PathAdjustment);
            }
            let Some(segment) = path.segment() else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            // This query selects the actual base Value declaration only.
            // Written generic arguments stay with the normal call/type/const
            // consumer; they grant no execution or substitution authority here.
            let Some(name) = segment.name_ref() else {
                return goal.needs(ValueGoalDemand::PathAdjustment);
            };
            (
                checked!(owned_spelling(name.text())),
                range(name.syntax().text_range()),
                None,
            )
        }
        _ => return goal.needs(ValueGoalDemand::Occurrence),
    };
    if name.len() > 1024 * 1024 {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    let edition = checked!(source.parsed().token_edition(name_range));
    let raw = checked!(source.identifier_context_or_original(name_range, root));
    let local_context = checked!(hygiene.name_local(raw));
    let mut occurrence = StableEncoder::new("lrr/v1/entered-value-occurrence");
    occurrence.string(&name);
    occurrence.u32(name_range.start);
    occurrence.u32(name_range.end);
    occurrence.u16(edition);
    occurrence.digest(Digest(raw.0));
    occurrence.digest(Digest(local_context.0.0));
    goal.occurrence = occurrence.finish();
    if edition != plan.entered.parent.source().outline.parse.profile.edition {
        return goal.needs(ValueGoalDemand::PathAdjustment);
    }
    if let Some(target) = target {
        if checked!(crate::frontend::names::owned_name(&name, edition)).as_str() != target.name {
            return QueryPoll::Ready(Err(error("Value binding target spelling differs")));
        }
        use crate::frontend::scalar_pattern_binding::occurrences::PatternLocalOccurrence;
        use crate::syntax::pattern_bindings::{BindingContext, BindingUncertainty};
        let Some(row) = plan.pattern_occurrences.for_local(target.identity) else {
            return goal.needs(ValueGoalDemand::Occurrence);
        };
        let row = checked!(plan.pattern_occurrences.select(
            source.index(),
            &goal.key,
            row.pattern()
        ));
        if row.pattern() != node
            || row.root() != node
            || row.name_range() != name_range
            || row.name() != target.name
            || row.edition() != Some(edition)
            || row.canonical_candidate() != Some(node)
            || !row.conflicts().is_resolved()
            || !matches!(row.local_source(), PatternLocalOccurrence::Joined { identity, .. } if identity == target.identity)
            || !matches!(
                (target.classification, row.uncertainty()),
                (
                    PatternNameKind::ExplicitBinding,
                    BindingUncertainty::ExplicitBinding
                ) | (
                    PatternNameKind::BindingOrConstant,
                    BindingUncertainty::BindingOrConstant
                )
            )
        {
            return goal.needs(ValueGoalDemand::Occurrence);
        }
        match row.context() {
            BindingContext::Original
                if checked!(source.parsed().identifier_context(name_range)).is_none() => {}
            BindingContext::Normalized(context) if context == local_context => {}
            _ => return goal.needs(ValueGoalDemand::Occurrence),
        }
        let id = target.identity;
        let explicit = target.classification == PatternNameKind::ExplicitBinding;
        goal.target = Some(target);
        if explicit {
            goal.local = Some(id);
            return QueryPoll::Ready(goal.finish());
        }
    }
    if let Some(missing) = plan.missing_expansions.first() {
        return goal.needs(ValueGoalDemand::ExpansionDefinition(*missing));
    }
    let Some(environment) = plan.environment() else {
        return goal.needs(ValueGoalDemand::ModulePublication);
    };
    let environment = environment.macros().source_environment();
    let site = OrdinaryNameSite {
        source: plan.entered.parent.source(),
        body,
        at: u64::from(name_range.start),
        name: &name,
        context: raw,
        namespace: Namespace::Value,
    };
    let (prefix, reads) = checked!(adapter.lookup_prepared_value_prefix_observed(
        &site,
        &plan.type_names,
        environment.names()
    ));
    let lookup = match prefix {
        PrefixResolution::Name(lookup) => lookup,
        PrefixResolution::ModuleTail => {
            match checked!(plan.lexical_module.route(&hygiene, raw)) {
                ModuleRoute::Definition(expansion) => {
                    return goal.needs(ValueGoalDemand::DefinitionModule(expansion));
                }
                ModuleRoute::Lexical(_) => {}
            }
            let request = checked!(plan.lexical_module.request_in_namespace(
                &name,
                edition,
                Namespace::Value
            ));
            let Some(request) = request else {
                return goal.needs(ValueGoalDemand::ModulePublication);
            };
            let answer = polled!(db.fetch(
                &SyntaxProvider,
                QueryKey::CurrentModuleName(Box::new(request.clone()))
            ));
            let QueryValue::CurrentModuleName(answer) = answer.value.as_ref() else {
                return QueryPoll::Ready(Err(error("Value goal module answer variant differs")));
            };
            if !answer.matches_request(&request) {
                return QueryPoll::Ready(Err(error("Value goal module request differs")));
            }
            if answer.completion() != ModuleNameCompletion::Complete {
                return goal.needs(ValueGoalDemand::ModuleCompletion(answer.request_identity()));
            }
            let Some(lookup) = answer.lookup() else {
                return goal.needs(ValueGoalDemand::ModulePublication);
            };
            goal.reads = answer.semantic_identity();
            if matches!(lookup, NameLookup::Absent) {
                if goal.target.is_none() {
                    return goal.needs(ValueGoalDemand::Prelude);
                }
                let Some(prelude) = polled!(pattern_prelude(db, environment, &request)) else {
                    return goal.needs(ValueGoalDemand::Prelude);
                };
                let mut identity = StableEncoder::new("lrr/v1/entered-pattern-module-prelude");
                identity.digest(goal.reads);
                identity.digest(prelude);
                goal.reads = identity.finish();
            }
            lookup.clone()
        }
        PrefixResolution::NeedImport(id) | PrefixResolution::NeedDeclaration(id) => {
            return goal.needs(ValueGoalDemand::Prefix(id));
        }
        PrefixResolution::Parameter(_) | PrefixResolution::NeedOwnerGeneric => {
            return goal.needs(ValueGoalDemand::PathAdjustment);
        }
    };
    match polled!(certify(
        &reads,
        db,
        environment,
        goal.occurrence,
        goal.reads
    )) {
        NamespaceReadCertification::Complete(identity) => goal.reads = identity,
        NamespaceReadCertification::PendingOwner(id) => {
            return goal.needs(ValueGoalDemand::SourceOwner(id));
        }
        NamespaceReadCertification::PendingScope(id) => {
            return goal.needs(ValueGoalDemand::ScopeCertificate(id));
        }
        NamespaceReadCertification::PendingName(id) => {
            return goal.needs(ValueGoalDemand::ModuleCompletion(id));
        }
    }
    match lookup {
        NameLookup::LocalBinding(identity) => {
            goal.local = Some(
                goal.target
                    .as_ref()
                    .map_or(identity, |target| target.identity),
            )
        }
        NameLookup::Absent if goal.target.is_some() => {
            goal.local = goal.target.as_ref().map(|target| target.identity)
        }
        NameLookup::Defined(selected) => {
            return selected_declaration(db, goal, selected);
        }
        NameLookup::NeedInput(id) => {
            use crate::frontend::scalar_pattern_binding::occurrences::PatternLocalOccurrence;
            let Some(prior) = plan.pattern_occurrences.for_local(id) else {
                return goal.needs(ValueGoalDemand::Prefix(id));
            };
            // A real earlier lexical binding may be shadowed by a fresh binding.
            // The SAME hygienic prefix selected this identity; source order makes
            // the query DAG acyclic, and pending/constant results do not grant it.
            if !matches!(prior.local_source(), PatternLocalOccurrence::Joined { visible_from, .. }
                if visible_from <= u64::from(name_range.start))
                || prior.pattern() == node
            {
                return goal.needs(ValueGoalDemand::Prefix(id));
            }
            let prior_key = PcKey {
                body: body.clone(),
                node: prior.pattern().0,
                phase: 0,
            };
            let prior =
                polled!(db.fetch(&SyntaxProvider, QueryKey::EnteredValue(prior_key.clone())));
            let QueryValue::EnteredValue(prior) = prior.value.as_ref() else {
                return QueryPoll::Ready(Err(error("Value goal prior binding variant differs")));
            };
            if prior.key() != &prior_key || prior.local_binding() != Some(id) {
                return goal.needs(ValueGoalDemand::BindingClassification(id));
            }
            let mut identity = StableEncoder::new("lrr/v1/entered-value-prior-binding");
            identity.digest(goal.reads);
            identity.digest(prior.fingerprint());
            goal.reads = identity.finish();
            goal.local = Some(goal.target.as_ref().map_or(id, |target| target.identity));
        }
        _ => return goal.needs(ValueGoalDemand::PathAdjustment),
    }
    QueryPoll::Ready(goal.finish())
}
fn certify(
    reads: &NamespaceReads,
    db: &mut Database,
    environment: &crate::expand::source_discovery::CompletedSourceEnvironment,
    occurrence: Digest,
    module: Digest,
) -> QueryPoll<NamespaceReadCertification> {
    let mut seed = StableEncoder::new("lrr/v1/entered-value-read-seed");
    seed.digest(occurrence);
    seed.digest(module);
    reads.certify(db, environment, seed.finish(), |_, _| Ok(()))
}
fn selected_declaration(
    db: &mut Database,
    mut goal: EnteredValueGoal,
    selected: NameDefinition,
) -> QueryPoll<EnteredValueGoal> {
    let id = selected.identity;
    if selected.namespace != Namespace::Value {
        return QueryPoll::Ready(Err(error("Value goal selected another namespace")));
    }
    let source = db.input(&InputKey::DefinitionSource(id));
    let header = db.input(&InputKey::DefinitionHeader(id));
    let context = db.input(&InputKey::DeclarationContext(id));
    let (Some(source), Some(header), Some(context)) = (source, header, context) else {
        return goal.needs(ValueGoalDemand::DeclarationPublication(id));
    };
    let (
        InputValue::DefinitionSource(source),
        InputValue::DefinitionHeader(header),
        InputValue::DeclarationContext(context),
    ) = (source.as_ref(), header.as_ref(), context.as_ref())
    else {
        return QueryPoll::Ready(Err(error("Value goal declaration input variants differ")));
    };
    if header.profile != source.profile || selected.defining_unit != context.scope.crate_key {
        return QueryPoll::Ready(Err(error("Value goal declaration source/profile differs")));
    }
    let header_stamp = checked!(
        db.published_input(&InputKey::DefinitionHeader(id))
            .map(|(_, stamp)| stamp)
            .ok_or_else(|| error("Value goal actual header stamp absent"))
    );
    let cached_header = checked!(goal.plan.field_types.header_identity(id, header_stamp));
    let owner = if context.scope.owner.is_some() {
        let owner = db.input(&InputKey::LocalDeclarationOwner(id));
        let Some(owner) = owner else {
            return goal.needs(ValueGoalDemand::DeclarationOwner(id));
        };
        let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
            return QueryPoll::Ready(Err(error("Value goal declaration owner variant differs")));
        };
        if owner != goal.plan.context().source().index().body() {
            return goal.needs(ValueGoalDemand::DeclarationOwner(id));
        }
        let Some(row) = goal
            .plan
            .context()
            .facts()
            .items
            .iter()
            .find(|row| row.item.def_key.fingerprint() == id)
        else {
            return goal.needs(ValueGoalDemand::DeclarationOccurrence(id));
        };
        if *source != goal.plan.context().source().parsed().key()
            || row.item.scope != context.scope
            || row.item.declaration_span != context.range
            || (cached_header.is_none() && row.item.header != *header)
        {
            return QueryPoll::Ready(Err(error("Value goal actual local declaration differs")));
        }
        Some(owner.clone())
    } else {
        None
    };
    if goal.target.is_some() {
        // A positive Value answer is not automatically a constant. rustc's
        // late::try_resolve_as_non_binding permits fresh bindings to shadow
        // Fn/AssocFn, but not constant/constructor/static definitions. Resolve
        // the actual canonical source kind before admitting this binding.
        return shadowed_definition_binding(db, goal, selected, source, header, context);
    }
    checked!(
        source
            .profile
            .semantics
            .len()
            .checked_add(context.scope.heap_bytes())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<EnteredValueGoal>()))
            .filter(|bytes| *bytes <= LIMIT)
            .ok_or_else(crate::runtime::arena::resource)
    );
    goal.declaration = Some(SelectedValueDeclaration {
        definition: selected,
        source: source.clone(),
        header: cached_header.unwrap_or_else(|| header.fingerprint()),
        context: context.clone(),
        owner,
    });
    QueryPoll::Ready(goal.finish())
}

pub(super) fn shadowed_definition_binding(
    db: &mut Database,
    mut goal: EnteredValueGoal,
    selected: NameDefinition,
    source: &ParseKey,
    header: &crate::syntax::outline::HeaderRecipe,
    context: &DeclarationContext,
) -> QueryPoll<EnteredValueGoal> {
    use crate::{
        frontend::callables::entered_module::{
            CanonicalModulePreparation, canonical_module_declaration,
        },
        source::manifest::ItemKind,
    };
    let id = selected.identity;
    let canonical_kind = if context.scope.owner.is_some() {
        let Some(row) = goal
            .plan
            .context()
            .facts()
            .items
            .iter()
            .find(|row| row.item.def_key.fingerprint() == id)
        else {
            return goal.needs(ValueGoalDemand::DeclarationOccurrence(id));
        };
        row.item.def_key.item_kind
    } else {
        match polled!(canonical_module_declaration(
            db, &goal.plan, &selected, source, header, context
        )) {
            CanonicalModulePreparation::Ready(actual) => actual.definition().item_kind,
            CanonicalModulePreparation::Pending(demand) => return goal.needs(demand),
        }
    };
    if canonical_kind != ItemKind::Function {
        return goal.needs(ValueGoalDemand::BindingClassification(id));
    }
    let parsed = polled!(db.fetch(&SyntaxProvider, QueryKey::ParseFile(source.clone())));
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return QueryPoll::Ready(Err(error("binding function source parse variant differs")));
    };
    let function = checked!(parsed.syntax_at(context.range, |node| ast::Fn::can_cast(node.kind())))
        .and_then(ast::Fn::cast);
    let Some(function) = function else {
        return QueryPoll::Ready(Err(error(
            "binding selected function exact source is absent",
        )));
    };
    let excluded = function.body().map(|body| body.syntax().text_range());
    if parsed.key() != *source
        || crate::syntax::ast_adapter::tokens(function.syntax(), excluded) != header.tokens
    {
        return QueryPoll::Ready(Err(error(
            "binding selected function header/source differs",
        )));
    }
    let mut reads = StableEncoder::new("lrr/v1/pattern-shadows-source-function");
    reads.digest(goal.reads);
    reads.digest(id);
    reads.record(source);
    reads.record(header);
    reads.record(context);
    goal.reads = reads.finish();
    goal.local = goal.target.as_ref().map(|target| target.identity);
    QueryPoll::Ready(goal.finish())
}

/// Select only a genuinely published compiler Value-prelude receipt for the
/// exact lexical source/scope. No per-name parse/catalogue replay or absence
/// Boolean is accepted. Missing/positive finite-facet answers remain pending.
fn pattern_prelude(
    db: &mut Database,
    environment: &crate::expand::source_discovery::CompletedSourceEnvironment,
    request: &crate::db::key::CurrentModuleNameKey,
) -> QueryPoll<Option<Digest>> {
    use crate::expand::source_discovery::SourceScopeOwner;
    use crate::facade::value_binding_prelude::RetainedPreludeName;
    let SourceScopeOwner::Selected(source) = environment.scope_owner(&request.bucket.scope) else {
        return QueryPoll::Ready(Ok(None));
    };
    if source.selection().outline != request.source
        || source.selection().path.as_str() != request.path
    {
        return QueryPoll::Ready(Err(error("pattern prelude actual source differs")));
    }
    let Some(input) = checked!(db.current_scope_certificate(
        source.selection(),
        &request.bucket.scope,
        Namespace::Value
    )) else {
        return QueryPoll::Ready(Ok(None));
    };
    let InputValue::DiscoverySeal(input) = input.as_ref() else {
        return QueryPoll::Ready(Err(error("pattern prelude certificate variant differs")));
    };
    let current = polled!(db.fetch(&SyntaxProvider, QueryKey::DiscoverySeal(input.seal.key())));
    if !matches!(current.value.as_ref(), QueryValue::DiscoverySeal(seal) if seal.key() == input.seal.key())
    {
        return QueryPoll::Ready(Err(error("pattern prelude current seal differs")));
    }
    let mut proofs = input.value_binding_preludes.iter().filter(|proof| {
        proof.source() == source.selection() && proof.scope() == &request.bucket.scope
    });
    let Some(proof) = proofs.next() else {
        return QueryPoll::Ready(Ok(None));
    };
    if proofs.next().is_some() {
        return QueryPoll::Ready(Err(error("pattern prelude receipt is ambiguous")));
    }
    checked!(proof.metadata_bytes().and_then(|bytes| {
        (bytes <= LIMIT)
            .then_some(())
            .ok_or_else(crate::runtime::arena::resource)
    }));
    let retained = checked!(proof.retain(source, &request.bucket.scope));
    QueryPoll::Ready(Ok(matches!(
        retained.lookup(&request.bucket.name),
        RetainedPreludeName::AbsentInSelectedFacet
    )
    .then_some(retained.identity())))
}

fn owned_spelling(text: &str) -> Result<String, Diagnostic> {
    if text.len() > 1024 * 1024 {
        return Err(crate::runtime::arena::resource());
    }
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| crate::runtime::arena::resource())?;
    owned.push_str(text);
    Ok(owned)
}
