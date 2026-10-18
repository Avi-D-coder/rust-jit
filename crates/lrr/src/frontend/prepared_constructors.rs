//! Immutable record binding through the ordinary owning source/type route.
//! No runtime descriptors, guest effects or alternative expression evaluator.
use super::constructors::{RecordConstructorBinding, RecordConstructorDemand};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    source::manifest::PcKey,
    syntax::constructors::{ConstructorKind, ConstructorPlan},
};
use std::sync::Arc;

#[derive(Debug, Clone)]
enum PreparedRecordBinding {
    Ready(super::types::BoundNominalConstructor),
    Pending {
        mapping: super::types::BoundRecordFields,
        variant: u32,
        name: Option<String>,
        fields: Box<[Option<crate::traits::term::TyTerm>]>,
    },
}
#[derive(Debug, Clone)]
pub struct PreparedRecordConstructor {
    key: PcKey,
    plan: ConstructorPlan,
    plan_identity: Digest,
    binding: PreparedRecordBinding,
    /// Actual final-segment Type arguments selected at indexed occurrences.
    /// Runtime parameter descriptors are read only by the owning activation.
    arguments: Box<[Option<super::explicit_type_arguments::ExplicitType>]>,
}
impl PreparedRecordConstructor {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn plan(&self) -> &ConstructorPlan {
        &self.plan
    }
    pub(crate) fn plan_identity(&self) -> Digest {
        self.plan_identity
    }
    pub(crate) fn bound(&self) -> Option<&super::types::BoundNominalConstructor> {
        match &self.binding {
            PreparedRecordBinding::Ready(bound) => Some(bound),
            _ => None,
        }
    }
    pub(crate) fn mapping(&self) -> &super::types::BoundRecordFields {
        match &self.binding {
            PreparedRecordBinding::Ready(bound) => bound.mapping(),
            PreparedRecordBinding::Pending { mapping, .. } => mapping,
        }
    }
    pub(crate) fn pending_target(&self) -> Option<(u32, Option<&str>)> {
        match &self.binding {
            PreparedRecordBinding::Pending { variant, name, .. } => {
                Some((*variant, name.as_deref()))
            }
            _ => None,
        }
    }
    pub(crate) fn pending_fields(&self) -> Option<&[Option<crate::traits::term::TyTerm>]> {
        match &self.binding {
            PreparedRecordBinding::Pending { fields, .. } => Some(fields),
            _ => None,
        }
    }
    pub(crate) fn arguments(&self) -> &[Option<super::explicit_type_arguments::ExplicitType>] {
        &self.arguments
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        use super::explicit_type_arguments::ExplicitType;
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.plan
                    .estimated_bytes()?
                    .checked_sub(std::mem::size_of::<ConstructorPlan>())?,
            )?
            .checked_add(match &self.binding {
                PreparedRecordBinding::Ready(bound) => bound
                    .estimated_bytes()?
                    .checked_sub(std::mem::size_of::<super::types::BoundNominalConstructor>())?,
                PreparedRecordBinding::Pending {
                    mapping,
                    name,
                    fields,
                    ..
                } => {
                    let mut bytes = mapping
                        .heap_bytes()?
                        .checked_add(name.as_ref().map_or(0, String::capacity))?
                        .checked_add(fields.len().checked_mul(std::mem::size_of::<
                            Option<crate::traits::term::TyTerm>,
                        >())?)?;
                    for field in fields.iter().flatten() {
                        bytes = bytes.checked_add(field.heap_bytes()?)?;
                    }
                    bytes
                }
            })?
            .checked_add(
                self.arguments
                    .len()
                    .checked_mul(std::mem::size_of::<Option<ExplicitType>>())?,
            )?;
        for argument in self.arguments.iter().flatten() {
            bytes = bytes.checked_add(match argument {
                ExplicitType::Selected(source) => source.heap_bytes().ok()?,
                ExplicitType::Parameter(term) => term.heap_bytes()?,
                ExplicitType::Written { term, written } => {
                    term.heap_bytes()?.checked_add(written.heap_bytes())?
                }
            })?;
        }
        Some(bytes)
    }
}
impl StableEncode for PreparedRecordConstructor {
    const DOMAIN: &'static str = "lrr/v4/prepared-record-constructor";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(&self.plan);
        e.digest(self.plan_identity);
        match &self.binding {
            PreparedRecordBinding::Ready(bound) => {
                e.u32(0);
                e.record(bound);
            }
            PreparedRecordBinding::Pending {
                mapping,
                variant,
                name,
                fields,
            } => {
                e.u32(1);
                e.record(mapping);
                e.u32(*variant);
                e.option(name, |e, name| e.string(name));
                e.sequence(fields, |e, field| {
                    e.option(field, |e, field| e.record(field))
                });
            }
        }
        e.sequence(&self.arguments, |e, argument| {
            e.option(argument, |e, argument| e.record(argument))
        });
    }
}
/// Retains the actual query allocation and, when required, a run-shared ready
/// specialization. Cloning a handle shares both allocations; it never copies
/// a body, syntax plan or bound field list.
#[derive(Debug, Clone)]
pub(crate) struct PreparedRecordConstructorHandle {
    value: Arc<QueryValue>,
    specialized: Option<Arc<super::types::BoundNominalConstructor>>,
    guard: Option<Arc<crate::db::memo::PreparedInputGuard>>,
}
impl PreparedRecordConstructorHandle {
    pub(crate) fn from_query(value: Arc<QueryValue>, key: &PcKey) -> Result<Self, Diagnostic> {
        if !matches!(value.as_ref(), QueryValue::BoundRecordConstructor(value) if value.key() == key)
        {
            return Err(fault("prepared record query key/variant differs"));
        }
        Ok(Self {
            value,
            specialized: None,
            guard: None,
        })
    }
    pub(crate) fn same_source(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        if let Some(guard) = &self.guard {
            guard.require_current(db)?;
        }
        Ok(())
    }
    pub(crate) fn ready_bound(&self) -> Result<&super::types::BoundNominalConstructor, Diagnostic> {
        self.specialized
            .as_deref()
            .or_else(|| self.prepared().bound())
            .ok_or_else(|| fault("record template requires its reached Type activation"))
    }
    pub(crate) fn specialize(
        mut self,
        variant: &super::types::BoundNominalVariant,
        guard: Arc<crate::db::memo::PreparedInputGuard>,
    ) -> Result<Self, Diagnostic> {
        let Some((selected, _)) = self.prepared().pending_target() else {
            return Err(fault("record specialization lacks pending source template"));
        };
        let bound = super::types::BoundNominalConstructor::rejoin(
            self.prepared().mapping(),
            selected,
            variant,
        )?;
        self.specialized = Some(Arc::new(bound));
        self.guard = Some(guard);
        Ok(self)
    }
    pub(crate) fn prepared(&self) -> &PreparedRecordConstructor {
        match self.value.as_ref() {
            QueryValue::BoundRecordConstructor(value) => value,
            // The checked constructor and private field preserve this invariant.
            _ => unreachable!("typed prepared constructor handle"),
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<PreparedRecordConstructor> {
    macro_rules! ready {
        ($poll:expr) => {
            match $poll {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("prepared constructor requires entry phase")));
    }
    let syntax = ready!(db.fetch(&SyntaxProvider, QueryKey::ConstructorPlan(key.clone())));
    let QueryValue::ConstructorPlan(plan) = syntax.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("prepared constructor plan variant differs")));
    };
    if !matches!(plan.kind(), ConstructorKind::Record { rest: None, .. }) {
        return QueryPoll::Ready(Err(fault(
            "prepared record query requires a record constructor",
        )));
    }
    let owner = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(key.body.clone())));
    let QueryValue::SourceLocalPlan(owner) = owner.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("prepared constructor owning plan differs")));
    };
    let Some(environment) = owner.environment() else {
        return QueryPoll::Ready(Err(fault("prepared constructor environment absent")));
    };
    ready!(owner.require_current(db));
    let arguments = ready!(prepare_arguments(db, owner, key, plan));
    let source_arguments: Vec<_> = arguments
        .iter()
        .map(|argument| argument.as_ref().map(|argument| argument.source().clone()))
        .collect();
    let completed = environment.macros().source_environment();
    let members = if arguments.is_empty() {
        None
    } else {
        match completed.member_index() {
            Ok(members) => Some(members),
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
    };
    let mut sources = Vec::new();
    if sources
        .try_reserve_exact(completed.sources().len())
        .is_err()
    {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    sources.extend(completed.sources().iter());
    let bound = ready!(super::constructors::bind_prepared_record_constructor(
        db,
        completed.names(),
        &sources,
        owner.context().source().index(),
        plan,
        &source_arguments,
        members.as_deref(),
    ));
    let binding = match bound {
        RecordConstructorBinding::Ready(bound) => PreparedRecordBinding::Ready(*bound),
        RecordConstructorBinding::Deferred(demand) => {
            let diagnostic = demand.diagnostic();
            let RecordConstructorDemand::Fields(fields) = *demand else {
                return QueryPoll::Ready(Err(diagnostic));
            };
            let super::types::NominalConstructorBinding::Deferred {
                mapping, variant, ..
            } = *fields
            else {
                return QueryPoll::Ready(Err(diagnostic));
            };
            let schema =
                ready!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(mapping.definition())));
            let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("pending record schema outcome differs")));
            };
            if schema.fingerprint() != mapping.schema() {
                return QueryPoll::Ready(Err(fault("pending record mapping schema differs")));
            }
            let Some(selected) = schema.variants.get(variant as usize) else {
                return QueryPoll::Ready(Err(fault("pending record selected variant absent")));
            };
            let names = ready!(super::types::SchemaNames::from_current_database(
                db,
                completed.names(),
                &sources,
                schema,
            ));
            let fields = ready!(names.source_variant_fields(db, &sources, variant));
            PreparedRecordBinding::Pending {
                mapping,
                variant,
                name: selected.name.clone(),
                fields: fields.into_boxed_slice(),
            }
        }
    };
    QueryPoll::Ready(Ok(PreparedRecordConstructor {
        key: key.clone(),
        plan: plan.as_ref().clone(),
        plan_identity: plan.fingerprint(),
        binding,
        arguments,
    }))
}

/// Shared explicit Type selection for the actual record path, retaining its
/// indexed source environment and all named-leaf query dependencies.
fn prepare_arguments(
    db: &mut Database,
    owner: &Arc<super::source_locals::PreparedSourceLocalPlan>,
    key: &PcKey,
    plan: &ConstructorPlan,
) -> QueryPoll<Box<[Option<super::explicit_type_arguments::ExplicitType>]>> {
    use ra_ap_syntax::{AstNode, ast, ast::HasGenericArgs};
    macro_rules! ready {
        ($poll:expr) => {
            match $poll {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let source = owner.context().source();
    let syntax =
        match source
            .parsed()
            .entered_node_syntax(source.body(), source.index(), plan.node())
        {
            Ok(syntax) => syntax,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
    let path = syntax
        .and_then(ast::RecordExpr::cast)
        .and_then(|record| record.path());
    if path.as_ref().is_some_and(|path| {
        std::iter::successors(path.qualifier(), |path| path.qualifier()).any(|path| {
            path.segment()
                .is_some_and(|segment| segment.generic_arg_list().is_some())
        })
    }) {
        return QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-UNSUPPORTED",
            "record owner Type arguments require their indexed owner context integration".into(),
        )));
    }
    let list = path
        .and_then(|path| path.segment())
        .and_then(|segment| segment.generic_arg_list());
    let Some(list) = list else {
        return QueryPoll::Ready(Ok(Box::new([])));
    };
    let mut arguments = Vec::new();
    for argument in list.generic_args() {
        let ty = match argument {
            ast::GenericArg::LifetimeArg(_) => continue,
            ast::GenericArg::TypeArg(argument) => match argument.ty() {
                Some(ty) => ty,
                None => return QueryPoll::Ready(Err(fault("record Type argument syntax absent"))),
            },
            _ => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "record const or associated argument requires its typed source goal".into(),
                )));
            }
        };
        let range = crate::syntax::ast_adapter::range(ty.syntax().text_range());
        let relative = match (
            range.start.checked_sub(source.body().range.start),
            range.end.checked_sub(source.body().range.start),
        ) {
            (Some(start), Some(end)) => crate::syntax::spans::SourceRange { start, end },
            _ => return QueryPoll::Ready(Err(fault("record Type argument escapes its body"))),
        };
        let Some(node) = source
            .index()
            .nodes_at_range(relative)
            .find(|node| node.role == crate::syntax::body_index::NodeRole::Syntax)
        else {
            return QueryPoll::Ready(Err(fault("record Type argument indexed occurrence absent")));
        };
        let site = match source.index().pc(node.id, crate::ir::pc::Phase::Entry) {
            Ok(site) => site,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let selected = ready!(super::explicit_type_arguments::select_argument(
            db, owner, key, &site, range
        ));
        arguments.push(match selected {
            Some(selected) => {
                for guard in &selected.guards {
                    if let Err(error) = guard.require_current(db) {
                        return QueryPoll::Ready(Err(error));
                    }
                }
                Some(selected.source)
            }
            None => None,
        });
    }
    QueryPoll::Ready(Ok(arguments.into_boxed_slice()))
}

impl RecordConstructorDemand {
    /// Same typed pending-to-diagnostic boundary for prepared and historical
    /// ordinary-source entries. Macro identity never changes the outcome.
    pub(crate) fn diagnostic(&self) -> Diagnostic {
        use super::names::NameLookup;
        let unsupported = |message: &str| Diagnostic::new("LRR-E-UNSUPPORTED", message.into());
        match self {
            Self::Name(NameLookup::Absent) => {
                Diagnostic::new("LRR-E-NAME-MISSING", "constructor type is absent".into())
            }
            Self::Name(NameLookup::Inaccessible(_)) => Diagnostic::new(
                "LRR-E-NAME-VISIBILITY",
                "constructor type is inaccessible".into(),
            ),
            Self::Name(NameLookup::Ambiguous(_)) | Self::Name(NameLookup::AmbiguousModules(_)) => {
                Diagnostic::new(
                    "LRR-E-NAME-AMBIGUOUS",
                    "constructor type is ambiguous".into(),
                )
            }
            Self::Name(NameLookup::NeedInput(_)) => {
                unsupported("constructor lookup requires unresolved source producers or imports")
            }
            Self::Parameter(_) | Self::OwnerContext(_) => {
                unsupported("constructor requires its entered generic or owner type context")
            }
            Self::Fields(_) => {
                unsupported("constructor field types require further source type binding")
            }
            _ => unsupported("constructor requires further source member/type binding"),
        }
    }
}
