//! Source-authenticated alias argument namespaces prepared before solver entry.
//! Policy: pinned rustc_middle/src/ty/context.rs:2162–2230. Parameter ordering
//! remains DeclarationParameters' job, shared with ordinary Type construction.
use super::{
    parameters::{DeclarationParameters, DeclaredParameterPreparation},
    source_definition_headers::{DefinitionHeaderKey, SourceHeaderParent},
    source_definition_semantics::{SourceAliasDomain, SourceSemantic},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::trait_associated_types::TraitAssociatedTypesKey,
    traits::term::BoundKind,
};
use std::sync::Arc;

pub type SourceAliasLayoutsKey = DefinitionHeaderKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum SourceAliasLayoutKind {
    FreeType = 0,
    FreeConst = 1,
    ProjectionType = 2,
    ProjectionConst = 3,
    InherentType = 4,
    InherentConstSelf = 5,
    InherentConstImpl = 6,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum AliasLayoutsPending {
    Header = 0,
    SemanticDomain = 1,
    ParameterLayout = 2,
    AssociatedIdentity = 3,
    NotApplicable = 4,
    AliasKind = 5,
}
#[derive(Debug)]
pub struct PreparedSourceAliasLayouts {
    key: SourceAliasLayoutsKey,
    semantics: Arc<QueryValue>,
    members: Option<Arc<QueryValue>>,
    layouts: Vec<(SourceAliasLayoutKind, Vec<BoundKind>)>,
    pending: Option<AliasLayoutsPending>,
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
impl PreparedSourceAliasLayouts {
    pub(crate) fn key(&self) -> &SourceAliasLayoutsKey {
        &self.key
    }
    pub(crate) fn pending(&self) -> Option<AliasLayoutsPending> {
        self.pending
    }
    pub(crate) fn layout(
        &self,
        kind: SourceAliasLayoutKind,
    ) -> Result<&[BoundKind], AliasLayoutsPending> {
        if let Some(why) = self.pending {
            return Err(why);
        }
        self.layouts
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, kinds)| kinds.as_slice())
            .ok_or(AliasLayoutsPending::AliasKind)
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceAliasLayoutsKey) -> QueryPoll<Self> {
        let semantics = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceDefinitionSemantics(Box::new(key.clone()))
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            semantics,
            members: None,
            layouts: Vec::new(),
            pending: None,
        };
        let QueryValue::SourceDefinitionSemantics(semantic) = result.semantics.as_ref() else {
            return QueryPoll::Ready(Err(fault("alias semantic query differs")));
        };
        if semantic.key() != key || semantic.header().pending().is_some() {
            result.pending = Some(AliasLayoutsPending::Header);
            return QueryPoll::Ready(Ok(result));
        }
        let domain = match semantic.alias_domain() {
            SourceSemantic::Known(domain) => domain,
            SourceSemantic::NotApplicable => {
                result.pending = Some(AliasLayoutsPending::NotApplicable);
                return QueryPoll::Ready(Ok(result));
            }
            SourceSemantic::Pending(_) => {
                result.pending = Some(AliasLayoutsPending::SemanticDomain);
                return QueryPoll::Ready(Ok(result));
            }
        };
        let parameters = match DeclarationParameters::prepare(semantic.header().declaration()) {
            Ok(DeclaredParameterPreparation::Ready(parameters)) => parameters,
            Ok(DeclaredParameterPreparation::Pending(_)) => {
                result.pending = Some(AliasLayoutsPending::ParameterLayout);
                return QueryPoll::Ready(Ok(result));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let ordinary = match (0..parameters.count())
            .map(|index| {
                parameters
                    .from_upstream(index)
                    .map(|parameter| parameter.kind())
            })
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(kinds) => kinds,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let self_form = || {
            std::iter::once(BoundKind::Type)
                .chain(
                    ordinary[parameters.parent_count() as usize..]
                        .iter()
                        .copied(),
                )
                .collect::<Vec<_>>()
        };
        use SourceAliasDomain as D;
        use SourceAliasLayoutKind as K;
        match domain {
            D::FreeType => result.layouts.push((K::FreeType, ordinary)),
            D::FreeConst => result.layouts.push((K::FreeConst, ordinary)),
            D::InherentType => result.layouts.push((K::InherentType, self_form())),
            D::InherentConst => {
                result.layouts.push((K::InherentConstSelf, self_form()));
                result.layouts.push((K::InherentConstImpl, ordinary));
            }
            D::ProjectionConst => {
                // Q77 currently authenticates type members only. The actual
                // constant member identity/parent join is a separate producer.
                result.pending = Some(AliasLayoutsPending::AssociatedIdentity);
            }
            D::ProjectionType => {
                let SourceHeaderParent::Associated(parent) = semantic.header().source_parent()
                else {
                    result.pending = Some(AliasLayoutsPending::AssociatedIdentity);
                    return QueryPoll::Ready(Ok(result));
                };
                let parent = *parent;
                let Some(name) = semantic.header().name() else {
                    return QueryPoll::Ready(Err(fault("projection declaration name absent")));
                };
                let name_key = crate::frontend::member_index::associated_name_key(name);
                let members = ready!(db.fetch(
                    &SyntaxProvider,
                    QueryKey::TraitAssociatedTypes(Box::new(TraitAssociatedTypesKey {
                        inventory: key.inventory.clone(),
                        trait_definition: parent,
                    }))
                ))
                .value;
                let QueryValue::TraitAssociatedTypes(actual) = members.as_ref() else {
                    return QueryPoll::Ready(Err(fault("projection member query differs")));
                };
                if actual.lookup(name_key) != Ok(key.definition) {
                    result.pending = Some(AliasLayoutsPending::AssociatedIdentity);
                } else {
                    result.layouts.push((K::ProjectionType, ordinary));
                }
                result.members = Some(members);
            }
        }
        QueryPoll::Ready(Ok(result))
    }
}
impl StableEncode for PreparedSourceAliasLayouts {
    const DOMAIN: &'static str = "lrr/v1/source-alias-layouts";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        let QueryValue::SourceDefinitionSemantics(plan) = self.semantics.as_ref() else {
            unreachable!("captured alias semantic query")
        };
        e.record(plan.as_ref());
        e.option(&self.members, |e, members| {
            let QueryValue::TraitAssociatedTypes(members) = members.as_ref() else {
                unreachable!("captured associated member query")
            };
            e.record(members.as_ref());
        });
        e.sequence(&self.layouts, |e, (kind, kinds)| {
            e.u32(*kind as u32);
            e.sequence(kinds, |e, kind| e.u32(kind.tag()));
        });
        e.option(&self.pending, |e, why| e.u32(*why as u32));
    }
}
pub(crate) struct CurrentSourceAliasLayouts {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceAliasLayouts {
    pub(crate) fn capture(db: &mut Database, key: &SourceAliasLayoutsKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceAliasLayouts(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(),
            QueryValue::SourceAliasLayouts(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("alias layout memo identity differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceAliasLayouts {
        let QueryValue::SourceAliasLayouts(plan) = self.value.as_ref() else {
            unreachable!("captured alias layout plan")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-ALIAS-LAYOUTS", message.into())
}
