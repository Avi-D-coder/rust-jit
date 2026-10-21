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
/// Existing ordinary frame storage selected by the shared lexical goal.
#[derive(Debug, Clone, Copy)]
pub(crate) enum LocalValueAccess {
    Binding(NodeId),
    Parameter(u32),
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
    /// An enum variant selected through the enum's module-like namespace (a
    /// named import or glob of its variants). Its source, header and context
    /// are the owning enum's declaration; `definition.identity` is the
    /// variant's own identity.
    variant: Option<crate::frontend::names::EnumVariantDefinition>,
    selected_kind: Option<crate::source::manifest::ItemKind>,
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
    pub(crate) fn variant(&self) -> Option<&crate::frontend::names::EnumVariantDefinition> {
        self.variant.as_ref()
    }
}
/// The lexical query stops before global declaration/type preparation. The
/// ordinary Value query resumes this retained selection without repeating lookup.
#[derive(Debug, Clone)]
enum ValueContinuation {
    Module {
        name: String,
        raw: Key,
        edition: u16,
        name_range: SourceRange,
    },
    Qualified,
    Declaration {
        selected: NameDefinition,
        foreign: bool,
    },
}
#[derive(Clone)]
pub struct EnteredValueGoal {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    local: Option<Digest>,
    continuation: Option<ValueContinuation>,
    target: Option<BindingTarget>,
    declaration: Option<SelectedValueDeclaration>,
    // The ordinary Value lookup selected a closed library facade function.
    facade: Option<crate::facade::functions::LibraryFunction>,
    // The selected facade function's definition identity.
    facade_identity: Option<Digest>,
    // A record pattern's name lives in the Type namespace; every other
    // goal selects in the Value namespace.
    type_namespace: bool,
    // `Root::member`: the root selected lexically (a block item or import
    // binds it) and the member name left for the dispatcher to resolve
    // against the selected root (an enum's variant).
    rooted: Option<String>,
    // The written path is qualified (`a::b`), whichever route selected it.
    qualified_path: bool,
    // Selected through a qualified written path (the dispatcher's route).
    qualified: bool,
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
    /// Bridge the selected lexical identity to the ordinary frame operation.
    /// No name lookup or spelling fallback occurs after source selection.
    pub(crate) fn indexed_local_access(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        key: &PcKey,
    ) -> Result<Option<LocalValueAccess>, Diagnostic> {
        self.indexed_local_access_at(index, key, NodeId(key.node))
    }
    pub(crate) fn indexed_local_access_at(
        &self,
        index: &crate::syntax::body_index::BodyIndex,
        key: &PcKey,
        at: NodeId,
    ) -> Result<Option<LocalValueAccess>, Diagnostic> {
        use crate::frontend::pattern_binding_occurrences::PatternLocalOccurrence;
        use crate::syntax::local_bindings::BindingForm;
        if &self.key != key
            || self.body() != index.body()
            || !std::ptr::eq(index, self.plan.context().source().index().as_ref())
        {
            return Err(error("selected local access body/PC differs"));
        }
        // A capture may project the selected use into its enclosing creation
        // frame, never an unrelated indexed frame. Ordinary reads use themselves.
        let mut ancestor = Some(NodeId(key.node));
        while let Some(node) = ancestor {
            if node == at {
                break;
            }
            ancestor = index
                .nodes()
                .get(node.0 as usize)
                .and_then(|node| node.parent);
        }
        if ancestor.is_none()
            || (at.0 != key.node
                && index
                    .nodes()
                    .get(at.0 as usize)
                    .is_none_or(|node| node.role != NodeRole::Expression(ExpressionKind::Closure)))
        {
            return Err(error(
                "selected local destination is not its enclosing closure",
            ));
        }
        let Some(identity) = self.local else {
            return Ok(None);
        };
        if self.demand.is_some() {
            return Err(error("selected local access remains pending"));
        }
        if let Some(occurrence) = self.plan.pattern_occurrences.for_local(identity) {
            if !matches!(occurrence.local_source(), PatternLocalOccurrence::Joined { identity: actual, .. } if actual == identity)
                || !occurrence.conflicts().is_resolved()
            {
                return Err(error("selected local lacks its actual pattern occurrence"));
            }
            let node = occurrence
                .canonical_candidate()
                .ok_or_else(|| error("selected local canonical pattern absent"))?;
            if index
                .nodes()
                .get(node.0 as usize)
                .is_none_or(|node| node.role != NodeRole::Pattern)
            {
                return Err(error("selected local canonical node differs"));
            }
            return Ok(Some(LocalValueAccess::Binding(node)));
        }
        let adapter = self.plan.adapter();
        let recipe = adapter
            .parameter_recipe(identity)
            .ok_or_else(|| error("selected local has no indexed pattern or parameter"))?;
        let source = self.plan.context().source();
        let root = source
            .parsed()
            .entered_node_syntax(source.body(), index, NodeId(0))?
            .ok_or_else(|| error("selected parameter owning root absent"))?;
        let function = root
            .ancestors()
            .find_map(ast::Fn::cast)
            .ok_or_else(|| error("selected parameter owning function absent"))?;
        let parameters = function
            .param_list()
            .ok_or_else(|| error("selected parameter list absent"))?;
        if recipe.form == BindingForm::Receiver
            && parameters.self_param().is_some_and(|parameter| {
                range(parameter.syntax().text_range()) == recipe.pattern_range
            })
        {
            return Ok(Some(LocalValueAccess::Parameter(
                index.frame_parameter_ordinal(at, 0)?,
            )));
        }
        for (ordinal, parameter) in parameters.params().enumerate() {
            if parameter
                .pat()
                .is_some_and(|pattern| range(pattern.syntax().text_range()) == recipe.pattern_range)
            {
                let ordinal = ordinal
                    .checked_add(usize::from(parameters.self_param().is_some()))
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| error("selected parameter ordinal overflow"))?;
                return Ok(Some(LocalValueAccess::Parameter(
                    index.frame_parameter_ordinal(at, ordinal)?,
                )));
            }
        }
        Err(error("selected parameter actual declaration range differs"))
    }
    pub(crate) fn binding_target(&self) -> Option<&BindingTarget> {
        self.target.as_ref()
    }
    pub(crate) fn declaration(&self) -> Option<&SelectedValueDeclaration> {
        self.declaration.as_ref()
    }
    /// Actual source kind, shared with non-binding classification. This reads
    /// declaration metadata only; it never enters a selected executable body.
    pub(crate) fn declaration_kind(&self, db: &mut Database) -> QueryPoll<CanonicalValueKind> {
        let Some(selected) = &self.declaration else {
            return QueryPoll::Ready(Ok(CanonicalValueKind::Absent));
        };
        if let Some(kind) = selected.selected_kind {
            return QueryPoll::Ready(Ok(CanonicalValueKind::Ready(kind)));
        }
        if selected.variant.is_some() {
            return QueryPoll::Ready(Ok(CanonicalValueKind::Ready(
                crate::source::manifest::ItemKind::Enum,
            )));
        }
        let Some(header) = db.input(&InputKey::DefinitionHeader(selected.identity())) else {
            return QueryPoll::Ready(Ok(CanonicalValueKind::Pending(
                ValueGoalDemand::DeclarationPublication(selected.identity()),
            )));
        };
        let InputValue::DefinitionHeader(header) = header.as_ref() else {
            return QueryPoll::Ready(Err(error("selected Value header variant differs")));
        };
        canonical_value_kind(
            db,
            self,
            &selected.definition,
            &selected.source,
            header,
            &selected.context,
        )
    }
    /// A closed library facade function selected by the ordinary lookup, as
    /// `CallResolver::select` selects it for the dispatcher.
    pub(crate) fn facade(&self) -> Option<crate::facade::functions::LibraryFunction> {
        self.facade
    }
    /// The definition identity of the selected closed facade function.
    pub(crate) fn facade_identity(&self) -> Option<Digest> {
        self.facade_identity
    }
    /// The member of a `Root::member` path whose root was selected lexically.
    pub(crate) fn rooted(&self) -> Option<&str> {
        self.rooted.as_deref()
    }
    /// Whether the written path is qualified (the module route selects it
    /// unless its root is lexically bound, see `rooted`).
    pub(crate) fn is_qualified_path(&self) -> bool {
        self.qualified_path
    }
    /// Whether this selection came from a qualified written path, which the
    /// dispatcher resolves on its own written-path route.
    pub(crate) fn qualified(&self) -> bool {
        self.qualified
    }
    pub(crate) fn pending(&self) -> Option<ValueGoalDemand> {
        self.demand
    }
    fn finish(mut self) -> Result<Self, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(n)
                .ok_or_else(crate::runtime::arena::resource)?;
            Ok(())
        };
        if let Some(ValueContinuation::Module { name, .. }) = &self.continuation {
            add(name.capacity())?;
        }
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
    const DOMAIN: &'static str = "lrr/v2/entered-value-source-goal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.context().source().identity());
        e.digest(self.occurrence);
        e.digest(self.reads);
        e.option(&self.local, |e, id| e.digest(*id));
        e.option(&self.continuation, |e, continuation| match continuation {
            ValueContinuation::Module {
                name,
                raw,
                edition,
                name_range,
            } => {
                e.u8(0);
                e.string(name);
                e.digest(Digest(raw.0));
                e.u16(*edition);
                e.u32(name_range.start);
                e.u32(name_range.end);
            }
            ValueContinuation::Qualified => e.u8(1),
            ValueContinuation::Declaration { selected, foreign } => {
                e.u8(2);
                e.record(selected);
                e.bool(*foreign);
            }
        });
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
            e.option(&declaration.selected_kind, |e, kind| e.u32(kind.tag()));
            e.option(&declaration.variant, |e, variant| {
                e.digest(variant.enumeration);
                e.string(&variant.name);
            });
        });
        // The facade function's definition identity is folded into `reads`.
        e.bool(self.facade.is_some());
        e.bool(self.type_namespace);
        e.option(&self.rooted, |e, member| e.string(member));
        e.bool(self.qualified_path);
        e.bool(self.qualified);
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
    let key = PcKey {
        body: body.clone(),
        node: node.0,
        phase: 0,
    };
    let prefix = polled!(db.fetch(&SyntaxProvider, QueryKey::LexicalValue(key)));
    let QueryValue::LexicalValue(prefix) = prefix.value.as_ref() else {
        return QueryPoll::Ready(Err(error("Value lexical query variant differs")));
    };
    let mut goal = prefix.as_ref().clone();
    if goal.pending().is_some() {
        return QueryPoll::Ready(goal.finish());
    }
    let Some(continuation) = goal.continuation.take() else {
        return QueryPoll::Ready(goal.finish());
    };
    match continuation {
        ValueContinuation::Qualified => {
            let source = goal.plan.context().source();
            let syntax = checked!(source.parsed().entered_node_syntax(
                source.body(),
                source.index(),
                node
            ));
            let Some(expression) = syntax.and_then(ast::PathExpr::cast) else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            goal.rooted = None;
            goal.type_namespace = false;
            qualified(db, goal, &expression)
        }
        ValueContinuation::Declaration { foreign: true, .. } => {
            goal.needs(ValueGoalDemand::PathAdjustment)
        }
        ValueContinuation::Declaration {
            selected,
            foreign: false,
        } => selected_declaration(db, goal, selected),
        ValueContinuation::Module {
            name,
            raw,
            edition,
            name_range,
        } => complete_module(db, goal, name, raw, edition, name_range),
    }
}
/// Shared hygienic lexical selection. It never prepares a global declaration
/// or callee signature/body. Capture discovery and entered reads share this memo.
pub fn prepare_lexical(
    db: &mut Database,
    body: &BodyKey,
    node: NodeId,
) -> QueryPoll<EnteredValueGoal> {
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
        continuation: None,
        target: None,
        declaration: None,
        facade: None,
        facade_identity: None,
        type_namespace: false,
        rooted: None,
        qualified_path: false,
        qualified: false,
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
    // The owning root pattern of a binding target (absolute source range).
    let mut recipe_root = crate::syntax::spans::SourceRange { start: 0, end: 0 };
    // A path, tuple-struct or record pattern names a constructor through its
    // single written name; a record pattern's name is a Type-namespace name.
    let pattern_path = if indexed.role == NodeRole::Pattern {
        ast::TupleStructPat::cast(syntax.clone())
            .and_then(|pattern| pattern.path().map(|path| (path, false)))
            .or_else(|| {
                ast::RecordPat::cast(syntax.clone())
                    .and_then(|pattern| pattern.path().map(|path| (path, true)))
            })
            .or_else(|| {
                ast::PathPat::cast(syntax.clone())
                    .and_then(|pattern| pattern.path().map(|path| (path, false)))
            })
    } else {
        None
    };
    // A record expression's written path (`Root::Variant { .. }`) names its
    // root in the Type namespace like a record pattern does.
    let record_path = if matches!(indexed.role, NodeRole::Expression(kind) if kind != ExpressionKind::Path)
    {
        ast::RecordExpr::cast(syntax.clone()).and_then(|record| record.path())
    } else {
        None
    };
    // A `Root::member` expression path whose lexical root does not bind
    // returns to the module route at the prefix result.
    let mut rooted_fallback: Option<ast::PathExpr> = None;
    let (name, name_range, target) = match indexed.role {
        NodeRole::Expression(_) if record_path.is_some() => {
            let Some(path) = record_path else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            let Some((name, member)) = crate::syntax::parse::ParsedFile::lexical_root_name(&path)
            else {
                return goal.needs(ValueGoalDemand::PathAdjustment);
            };
            goal.qualified_path = path.qualifier().is_some();
            goal.type_namespace = true;
            goal.rooted = member.map(|member| member.text().to_string());
            (
                checked!(owned_spelling(name.text())),
                range(name.syntax().text_range()),
                None,
            )
        }
        NodeRole::Pattern if pattern_path.is_some() => {
            let Some((path, record)) = pattern_path else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            let qualified_pattern = path.qualifier().is_some() || path.coloncolon_token().is_some();
            let lexical_root = crate::syntax::parse::ParsedFile::lexical_root_name(&path);
            // A qualified pattern path whose root no block item or import
            // can bind (three or more segments, a path-keyword or absolute
            // root, or `Root::member` with an unbound root) belongs to the
            // module route, exactly like a qualified expression path: the
            // consumer resolves its written spelling through
            // `CallResolver::lookup_written` in the caller's environment.
            let module_route = qualified_pattern
                && match &lexical_root {
                    Some((_, Some(_))) => !checked!(
                        source
                            .parsed()
                            .pattern_path_binder(&path, source.index().cfg())
                    ),
                    Some((_, None)) => false,
                    None => true,
                };
            if module_route {
                goal.qualified_path = true;
                goal.type_namespace = record;
                let mut occurrence =
                    StableEncoder::new("lrr/v1/entered-value-qualified-pattern-occurrence");
                let written = range(path.syntax().text_range());
                occurrence.string(&path.syntax().text().to_string());
                occurrence.u32(written.start);
                occurrence.u32(written.end);
                goal.occurrence = occurrence.finish();
                if let Some(missing) = plan.missing_expansions.first() {
                    return goal.needs(ValueGoalDemand::ExpansionDefinition(*missing));
                }
                return QueryPoll::Ready(goal.finish());
            }
            let Some((name, member)) = lexical_root else {
                return goal.needs(ValueGoalDemand::PathAdjustment);
            };
            goal.qualified_path = qualified_pattern;
            // The root of `Root::member` names a type; a single name is a
            // Type-namespace name only for a record pattern.
            goal.type_namespace = record || member.is_some();
            goal.rooted = member.map(|member| member.text().to_string());
            (
                checked!(owned_spelling(name.text())),
                range(name.syntax().text_range()),
                None,
            )
        }
        NodeRole::Pattern => {
            let Some(pattern) = ast::IdentPat::cast(syntax) else {
                return goal.needs(ValueGoalDemand::Occurrence);
            };
            // An `@` subpattern target requires the retained canonical group
            // join. A binding nested in a tuple/struct/reference pattern is
            // the same local at its own occurrence; its recipe names the
            // owning root pattern, checked against the occurrence row below.
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
            if !recipe
                .pattern_range
                .contains(range(pattern.syntax().text_range()))
            {
                return goal.needs(ValueGoalDemand::Occurrence);
            }
            checked!(
                recipe
                    .name
                    .len()
                    .checked_add(recipe.raw_name.len())
                    .ok_or_else(crate::runtime::arena::resource)
            );
            let target = BindingTarget {
                identity: recipe.identity,
                name: recipe.name.clone(),
                raw_name: recipe.raw_name.clone(),
                name_range,
                classification: recipe.classification,
            };
            recipe_root = recipe.pattern_range;
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
                goal.qualified_path = true;
                // `Root::member` whose root a block item or import may bind:
                // the root is selected through the entered lexical prefix;
                // any other qualified path keeps the module route.
                let lexical = match crate::syntax::parse::ParsedFile::lexical_root_name(&path) {
                    Some((root, Some(member))) => {
                        let binds =
                            checked!(source.parsed().entered_value_requires_lexical_context(
                                source.body(),
                                source.index(),
                                &goal.key
                            ));
                        binds.then_some((root, member))
                    }
                    _ => None,
                };
                let Some((root, member)) = lexical else {
                    goal.continuation = Some(ValueContinuation::Qualified);
                    return QueryPoll::Ready(goal.finish());
                };
                goal.type_namespace = true;
                goal.rooted = Some(member.text().to_string());
                rooted_fallback = Some(expression);
                (
                    checked!(owned_spelling(root.text())),
                    range(root.syntax().text_range()),
                    None,
                )
            } else {
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
        }
        _ => return goal.needs(ValueGoalDemand::Occurrence),
    };
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
    // A token owned by another edition (macro output from another crate):
    // its hygienic local bindings resolve exactly as the body's own, and a
    // module request carries the token's edition. An item selection by such
    // a token remains held below.
    let foreign = edition != plan.entered.parent.source().outline.parse.profile.edition;
    if let Some(target) = target {
        if checked!(crate::frontend::names::owned_name(&name, edition)).as_str() != target.name {
            return QueryPoll::Ready(Err(error("Value binding target spelling differs")));
        }
        use crate::frontend::pattern_binding_occurrences::PatternLocalOccurrence;
        use crate::syntax::pattern_bindings::{BindingContext, BindingUncertainty};
        let Some(row) = plan.pattern_occurrences.for_local(target.identity) else {
            return goal.needs(ValueGoalDemand::Occurrence);
        };
        let row = checked!(plan.pattern_occurrences.select(
            source.index(),
            &goal.key,
            row.pattern()
        ));
        let root_range = source
            .index()
            .nodes()
            .get(row.root().0 as usize)
            .and_then(|root| {
                let start = source.body().range.start;
                Some(crate::syntax::spans::SourceRange {
                    start: start.checked_add(root.range.start)?,
                    end: start.checked_add(root.range.end)?,
                })
            });
        if row.pattern() != node
            || root_range != Some(recipe_root)
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
    let namespace = if goal.type_namespace {
        Namespace::Type
    } else {
        Namespace::Value
    };
    let site = OrdinaryNameSite {
        source: plan.entered.parent.source(),
        body,
        at: u64::from(name_range.start),
        name: &name,
        context: raw,
        namespace,
    };
    let (prefix, reads) = checked!(adapter.lookup_prepared_value_prefix_observed(
        &site,
        &plan.type_names,
        environment.names()
    ));
    let lookup = match prefix {
        PrefixResolution::Name(lookup) => lookup,
        PrefixResolution::ModuleTail => {
            goal.continuation = Some(if rooted_fallback.is_some() {
                ValueContinuation::Qualified
            } else {
                ValueContinuation::Module {
                    name,
                    raw,
                    edition,
                    name_range,
                }
            });
            // Certify the lexical absence/rib decision as well as positive
            // selections. A later local/item/import producer invalidates it.
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
            return QueryPoll::Ready(goal.finish());
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
    complete_lexical_lookup(db, goal, lookup, foreign, name_range)
}
fn complete_lexical_lookup(
    db: &mut Database,
    mut goal: EnteredValueGoal,
    lookup: NameLookup,
    foreign: bool,
    name_range: SourceRange,
) -> QueryPoll<EnteredValueGoal> {
    let plan = Arc::clone(&goal.plan);
    let adapter = plan.adapter();
    let body = goal.key.body.clone();
    let node = NodeId(goal.key.node);
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
            goal.continuation = Some(ValueContinuation::Declaration { selected, foreign });
        }
        NameLookup::NeedInput(id) => {
            use crate::frontend::pattern_binding_occurrences::PatternLocalOccurrence;
            let Some(prior) = plan.pattern_occurrences.for_local(id) else {
                // The same hygienic prefix selected one of this function's
                // own parameters: an ordinary local with no body pattern.
                if goal.target.is_none() && adapter.parameter_recipe(id).is_some() {
                    goal.local = Some(id);
                    return QueryPoll::Ready(goal.finish());
                }
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
fn complete_module(
    db: &mut Database,
    mut goal: EnteredValueGoal,
    name: String,
    raw: Key,
    edition: u16,
    name_range: SourceRange,
) -> QueryPoll<EnteredValueGoal> {
    let plan = Arc::clone(&goal.plan);
    let (_, hygiene) = plan.entered.local_hygiene();
    let Some(environment) = plan.environment() else {
        return goal.needs(ValueGoalDemand::ModulePublication);
    };
    let environment = environment.macros().source_environment();
    let namespace = if goal.type_namespace {
        Namespace::Type
    } else {
        Namespace::Value
    };
    let request = match checked!(plan.lexical_module.route(&hygiene, raw)) {
        ModuleRoute::Definition(expansion) => {
            let Some(request) =
                polled!(plan.definition_module_request(db, raw, &name, edition, namespace))
            else {
                // A pattern name written by a compiler builtin expander
                // (rustc `#[rustc_builtin_macro]` derives) at its def site:
                // the expander's own library definition site holds no
                // program module, and the identifiers the compiler-owned
                // expander introduces there (`__self_discr`, `__self_0`,
                // `cmp`) are fresh bindings, never items of that module.
                // The decision depends only on the selected expansion.
                if goal.target.is_some()
                    && namespace == Namespace::Value
                    && let Some(definition) =
                        polled!(plan.definition_module_compiler_owned(db, raw))
                {
                    let mut identity =
                        StableEncoder::new("lrr/v1/entered-pattern-compiler-definition-site");
                    identity.digest(goal.reads);
                    identity.digest(definition);
                    identity.digest(Digest(expansion.0));
                    goal.reads = identity.finish();
                    return complete_lexical_lookup(
                        db,
                        goal,
                        NameLookup::Absent,
                        false,
                        name_range,
                    );
                }
                return goal.needs(ValueGoalDemand::DefinitionModule(expansion));
            };
            Some(request)
        }
        ModuleRoute::Lexical(_) => checked!(
            plan.lexical_module
                .request_in_namespace(&name, edition, namespace)
        ),
    };
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
    let mut identity = StableEncoder::new("lrr/v1/entered-value-module-and-prefix");
    identity.digest(goal.reads);
    identity.digest(answer.semantic_identity());
    goal.reads = identity.finish();
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

    let foreign = edition != plan.entered.parent.source().outline.parse.profile.edition;
    match lookup.clone() {
        NameLookup::Defined(_) if foreign => goal.needs(ValueGoalDemand::PathAdjustment),
        NameLookup::Defined(selected) => selected_declaration(db, goal, selected),
        lookup => complete_lexical_lookup(db, goal, lookup, foreign, name_range),
    }
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
/// A qualified value path (`m::f`, `::core::panicking::panic`, a generated
/// `$crate::…` root) selected through the dispatcher's own route: its written
/// spelling resolved by `CallResolver::lookup_written` in the caller's
/// completed environment. Type-relative paths (`P::f`) remain pending.
fn qualified(
    db: &mut Database,
    mut goal: EnteredValueGoal,
    expression: &ast::PathExpr,
) -> QueryPoll<EnteredValueGoal> {
    let plan = Arc::clone(&goal.plan);
    let source = plan.context().source();
    // A call's callee path leaves its final segment's explicit type
    // arguments to the call.
    let is_callee = expression
        .syntax()
        .parent()
        .and_then(ast::CallExpr::cast)
        .and_then(|call| call.expr())
        .is_some_and(|callee| callee.syntax() == expression.syntax());
    let spelling = if is_callee {
        source
            .parsed()
            .written_callee_spelling(expression, source.index().cfg())
    } else {
        source
            .parsed()
            .written_value_spelling(expression, source.index().cfg())
    };
    let Some(spelled) = checked!(spelling) else {
        return goal.needs(ValueGoalDemand::PathAdjustment);
    };
    goal.qualified = true;
    let path_range = range(expression.syntax().text_range());
    let mut occurrence = StableEncoder::new("lrr/v1/entered-value-qualified-occurrence");
    occurrence.string(&spelled);
    occurrence.u32(path_range.start);
    occurrence.u32(path_range.end);
    goal.occurrence = occurrence.finish();
    if let Some(missing) = plan.missing_expansions.first() {
        return goal.needs(ValueGoalDemand::ExpansionDefinition(*missing));
    }
    let Some(environment) = plan.environment() else {
        return goal.needs(ValueGoalDemand::ModulePublication);
    };
    let environment = Arc::clone(environment.macros().source_environment());
    let sources: Vec<_> = environment.sources().iter().collect();
    let resolver = checked!(crate::frontend::callables::CallResolver::new(
        environment.names(),
        &sources
    ));
    let caller = goal.key.body.definition;
    let lookup = polled!(resolver.lookup_written(db, caller, &spelled, Namespace::Value));
    let mut impl_owned = false;
    let mut selected_kind = None;
    let selected = match lookup {
        NameLookup::Defined(selected) => selected,
        // A type-relative associated function (`P::f`, `Self::f`): the
        // dispatcher's own route with this environment's shared impl index.
        NameLookup::Absent => {
            let members = checked!(environment.member_index());
            let resolver = resolver.with_members(Some(&members));
            let selection = polled!(resolver.source_function(db, caller, &spelled));
            // An associated constant (`Type::NAME`, `<T as Tr>::NAME`): an
            // impl/trait member, never a body-local declaration.
            if let crate::frontend::callables::CallSelection::NeedValue(definition) = &selection
                && definition.item_kind == crate::source::manifest::ItemKind::Constant
            {
                let identity = definition.fingerprint();
                let edition = match db.input(&InputKey::DefinitionHeader(identity)).as_deref() {
                    Some(InputValue::DefinitionHeader(header)) => header.profile.edition,
                    _ => return goal.needs(ValueGoalDemand::DeclarationPublication(identity)),
                };
                let selected = NameDefinition {
                    namespace: Namespace::Value,
                    identity,
                    defining_unit: definition.crate_key,
                    definition_edition: edition,
                };
                let mut reads = StableEncoder::new("lrr/v1/entered-value-qualified-read");
                reads.string(&spelled);
                reads.record(&selected);
                goal.reads = reads.finish();
                return declaration(db, goal, selected, true, Some(definition.item_kind));
            }
            let crate::frontend::callables::CallSelection::Source(callee) = selection else {
                return goal.needs(ValueGoalDemand::PathAdjustment);
            };
            selected_kind = Some(callee.definition().item_kind);
            impl_owned = callee.signature().scope.owner.is_some()
                && callee.definition().enclosing_impl == callee.signature().scope.owner;
            NameDefinition {
                namespace: Namespace::Value,
                identity: callee.identity(),
                defining_unit: callee.definition().crate_key,
                definition_edition: callee.signature().header.profile.edition,
            }
        }
        _ => return goal.needs(ValueGoalDemand::PathAdjustment),
    };
    let mut reads = StableEncoder::new("lrr/v1/entered-value-qualified-read");
    reads.string(&spelled);
    reads.record(&selected);
    goal.reads = reads.finish();
    declaration(db, goal, selected, impl_owned, selected_kind)
}

fn selected_declaration(
    db: &mut Database,
    goal: EnteredValueGoal,
    selected: NameDefinition,
) -> QueryPoll<EnteredValueGoal> {
    declaration(db, goal, selected, false, None)
}
/// `impl_owned`: an associated function selected by its owning impl. An
/// impl introduces no body or namespace; it has the module's environment.
fn declaration(
    db: &mut Database,
    mut goal: EnteredValueGoal,
    selected: NameDefinition,
    impl_owned: bool,
    selected_kind: Option<crate::source::manifest::ItemKind>,
) -> QueryPoll<EnteredValueGoal> {
    let id = selected.identity;
    let expected = if goal.type_namespace {
        Namespace::Type
    } else {
        Namespace::Value
    };
    if selected.namespace != expected {
        return QueryPoll::Ready(Err(error("Value goal selected another namespace")));
    }
    // A closed library facade function has no source declaration; the
    // dispatcher selects it by this same definition identity.
    if goal.target.is_none()
        && let Some(function) = crate::facade::functions::select(&selected)
    {
        let mut identity = StableEncoder::new("lrr/v1/entered-value-facade");
        identity.digest(goal.reads);
        identity.digest(id);
        goal.reads = identity.finish();
        goal.facade = Some(function);
        goal.facade_identity = Some(id);
        return QueryPoll::Ready(goal.finish());
    }
    // A variant owns no declaration inputs: they are its enum's.
    let enum_variant = goal.plan.environment().and_then(|environment| {
        environment
            .macros()
            .source_environment()
            .names()
            .enum_variant(id)
            .cloned()
    });
    let input_id = enum_variant
        .as_ref()
        .map_or(id, |variant| variant.enumeration);
    let source = db.input(&InputKey::DefinitionSource(input_id));
    let header = db.input(&InputKey::DefinitionHeader(input_id));
    let context = db.input(&InputKey::DeclarationContext(input_id));
    let (Some(source), Some(header), Some(context)) = (source, header, context) else {
        return goal.needs(ValueGoalDemand::DeclarationPublication(input_id));
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
        db.published_input(&InputKey::DefinitionHeader(input_id))
            .map(|(_, stamp)| stamp)
            .ok_or_else(|| error("Value goal actual header stamp absent"))
    );
    let cached_header = checked!(
        goal.plan
            .field_types
            .header_identity(input_id, header_stamp)
    );
    let owner = if context.scope.owner.is_some() && !impl_owned {
        let owner = db.input(&InputKey::LocalDeclarationOwner(id));
        let Some(owner) = owner else {
            return goal.needs(ValueGoalDemand::DeclarationOwner(id));
        };
        let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
            return QueryPoll::Ready(Err(error("Value goal declaration owner variant differs")));
        };
        // The declaring body is this body or one of its enclosing bodies (a
        // nested `fn` or an impl member sees the block items of its parents).
        let mut declaring = goal.plan.context();
        let mut enclosing = declaring.source().local_parent();
        while declaring.source().index().body() != owner {
            let Some(parent) = enclosing else {
                return goal.needs(ValueGoalDemand::DeclarationOwner(id));
            };
            declaring = parent.context();
            enclosing = declaring.source().local_parent();
        }
        let Some(row) = declaring
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
    if goal.target.is_some() && enum_variant.is_some() {
        // An enum variant is never a function: an identifier pattern naming
        // one (a named import or glob of its variants) is a path to it.
        goal.declaration = Some(SelectedValueDeclaration {
            definition: selected,
            source: source.clone(),
            header: cached_header.unwrap_or_else(|| header.fingerprint()),
            context: context.clone(),
            owner,
            variant: enum_variant,
            selected_kind,
        });
        return QueryPoll::Ready(goal.finish());
    }
    if goal.target.is_some() {
        // A positive Value answer is not automatically a constant. rustc's
        // late::try_resolve_as_non_binding permits fresh bindings to shadow
        // Fn/AssocFn, but not constant/constructor/static definitions. Resolve
        // the actual canonical source kind before admitting this binding.
        return shadowed_definition_binding(
            db,
            goal,
            selected,
            source,
            header,
            context,
            Some((cached_header, owner)),
        );
    }
    checked!(
        source
            .profile
            .semantics
            .len()
            .checked_add(context.scope.heap_bytes())
            .and_then(|bytes| bytes.checked_add(std::mem::size_of::<EnteredValueGoal>()))
            .ok_or_else(crate::runtime::arena::resource)
    );
    goal.declaration = Some(SelectedValueDeclaration {
        definition: selected,
        source: source.clone(),
        header: cached_header.unwrap_or_else(|| header.fingerprint()),
        context: context.clone(),
        owner,
        variant: enum_variant,
        selected_kind,
    });
    QueryPoll::Ready(goal.finish())
}

pub(crate) enum CanonicalValueKind {
    Ready(crate::source::manifest::ItemKind),
    Pending(ValueGoalDemand),
    Absent,
}
fn canonical_value_kind(
    db: &mut Database,
    goal: &EnteredValueGoal,
    selected: &NameDefinition,
    source: &ParseKey,
    header: &crate::syntax::outline::HeaderRecipe,
    context: &DeclarationContext,
) -> QueryPoll<CanonicalValueKind> {
    use crate::frontend::callables::entered_module::{
        CanonicalModulePreparation, canonical_module_declaration_in_namespace,
    };
    let id = selected.identity;
    // A nested function/initializer retains the actual lexical parent plans.
    // Source ownership comes from the published declaration owner, not the
    // current executable body's occurrence table or a module spelling.
    let owner = db.input(&InputKey::LocalDeclarationOwner(id));
    if let Some(owner) = owner {
        let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
            return QueryPoll::Ready(Err(error("Value kind owner input variant differs")));
        };
        let mut declaring = goal.plan.context();
        let mut enclosing = declaring.source().local_parent();
        while declaring.source().index().body() != owner {
            let Some(parent) = enclosing else {
                return QueryPoll::Ready(Ok(CanonicalValueKind::Pending(
                    ValueGoalDemand::DeclarationOwner(id),
                )));
            };
            declaring = parent.context();
            enclosing = declaring.source().local_parent();
        }
        let Some(row) = declaring
            .facts()
            .declarations()
            .find(|row| row.item.def_key.fingerprint() == id)
        else {
            return QueryPoll::Ready(Ok(CanonicalValueKind::Pending(
                ValueGoalDemand::DeclarationOccurrence(id),
            )));
        };
        if declaring.source().parsed().key() != *source
            || row.item.scope != context.scope
            || row.item.declaration_span != context.range
            || !row.item.header.same_semantics(header)
        {
            return QueryPoll::Ready(Err(error("Value kind canonical parent row differs")));
        }
        return QueryPoll::Ready(Ok(CanonicalValueKind::Ready(row.item.def_key.item_kind)));
    }
    if context.scope.owner.is_some() {
        return QueryPoll::Ready(Ok(CanonicalValueKind::Pending(
            ValueGoalDemand::DeclarationOwner(id),
        )));
    }
    let namespace = if goal.type_namespace { Namespace::Type } else { Namespace::Value };
    match polled!(canonical_module_declaration_in_namespace(
        db, &goal.plan, selected, namespace, source, header, context
    )) {
        CanonicalModulePreparation::Ready(actual) => {
            QueryPoll::Ready(Ok(CanonicalValueKind::Ready(actual.definition().item_kind)))
        }
        CanonicalModulePreparation::Pending(demand) => {
            QueryPoll::Ready(Ok(CanonicalValueKind::Pending(demand)))
        }
    }
}

/// `non_binding`: the cached header identity and local owner of the selected
/// declaration. A non-function selection then makes the identifier pattern a
/// path to that declaration (a constant, unit constructor or static), as in
/// rustc; without it, such a selection stays pending.
pub(super) fn shadowed_definition_binding(
    db: &mut Database,
    mut goal: EnteredValueGoal,
    selected: NameDefinition,
    source: &ParseKey,
    header: &crate::syntax::outline::HeaderRecipe,
    context: &DeclarationContext,
    non_binding: Option<(Option<Digest>, Option<BodyKey>)>,
) -> QueryPoll<EnteredValueGoal> {
    use crate::source::manifest::ItemKind;
    let id = selected.identity;
    let canonical_kind = match polled!(canonical_value_kind(
        db, &goal, &selected, source, header, context
    )) {
        CanonicalValueKind::Ready(kind) => kind,
        CanonicalValueKind::Pending(demand) => return goal.needs(demand),
        CanonicalValueKind::Absent => {
            return goal.needs(ValueGoalDemand::DeclarationOccurrence(id));
        }
    };
    if canonical_kind != ItemKind::Function {
        let Some((cached_header, owner)) = non_binding else {
            return goal.needs(ValueGoalDemand::BindingClassification(id));
        };
        goal.declaration = Some(SelectedValueDeclaration {
            definition: selected,
            source: source.clone(),
            header: cached_header.unwrap_or_else(|| header.fingerprint()),
            context: context.clone(),
            owner,
            variant: None,
            selected_kind: Some(canonical_kind),
        });
        return QueryPoll::Ready(goal.finish());
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
    checked!(proof.metadata_bytes());
    let retained = checked!(proof.retain(source, &request.bucket.scope));
    QueryPoll::Ready(Ok(matches!(
        retained.lookup(&request.bucket.name),
        RetainedPreludeName::AbsentInSelectedFacet
    )
    .then_some(retained.identity())))
}

fn owned_spelling(text: &str) -> Result<String, Diagnostic> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| crate::runtime::arena::resource())?;
    owned.push_str(text);
    Ok(owned)
}
