//! Impl selection ahead of a call's later arguments.
//!
//! Several impls of one trait differ only by the trait's arguments (`Add<i64>`
//! and `Add<&i64>` for `i64`). The impl is normally selected once the call's
//! arguments are evaluated, but an unsuffixed numeric literal argument
//! (`c.add(&7)`) has no Type until the impl is known. rustc's inference rules
//! impls out as soon as an argument cannot unify with them: a literal is an
//! integer (float) variable, so it excludes every impl taking a reference,
//! `bool`, a nominal Type. When one impl survives what is already known, it is
//! the call's impl and its parameter Types hint the arguments still to be
//! evaluated.
use super::*;
use super::trait_calls::SelectionArgument;
use crate::{
    db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
    syntax::source_constraints::FactKind,
};

impl RunMachine {
    /// The numeric-literal class of the call argument `ordinal` as written:
    /// `Some((integer, borrows))` for an unsuffixed integer (float) literal
    /// under `borrows` shared `&`; `None` for any other expression.
    pub(super) fn written_literal_argument(
        &self,
        db: &mut Database,
        site: u32,
        ordinal: usize,
    ) -> QueryPoll<Option<(bool, usize)>> {
        let facts = match db.fetch(
            &SyntaxProvider,
            QueryKey::SourceConstraints(self.frame.pc().body.clone()),
        ) {
            QueryPoll::Ready(Ok(facts)) => facts,
            // A body without retained source constraints states nothing.
            QueryPoll::Ready(Err(_)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::SourceConstraints(facts) = facts.value.as_ref() else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(call) = facts.fact(NodeId(site)) else {
            return QueryPoll::Ready(Ok(None));
        };
        // A method call's operands are the receiver then the arguments; a
        // path call's first operand is its callee.
        let operand = match call.kind() {
            FactKind::MethodCall | FactKind::LangCall => ordinal,
            FactKind::Call => ordinal + 1,
            _ => return QueryPoll::Ready(Ok(None)),
        };
        let mut node = call.operand(operand);
        let mut borrows = 0usize;
        while let Some(current) = node {
            let Some(fact) = facts.fact(current) else {
                return QueryPoll::Ready(Ok(None));
            };
            match fact.kind() {
                FactKind::Integer => return QueryPoll::Ready(Ok(Some((true, borrows)))),
                FactKind::Float => return QueryPoll::Ready(Ok(Some((false, borrows)))),
                FactKind::Parenthesis => node = fact.operand(0),
                // `-lit`: still an unsuffixed literal.
                FactKind::Prefix if fact.operation() == Some(
                    crate::syntax::source_constraints::SourceOperator::Negate,
                ) => node = fact.operand(0),
                FactKind::Borrow if !fact.flag() => {
                    borrows += 1;
                    node = fact.operand(0);
                }
                _ => return QueryPoll::Ready(Ok(None)),
            }
        }
        QueryPoll::Ready(Ok(None))
    }

    /// The call's arguments `0..count` still written as unsuffixed numeric
    /// literals, with the Type `callee` declares for each parameter.
    pub(super) fn written_literals(
        &self,
        db: &mut Database,
        site: u32,
        callee: &SourceCallable,
        count: usize,
    ) -> QueryPoll<Vec<super::trait_parameters::LiteralArgument>> {
        let mut literals = Vec::new();
        for ordinal in 0..count {
            let (integer, borrows) = match self.written_literal_argument(db, site, ordinal) {
                QueryPoll::Ready(Ok(Some(literal))) => literal,
                QueryPoll::Ready(Ok(None)) => continue,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let declared = match &self.graph {
                Some(graph) => {
                    let sources: Vec<_> = self.sources.iter().collect();
                    match crate::frontend::source_parameter_goals::prepare(
                        db,
                        graph,
                        &sources,
                        callee.signature().definition,
                        ordinal,
                    ) {
                        QueryPoll::Ready(Ok(
                            crate::frontend::source_parameter_goals::ParameterTypeGoal::Ready(goal),
                        )) => Some(goal.source().clone()),
                        QueryPoll::Ready(Ok(_)) | QueryPoll::Ready(Err(_)) => None,
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    }
                }
                None => None,
            };
            literals.push(super::trait_parameters::LiteralArgument {
                integer,
                borrows,
                declared,
            });
        }
        QueryPoll::Ready(Ok(literals))
    }
    /// The one of `candidates` that what is known of the call's arguments
    /// leaves applicable: the first `evaluated` arguments by their reached
    /// Types, the rest by what they are written as (an unsuffixed literal) or
    /// not at all. `None` when several (or no) impls remain, or no later
    /// argument is a literal (nothing then gains from selecting early).
    pub(super) fn early_alternative(
        &mut self,
        db: &mut Database,
        site: u32,
        candidates: &[crate::core::digest::Digest],
        arguments: &[SlotId],
        evaluated: usize,
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        let mut views = Vec::with_capacity(arguments.len());
        let mut literal = false;
        for (ordinal, slot) in arguments.iter().enumerate() {
            if ordinal < evaluated {
                match self.selection_argument(*slot) {
                    Ok(view) => views.push(view),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
                continue;
            }
            match self.written_literal_argument(db, site, ordinal) {
                QueryPoll::Ready(Ok(Some((integer, borrows)))) => {
                    literal = true;
                    views.push(SelectionArgument::Literal {
                        integer,
                        behind_reference: borrows > 0,
                    });
                }
                QueryPoll::Ready(Ok(None)) => views.push(SelectionArgument::Unknown),
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        if !literal {
            return QueryPoll::Ready(Ok(None));
        }
        self.select_by_arguments_with(db, candidates, &views, None, false)
    }
}
