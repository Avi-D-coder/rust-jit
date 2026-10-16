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
}
impl OrdinaryLocalAdmission {
    pub(crate) fn parent(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.parent
    }
    pub(crate) fn item(&self) -> &ItemOutline {
        &self.parent.context().facts().items[self.item_index].item
    }
    pub(crate) fn scope(&self) -> u32 {
        self.parent.context().facts().items[self.item_index].scope
    }
    pub(crate) fn site(&self) -> &PcKey {
        &self.site
    }
    pub(crate) fn source(&self) -> &ParseKey {
        &self.source
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
    let demand = QueryKey::EnteredValue(PcKey {
        body: site.body.clone(),
        node: path.id.0,
        phase: 0,
    });
    let selected = polled!(db.fetch(&SyntaxProvider, demand.clone()));
    let QueryValue::EnteredValue(goal) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("ordinary callable Value goal differs")));
    };
    if goal.body() != &site.body
        || goal.node() != path.id
        || !parent.same_environment(goal.retained_plan())
    {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable selected source environment differs",
        )));
    }
    if let Some(pending) = goal.pending() {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Pending(pending)));
    }
    if let Some(binding) = goal.local_binding() {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Binding(binding)));
    }
    let Some(declaration) = goal.declaration() else {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable Value selection has no declaration",
        )));
    };
    let Some(owner) = declaration.owner() else {
        return QueryPoll::Ready(Ok(EnteredLocalCall::Module(declaration.clone())));
    };
    if owner != &site.body || declaration.source() != &source.parsed().key() {
        return QueryPoll::Ready(Err(fault(
            "ordinary callable actual local owner/source differs",
        )));
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
    if rows.next().is_some()
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
            .and_then(|bytes| bytes.checked_add(source.parsed().key().profile.semantics.capacity()))
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)
    );
    let source = source.parsed().key();
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
        },
    ))))
}
