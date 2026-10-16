//! Immutable metadata for an actual entered ordinary CallExpr. Name selection
//! is the common Value goal; this never publishes or prepares a callee body.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        callables::{
            EnteredLocalCall, ordinary_local::OrdinaryLocalAdmission,
            prepare_entered_local_function,
        },
        source_locals::PreparedSourceLocalPlan,
        source_value_goals::{SelectedValueDeclaration, ValueGoalDemand},
    },
    ir::pc::{NodeId, PcKey, Phase},
    syntax::{
        ast_adapter::range,
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericArgs},
};
use std::sync::Arc;

const LIMIT: usize = 32 * 1024 * 1024;
const ARC_HEADER: usize = 2 * std::mem::size_of::<usize>();
macro_rules! ready {
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
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
fn add(bytes: usize, additional: usize) -> Result<usize, Diagnostic> {
    bytes
        .checked_add(additional)
        .filter(|bytes| *bytes <= LIMIT)
        .ok_or_else(crate::runtime::arena::resource)
}

/// Actual written source data, not a name-selection or call-entry authority.
/// Fields are private; only the indexed immutable PathExpr can construct it.
#[derive(Debug)]
pub(crate) struct EnteredSourceName {
    node: NodeId,
    /// Actual immutable syntax-tree coordinates, for the normal AST lowerer.
    range: SourceRange,
    /// Body-relative StructuralNode coordinates; never compared as absolute.
    indexed_range: SourceRange,
    name: String,
    explicit: Vec<String>,
}
impl EnteredSourceName {
    pub(crate) fn node(&self) -> NodeId {
        self.node
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn indexed_range(&self) -> SourceRange {
        self.indexed_range
    }
    pub(crate) fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn explicit(&self) -> &[String] {
        &self.explicit
    }
    fn heap_bytes(&self) -> Result<usize, Diagnostic> {
        self.explicit.iter().try_fold(
            add(
                self.name.capacity(),
                self.explicit
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )?,
            |bytes, argument| add(bytes, argument.capacity()),
        )
    }
}
impl StableEncode for EnteredSourceName {
    const DOMAIN: &'static str = "lrr/v2/entered-call-written-source-name";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.node.0);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.u32(self.indexed_range.start);
        e.u32(self.indexed_range.end);
        e.string(&self.name);
        e.sequence(&self.explicit, |e, argument| e.string(argument));
    }
}
enum Selection {
    Local(Arc<OrdinaryLocalAdmission>),
    Module {
        plan: Arc<PreparedSourceLocalPlan>,
        declaration: SelectedValueDeclaration,
    },
    Binding {
        plan: Arc<PreparedSourceLocalPlan>,
        identity: Digest,
    },
    Pending {
        plan: Arc<PreparedSourceLocalPlan>,
        demand: ValueGoalDemand,
    },
}
pub(crate) enum EnteredCallSelection<'a> {
    Local(&'a OrdinaryLocalAdmission),
    Module(&'a SelectedValueDeclaration),
    Binding(Digest),
    Pending(ValueGoalDemand),
}
/// Not Clone: the single physical parent handle is either the Local proof's
/// handle or the nonlocal variant's handle. SourceArtifactRef::LocalPlan must
/// account that actual handle, not a hidden duplicate or extracted proof Arc.
pub struct EnteredCallGoal {
    key: PcKey,
    selection: Selection,
    target: Option<EnteredSourceName>,
    call_range: SourceRange,
    structure: Digest,
    source: Digest,
    value: Option<Digest>,
    guard: Arc<PreparedInputGuard>,
    fingerprint: Digest,
    bytes: usize,
}
impl std::fmt::Debug for EnteredCallGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnteredCallGoal")
            .field("key", &self.key)
            .field("structure", &self.structure)
            .field("value", &self.value)
            .finish_non_exhaustive()
    }
}
impl EnteredCallGoal {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn guard(&self) -> &Arc<PreparedInputGuard> {
        &self.guard
    }
    pub(crate) fn value_identity(&self) -> Option<Digest> {
        self.value
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        match &self.selection {
            Selection::Local(proof) => proof.parent(),
            Selection::Module { plan, .. }
            | Selection::Binding { plan, .. }
            | Selection::Pending { plan, .. } => plan,
        }
    }
    pub(crate) fn selection(&self) -> EnteredCallSelection<'_> {
        match &self.selection {
            Selection::Local(proof) => EnteredCallSelection::Local(proof),
            Selection::Module { declaration, .. } => EnteredCallSelection::Module(declaration),
            Selection::Binding { identity, .. } => EnteredCallSelection::Binding(*identity),
            Selection::Pending { demand, .. } => EnteredCallSelection::Pending(*demand),
        }
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
    /// The reached coordinator must publish the missing exact body locator
    /// first. This reuses the immutable proof without moving it out of a memo;
    /// the resulting callable retains its own explicit parent-plan handle.
    pub(crate) fn local_callable(
        &self,
        db: &mut Database,
    ) -> Result<Box<crate::frontend::callables::SourceCallable>, Diagnostic> {
        self.require_current(db)?;
        match &self.selection {
            Selection::Local(proof) => proof.callable_shared(db),
            _ => Err(fault(
                "entered call is not an admitted local source function",
            )),
        }
    }
    /// Permission is limited to the SAME normal source CallExpr recipe. It
    /// does not bypass argument/type/call contracts, publication or body entry.
    /// Module/Binding/Pending never authorize a module-by-name fallback here.
    pub(crate) fn local_source_name(
        &self,
        db: &Database,
        index: &BodyIndex,
        phase: &PcKey,
    ) -> Result<Option<&EnteredSourceName>, Diagnostic> {
        self.require_current(db)?;
        self.indexed_local_source_name(index, phase)
    }
    /// The pure normal lowerer consumes this exact body/site/index proof after
    /// its DB coordinator validated currentness and attached the real guard.
    pub(crate) fn indexed_local_source_name(
        &self,
        index: &BodyIndex,
        phase: &PcKey,
    ) -> Result<Option<&EnteredSourceName>, Diagnostic> {
        let Selection::Local(proof) = &self.selection else {
            return Ok(None);
        };
        if self.key.phase != 0
            || phase.body != self.key.body
            || phase.node != self.key.node
            || index.body() != &self.key.body
            || index.semantic_identity() != self.structure
            || self.retained_plan().context().source().identity() != self.source
            || self
                .retained_plan()
                .context()
                .source()
                .index()
                .semantic_identity()
                != self.structure
            || proof.site() != &self.key
            || !matches!(
                Phase::from_code(phase.phase),
                Phase::Entry | Phase::Operand(_)
            )
            || index.pc(NodeId(phase.node), Phase::from_code(phase.phase))? != *phase
        {
            return Err(fault("entered call lowering source/body/site differs"));
        }
        let target = self
            .target
            .as_ref()
            .ok_or_else(|| fault("entered local call written target absent"))?;
        let call = index
            .nodes()
            .get(self.key.node as usize)
            .ok_or_else(|| fault("entered local call index absent"))?;
        let path = index
            .nodes()
            .get(target.node.0 as usize)
            .ok_or_else(|| fault("entered local call path absent"))?;
        if call.id != NodeId(self.key.node)
            || call.range != self.call_range
            || call.role != NodeRole::Expression(ExpressionKind::Call)
            || call.operands.first() != Some(&target.node)
            || path.parent != Some(call.id)
            || path.id != target.node
            || path.range != target.indexed_range
            || path.role != NodeRole::Expression(ExpressionKind::Path)
        {
            return Err(fault("entered local call indexed target differs"));
        }
        Ok(Some(target))
    }
    fn finish(mut self) -> Result<Self, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        if let Some(target) = &self.target {
            bytes = add(bytes, target.heap_bytes()?)?;
        }
        match &self.selection {
            // Includes the admission Arc header and the guard allocation/header;
            // self.guard aliases that existing allocation, not another payload.
            Selection::Local(proof) => bytes = add(bytes, proof.metadata_bytes())?,
            Selection::Module { declaration, .. } => {
                bytes = add(bytes, declaration.source().profile.semantics.capacity())?;
                bytes = add(bytes, declaration.context().scope.heap_bytes())?;
                bytes = add(bytes, add(self.guard.metadata_bytes(), ARC_HEADER)?)?;
            }
            Selection::Binding { .. } | Selection::Pending { .. } => {
                bytes = add(bytes, add(self.guard.metadata_bytes(), ARC_HEADER)?)?;
            }
        }
        self.bytes = bytes;
        self.fingerprint = StableEncode::fingerprint(&self);
        Ok(self)
    }
}
fn encode_demand(demand: ValueGoalDemand, e: &mut StableEncoder) {
    match demand {
        ValueGoalDemand::Occurrence => e.u8(0),
        ValueGoalDemand::PathAdjustment => e.u8(1),
        ValueGoalDemand::BindingClassification(id) => {
            e.u8(2);
            e.digest(id);
        }
        ValueGoalDemand::Prefix(id) => {
            e.u8(3);
            e.digest(id);
        }
        ValueGoalDemand::ModulePublication => e.u8(4),
        ValueGoalDemand::ModuleCompletion(id) => {
            e.u8(5);
            e.digest(id);
        }
        ValueGoalDemand::SourceOwner(id) => {
            e.u8(6);
            e.digest(id);
        }
        ValueGoalDemand::ScopeCertificate(id) => {
            e.u8(7);
            e.digest(id);
        }
        ValueGoalDemand::DeclarationPublication(id) => {
            e.u8(8);
            e.digest(id);
        }
        ValueGoalDemand::DeclarationOwner(id) => {
            e.u8(9);
            e.digest(id);
        }
        ValueGoalDemand::DeclarationOccurrence(id) => {
            e.u8(10);
            e.digest(id);
        }
        ValueGoalDemand::DefinitionModule(key) => {
            e.u8(11);
            e.digest(Digest(key.0));
        }
        ValueGoalDemand::ExpansionDefinition(key) => {
            e.u8(12);
            e.digest(Digest(key.0));
        }
        ValueGoalDemand::Prelude => e.u8(13),
    }
}
impl StableEncode for EnteredCallGoal {
    const DOMAIN: &'static str = "lrr/v2/entered-ordinary-source-call-goal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.structure);
        e.digest(self.source);
        e.digest(self.retained_plan().source_identity());
        e.u32(self.call_range.start);
        e.u32(self.call_range.end);
        e.option(&self.value, |e, value| e.digest(*value));
        e.digest(self.guard.fingerprint());
        e.option(&self.target, |e, target| e.record(target));
        match &self.selection {
            Selection::Local(proof) => {
                e.u8(0);
                e.record(&proof.item().def_key);
                e.record(proof.source());
                e.digest(proof.item().header.fingerprint());
                e.u32(proof.scope());
                e.record(&proof.item().scope);
                e.u32(proof.item().declaration_span.start);
                e.u32(proof.item().declaration_span.end);
            }
            Selection::Module { declaration, .. } => {
                e.u8(1);
                e.record(declaration.definition());
                e.record(declaration.source());
                e.digest(declaration.header());
                e.record(declaration.context());
                e.option(&declaration.owner(), |e, owner| e.record(*owner));
            }
            Selection::Binding { identity, .. } => {
                e.u8(2);
                e.digest(*identity);
            }
            Selection::Pending { demand, .. } => {
                e.u8(3);
                encode_demand(*demand, e);
            }
        }
    }
}

fn written_target(
    plan: &PreparedSourceLocalPlan,
    path: NodeId,
) -> Result<Option<EnteredSourceName>, Diagnostic> {
    let source = plan.context().source();
    let Some(syntax) = source
        .parsed()
        .entered_node_syntax(source.body(), source.index(), path)?
    else {
        return Ok(None);
    };
    let Some(expression) = ast::PathExpr::cast(syntax) else {
        return Ok(None);
    };
    let Some(path_syntax) = expression.path() else {
        return Ok(None);
    };
    if path_syntax.qualifier().is_some() || path_syntax.coloncolon_token().is_some() {
        return Ok(None);
    }
    let Some(segment) = path_syntax.segment() else {
        return Ok(None);
    };
    let Some(name) = segment.name_ref() else {
        return Ok(None);
    };
    let count = segment
        .generic_arg_list()
        .map_or(0, |list| list.generic_args().count());
    if count > 100_000 {
        return Err(crate::runtime::arena::resource());
    }
    let mut bytes = add(
        name.text().len(),
        count
            .checked_mul(std::mem::size_of::<String>())
            .ok_or_else(crate::runtime::arena::resource)?,
    )?;
    let mut explicit = Vec::new();
    explicit
        .try_reserve_exact(count)
        .map_err(|_| crate::runtime::arena::resource())?;
    if let Some(list) = segment.generic_arg_list() {
        for argument in list.generic_args() {
            bytes = add(bytes, usize::from(argument.syntax().text_range().len()))?;
            explicit.push(argument.syntax().text().to_string());
        }
    }
    let actual_range = range(expression.syntax().text_range());
    let indexed_range = source
        .index()
        .nodes()
        .get(path.0 as usize)
        .ok_or_else(|| fault("entered written target indexed node absent"))?
        .range;
    let projected = SourceRange {
        start: source
            .body()
            .range
            .start
            .checked_add(indexed_range.start)
            .ok_or_else(crate::runtime::arena::resource)?,
        end: source
            .body()
            .range
            .start
            .checked_add(indexed_range.end)
            .ok_or_else(crate::runtime::arena::resource)?,
    };
    if actual_range != projected || !source.body().range.contains(actual_range) {
        return Err(fault(
            "entered written target absolute/relative source ranges differ",
        ));
    }
    let target = EnteredSourceName {
        node: path,
        range: actual_range,
        indexed_range,
        name: name.text().to_string(),
        explicit,
    };
    target.heap_bytes()?;
    Ok(Some(target))
}

/// Future pure memo provider. No source capture/publication, body entry, child
/// BodyStructure, executable argument preparation or normal runtime work occurs.
pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<EnteredCallGoal> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("entered call requires entry key")));
    }
    let parent_key = QueryKey::SourceLocalPlan(key.body.clone());
    let parent_value = ready!(db.fetch(&SyntaxProvider, parent_key.clone()));
    let QueryValue::SourceLocalPlan(parent) = parent_value.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("entered call owning plan differs")));
    };
    let parent = Arc::clone(parent);
    let index = parent.context().source().index();
    let call = checked!(
        index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| fault("entered call node absent"))
    );
    if call.id != NodeId(key.node)
        || index.body() != &key.body
        || call.role != NodeRole::Expression(ExpressionKind::Call)
    {
        return QueryPoll::Ready(Err(fault("entered call actual indexed owner differs")));
    }
    let path = call.operands.first().copied().filter(|id| {
        index.nodes().get(id.0 as usize).is_some_and(|node| {
            node.parent == Some(call.id) && node.role == NodeRole::Expression(ExpressionKind::Path)
        })
    });
    let selected = ready!(prepare_entered_local_function(db, key));
    let (value, demand) = if let Some(path) = path {
        let demand = QueryKey::EnteredValue(PcKey {
            body: key.body.clone(),
            node: path.0,
            phase: 0,
        });
        let value = ready!(db.fetch(&SyntaxProvider, demand.clone()));
        let QueryValue::EnteredValue(goal) = value.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("entered call Value observation differs")));
        };
        if !value.cacheable
            || goal.body() != &key.body
            || goal.node() != path
            || !parent.same_environment(goal.retained_plan())
        {
            return QueryPoll::Ready(Err(fault("entered call Value owner differs")));
        }
        (Some(value.fingerprint), (demand, value.fingerprint))
    } else {
        if !matches!(
            selected,
            EnteredLocalCall::Pending(ValueGoalDemand::PathAdjustment)
        ) || !parent_value.cacheable
        {
            return QueryPoll::Ready(Err(fault("entered call nonpath selection differs")));
        }
        (None, (parent_key, parent_value.fingerprint))
    };
    let guard = match &selected {
        EnteredLocalCall::Local(proof) => {
            checked!(proof.revalidate(db));
            if proof.site() != key || !parent.same_environment(proof.parent()) {
                return QueryPoll::Ready(Err(fault("entered call local admission owner differs")));
            }
            Arc::clone(proof.guard())
        }
        _ => Arc::new(ready!(db.prepare_input_guard(
            &SyntaxProvider,
            &demand.0,
            demand.1
        ))),
    };
    let target = match path {
        Some(path) => checked!(written_target(&parent, path)),
        None => None,
    };
    if matches!(selected, EnteredLocalCall::Local(_)) && target.is_none() {
        return QueryPoll::Ready(Err(fault("entered local call exact written name absent")));
    }
    let call_range = call.range;
    let structure = index.semantic_identity();
    let source = parent.context().source().identity();
    let selection = match selected {
        EnteredLocalCall::Local(proof) => Selection::Local(Arc::from(proof)),
        EnteredLocalCall::Module(declaration) => Selection::Module {
            plan: parent,
            declaration,
        },
        EnteredLocalCall::Binding(identity) => Selection::Binding {
            plan: parent,
            identity,
        },
        EnteredLocalCall::Pending(demand) => Selection::Pending {
            plan: parent,
            demand,
        },
    };
    QueryPoll::Ready(
        EnteredCallGoal {
            key: key.clone(),
            selection,
            target,
            call_range,
            structure,
            source,
            value,
            guard,
            fingerprint: Digest([0; 32]),
            bytes: 0,
        }
        .finish(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::{callables::CallSelection, compile_input_catalog::CompileInputCatalog},
        source::manifest::Namespace,
    };
    const SUFFIX: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
    );
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("entered call source: {error:?}"),
            QueryPoll::NeedInput(input) => panic!("entered call demand: {input:?}"),
        }
    }
    fn world(text: &str, edition: u16, entry: &str) -> (Database, Arc<PreparedSourceLocalPlan>) {
        world_with_retained(text, edition, entry, false)
    }
    fn world_with_retained(
        text: &str,
        edition: u16,
        entry: &str,
        retained: bool,
    ) -> (Database, Arc<PreparedSourceLocalPlan>) {
        let (provider, _, inputs) =
            crate::expand::source_declarative::assembly::tests::world(text, edition);
        let source = provider.module_source().share();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        db.trace.detailed = true;
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"entered ordinary call control"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"entered ordinary call compiler control"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Sealed(_) => {
                    discovery.publish(&mut db).unwrap();
                    break;
                }
                _ => panic!("actual entered-call discovery did not complete"),
            }
        }
        let CallSelection::Source(selected) = ready(discovery.entry_function(&mut db, entry))
        else {
            panic!("actual selected entry is not a source function")
        };
        let mut body = selected.body().unwrap().key();
        if retained {
            let prepared = discovery
                .prepare_body_syntax(source.selection(), selected.identity())
                .unwrap();
            let input = ready(crate::db::retained_syntax::RetainedSyntaxInput::prepare(
                &mut db,
                prepared,
                discovery.key().clone(),
            ));
            ready(db.publish_retained_syntax(Arc::clone(&input)));
            body = ready(db.publish_retained_body(input, selected.identity()));
        }
        let plan = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
        (db, plan)
    }
    fn calls(plan: &PreparedSourceLocalPlan) -> Vec<PcKey> {
        let index = plan.context().source().index();
        index
            .nodes()
            .iter()
            .filter(|node| node.role == NodeRole::Expression(ExpressionKind::Call))
            .map(|node| index.pc(node.id, Phase::Entry).unwrap())
            .collect()
    }
    fn no_child_code(db: &Database, start: usize) {
        assert!(
            !db.trace.events[start..].iter().any(|event| matches!(
                event.key,
                QueryKey::BodyStructure(_)
                    | QueryKey::Thunk(_)
                    | QueryKey::PatternPlan(_)
                    | QueryKey::Constant(_)
            )),
            "call goal prepared executable code"
        );
    }
    #[test]
    fn unchanged_calls_retain_actual_source_targets_and_phase_proof_all_editions_and_origins() {
        for edition in [2015, 2018, 2021, 2024] {
            for mode in 0..4 {
                let text = match mode {
                    0 | 1 => SUFFIX.to_owned(),
                    2 => format!("macro_rules! emit{{()=>{{{SUFFIX}}}}}emit!();"),
                    _ => format!(
                        "macro_rules! emit{{()=>{{{SUFFIX}}}}}macro_rules! wrap{{()=>{{emit!();}}}}wrap!();"
                    ),
                };
                let (mut db, plan) = world_with_retained(&text, edition, "main", mode == 1);
                let body_start = plan.context().source().body().range.start;
                assert!(
                    body_start > 0,
                    "coordinate control requires a nonzero body offset"
                );
                let sites = calls(&plan);
                assert_eq!(sites.len(), 18);
                let index = plan.context().source().index();
                let start = db.trace.events.len();
                for site in &sites {
                    let goal = ready(prepare(&mut db, site));
                    let EnteredCallSelection::Local(proof) = goal.selection() else {
                        panic!("unchanged actual local call was not admitted")
                    };
                    assert!(Arc::ptr_eq(goal.retained_plan(), &plan));
                    assert_eq!(goal.fingerprint(), StableEncode::fingerprint(&goal));
                    let target = goal.local_source_name(&db, index, site).unwrap().unwrap();
                    assert_eq!(
                        target.range().start,
                        body_start + target.indexed_range().start
                    );
                    assert_eq!(target.range().end, body_start + target.indexed_range().end);
                    assert_ne!(target.range(), target.indexed_range());
                    assert_eq!(
                        index.nodes()[target.node().0 as usize].range,
                        target.indexed_range()
                    );
                    assert_eq!(target.name(), proof.item().def_key.declared_name);
                    assert!(target.explicit().is_empty());
                    let before = db.trace.events.len();
                    let returned = index.pc(NodeId(site.node), Phase::Operand(1)).unwrap();
                    assert!(std::ptr::eq(
                        target,
                        goal.indexed_local_source_name(index, &returned)
                            .unwrap()
                            .unwrap()
                    ));
                    assert_eq!(db.trace.events.len(), before);
                    let mut wrong = site.clone();
                    wrong.node += 1;
                    assert!(goal.indexed_local_source_name(index, &wrong).is_err());
                    wrong = site.clone();
                    wrong.phase = Phase::Commit.code().unwrap();
                    assert!(goal.indexed_local_source_name(index, &wrong).is_err());
                }
                no_child_code(&db, start);
                if mode >= 2 {
                    let retained = ready(prepare(&mut db, &sites[0]));
                    db.begin_expansion_epoch(2).unwrap();
                    assert!(retained.require_current(&db).is_err());
                }
            }
        }
    }
    #[test]
    fn module_binding_and_pending_are_retained_without_local_lowering_permission() {
        let text = format!(
            "{SUFFIX}\nfn outside(){{}} fn generic<T>(){{}} fn entered_call_control(){{outside();let f=outside;f();(outside)();generic::<u8>();}}\n"
        );
        let (mut db, plan) = world(&text, 2024, "entered_call_control");
        let sites = calls(&plan);
        assert_eq!(sites.len(), 4);
        let index = plan.context().source().index();
        let start = db.trace.events.len();
        let module = ready(prepare(&mut db, &sites[0]));
        assert!(
            matches!(module.selection(), EnteredCallSelection::Module(declaration)
            if declaration.owner().is_none())
        );
        let binding = ready(prepare(&mut db, &sites[1]));
        assert!(matches!(
            binding.selection(),
            EnteredCallSelection::Binding(_)
        ));
        for site in &sites[2..] {
            let pending = ready(prepare(&mut db, site));
            assert!(matches!(
                pending.selection(),
                EnteredCallSelection::Pending(ValueGoalDemand::PathAdjustment)
            ));
            assert!(
                pending
                    .local_source_name(&db, index, site)
                    .unwrap()
                    .is_none()
            );
            assert!(pending.local_callable(&mut db).is_err());
        }
        assert!(
            module
                .local_source_name(&db, index, &sites[0])
                .unwrap()
                .is_none()
        );
        assert!(
            binding
                .local_source_name(&db, index, &sites[1])
                .unwrap()
                .is_none()
        );
        assert!(module.local_callable(&mut db).is_err());
        assert!(binding.local_callable(&mut db).is_err());
        no_child_code(&db, start);
    }
    #[test]
    fn shared_callable_retains_an_explicit_parent_handle_without_repreparing_proof() {
        let (mut db, plan) = world(SUFFIX, 2024, "main");
        let site = calls(&plan).remove(0);
        let goal = ready(prepare(&mut db, &site));
        assert!(
            goal.local_callable(&mut db).is_err(),
            "unpublished body was granted"
        );
        db.begin_run().unwrap();
        let EnteredCallSelection::Local(proof) = goal.selection() else {
            panic!()
        };
        let parent_handles = Arc::strong_count(goal.retained_plan());
        let start = db.trace.events.len();
        db.publish_ordinary_local_function(proof).unwrap();
        goal.require_current(&db).unwrap();
        let callee = goal.local_callable(&mut db).unwrap();
        assert_eq!(Arc::strong_count(goal.retained_plan()), parent_handles + 1);
        let alias = callee.clone();
        assert_eq!(Arc::strong_count(goal.retained_plan()), parent_handles + 2);
        drop(alias);
        drop(callee);
        assert_eq!(Arc::strong_count(goal.retained_plan()), parent_handles);
        no_child_code(&db, start);
        db.finish_run().unwrap();
    }
}
