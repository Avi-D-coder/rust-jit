//! Associated type values of one source trait impl, for the pinned solver's
//! specialization leaves and `type_of`: each `type Name = Target;` of the
//! impl, joined to its alias declaration identity and its declared target
//! written in the impl's generic scope. Source preparation only; no impl is
//! matched, no projection normalized and no goal evaluated here.
use super::{
    source_impl_semantics::ImplSemantic,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    traits::{header_syntax::ImplHeaderSyntaxKey, term::TyTerm},
};
use std::sync::Arc;

pub type SourceImplAssociatedKey = ImplHeaderSyntaxKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImplAssociatedPending {
    Semantics,
    Environment,
    Members,
    Target,
}
#[derive(Debug)]
pub struct ImplAssociatedType {
    pub(super) name: String,
    pub(super) alias: Digest,
    pub(super) target: TyTerm,
}
#[derive(Debug)]
pub struct PreparedSourceImplAssociated {
    key: SourceImplAssociatedKey,
    semantics: Arc<QueryValue>,
    implementation: Option<Digest>,
    types: Vec<ImplAssociatedType>,
    pending: Option<ImplAssociatedPending>,
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
impl PreparedSourceImplAssociated {
    pub fn key(&self) -> &SourceImplAssociatedKey {
        &self.key
    }
    pub fn pending(&self) -> Option<ImplAssociatedPending> {
        self.pending
    }
    /// The impl's canonical declaration identity.
    pub(super) fn implementation(&self) -> Option<Digest> {
        self.implementation
    }
    /// The complete associated type rows; a pending preparation never
    /// certifies an empty list.
    pub(super) fn types(&self) -> Result<&[ImplAssociatedType], ImplAssociatedPending> {
        self.pending.map_or(Ok(&self.types), Err)
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceImplAssociatedKey) -> QueryPoll<Self> {
        let semantics = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceImplSemantics(Box::new(key.clone())))).value;
        let mut result = Self {
            key: key.clone(),
            semantics,
            implementation: None,
            types: Vec::new(),
            pending: Some(ImplAssociatedPending::Semantics),
        };
        let QueryValue::SourceImplSemantics(plan) = result.semantics.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl associated semantics query differs")));
        };
        let ImplSemantic::Known(implementation) = plan.impl_definition() else {
            return QueryPoll::Ready(Ok(result));
        };
        result.implementation = Some(implementation);
        let inventory = ready!(db.fetch(&SyntaxProvider, QueryKey::PreparedScopeImplHeaders(key.inventory.clone()))).value;
        let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl associated inventory query differs")));
        };
        let Some(environment) = inventory.inventory_root().environment() else {
            result.pending = Some(ImplAssociatedPending::Environment);
            return QueryPoll::Ready(Ok(result));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        // The member index names an impl by its associated-scope owner.
        let declared = sources.iter().find_map(|source| source.outline_item(implementation));
        let Some(row) = declared.and_then(|item| {
            members.impl_members(item.def_key.enclosing_impl?, &item.scope)
        }) else {
            result.pending = Some(ImplAssociatedPending::Members);
            return QueryPoll::Ready(Ok(result));
        };
        let mut types = Vec::new();
        for (name, alias) in &row.types {
            let target = ready!(crate::frontend::source_types::alias_declared_target(
                db, environment.names(), &sources, *alias,
            ));
            let Some(target) = target.filter(|target| !shorthand_projection(target)) else {
                result.pending = Some(ImplAssociatedPending::Target);
                return QueryPoll::Ready(Ok(result));
            };
            types.push(ImplAssociatedType { name: name.clone(), alias: *alias, target });
        }
        result.types = types;
        result.pending = None;
        QueryPoll::Ready(Ok(result))
    }
}
/// A shorthand projection (`I::Item`) whose trait the written Type walk could
/// not select carries the zero trait key: it names no source trait, so it is
/// not a term the solver may be given.
fn shorthand_projection(term: &TyTerm) -> bool {
    use crate::traits::term::GenericArg;
    match term {
        TyTerm::Projection(key) => {
            key.trait_ref.trait_key == Digest([0; 32])
                || shorthand_projection(&key.trait_ref.self_ty)
                || key.trait_ref.args.iter().chain(&key.item_args).any(|arg| match arg {
                    GenericArg::Type(ty) => shorthand_projection(ty),
                    _ => false,
                })
        }
        TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => shorthand_projection(inner),
        TyTerm::Tuple(members) => members.iter().any(shorthand_projection),
        TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(ty) => shorthand_projection(ty),
            _ => false,
        }),
        _ => false,
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into())
}
impl StableEncode for PreparedSourceImplAssociated {
    const DOMAIN: &'static str = "lrr/v1/source-impl-associated-types";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.option(&self.implementation, |e, implementation| e.digest(*implementation));
        e.sequence(&self.types, |e, row| {
            e.string(&row.name);
            e.digest(row.alias);
            e.record(&row.target);
        });
        e.option(&self.pending, |e, pending| e.u32(*pending as u32));
    }
}
