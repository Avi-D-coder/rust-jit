//! Language-item calls in the source constraint service. `a..=b` is rustc's
//! `RangeInclusive::new(a, b)`: its arguments relate to the declared
//! parameters and its value to the declared result exactly as a written call
//! of that function does, through the same declared-signature services. Only
//! the callee's selection is specific: the language item resolves through the
//! profile's own `core` crate, never through the caller's names.
use super::{Component, ConstraintDemand, fault};
use crate::{
    core::digest::Digest,
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    ir::pc::NodeId,
    traits::term::{GenericArg, TyTerm},
};

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

impl Component<'_> {
    /// The function a language-item call calls (`None` after deferring on the
    /// missing selection).
    fn lang_function(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<Option<Digest>> {
        let Some(environment) = ready!(self.environment(db)) else {
            self.defer(node, ConstraintDemand::ModuleEnvironment);
            return QueryPoll::Ready(Ok(None));
        };
        let plan = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::SourceLocalPlan(self.key.body.clone())
        ));
        let QueryValue::SourceLocalPlan(plan) = plan.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint source plan variant differs")));
        };
        let name = crate::syntax::constructors::range_inclusive_new_path(
            plan.context().source().index().profile(),
        );
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let resolver =
            match crate::frontend::callables::CallResolver::new(environment.names(), &sources) {
                Ok(resolver) => resolver.with_members(Some(&members)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        match resolver.source_function(db, self.key.body.definition, &name) {
            QueryPoll::Ready(Ok(crate::frontend::callables::CallSelection::Source(callee))) => {
                QueryPoll::Ready(Ok(Some(callee.signature().definition)))
            }
            QueryPoll::Ready(Ok(_)) => {
                self.defer(node, ConstraintDemand::SelectedDeclaration);
                QueryPoll::Ready(Ok(None))
            }
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                self.defer(node, ConstraintDemand::SelectedDeclaration);
                QueryPoll::Ready(Ok(None))
            }
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// The call's value is the callee's declared result.
    pub(super) fn lang_inward(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        match ready!(self.lang_function(db, node)) {
            Some(function) => self.result(db, node, function),
            None => QueryPoll::Ready(Ok(())),
        }
    }
    /// A call argument takes its declared parameter.
    pub(super) fn lang_outward(
        &mut self,
        db: &mut Database,
        call: NodeId,
        ordinal: u32,
    ) -> QueryPoll<()> {
        match ready!(self.lang_function(db, call)) {
            Some(function) => self.declared_parameter(db, call, function, ordinal),
            None => QueryPoll::Ready(Ok(())),
        }
    }
    /// The call's static Type: the callee's declared result, with the
    /// callee's own type parameters left to inference (`RangeInclusive<_>`
    /// until the bounds decide `Idx`).
    pub(super) fn lang_type(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        match ready!(self.lang_function(db, node)) {
            Some(function) => QueryPoll::Ready(Ok(ready!(self.result_term(db, node, function))
                .map(|term| open_parameters(term, function)))),
            None => QueryPoll::Ready(Ok(None)),
        }
    }
}
/// `term` with each type parameter of `definition` an inference hole.
fn open_parameters(term: TyTerm, definition: Digest) -> TyTerm {
    match term {
        TyTerm::Param(key) if key.definition == definition => super::methods::open(),
        TyTerm::Nominal(identity, arguments) => TyTerm::Nominal(
            identity,
            arguments
                .into_iter()
                .map(|argument| match argument {
                    GenericArg::Type(ty) => GenericArg::Type(open_parameters(ty, definition)),
                    other => other,
                })
                .collect(),
        ),
        TyTerm::Ref(region, mutable, inner) => TyTerm::Ref(
            region,
            mutable,
            Box::new(open_parameters(*inner, definition)),
        ),
        TyTerm::Slice(inner) => TyTerm::Slice(Box::new(open_parameters(*inner, definition))),
        TyTerm::Array(inner, length) => {
            TyTerm::Array(Box::new(open_parameters(*inner, definition)), length)
        }
        TyTerm::Tuple(items) => TyTerm::Tuple(
            items
                .into_iter()
                .map(|item| open_parameters(item, definition))
                .collect(),
        ),
        other => other,
    }
}
