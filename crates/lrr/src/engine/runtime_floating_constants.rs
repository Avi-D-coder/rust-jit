//! Pure scalar source demand within the exact published runtime source index.
use super::RuntimeMacroContext;
use crate::{
    core::diagnostic::Diagnostic,
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    frontend::{
        call_contracts::ArgumentType,
        names::NameNamespaces,
        primitive_constants::{
            FloatingConstantOutcome, FloatingConstantRecipe, FloatingConstantSite,
            plan_floating_constant,
        },
        runtime_macro_plan::RuntimeNoItemProof,
        source_locals::SourceLocalIndex,
    },
    ir::pc::{PcKey, Phase},
    syntax::body_index::{BodyIndex, ExpressionKind, NodeRole},
};
#[derive(Clone, Copy)]
pub(crate) struct FloatingFrameSite<'a> {
    pub body: &'a BodyIndex,
    pub pc: &'a PcKey,
    // Demand context only after an actual source path selects this service.
    // Ordinary reference/local paths must be free to use ordinary lowering.
    pub expected: &'a dyn Fn() -> Result<Option<ArgumentType>, Diagnostic>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
impl RuntimeMacroContext<'_> {
    pub(crate) fn prepare_floating_constant<'p>(
        &self,
        db: &mut Database,
        graph: &NameNamespaces,
        frame: FloatingFrameSite<'_>,
        proofs: impl Iterator<Item = &'p RuntimeNoItemProof>,
    ) -> QueryPoll<Option<FloatingConstantRecipe>> {
        self.with_floating_site(
            db,
            frame,
            proofs,
            |db, locals, site| match plan_floating_constant(db, graph, locals, site) {
                QueryPoll::Ready(Ok(FloatingConstantOutcome::Ready(plan))) => {
                    plan.lower_recipe(db, graph, locals, site, frame.body, frame.pc)
                }
                QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(fault(
                    "reached floating constant requires unresolved source type integration",
                ))),
                QueryPoll::Ready(Err(e)) => QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => QueryPoll::NeedInput(i),
            },
        )
    }
    pub(crate) fn revalidate_floating_constant<'p>(
        &self,
        db: &mut Database,
        graph: &NameNamespaces,
        frame: FloatingFrameSite<'_>,
        proofs: impl Iterator<Item = &'p RuntimeNoItemProof>,
        retained: &FloatingConstantRecipe,
    ) -> QueryPoll<Option<()>> {
        self.with_floating_site(db, frame, proofs, |db, locals, site| {
            retained.revalidate(db, graph, locals, site, frame.body, frame.pc)
        })
    }
    fn with_floating_site<'p, T>(
        &self,
        db: &mut Database,
        frame: FloatingFrameSite<'_>,
        proofs: impl Iterator<Item = &'p RuntimeNoItemProof>,
        action: impl FnOnce(
            &mut Database,
            &SourceLocalIndex<'_>,
            FloatingConstantSite<'_>,
        ) -> QueryPoll<T>,
    ) -> QueryPoll<Option<T>> {
        let prepare = || -> Result<_, Diagnostic> {
            let Some(owner) = self
                .source
                .outline()
                .items
                .iter()
                .filter_map(|item| item.body.as_ref())
                .find(|owner| owner.key() == frame.pc.body)
            else {
                return Ok(None);
            };
            if frame.body.body() != &frame.pc.body || frame.pc.phase != Phase::Entry.code()? {
                return Ok(None);
            }
            let node = frame
                .body
                .nodes()
                .get(frame.pc.node as usize)
                .ok_or_else(|| fault("floating dispatch source node absent"))?;
            if node.role != NodeRole::Expression(ExpressionKind::Path) {
                return Ok(None);
            }
            let span = frame.body.span(node.id)?;
            let start = owner
                .range
                .start
                .checked_add(span.relative_start)
                .ok_or_else(|| fault("floating dispatch start overflow"))?;
            let end = owner
                .range
                .start
                .checked_add(span.relative_end)
                .ok_or_else(|| fault("floating dispatch end overflow"))?;
            Ok(Some((owner, span, start, end)))
        };
        let (owner, span, start, end) = match prepare() {
            Ok(Some(s)) => s,
            Ok(None) => return QueryPoll::Ready(Ok(None)),
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let parsed = match db.fetch(
            &SyntaxProvider,
            QueryKey::ParseFile(self.source.selection().outline.parse.clone()),
        ) {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("floating dispatch parse variant differs")));
        };
        let written =
            match crate::syntax::floating_paths::qualified_floating_path(parsed, owner, start, end)
            {
                Ok(Some(path)) => path,
                Ok(None) => return QueryPoll::Ready(Ok(None)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let expected = match (frame.expected)() {
            Ok(expected) => expected,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut entered = match self
            .index
            .enter_body(self.source.selection(), &frame.pc.body)
        {
            Ok(e) => e,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        for proof in proofs.filter(|p| p.body() == &frame.pc.body) {
            if let Err(e) = entered.accept_no_item_proof(
                self.source.selection(),
                &frame.pc.body,
                proof,
                self.catalog,
            ) {
                return QueryPoll::Ready(Err(e));
            }
        }
        // Unresolved value-pattern facts remain unresolved. Primitive qualification
        // consults the Type namespace and never invents a binding disposition.
        let locals = match entered.local_index(
            self.source.selection(),
            &frame.pc.body,
            &Default::default(),
            &[],
        ) {
            Ok(l) => l,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let site = FloatingConstantSite {
            source: self.source,
            body: &frame.pc.body,
            at: u64::from(start),
            span: &span,
            qualifier: &written.qualifier,
            member: &written.member,
            context: locals.context(),
            expected: expected.as_ref(),
        };
        match action(db, &locals, site) {
            QueryPoll::Ready(Ok(value)) => QueryPoll::Ready(Ok(Some(value))),
            QueryPoll::Ready(Err(e)) => QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => QueryPoll::NeedInput(i),
        }
    }
}
