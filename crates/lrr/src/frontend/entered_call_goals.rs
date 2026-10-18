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
            EnteredLocalCall,
            entered_module::{
                EnteredModuleAdmission, ModuleFunctionPreparation, prepare_entered_module_function,
            },
            ordinary_local::OrdinaryLocalAdmission,
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
        .ok_or_else(crate::runtime::arena::resource)
}

/// Syntactic argument kind only, never a callee generic ordinal or Type grant.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum EnteredGenericArgumentKind {
    Type,
    Const,
    Lifetime,
    AssociatedType,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum EnteredTypeArgumentDemand {
    TypeSyntax,
    IndexedSyntax,
}
/// Canonical Syntax location of the actual TypeArg.ty node range. The same
/// first Syntax node at this range is used by PreparedBodyTypeOccurrences;
/// several wrapper AST nodes may share that range. No Type is resolved here.
#[derive(Debug)]
pub(crate) struct EnteredTypeArgumentSite {
    key: PcKey,
    range: SourceRange,
    indexed_range: SourceRange,
    path: bool,
}
impl EnteredTypeArgumentSite {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn indexed_range(&self) -> SourceRange {
        self.indexed_range
    }
    pub(crate) fn is_path(&self) -> bool {
        self.path
    }
}
#[derive(Debug)]
pub(crate) enum EnteredTypeArgument {
    Indexed(EnteredTypeArgumentSite),
    Pending {
        range: Option<SourceRange>,
        demand: EnteredTypeArgumentDemand,
    },
}
/// An original AST observation in the actual owning source (which may itself
/// be generated/retained). Diagnostic token origin does not grant provenance.
#[derive(Debug)]
pub(crate) struct EnteredGenericArgument {
    kind: EnteredGenericArgumentKind,
    range: SourceRange,
    ty: Option<EnteredTypeArgument>,
}
impl EnteredGenericArgument {
    pub(crate) fn kind(&self) -> EnteredGenericArgumentKind {
        self.kind
    }
    pub(crate) fn range(&self) -> SourceRange {
        self.range
    }
    pub(crate) fn type_argument(&self) -> Option<&EnteredTypeArgument> {
        self.ty.as_ref()
    }
}
impl StableEncode for EnteredGenericArgument {
    const DOMAIN: &'static str = "lrr/v1/entered-written-generic-argument";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u8(match self.kind {
            EnteredGenericArgumentKind::Type => 0,
            EnteredGenericArgumentKind::Const => 1,
            EnteredGenericArgumentKind::Lifetime => 2,
            EnteredGenericArgumentKind::AssociatedType => 3,
        });
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.option(&self.ty, |e, ty| match ty {
            EnteredTypeArgument::Indexed(site) => {
                e.u8(0);
                e.record(&site.key);
                e.u32(site.range.start);
                e.u32(site.range.end);
                e.u32(site.indexed_range.start);
                e.u32(site.indexed_range.end);
                e.u8(u8::from(site.path));
            }
            EnteredTypeArgument::Pending { range, demand } => {
                e.u8(1);
                e.option(range, |e, range| {
                    e.u32(range.start);
                    e.u32(range.end);
                });
                e.u8(match demand {
                    EnteredTypeArgumentDemand::TypeSyntax => 0,
                    EnteredTypeArgumentDemand::IndexedSyntax => 1,
                });
            }
        });
    }
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
    arguments: Vec<EnteredGenericArgument>,
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
    pub(crate) fn arguments(&self) -> &[EnteredGenericArgument] {
        &self.arguments
    }
    fn heap_bytes(&self) -> Result<usize, Diagnostic> {
        let bytes = self.explicit.iter().try_fold(
            add(
                self.name.capacity(),
                self.explicit
                    .capacity()
                    .checked_mul(std::mem::size_of::<String>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )?,
            |bytes, argument| add(bytes, argument.capacity()),
        )?;
        add(
            bytes,
            self.arguments
                .capacity()
                .checked_mul(std::mem::size_of::<EnteredGenericArgument>())
                .ok_or_else(crate::runtime::arena::resource)?,
        )
    }
}
impl StableEncode for EnteredSourceName {
    const DOMAIN: &'static str = "lrr/v3/entered-call-written-source-name";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.u32(self.node.0);
        e.u32(self.range.start);
        e.u32(self.range.end);
        e.u32(self.indexed_range.start);
        e.u32(self.indexed_range.end);
        e.string(&self.name);
        e.sequence(&self.explicit, |e, argument| e.string(argument));
        e.sequence(&self.arguments, |e, argument| e.record(argument));
    }
}
enum Selection {
    Local(Arc<OrdinaryLocalAdmission>),
    Module(Arc<EnteredModuleAdmission>),
    Binding {
        plan: Arc<PreparedSourceLocalPlan>,
        identity: Digest,
    },
    Value {
        plan: Arc<PreparedSourceLocalPlan>,
        identity: Digest,
        syntax_guard: Arc<PreparedInputGuard>,
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
    Value(Digest),
    Pending(ValueGoalDemand),
}
/// Retained genuine selection for installed ordinary calls. The admission Arc
/// shares its existing parent handle with Q44; this target adds no parent Arc.
/// Q47 separately retains one visible parent handle for source-root accounting.
#[derive(Debug)]
pub(crate) struct EnteredCallTarget {
    admission: TargetAdmission,
    identity: Digest,
    name: String,
    explicit: Vec<String>,
    bytes: usize,
}
#[derive(Debug)]
enum TargetAdmission {
    Local(Arc<OrdinaryLocalAdmission>),
    Module(Arc<EnteredModuleAdmission>),
}
impl EnteredCallTarget {
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn require_written(&self, name: &str, syntax: &[String]) -> Result<(), Diagnostic> {
        if self.name != name || self.explicit != syntax {
            return Err(fault("retained call written target differs"));
        }
        Ok(())
    }
    /// Reached coordinator only; pure preparation never invokes this method.
    /// No Value/Type goal refetch, name resolution or executable body lowering.
    pub(super) fn callable(
        &self,
        db: &mut Database,
    ) -> Result<Box<crate::frontend::callables::SourceCallable>, Diagnostic> {
        match &self.admission {
            TargetAdmission::Local(proof) => {
                proof.revalidate(db)?;
                db.publish_ordinary_local_function(proof)?;
                proof.callable_shared(db)
            }
            TargetAdmission::Module(proof) => {
                proof.revalidate(db)?;
                proof.callable_shared(db)
            }
        }
    }
}
impl StableEncode for EnteredCallTarget {
    const DOMAIN: &'static str = "lrr/v1/retained-entered-call-target";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.u8(match &self.admission {
            TargetAdmission::Local(_) => 0,
            TargetAdmission::Module(_) => 1,
        });
        e.string(&self.name);
        e.sequence(&self.explicit, |e, value| e.string(value));
    }
}
fn retained_string(value: &str) -> Result<String, Diagnostic> {
    add(0, value.len())?;
    let mut result = String::new();
    result
        .try_reserve_exact(value.len())
        .map_err(|_| crate::runtime::arena::resource())?;
    result.push_str(value);
    Ok(result)
}
/// Not Clone: the single physical parent handle is in a Local/Module proof
/// or the Binding/Pending variant. SourceArtifactRef::LocalPlan must
/// account that actual handle, not a hidden duplicate or extracted proof Arc.
pub struct EnteredCallGoal {
    key: PcKey,
    selection: Selection,
    /// The plan of the call's own body when the selected declaration belongs to
    /// an enclosing body (a block item named from a nested function): the
    /// admission's parent is then that declaring plan, while the call's index,
    /// source and structure identities are the site's. Absent otherwise.
    site: Option<Arc<PreparedSourceLocalPlan>>,
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
    /// Cold capture only after the owning indexed call has been proved. The
    /// private variant preserves selection; written strings are comparisons,
    /// never a source-name resolution or Type authority.
    pub(crate) fn retained_target(&self) -> Result<Arc<EnteredCallTarget>, Diagnostic> {
        let (admission, proof_bytes) = match &self.selection {
            Selection::Local(proof) => (
                TargetAdmission::Local(Arc::clone(proof)),
                proof.metadata_bytes(),
            ),
            Selection::Module(proof) => (
                TargetAdmission::Module(Arc::clone(proof)),
                proof.metadata_bytes(),
            ),
            Selection::Binding { .. } | Selection::Value { .. } | Selection::Pending { .. } => {
                return Err(fault(
                    "retained call target requires an admitted declaration",
                ));
            }
        };
        let written = self
            .target
            .as_ref()
            .ok_or_else(|| fault("retained call target source absent"))?;
        let mut bytes = add(std::mem::size_of::<EnteredCallTarget>(), ARC_HEADER)?;
        bytes = add(bytes, proof_bytes)?;
        let vector_bytes = written
            .explicit
            .len()
            .checked_mul(std::mem::size_of::<String>())
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut preflight = add(add(bytes, written.name.len())?, vector_bytes)?;
        for value in &written.explicit {
            preflight = add(preflight, value.len())?;
        }
        add(0, preflight)?;
        let name = retained_string(&written.name)?;
        bytes = add(bytes, name.capacity())?;
        let mut explicit = Vec::new();
        explicit
            .try_reserve_exact(written.explicit.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        bytes = add(
            bytes,
            explicit
                .capacity()
                .checked_mul(std::mem::size_of::<String>())
                .ok_or_else(crate::runtime::arena::resource)?,
        )?;
        for value in &written.explicit {
            // Account existing allocations and the next allocation before copy.
            add(bytes, value.len())?;
            let value = retained_string(value)?;
            bytes = add(bytes, value.capacity())?;
            explicit.push(value);
        }
        Ok(Arc::new(EnteredCallTarget {
            admission,
            identity: self.fingerprint,
            name,
            explicit,
            bytes,
        }))
    }
    /// Actual Q39 selection closure only. Module canonical/signature/body
    /// authority additionally requires require_current/callable admission.
    pub(crate) fn guard(&self) -> &Arc<PreparedInputGuard> {
        &self.guard
    }
    pub(crate) fn value_identity(&self) -> Option<Digest> {
        self.value
    }
    /// Retained written metadata only. Module/Binding/Pending facts do not gain
    /// lowering, Type/const argument, callable or entry authority through this.
    /// In particular a body-local Type shadowing a primitive spelling still
    /// needs its actual occurrence proof, never module string substitution.
    pub(crate) fn written_source_name(&self) -> Option<&EnteredSourceName> {
        self.target.as_ref()
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        if let Some(site) = &self.site {
            return site;
        }
        match &self.selection {
            Selection::Local(proof) => proof.parent(),
            Selection::Module(proof) => proof.parent(),
            Selection::Binding { plan, .. }
            | Selection::Value { plan, .. }
            | Selection::Pending { plan, .. } => plan,
        }
    }
    pub(crate) fn selection(&self) -> EnteredCallSelection<'_> {
        match &self.selection {
            Selection::Local(proof) => EnteredCallSelection::Local(proof),
            Selection::Module(proof) => EnteredCallSelection::Module(proof.declaration()),
            Selection::Binding { identity, .. } => EnteredCallSelection::Binding(*identity),
            Selection::Value { identity, .. } => EnteredCallSelection::Value(*identity),
            Selection::Pending { demand, .. } => EnteredCallSelection::Pending(*demand),
        }
    }
    /// Declaration-only facts for caller Type arguments. No body locator
    /// publication, SourceCallable construction or executable preparation.
    pub(crate) fn signature(&self) -> Option<&crate::syntax::signatures::FunctionSignature> {
        match &self.selection {
            Selection::Local(proof) => Some(proof.signature()),
            Selection::Module(proof) => Some(proof.signature()),
            Selection::Binding { .. } | Selection::Value { .. } | Selection::Pending { .. } => None,
        }
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)?;
        if let Selection::Module(proof) = &self.selection {
            proof.revalidate(db)?;
        }
        if let Selection::Value { syntax_guard, .. } = &self.selection {
            syntax_guard.require_current(db)?;
        }
        Ok(())
    }
    pub(crate) fn is_value_callee(&self) -> bool {
        matches!(
            self.selection,
            Selection::Binding { .. } | Selection::Value { .. }
        )
    }
    /// The exact owning call proof, without granting source-function authority.
    pub(crate) fn require_value_call_site(
        &self,
        index: &BodyIndex,
        phase: &PcKey,
    ) -> Result<(), Diagnostic> {
        if !self.is_value_callee()
            || self.key.phase != 0
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
            || index.pc(NodeId(phase.node), Phase::from_code(phase.phase))? != *phase
        {
            return Err(fault("value call source/body/site differs"));
        }
        let call = index
            .nodes()
            .get(phase.node as usize)
            .ok_or_else(|| fault("value call indexed node absent"))?;
        if call.id != NodeId(self.key.node)
            || call.range != self.call_range
            || call.role != NodeRole::Expression(ExpressionKind::Call)
        {
            return Err(fault("value call actual indexed occurrence differs"));
        }
        Ok(())
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
    /// Reached coordinator projection only. The Q39 guard proves selection;
    /// the module admission separately checks actual canonical/signature/body
    /// authority before constructing the SAME SourceCallable as other calls.
    pub(crate) fn module_callable(
        &self,
        db: &mut Database,
    ) -> Result<Box<crate::frontend::callables::SourceCallable>, Diagnostic> {
        self.require_current(db)?;
        match &self.selection {
            Selection::Module(proof) => proof.callable_shared(db),
            _ => Err(fault(
                "entered call is not an admitted module source function",
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
        if proof.site() != &self.key {
            return Err(fault("entered local admission site differs"));
        }
        self.indexed_source_name(index, phase)
    }
    /// One actual ordinary call/path proof for admitted Local and Module
    /// declarations. Metadata never upgrades a Binding or Pending selection.
    pub(crate) fn indexed_source_name(
        &self,
        index: &BodyIndex,
        phase: &PcKey,
    ) -> Result<Option<&EnteredSourceName>, Diagnostic> {
        if matches!(
            &self.selection,
            Selection::Binding { .. } | Selection::Value { .. } | Selection::Pending { .. }
        ) {
            return Ok(None);
        }
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
            // Admission metadata includes its Arc header and selection guard;
            // self.guard aliases that allocation, never another parent handle.
            Selection::Module(proof) => bytes = add(bytes, proof.metadata_bytes())?,
            Selection::Binding { .. } | Selection::Value { .. } | Selection::Pending { .. } => {
                bytes = add(bytes, add(self.guard.metadata_bytes(), ARC_HEADER)?)?;
                if let Selection::Value { syntax_guard, .. } = &self.selection {
                    bytes = add(bytes, add(syntax_guard.metadata_bytes(), ARC_HEADER)?)?;
                }
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
    const DOMAIN: &'static str = "lrr/v5/entered-ordinary-source-call-goal";
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
            Selection::Module(proof) => {
                e.u8(1);
                e.record(proof.as_ref());
            }
            Selection::Binding { identity, .. } => {
                e.u8(2);
                e.digest(*identity);
            }
            Selection::Value {
                identity,
                syntax_guard,
                ..
            } => {
                e.u8(4);
                e.digest(*identity);
                e.digest(syntax_guard.fingerprint());
            }
            Selection::Pending { demand, .. } => {
                e.u8(3);
                encode_demand(*demand, e);
            }
        }
    }
}

pub(super) fn written_argument(
    plan: &PreparedSourceLocalPlan,
    argument: ast::GenericArg,
) -> Result<EnteredGenericArgument, Diagnostic> {
    let source = plan.context().source();
    let argument_range = range(argument.syntax().text_range());
    if !source.body().range.contains(argument_range) {
        return Err(fault("written generic argument escapes actual body"));
    }
    let (kind, ty) = match argument {
        ast::GenericArg::TypeArg(argument) => {
            let observation = match argument.ty() {
                None => EnteredTypeArgument::Pending {
                    range: None,
                    demand: EnteredTypeArgumentDemand::TypeSyntax,
                },
                Some(ty) => {
                    let actual = range(ty.syntax().text_range());
                    if !argument_range.contains(actual) || !source.body().range.contains(actual) {
                        return Err(fault("written Type argument escapes actual argument/body"));
                    }
                    let relative = SourceRange::checked(
                        actual
                            .start
                            .checked_sub(source.body().range.start)
                            .ok_or_else(crate::runtime::arena::resource)?,
                        actual
                            .end
                            .checked_sub(source.body().range.start)
                            .ok_or_else(crate::runtime::arena::resource)?,
                    )?;
                    let canonical = source
                        .index()
                        .nodes_at_range(relative)
                        .find(|node| node.role == NodeRole::Syntax);
                    match canonical {
                        None => EnteredTypeArgument::Pending {
                            range: Some(actual),
                            demand: EnteredTypeArgumentDemand::IndexedSyntax,
                        },
                        Some(node) => {
                            let projected = SourceRange::checked(
                                source
                                    .body()
                                    .range
                                    .start
                                    .checked_add(node.range.start)
                                    .ok_or_else(crate::runtime::arena::resource)?,
                                source
                                    .body()
                                    .range
                                    .start
                                    .checked_add(node.range.end)
                                    .ok_or_else(crate::runtime::arena::resource)?,
                            )?;
                            if projected != actual || node.range != relative {
                                return Err(fault(
                                    "written Type argument indexed projection differs",
                                ));
                            }
                            EnteredTypeArgument::Indexed(EnteredTypeArgumentSite {
                                key: source.index().pc(node.id, Phase::Entry)?,
                                range: actual,
                                indexed_range: relative,
                                path: matches!(ty, ast::Type::PathType(_)),
                            })
                        }
                    }
                }
            };
            (EnteredGenericArgumentKind::Type, Some(observation))
        }
        ast::GenericArg::ConstArg(_) => (EnteredGenericArgumentKind::Const, None),
        ast::GenericArg::LifetimeArg(_) => (EnteredGenericArgumentKind::Lifetime, None),
        ast::GenericArg::AssocTypeArg(_) => (EnteredGenericArgumentKind::AssociatedType, None),
    };
    Ok(EnteredGenericArgument {
        kind,
        range: argument_range,
        ty,
    })
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
    let mut bytes = add(
        name.text().len(),
        count
            .checked_mul(std::mem::size_of::<String>())
            .ok_or_else(crate::runtime::arena::resource)?,
    )?;
    bytes = add(
        bytes,
        count
            .checked_mul(std::mem::size_of::<EnteredGenericArgument>())
            .ok_or_else(crate::runtime::arena::resource)?,
    )?;
    let mut arguments = Vec::new();
    arguments
        .try_reserve_exact(count)
        .map_err(|_| crate::runtime::arena::resource())?;
    let mut explicit = Vec::new();
    explicit
        .try_reserve_exact(count)
        .map_err(|_| crate::runtime::arena::resource())?;
    if let Some(list) = segment.generic_arg_list() {
        for argument in list.generic_args() {
            bytes = add(bytes, usize::from(argument.syntax().text_range().len()))?;
            explicit.push(argument.syntax().text().to_string());
            arguments.push(written_argument(plan, argument)?);
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
        arguments,
    };
    target.heap_bytes()?;
    Ok(Some(target))
}

/// Future pure memo provider. No source capture/publication, body entry, child
/// BodyStructure, executable argument preparation or normal runtime work occurs.
/// An actual const/static declaration is a value operand, not a source Fn.
/// Its initializer remains cold until the ordinary callee expression executes.
fn item_value_syntax_guard(
    db: &mut Database,
    declaration: &SelectedValueDeclaration,
) -> QueryPoll<Option<Arc<PreparedInputGuard>>> {
    if declaration.variant().is_some() {
        return QueryPoll::Ready(Ok(None));
    }
    let key = QueryKey::ParseFile(declaration.source().clone());
    let parsed = ready!(db.fetch(&SyntaxProvider, key.clone()));
    let QueryValue::ParseFile(syntax) = parsed.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("value callee declaration parse differs")));
    };
    let node = checked!(syntax.syntax_at(declaration.context().range, |node| {
        ast::Item::can_cast(node.kind())
    }));
    let Some(item) = node.and_then(ast::Item::cast) else {
        return QueryPoll::Ready(Err(fault("value callee actual declaration absent")));
    };
    if range(item.syntax().text_range()) != declaration.context().range {
        return QueryPoll::Ready(Err(fault("value callee actual declaration range differs")));
    }
    if !matches!(item, ast::Item::Const(_) | ast::Item::Static(_)) {
        return QueryPoll::Ready(Ok(None));
    }
    QueryPoll::Ready(Ok(Some(Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &key,
        parsed.fingerprint
    ))))))
}

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
    // Const/static callees (including qualified paths) join the existing value
    // call plan. The selected Value and its real syntax are both retained;
    // neither this query nor the kind observation evaluates the initializer.
    if let Some(path) = path {
        let demand = QueryKey::EnteredValue(PcKey {
            body: key.body.clone(),
            node: path.0,
            phase: 0,
        });
        let selected = ready!(db.fetch(&SyntaxProvider, demand.clone()));
        let QueryValue::EnteredValue(goal) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("item value call selection differs")));
        };
        if !selected.cacheable
            || goal.body() != &key.body
            || goal.node() != path
            || !parent.same_environment(goal.retained_plan())
        {
            return QueryPoll::Ready(Err(fault("item value call owner differs")));
        }
        if goal.pending().is_none()
            && let Some(declaration) = goal.declaration()
            && let Some(syntax_guard) = ready!(item_value_syntax_guard(db, declaration))
        {
            let identity = declaration.identity();
            let guard = Arc::new(ready!(db.prepare_input_guard(
                &SyntaxProvider,
                &demand,
                selected.fingerprint
            )));
            let target = checked!(written_target(&parent, path));
            let call_range = call.range;
            let structure = index.semantic_identity();
            let source = parent.context().source().identity();
            return QueryPoll::Ready(
                EnteredCallGoal {
                    key: key.clone(),
                    selection: Selection::Value {
                        plan: parent,
                        identity,
                        syntax_guard,
                    },
                    site: None,
                    target,
                    call_range,
                    structure,
                    source,
                    value: Some(selected.fingerprint),
                    guard,
                    fingerprint: Digest([0; 32]),
                    bytes: 0,
                }
                .finish(),
            );
        }
    }
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
            if proof.site() != key
                || (!proof.is_member() && !parent.same_environment(proof.parent()))
            {
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
    if matches!(
        selected,
        EnteredLocalCall::Local(_) | EnteredLocalCall::Module(_)
    ) && target.is_none()
    {
        return QueryPoll::Ready(Err(fault("entered source call exact written name absent")));
    }
    let call_range = call.range;
    let structure = index.semantic_identity();
    let source = parent.context().source().identity();
    let mut site = None;
    let selection = match selected {
        EnteredLocalCall::Local(proof) => {
            if proof.is_member() {
                site = Some(Arc::clone(&parent));
            }
            Selection::Local(Arc::from(proof))
        }
        EnteredLocalCall::Module(declaration) => {
            match ready!(prepare_entered_module_function(
                db,
                Arc::clone(&parent),
                &declaration,
                Arc::clone(&guard)
            )) {
                ModuleFunctionPreparation::Ready(proof) => {
                    if !Arc::ptr_eq(proof.parent(), &parent)
                        || !Arc::ptr_eq(proof.selection_guard(), &guard)
                        || proof.declaration().identity() != declaration.identity()
                    {
                        return QueryPoll::Ready(Err(fault(
                            "entered module admission owner/selection differs",
                        )));
                    }
                    checked!(proof.revalidate(db));
                    Selection::Module(proof)
                }
                ModuleFunctionPreparation::Pending(demand) => Selection::Pending {
                    plan: parent,
                    demand,
                },
            }
        }
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
            site,
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
        for site in &sites[2..3] {
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
            assert!(pending.module_callable(&mut db).is_err());
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
        assert!(binding.module_callable(&mut db).is_err());
        let Selection::Module(proof) = &module.selection else {
            panic!()
        };
        assert!(Arc::ptr_eq(module.retained_plan(), proof.parent()));
        assert!(Arc::ptr_eq(module.guard(), proof.selection_guard()));
        let parent_handles = Arc::strong_count(module.retained_plan());
        let before_projection = db.trace.events.len();
        let callable = module.module_callable(&mut db).unwrap();
        assert_eq!(callable.identity(), proof.declaration().identity());
        assert_eq!(
            Arc::strong_count(module.retained_plan()),
            parent_handles + 1
        );
        assert_eq!(db.trace.events.len(), before_projection);
        drop(callable);
        assert_eq!(Arc::strong_count(module.retained_plan()), parent_handles);
        // Explicit syntax no longer blocks the SAME Q39 base-name fact. This
        // is a module declaration observation, not permission to resolve its
        // written generic argument or execute it through a name fallback.
        let generic = ready(prepare(&mut db, &sites[3]));
        assert!(matches!(
            generic.selection(),
            EnteredCallSelection::Module(_)
        ));
        assert_eq!(generic.written_source_name().unwrap().explicit(), ["u8"]);
        assert!(
            generic
                .local_source_name(&db, index, &sites[3])
                .unwrap()
                .is_none()
        );
        assert!(generic.local_callable(&mut db).is_err());
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
