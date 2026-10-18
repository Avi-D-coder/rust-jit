//! Actual associated-value qualifier occurrences, before Type/member application.
//! No name lookup, generic substitution or initializer entry is authorized by
//! these rows. They retain the same source plan and actual AST as ordinary
//! entered calls, including generic arguments that are not yet semantically
//! supported by the application/packet service.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{memo::Database, poll::QueryPoll},
    frontend::{
        entered_call_goals::{EnteredGenericArgument, written_argument},
        source_locals::{
            PreparedSourceLocalPlan, owned_plan::source_value_goals::EnteredValueGoal,
        },
    },
    ir::pc::{NodeId, PcKey},
    syntax::{
        ast_adapter::range,
        body_index::{ExpressionKind, NodeRole},
        token_wire::Key,
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-ITEM-APPLICATION", message.into())
}
/// An actual source occurrence is not a completed lexical Type selection.
/// The common Type/member service must discharge this demand before closing
/// an initializer application. No descriptor or caller environment does so.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AssociatedConstApplicationDemand {
    LexicalTypeAndSubstitution,
}
/// Every actual generic-argument AST is kept, including nested arguments,
/// lifetimes, consts and associated bindings. The observation supplies the
/// existing canonical indexed Type site or its explicit pending demand.
pub(crate) struct ItemGenericArgumentOccurrence {
    syntax: ast::GenericArg,
    observation: EnteredGenericArgument,
}
impl ItemGenericArgumentOccurrence {
    pub(crate) fn syntax(&self) -> &ast::GenericArg {
        &self.syntax
    }
    pub(crate) fn observation(&self) -> &EnteredGenericArgument {
        &self.observation
    }
}
/// Privately constructed source occurrence, not a selected item application.
/// `qualifier` may name a module or Type: only the ordinary resolver decides.
/// Shared Rowan handles retain genuine structure, never reparsed display text.
pub(crate) struct AssociatedConstPathOccurrence {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    qualifier: ast::Path,
    member: ast::NameRef,
    member_context: Option<Key>,
    member_edition: u16,
    arguments: Box<[ItemGenericArgumentOccurrence]>,
    fingerprint: Digest,
    bytes: usize,
}
impl AssociatedConstPathOccurrence {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn qualifier(&self) -> &ast::Path {
        &self.qualifier
    }
    pub(crate) fn member(&self) -> &ast::NameRef {
        &self.member
    }
    pub(crate) fn member_context(&self) -> Option<Key> {
        self.member_context
    }
    pub(crate) fn member_edition(&self) -> u16 {
        self.member_edition
    }
    pub(crate) fn arguments(&self) -> &[ItemGenericArgumentOccurrence] {
        &self.arguments
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn demand(&self) -> AssociatedConstApplicationDemand {
        AssociatedConstApplicationDemand::LexicalTypeAndSubstitution
    }
    /// Reuse checks the existing source/body/context/environment authority;
    /// this does not upgrade the occurrence to a Type/member lookup result.
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        self.plan.require_current(db)
    }
}
impl StableEncode for AssociatedConstPathOccurrence {
    const DOMAIN: &'static str = "lrr/v1/associated-constant-path-occurrence";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.source_identity());
        let qualifier = range(self.qualifier.syntax().text_range());
        let member = range(self.member.syntax().text_range());
        e.u32(qualifier.start);
        e.u32(qualifier.end);
        e.u32(member.start);
        e.u32(member.end);
        e.option(&self.member_context, |e, context| {
            e.digest(Digest(context.0))
        });
        e.u16(self.member_edition);
        e.sequence(&self.arguments, |e, argument| {
            e.record(&argument.observation)
        });
    }
}
/// Cold source preparation only. An owning immutable query may retain this
/// result; installed execution must consume that retained plan, never call
/// this helper to rediscover an operand. `None` means no qualifier occurred,
/// not that an associated constant or source Type was absent.
pub(crate) fn prepare_occurrence(
    db: &mut Database,
    value: &EnteredValueGoal,
) -> QueryPoll<Option<AssociatedConstPathOccurrence>> {
    let plan = Arc::clone(value.retained_plan());
    match plan.require_current(db) {
        QueryPoll::Ready(Ok(())) => {}
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    }
    QueryPoll::Ready((|| {
        let source = plan.context().source();
        let key = value.key();
        let node = NodeId(key.node);
        if key.phase != 0
            || source.body().key() != key.body
            || source.index().body() != &key.body
            || source
                .index()
                .nodes()
                .get(node.0 as usize)
                .is_none_or(|indexed| {
                    indexed.id != node || indexed.role != NodeRole::Expression(ExpressionKind::Path)
                })
        {
            return Err(fault("associated value occurrence indexed source differs"));
        }
        let Some(syntax) =
            source
                .parsed()
                .entered_node_syntax(source.body(), source.index(), node)?
        else {
            return Err(fault("associated value occurrence syntax absent"));
        };
        let expression = ast::PathExpr::cast(syntax)
            .ok_or_else(|| fault("associated value occurrence is not its actual path"))?;
        let path = expression
            .path()
            .ok_or_else(|| fault("associated value path absent"))?;
        let Some(qualifier) = path.qualifier() else {
            return Ok(None);
        };
        let member = path
            .segment()
            .and_then(|segment| segment.name_ref())
            .ok_or_else(|| fault("associated value member name absent"))?;
        let path_range = range(path.syntax().text_range());
        let member_range = range(member.syntax().text_range());
        if !source.body().range.contains(path_range) || !path_range.contains(member_range) {
            return Err(fault("associated value occurrence escapes actual body"));
        }
        let member_context = source.identifier_context(member_range)?;
        let member_edition = source.parsed().token_edition(member_range)?;
        let mut arguments = Vec::new();
        // Walk only this actual path once, not the body or parsed file. Nested
        // GenericArg handles preserve their real parent structure and order;
        // no application ordinal or erasure is inferred from this inventory.
        for syntax in path
            .syntax()
            .descendants()
            .filter_map(ast::GenericArg::cast)
        {
            let observation = written_argument(&plan, syntax.clone())?;
            if !path_range.contains(observation.range()) {
                return Err(fault(
                    "associated value generic argument escapes actual path",
                ));
            }
            arguments
                .try_reserve(1)
                .map_err(|_| crate::runtime::arena::resource())?;
            arguments.push(ItemGenericArgumentOccurrence {
                syntax,
                observation,
            });
        }
        let bytes = std::mem::size_of::<AssociatedConstPathOccurrence>()
            .checked_add(
                arguments
                    .len()
                    .checked_mul(std::mem::size_of::<ItemGenericArgumentOccurrence>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut occurrence = AssociatedConstPathOccurrence {
            key: key.clone(),
            plan: Arc::clone(&plan),
            qualifier,
            member,
            member_context,
            member_edition,
            arguments: arguments.into_boxed_slice(),
            fingerprint: Digest([0; 32]),
            bytes,
        };
        occurrence.fingerprint = StableEncode::fingerprint(&occurrence);
        Ok(Some(occurrence))
    })())
}
