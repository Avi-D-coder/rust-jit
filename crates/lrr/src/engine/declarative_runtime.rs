//! Runtime use of already sealed original declarative expression records.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{memo::Database, poll::QueryPoll},
    expand::{
        identity::{GrammarSlot, MacroIdentity},
        record::{ExpansionDescriptor, ExpansionRecord},
    },
    frontend::runtime_macro_plan::PreparedDeclarativeExpression,
    ir::pc::PcKey,
    source::changes::PreparedSourceView,
    syntax::body_index::BodyIndex,
};
use std::{collections::BTreeMap, sync::Arc};
type Site = (Digest, u32, u32);
#[derive(Default)]
pub(super) struct DeclarativeRuntimePlans {
    sites: BTreeMap<Site, ExpansionDescriptor>,
    plans: BTreeMap<PcKey, PreparedDeclarativeExpression>,
}
fn invalid() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-EXPANSION-INPUT",
        "sealed declarative runtime source attachment differs".into(),
    )
}
fn source_identity(source: &PreparedSourceView) -> Digest {
    let selection = source.selection();
    let mut encoder = StableEncoder::new("lrr/v2/declarative-runtime-source-site");
    encoder.string(selection.path.as_str());
    encoder.record(&crate::db::key::QueryKey::Outline(
        selection.outline.clone(),
    ));
    encoder.finish()
}
impl DeclarativeRuntimePlans {
    pub(super) fn from_sealed(
        records: &[Arc<ExpansionRecord>],
        sources: &[PreparedSourceView],
    ) -> Result<Self, Diagnostic> {
        let mut wanted = BTreeMap::new();
        for record in records.iter().filter(|record| {
            matches!(
                record.key().grammar,
                GrammarSlot::Expression | GrammarSlot::Statements
            ) && matches!(
                record.key().macro_identity,
                MacroIdentity::Declaration { .. }
            )
        }) {
            let descriptor = record.descriptor();
            if let Some(prior) = wanted.insert(descriptor.key.invocation, descriptor.clone())
                && prior.fingerprint() != descriptor.fingerprint()
            {
                return Err(invalid());
            }
        }
        let mut this = Self::default();
        if wanted.is_empty() {
            return Ok(this);
        }
        for source in sources {
            let scan = source.declaration_inventory(Default::default())?;
            for producer in scan.producers() {
                let Some(descriptor) = wanted.get(&producer.identity) else {
                    continue;
                };
                if producer.grammar != descriptor.key.grammar {
                    return Err(invalid());
                }
                let site = (
                    source_identity(source),
                    producer.range.start,
                    producer.range.end,
                );
                if let Some(prior) = this.sites.get(&site) {
                    if prior.fingerprint() != descriptor.fingerprint() {
                        return Err(invalid());
                    }
                    continue;
                }
                this.sites.insert(site, descriptor.clone());
            }
        }
        Ok(this)
    }
    pub(super) fn plans(&self) -> impl Iterator<Item = &PreparedDeclarativeExpression> {
        self.plans.values()
    }
    pub(super) fn prepare(
        &mut self,
        db: &mut Database,
        sources: &[PreparedSourceView],
        body: &BodyIndex,
        pc: &PcKey,
    ) -> QueryPoll<Option<PreparedDeclarativeExpression>> {
        if pc.phase != 0 {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(node) = body.nodes().get(pc.node as usize) else {
            return QueryPoll::Ready(Err(invalid()));
        };
        let selected = sources.iter().find_map(|source| {
            let handle = source
                .outline()
                .items
                .iter()
                .filter_map(|item| item.body.as_ref())
                .find(|handle| handle.key() == pc.body)?;
            let Some(start) = handle.range.start.checked_add(node.range.start) else {
                return Some(Err(invalid()));
            };
            let Some(end) = handle.range.start.checked_add(node.range.end) else {
                return Some(Err(invalid()));
            };
            let descriptor = self.sites.get(&(source_identity(source), start, end))?;
            Some(Ok((source, descriptor)))
        });
        let (source, descriptor) = match selected {
            Some(Ok(selected)) => selected,
            Some(Err(error)) => return QueryPoll::Ready(Err(error)),
            None => return QueryPoll::Ready(Ok(None)),
        };
        let plan = match super::runtime_macros::prepare_declarative_expression(
            db, source, body, pc, descriptor,
        ) {
            QueryPoll::Ready(Ok(plan)) => plan,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        if self.plans.contains_key(pc) {
            return QueryPoll::Ready(Err(invalid()));
        }
        // Retain nothing until the ordinary installed-recipe table accepts it.
        // A failed installation must leave no unused observer witness.
        QueryPoll::Ready(Ok(Some(plan)))
    }
    pub(super) fn retain(&mut self, plan: PreparedDeclarativeExpression) -> Result<(), Diagnostic> {
        let pc = plan.recipe().recipe().key.clone();
        if self.plans.contains_key(&pc) {
            return Err(invalid());
        }
        self.plans.insert(pc, plan);
        Ok(())
    }
}
