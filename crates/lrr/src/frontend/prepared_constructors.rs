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
pub struct PreparedRecordConstructor {
    key: PcKey,
    plan: ConstructorPlan,
    plan_identity: Digest,
    bound: super::types::BoundNominalConstructor,
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
    pub(crate) fn bound(&self) -> &super::types::BoundNominalConstructor {
        &self.bound
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(
                self.plan
                    .estimated_bytes()?
                    .checked_sub(std::mem::size_of::<ConstructorPlan>())?,
            )?
            .checked_add(
                self.bound
                    .estimated_bytes()?
                    .checked_sub(std::mem::size_of::<super::types::BoundNominalConstructor>())?,
            )
    }
}
impl StableEncode for PreparedRecordConstructor {
    const DOMAIN: &'static str = "lrr/v1/prepared-record-constructor";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(&self.plan);
        e.digest(self.plan_identity);
        e.record(&self.bound);
    }
}
/// Retains the actual query allocation, whose lifetime/accounting is already
/// tracked by Database's query pins. No deep copy of a body, plan or binding.
#[derive(Debug, Clone)]
pub(crate) struct PreparedRecordConstructorHandle(Arc<QueryValue>);
impl PreparedRecordConstructorHandle {
    pub(crate) fn from_query(value: Arc<QueryValue>, key: &PcKey) -> Result<Self, Diagnostic> {
        if !matches!(value.as_ref(), QueryValue::BoundRecordConstructor(value) if value.key() == key)
        {
            return Err(fault("prepared record query key/variant differs"));
        }
        Ok(Self(value))
    }
    pub(crate) fn prepared(&self) -> &PreparedRecordConstructor {
        match self.0.as_ref() {
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
    let completed = environment.macros().source_environment();
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
    ));
    match bound {
        RecordConstructorBinding::Ready(bound) => QueryPoll::Ready(Ok(PreparedRecordConstructor {
            key: key.clone(),
            plan: plan.as_ref().clone(),
            plan_identity: plan.fingerprint(),
            bound: *bound,
        })),
        RecordConstructorBinding::Deferred(demand) => QueryPoll::Ready(Err(demand.diagnostic())),
    }
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
