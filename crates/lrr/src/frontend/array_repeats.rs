//! Source-selected repetition policy; no initializer execution or runtime Type.
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
        source_locals::PreparedSourceLocalPlan, source_return_goals::ReturnTypeGoal,
        source_value_goals::CanonicalValueKind,
    },
    ir::{pc::PcKey, written_type::WrittenType},
    source::manifest::ItemKind,
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        constructors::{ConstructorKind, ConstructorPlan},
    },
};
use std::sync::Arc;
macro_rules! ready {
    ($v:expr) => {
        match $v {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}

#[derive(Debug)]
pub struct ArrayRepeatPlan {
    key: PcKey,
    syntax: Arc<QueryValue>,
    syntax_identity: Digest,
    parent: Option<Arc<PreparedSourceLocalPlan>>,
    constant: Option<Digest>,
    written: Option<Arc<WrittenType>>,
}
impl ArrayRepeatPlan {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn parent(&self) -> Option<&Arc<PreparedSourceLocalPlan>> {
        self.parent.as_ref()
    }
    pub(crate) fn syntax(&self) -> &ConstructorPlan {
        let QueryValue::ConstructorPlan(plan) = self.syntax.as_ref() else {
            unreachable!("private repeat syntax variant")
        };
        plan
    }
    pub(crate) fn written(&self) -> Option<&Arc<WrittenType>> {
        self.written.as_ref()
    }
}
impl StableEncode for ArrayRepeatPlan {
    const DOMAIN: &'static str = "lrr/v1/array-repeat-source-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.syntax_identity);
        e.option(&self.parent, |e, p| e.digest(p.source_identity()));
        e.option(&self.constant, |e, c| e.digest(*c));
        e.option(&self.written, |e, w| e.record(w.as_ref()));
    }
}
#[derive(Debug)]
pub struct PreparedArrayRepeat {
    raw: Arc<QueryValue>,
    guard: Arc<PreparedInputGuard>,
}
impl PreparedArrayRepeat {
    pub(crate) fn plan(&self) -> &ArrayRepeatPlan {
        let QueryValue::ArrayRepeatPlan(plan) = self.raw.as_ref() else {
            unreachable!("private prepared repeat variant")
        };
        plan
    }
    pub(crate) fn guard(&self) -> &Arc<PreparedInputGuard> {
        &self.guard
    }
}
impl StableEncode for PreparedArrayRepeat {
    const DOMAIN: &'static str = "lrr/v1/prepared-array-repeat";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(self.plan());
        e.digest(self.guard.fingerprint());
    }
}
pub(crate) fn prepare(db: &mut Database, key: &PcKey) -> QueryPoll<ArrayRepeatPlan> {
    if key.phase != 0 {
        return QueryPoll::Ready(Err(fault("repeat plan requires its actual entry PC")));
    }
    let syntax = ready!(db.fetch(&SyntaxProvider, QueryKey::ConstructorPlan(key.clone())));
    let QueryValue::ConstructorPlan(plan) = syntax.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("repeat constructor query variant differs")));
    };
    let ConstructorKind::Repeat { element, .. } = plan.kind() else {
        return QueryPoll::Ready(Err(fault("repeat source plan requires repeat syntax")));
    };
    let structure = ready!(db.fetch(&SyntaxProvider, QueryKey::BodyStructure(key.body.clone())));
    let QueryValue::BodyStructure(index) = structure.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("repeat body index differs")));
    };
    let mut element_node = element.node;
    loop {
        let node = &index.nodes()[element_node.0 as usize];
        if node.parenthesized {
            let Some(next) = node.operands.first() else {
                return QueryPoll::Ready(Err(fault("repeat transparent operand absent")));
            };
            element_node = *next;
        } else {
            break;
        }
    }
    let mut output = ArrayRepeatPlan {
        key: key.clone(),
        syntax_identity: syntax.fingerprint,
        syntax: Arc::clone(&syntax.value),
        parent: None,
        constant: None,
        written: None,
    };
    // An inline `const { .. }` element is an ordinary block here: evaluated
    // once and copied like any other non-path element.
    if index.nodes()[element_node.0 as usize].role != NodeRole::Expression(ExpressionKind::Path) {
        return QueryPoll::Ready(Ok(output));
    }
    let selected = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::EnteredValue(PcKey {
            body: key.body.clone(),
            node: element_node.0,
            phase: 0
        })
    ));
    let QueryValue::EnteredValue(goal) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("repeat Value goal differs")));
    };
    if goal.pending().is_some() {
        return QueryPoll::Ready(Err(fault(
            "repeat operand requires its actual source selection",
        )));
    }
    let parent = Arc::clone(goal.retained_plan());
    ready!(parent.require_current(db));
    output.parent = Some(Arc::clone(&parent));
    match ready!(goal.declaration_kind(db)) {
        CanonicalValueKind::Ready(ItemKind::Constant) => {}
        CanonicalValueKind::Ready(ItemKind::Function) => return QueryPoll::Ready(Ok(output)),
        CanonicalValueKind::Pending(_) => {
            return QueryPoll::Ready(Err(fault(
                "repeat declaration kind requires its actual source metadata",
            )));
        }
        _ if goal.rooted().is_some()
            && goal.declaration().is_some_and(|d| d.variant().is_none()) =>
        {
            return QueryPoll::Ready(Err(fault(
                "repeat operand requires its selected source member, not its lexical type root",
            )));
        }
        _ => return QueryPoll::Ready(Ok(output)),
    }
    let Some(selected) = goal.declaration() else {
        return QueryPoll::Ready(Err(fault("repeat constant declaration absent")));
    };
    let definition = selected.identity();
    let Some(environment) = parent.environment() else {
        return QueryPoll::Ready(Err(fault("repeat owning environment absent")));
    };
    let environment = environment.macros().source_environment();
    let sources: Vec<_> = environment.sources().iter().collect();
    let source = ready!(super::source_return_goals::prepare(
        db,
        environment.names(),
        &sources,
        definition
    ));
    let ReturnTypeGoal::Ready(source) = source else {
        return QueryPoll::Ready(Err(fault(
            "constant repeat element requires its actual declared result Type",
        )));
    };
    let Some(written) = ready!(super::written_terms::closed_written_type(
        db,
        &sources,
        source.source()
    )) else {
        return QueryPoll::Ready(Err(fault(
            "constant repeat requires its instantiated source result Type",
        )));
    };
    output.constant = Some(definition);
    output.written = Some(Arc::new(written));
    QueryPoll::Ready(Ok(output))
}
pub(crate) fn prepare_guarded(db: &mut Database, key: &PcKey) -> QueryPoll<PreparedArrayRepeat> {
    let query = QueryKey::ArrayRepeatPlan(key.clone());
    let raw = ready!(db.fetch(&SyntaxProvider, query.clone()));
    if !raw.cacheable
        || !matches!(raw.value.as_ref(), QueryValue::ArrayRepeatPlan(plan) if plan.key() == key)
    {
        return QueryPoll::Ready(Err(fault("repeat source dependencies are not retained")));
    }
    let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, raw.fingerprint));
    QueryPoll::Ready(Ok(PreparedArrayRepeat {
        raw: raw.value,
        guard: Arc::new(guard),
    }))
}
