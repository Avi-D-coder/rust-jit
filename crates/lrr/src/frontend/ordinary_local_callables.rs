//! Actual entered Value selection joined to the ordinary callable/header route.
//! This prepares metadata only. Reached coordinator publication and ordinary
//! call acceptance still precede executable body preparation and entry.
use super::{SourceCallable, fault};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        source_locals::PreparedSourceLocalPlan,
        source_value_goals::{SelectedValueDeclaration, ValueGoalDemand},
    },
    ir::pc::{NodeId, PcKey, Phase},
    source::manifest::ItemKind,
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        outline::{DeclarationContext, ItemOutline},
        parse::ParseKey,
        signatures::FunctionSignature,
    },
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

/// Missing source evidence is not permission to try the old lexical resolver.
#[derive(Debug)]
pub(crate) enum EnteredLocalCall {
    Local(Box<OrdinaryLocalAdmission>),
    Module(SelectedValueDeclaration),
    Binding(crate::core::digest::Digest),
    Pending(ValueGoalDemand),
}

/// Private construction requires the actual hygienic query39 observation and
/// its complete relevant input closure, including negative lookup dependencies.
/// The retained canonical item supplies a body locator, never body execution.
#[derive(Debug)]
pub(crate) struct OrdinaryLocalAdmission {
    parent: Arc<PreparedSourceLocalPlan>,
    item_index: usize,
    site: PcKey,
    source: ParseKey,
    signature: Box<FunctionSignature>,
    guard: Arc<PreparedInputGuard>,
    query_revision: u64,
    metadata_bytes: usize,
    /// The admission is not tied to a call in the declaring body: a member of
    /// a block-level impl or trait (E-R5-009), selected through the member
    /// index from any call site, or a block item of an enclosing body named
    /// from a nested one. `site` is then the call or the declaring body's
    /// entry, and the declaration's own parent plan owns the body context.
    /// A reached const/static initializer likewise uses its declaration's
    /// parent independently of the ValuePath that first demands it.
    member: bool,
}
impl OrdinaryLocalAdmission {
    pub(crate) fn is_member(&self) -> bool {
        self.member
    }
    pub(crate) fn parent(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.parent
    }
    pub(crate) fn item(&self) -> &ItemOutline {
        &self.row().item
    }
    pub(crate) fn scope(&self) -> u32 {
        self.row().scope
    }
    fn row(&self) -> &crate::syntax::body_items::BodyItem {
        let facts = self.parent.context().facts();
        facts
            .items
            .get(self.item_index)
            .or_else(|| facts.members.get(self.item_index - facts.items.len()))
            .expect("admitted local declaration row")
    }
    pub(crate) fn site(&self) -> &PcKey {
        &self.site
    }
    pub(crate) fn source(&self) -> &ParseKey {
        &self.source
    }
    pub(crate) fn signature(&self) -> &FunctionSignature {
        &self.signature
    }
    pub(crate) fn query_revision(&self) -> u64 {
        self.query_revision
    }
    pub(crate) fn guard(&self) -> &Arc<PreparedInputGuard> {
        &self.guard
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.metadata_bytes
    }
    pub(crate) fn derived_inputs(&self) -> [(InputKey, InputValue); 4] {
        let item = self.item();
        let id = item.def_key.fingerprint();
        [
            (
                InputKey::DefinitionSource(id),
                InputValue::DefinitionSource(self.source.clone()),
            ),
            (
                InputKey::DefinitionHeader(id),
                InputValue::DefinitionHeader(item.header.clone()),
            ),
            (
                InputKey::DeclarationContext(id),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: item.scope.clone(),
                    range: item.declaration_span,
                }),
            ),
            (
                InputKey::DefinitionBody(id),
                InputValue::DefinitionBody(
                    item.body
                        .as_ref()
                        .expect("private reached function body")
                        .clone(),
                ),
            ),
        ]
    }
    pub(crate) fn revalidate(&self, db: &Database) -> Result<(), Diagnostic> {
        // Guard creation happened once over the genuine query's memo DAG.
        // This check performs no query, parsing, selection or namespace census.
        self.guard.require_current(db)
    }
    /// The parent coordinator must first atomically publish the exact derived
    /// rows under active-run/currentness/quota checks. This method cannot mutate
    /// source or manufacture the missing executable locator itself.
    pub(crate) fn callable(self, db: &mut Database) -> Result<Box<SourceCallable>, Diagnostic> {
        Arc::new(self).callable_shared(db)
    }
    pub(crate) fn callable_shared(
        self: &Arc<Self>,
        db: &mut Database,
    ) -> Result<Box<SourceCallable>, Diagnostic> {
        self.revalidate(db)?;
        for (key, expected) in self.derived_inputs() {
            let actual = db
                .input(&key)
                .ok_or_else(|| fault("ordinary local callable publication absent"))?;
            let same = match (actual.as_ref(), &expected) {
                (InputValue::DefinitionSource(a), InputValue::DefinitionSource(b)) => a == b,
                (InputValue::DefinitionHeader(a), InputValue::DefinitionHeader(b)) => a == b,
                (InputValue::DeclarationContext(a), InputValue::DeclarationContext(b)) => a == b,
                (InputValue::DefinitionBody(a), InputValue::DefinitionBody(b)) => a == b,
                _ => false,
            };
            if !same
                || db
                    .published_input(&key)
                    .is_none_or(|(_, stamp)| stamp != expected.fingerprint())
            {
                return Err(fault("ordinary local callable publication differs"));
            }
        }
        let item = self.item();
        let callee = SourceCallable {
            definition: item.def_key.clone(),
            identity: item.def_key.fingerprint(),
            signature: self.signature.clone(),
            body: item.body.clone(),
            source: self.source.clone(),
            revision: db.source_revision(),
            context: DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            },
            lexical: None,
            ordinary_local: Some(Arc::clone(self)),
            ordinary_module: None,
            ordinary_owner: Some(Arc::clone(self.parent())),
            retained: None,
        };
        callee.revalidate(db)?;
        Ok(Box::new(callee))
    }
}

/// Reached Call entry only; source constraints/signature queries do not invoke
/// this constructor. All source origins consume the same entered Value proof.
pub(crate) fn prepare_entered_local_function(
    db: &mut Database,
    site: &PcKey,
) -> QueryPoll<EnteredLocalCall> {
    let value = polled!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceLocalPlan(site.body.clone())
    ));
    let QueryValue::SourceLocalPlan(parent) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("ordinary local callable owning plan differs")));
    };
    let parent = Arc::clone(parent);
    polled!(parent.require_current(db));
    let source = parent.context().source();
    let index = source.index();
    let Some(call) = index.nodes().get(site.node as usize) else {
        return QueryPoll::Ready(Err(fault("ordinary local call node absent")));
    };
    if site.phase != checked!(Phase::Entry.code())
        || index.body() != &site.body
        || call.id != NodeId(site.node)
        || call.role != NodeRole::Expression(ExpressionKind::Call)
    {
        return QueryPoll::Ready(Err(fault(
            "ordinary local callable requires exact call entry",
        )));
    }
    let Some(path) = call
        .operands
        .first()
        .and_then(|id| index.nodes().get(id.0 as usize))
    else {
        return QueryPoll::Ready(Err(fault("ordinary direct call operand absent")));
    };
    if path.parent != Some(call.id) || path.role != NodeRole::Expression(ExpressionKind::Path) {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Pending(
            ValueGoalDemand::PathAdjustment,
        )));
    }
    let path = path.id;
    admit_entered_local(db, parent, site, path)
}

/// A function item value (`let f = local_fn;`): the same entered Value
/// selection and canonical admission as a direct call of that path, at the
/// value path's own site. Calling the value later consumes this proof
/// exactly as a direct call consumes its call-site proof.
pub(crate) fn prepare_entered_local_function_value(
    db: &mut Database,
    site: &PcKey,
) -> QueryPoll<EnteredLocalCall> {
    let value = polled!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceLocalPlan(site.body.clone())
    ));
    let QueryValue::SourceLocalPlan(parent) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("ordinary local callable owning plan differs")));
    };
    let parent = Arc::clone(parent);
    polled!(parent.require_current(db));
    let index = parent.context().source().index();
    let Some(path) = index.nodes().get(site.node as usize) else {
        return QueryPoll::Ready(Err(fault("ordinary local value path node absent")));
    };
    if site.phase != checked!(Phase::Entry.code())
        || index.body() != &site.body
        || path.id != NodeId(site.node)
        || path.role != NodeRole::Expression(ExpressionKind::Path)
    {
        return QueryPoll::Ready(Err(fault(
            "ordinary local function value requires its exact path entry",
        )));
    }
    let path = path.id;
    admit_entered_local(db, parent, site, path)
}

fn admit_entered_local(
    db: &mut Database,
    parent: Arc<PreparedSourceLocalPlan>,
    site: &PcKey,
    path: NodeId,
) -> QueryPoll<EnteredLocalCall> {
    let source_key = parent.context().source().parsed().key();
    let demand = QueryKey::EnteredValue(PcKey {
        body: site.body.clone(),
        node: path.0,
        phase: 0,
    });
    let selected = polled!(db.fetch(&SyntaxProvider, demand.clone()));
    let QueryValue::EnteredValue(goal) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("ordinary callable Value goal differs")));
    };
    if goal.body() != &site.body
        || goal.node() != path
        || !parent.same_environment(goal.retained_plan())
    {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable selected source environment differs",
        )));
    }
    if let Some(pending) = goal.pending() {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Pending(pending)));
    }
    // A qualified callee (a module path, a `Type::item` associated function
    // or a closed library facade function) keeps the dispatcher's own
    // written-path route, which selects the same definition.
    if goal.qualified() {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Pending(
            ValueGoalDemand::PathAdjustment,
        )));
    }
    if let Some(binding) = goal.local_binding() {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Binding(binding)));
    }
    // A closed library facade function has no source declaration: the
    // dispatcher selects it by its definition identity. A `Root::member`
    // path selects the member against its lexically selected root there too.
    if goal.facade().is_some() || goal.rooted().is_some() {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Pending(
            ValueGoalDemand::PathAdjustment,
        )));
    }
    let Some(declaration) = goal.declaration() else {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable Value selection has no declaration",
        )));
    };
    let Some(owner) = declaration.owner() else {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Module(declaration.clone())));
    };
    if declaration.source() != &source_key {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable actual local owner/source differs",
        )));
    }
    // A declaration of an enclosing body (a block item a nested body names):
    // admitted against its declaring body's plan, from this call site.
    let mut detached = false;
    let mut parent = parent;
    if owner != &site.body {
        let enclosing =
            polled!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(owner.clone())));
        let QueryValue::SourceLocalPlan(declaring) = enclosing.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("ordinary callable declaring plan differs")));
        };
        parent = Arc::clone(declaring);
        polled!(parent.require_current(db));
        detached = true;
    }
    let mut rows = parent
        .context()
        .facts()
        .items
        .iter()
        .enumerate()
        .filter(|(_, row)| row.item.def_key.fingerprint() == declaration.identity());
    let Some((item_index, row)) = rows.next() else {
        return QueryPoll::Ready(Err(fault("ordinary callable canonical declaration absent")));
    };
    let unique = rows.next().is_none();
    // A Value item that is no function (a tuple struct's constructor) is no
    // source callable: the dispatcher binds it as a constructor.
    if unique && row.attribute_input.is_none() && row.item.def_key.item_kind == ItemKind::Struct {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Pending(
            ValueGoalDemand::BindingClassification(declaration.identity()),
        )));
    }
    if !unique
        || row.attribute_input.is_some()
        || row.item.def_key.item_kind != ItemKind::Function
        || row.item.scope != declaration.context().scope
        || row.item.declaration_span != declaration.context().range
        || row.item.header.fingerprint() != declaration.header()
    {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable canonical declaration differs",
        )));
    }
    checked!(
        row.item
            .body
            .as_ref()
            .ok_or_else(|| fault("ordinary callable body locator absent"))
            .and_then(|body| body.require_entry())
    );
    let signature = polled!(db.fetch(
        &SyntaxProvider,
        QueryKey::FunctionSignature(declaration.identity())
    ));
    let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("ordinary callable signature variant differs")));
    };
    if signature.definition != declaration.identity()
        || signature.scope != row.item.scope
        || signature.header.fingerprint() != row.item.header.fingerprint()
    {
        return QueryPoll::Ready(Err(fault("ordinary callable actual signature differs")));
    }
    if !selected.cacheable {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable selection dependencies are unretained",
        )));
    }
    let guard = Arc::new(polled!(db.prepare_input_guard(
        &SyntaxProvider,
        &demand,
        selected.fingerprint
    )));
    let mut signature = signature.clone();
    signature.header = row.item.header.clone();
    let metadata_bytes = checked!(
        std::mem::size_of::<OrdinaryLocalAdmission>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|bytes| bytes.checked_add(signature.estimated_bytes()))
            .and_then(|bytes| bytes.checked_add(guard.metadata_bytes()))
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .and_then(|bytes| bytes.checked_add(source_key.profile.semantics.capacity()))
            .ok_or_else(crate::runtime::arena::resource)
    );
    let source = source_key;
    QueryPoll::Ready(Ok(EnteredLocalCall::Local(Box::new(
        OrdinaryLocalAdmission {
            parent,
            item_index,
            site: site.clone(),
            source,
            signature,
            guard,
            query_revision: db.revision(),
            metadata_bytes,
            member: detached,
        },
    ))))
}

/// The member of a block-level impl or trait that the member index selected,
/// admitted from its declaration alone: the same signature fact, guard and
/// callable a block-level `fn` yields, bound to the member row of the owning
/// body's plan instead of an entered Value goal.
pub(crate) fn admit_container_member(
    db: &mut Database,
    definition: crate::core::digest::Digest,
) -> QueryPoll<Option<Arc<OrdinaryLocalAdmission>>> {
    admit_declaration_body(db, definition, false)
}

/// Reached const/static initialization uses the same selected declaration,
/// signature, source guard and ordinary callable as a container member.
pub(crate) fn admit_initializer(
    db: &mut Database,
    definition: crate::core::digest::Digest,
) -> QueryPoll<Option<Arc<OrdinaryLocalAdmission>>> {
    admit_declaration_body(db, definition, true)
}

fn admit_declaration_body(
    db: &mut Database,
    definition: crate::core::digest::Digest,
    initializer: bool,
) -> QueryPoll<Option<Arc<OrdinaryLocalAdmission>>> {
    let Some(owner) = db.input(&InputKey::LocalDeclarationOwner(definition)) else {
        return QueryPoll::Ready(Ok(None));
    };
    let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
        return QueryPoll::Ready(Err(fault("local member owner input differs")));
    };
    let plan = polled!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(owner.clone())));
    let QueryValue::SourceLocalPlan(parent) = plan.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("local member owning plan differs")));
    };
    let parent = Arc::clone(parent);
    polled!(parent.require_current(db));
    let facts = parent.context().facts();
    let Some((item_index, row)) = facts
        .items
        .iter()
        .chain(&facts.members)
        .enumerate()
        .find(|(_, row)| row.item.def_key.fingerprint() == definition)
    else {
        return QueryPoll::Ready(Ok(None));
    };
    let eligible = if initializer {
        matches!(
            row.item.def_key.item_kind,
            ItemKind::Constant | ItemKind::Static
        )
    } else {
        item_index >= facts.items.len()
            && (row.item.def_key.item_kind == ItemKind::Function
                || row.item.is_discriminant_constant())
    };
    if row.attribute_input.is_some() || !eligible || row.item.body.is_none() {
        return QueryPoll::Ready(Ok(None));
    }
    let selected = polled!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
    let QueryValue::FunctionSignature(signature) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("local member signature variant differs")));
    };
    if signature.definition != definition
        || signature.scope != row.item.scope
        || signature.header.fingerprint() != row.item.header.fingerprint()
    {
        return QueryPoll::Ready(Err(fault("local member actual signature differs")));
    }
    let demand = QueryKey::FunctionSignature(definition);
    let guard = Arc::new(polled!(db.prepare_input_guard(
        &SyntaxProvider,
        &demand,
        selected.fingerprint
    )));
    let mut signature = signature.clone();
    signature.header = row.item.header.clone();
    let source = parent.context().source().parsed().key();
    let site = checked!(
        parent
            .context()
            .source()
            .index()
            .pc(NodeId(0), Phase::Entry)
    );
    let metadata_bytes = std::mem::size_of::<OrdinaryLocalAdmission>()
        .saturating_add(signature.estimated_bytes())
        .saturating_add(guard.metadata_bytes());
    QueryPoll::Ready(Ok(Some(Arc::new(OrdinaryLocalAdmission {
        parent,
        item_index,
        site,
        source,
        signature,
        guard,
        query_revision: db.revision(),
        metadata_bytes,
        member: true,
    }))))
}
