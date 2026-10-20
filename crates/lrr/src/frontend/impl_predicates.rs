//! Source impl predicate requirements through ordinary declaration-bound services.
//! These recipes cannot establish applicability, absence or catalog completeness.
use super::impl_head_types::{ImplHeadPending, ImplHeadTypeResult, PreparedImplHeadTypes};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    traits::{header_syntax::ImplHeaderSyntaxKey, predicate::SourcePredicateRecipe},
};
use ra_ap_syntax::{AstNode, ast::{self, HasGenericParams}};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug)]
pub enum ImplPredicateResult {
    HeadPending(ImplHeadPending),
    Requirements(Vec<SourcePredicateRecipe>),
}
#[derive(Debug)]
pub struct PreparedImplPredicates {
    key: ImplHeaderSyntaxKey,
    head: Arc<QueryValue>,
    result: ImplPredicateResult,
    parameter_ranges: BTreeMap<u32, crate::syntax::spans::SourceRange>,
}
impl PreparedImplPredicates {
    pub fn key(&self) -> &ImplHeaderSyntaxKey {
        &self.key
    }
    pub fn head(&self) -> &PreparedImplHeadTypes {
        let QueryValue::ImplHeadTypes(head) = self.head.as_ref() else {
            unreachable!("private impl head owner")
        };
        head
    }
    pub(crate) fn parameter_range(&self, key: crate::traits::term::ParamKey) -> Option<crate::syntax::spans::SourceRange> {
        if self.head().syntax().canonical_owner()?.identity() != key.definition { return None; }
        self.parameter_ranges.get(&key.ordinal).copied()
    }
    pub fn result(&self) -> &ImplPredicateResult {
        &self.result
    }
    pub(crate) fn prepare(db: &mut Database, key: &ImplHeaderSyntaxKey) -> QueryPoll<Self> {
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let head_owner = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::ImplHeadTypes(Box::new(key.clone()))
        ))
        .value;
        let QueryValue::ImplHeadTypes(head) = head_owner.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl predicate head query differs")));
        };
        let self_type = match head.result() {
            ImplHeadTypeResult::Resolved { self_type, .. } => self_type,
            ImplHeadTypeResult::Pending(why) => {
                return QueryPoll::Ready(Ok(Self {
                    key: key.clone(),
                    head: head_owner.clone(),
                    result: ImplPredicateResult::HeadPending(*why),
                    parameter_ranges: BTreeMap::new(),
                }));
            }
        };
        let Some(context) = head.declaration() else {
            return QueryPoll::Ready(Err(fault(
                "resolved impl predicate declaration context absent",
            )));
        };
        let syntax = head.syntax();
        let Some(canonical) = syntax.canonical_owner() else {
            return QueryPoll::Ready(Err(fault("resolved impl predicate owner absent")));
        };
        let node = match syntax
            .parsed()
            .syntax_at(context.range(), |node| ast::Impl::can_cast(node.kind()))
        {
            Ok(Some(node)) => node,
            Ok(None) => {
                return QueryPoll::Ready(Err(fault("impl predicate retained syntax absent")));
            }
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut parameter_ranges = BTreeMap::new();
        if let Some(parameters) = ast::Impl::cast(node.clone()).expect("checked retained Impl").generic_param_list() {
            for (index, parameter) in parameters.generic_params().enumerate() {
                let ordinal = match u32::try_from(index) {
                    Ok(index) => index, Err(_) => return QueryPoll::Ready(Err(fault("source generic ordinal overflow"))),
                };
                parameter_ranges.insert(ordinal, crate::syntax::ast_adapter::range(parameter.syntax().text_range()));
            }
        }
        let inventory = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ));
        let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl predicate inventory query differs")));
        };
        let Some(environment) = inventory.inventory_root().environment() else {
            return QueryPoll::Ready(Err(fault(
                "resolved impl predicate source environment absent",
            )));
        };
        let declaration = super::source_types::WrittenTypeDeclaration::implementation(
            context.owner(canonical.identity()),
            syntax.generics(),
            &node,
            Some(self_type),
        );
        let sources: Vec<_> = environment.sources().iter().collect();
        let requirements = ready!(super::source_predicates::prepare(
            db,
            environment.names(),
            &sources,
            &declaration,
            &node,
            environment.language_item_core(declaration.profile()),
        ));
        QueryPoll::Ready(Ok(Self {
            key: key.clone(),
            head: head_owner.clone(),
            result: ImplPredicateResult::Requirements(requirements),
            parameter_ranges,
        }))
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-HEADER", message.into())
}
impl StableEncode for PreparedImplPredicates {
    const DOMAIN: &'static str = "lrr/v2/prepared-impl-predicate-requirements";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.head());
        e.map(&self.parameter_ranges, |e, ordinal, range| { e.u32(*ordinal); e.u32(range.start); e.u32(range.end); });
        match &self.result {
            ImplPredicateResult::HeadPending(_) => e.u32(0), // exact reason encoded in actual head
            ImplPredicateResult::Requirements(requirements) => {
                e.u32(1);
                e.sequence(requirements, |e, r| e.record(r));
            }
        }
    }
}
