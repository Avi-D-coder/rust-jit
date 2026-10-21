//! Shadow evaluation of the source-component rows (`LRR_SCR_SHADOW`): at the
//! first reach of an unsuffixed numeric literal, build the literal's component
//! rows and report them next to the static hint the legacy `Component` service
//! prepared. Diagnostic only: nothing here changes a hint, a value or control
//! flow, and it is off unless the variable is set.
use super::*;
use crate::{
    core::digest::{Digest, StableEncoder},
    frontend::binding_constraints::rows::{self, BinaryRowKind, ComponentRows, DeferredRow, RowFamily, RowTerm},
    ir::{op::LiteralKind, pc::PcKey},
    syntax::signatures::GenericKind,
    traits::{
        term::{GenericArg, InferVarKey, ParamKey, RegionTerm, TyTerm},
        upstream::{
            ReachedCallOutcome, ReachedSubject, poll_reached_root,
            root_input::{
                ComponentInput, ComponentSiteInput, DeferredRelation, DeferredRelationKind, SourceCallRelation,
                SourceCallRelationKind, SourceRootCallInput, SourceVariableFamily,
            },
        },
    },
};
use std::sync::{Mutex, OnceLock};

fn mode() -> Option<&'static str> {
    static MODE: OnceLock<Option<String>> = OnceLock::new();
    MODE.get_or_init(|| std::env::var("LRR_SCR_SHADOW").ok()).as_deref()
}

/// Literals a solved component already answered: its other literals share the
/// same component rows, hence the same pinned answer.
type Answers = std::collections::BTreeMap<(crate::core::digest::Digest, u32), Option<crate::ir::op::PrimitiveType>>;
fn answered() -> &'static Mutex<Answers> {
    static ANSWERED: OnceLock<Mutex<Answers>> = OnceLock::new();
    ANSWERED.get_or_init(|| Mutex::new(Answers::new()))
}

fn seen() -> &'static Mutex<std::collections::BTreeSet<(crate::core::digest::Digest, u32)>> {
    static SEEN: OnceLock<Mutex<std::collections::BTreeSet<(crate::core::digest::Digest, u32)>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(std::collections::BTreeSet::new()))
}

impl RunMachine {
    /// Build and report the component rows of the unsuffixed literal the
    /// resolved recipe is about to execute (once per source literal).
    pub(super) fn shadow_component(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> QueryPoll<()> {
        let Some(mode) = mode() else {
            return QueryPoll::Ready(Ok(()));
        };
        // Solving needs the impl catalog; discovery suspends, so request it
        // before the first operation runs (no prepared input is in flight).
        if (mode == "solve" || mode == "apply")
            && self.frame.counters.operations_started == 0
            && let Some(inventory) = self.impl_catalog.clone()
            && db.input(&crate::db::dependency::InputKey::DiscoverySeal(inventory.clone())).is_none()
        {
            return QueryPoll::NeedInput(crate::db::poll::InputRequest::Discover { key: inventory });
        }
        let literal = match self.installed.get_current(db, id) {
            Ok(recipe) => recipe
                .recipe()
                .operations
                .iter()
                .skip(self.frame.resume_operation_index())
                .find_map(|op| match op {
                    Op::Literal { literal, hint, destination }
                        if matches!(literal.kind, LiteralKind::Integer | LiteralKind::Float) =>
                    {
                        Some((literal.spelling.clone(), hint.clone(), *destination))
                    }
                    _ => None,
                }),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some((spelling, hint, destination)) = literal else {
            return QueryPoll::Ready(Ok(()));
        };
        let pc = self.frame.pc().clone();
        let key = PcKey {
            body: pc.body.clone(),
            node: pc.node,
            phase: 0,
        };
        // A component already answered (this literal or one sharing its rows):
        // the pinned answer applies at every reach, loops and calls included.
        let cached = answered()
            .lock()
            .ok()
            .and_then(|answers| answers.get(&(key.body.definition, key.node)).copied());
        if let Some(leaf) = cached {
            if mode == "apply" && matches!(hint, crate::ir::op::HintRecipe::None) {
                if let Err(error) = self.apply_leaf(destination, leaf) {
                    return QueryPoll::Ready(Err(error));
                }
            }
            return QueryPoll::Ready(Ok(()));
        }
        let first = seen()
            .lock()
            .map(|mut seen| seen.insert((key.body.definition, key.node)))
            .unwrap_or(false);
        if !first {
            return QueryPoll::Ready(Ok(()));
        }
        let rows = match rows::build(db, &key) {
            QueryPoll::Ready(Ok(Some(rows))) => rows,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(())),
            QueryPoll::Ready(Err(error)) => {
                eprintln!("SCR-SHADOW node={} lit={spelling} rows-error {}: {}", key.node, error.code, error.message);
                return QueryPoll::Ready(Ok(()));
            }
            QueryPoll::NeedInput(input) => {
                // Not recorded as seen: the rows are rebuilt after the host serves the input.
                if let Ok(mut seen) = seen().lock() {
                    seen.remove(&(key.body.definition, key.node));
                }
                return QueryPoll::NeedInput(input);
            }
        };
        let solved = if mode == "solve" || mode == "apply" {
            match self.solve_component(db, &key, &rows) {
                QueryPoll::Ready(Ok(answer)) => {
                    if let Ok(mut answers) = answered().lock() {
                        for (node, _) in &rows.literals {
                            answers.entry((key.body.definition, node.0)).or_insert(None);
                        }
                        for (node, leaf) in &answer.literals {
                            answers.insert((key.body.definition, node.0), *leaf);
                        }
                        answers.insert((key.body.definition, key.node), answer.leaf);
                    }
                    // `apply`: the pinned owner decides a literal the legacy
                    // service left without a static answer. The reached
                    // slot hint is the ordinary hint route; a legacy answer
                    // (a primitive or reached-sibling evidence) is never
                    // replaced, so the two can only be compared, not mixed.
                    if mode == "apply" && matches!(hint, crate::ir::op::HintRecipe::None) {
                        if let Err(error) = self.apply_leaf(destination, answer.leaf) {
                            return QueryPoll::Ready(Err(error));
                        }
                    }
                    Some(answer.text)
                }
                QueryPoll::Ready(Err(error)) => Some(format!("error {}: {}", error.code, error.message.chars().take(160).collect::<String>())),
                QueryPoll::NeedInput(input) => {
                    if let Ok(mut seen) = seen().lock() {
                        seen.remove(&(key.body.definition, key.node));
                    }
                    return QueryPoll::NeedInput(input);
                }
            }
        } else {
            None
        };
        report(mode, &spelling, &hint, &rows, solved.as_deref());
        QueryPoll::Ready(Ok(()))
    }

    fn apply_leaf(
        &mut self,
        destination: crate::ir::pc::SlotId,
        leaf: Option<crate::ir::op::PrimitiveType>,
    ) -> Result<(), Diagnostic> {
        match leaf {
            Some(primitive) => {
                let descriptor = crate::frontend::call_contracts::primitive_runtime_id(primitive)?;
                self.frame.set_hint(destination, Some(descriptor))
            }
            None => Ok(()),
        }
    }

    /// Run the rows through the pinned owner; the leaf literal's answer or a
    /// typed reason it has none.
    fn solve_component(&mut self, db: &mut Database, key: &PcKey, rows: &ComponentRows) -> QueryPoll<ScrAnswer> {
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let Some(inventory) = self.impl_catalog.clone() else {
            return QueryPoll::Ready(Ok(ScrAnswer::text("no-catalog")));
        };
        // Catalog discovery suspends the machine. Only the very first
        // operation may do that without disturbing in-flight prepared inputs
        // (a call evaluating its arguments); later literals wait for it.
        if db.input(&crate::db::dependency::InputKey::DiscoverySeal(inventory.clone())).is_none()
            && self.frame.counters.operations_started != 0
        {
            return QueryPoll::Ready(Ok(ScrAnswer::text("catalog-not-yet-published")));
        }
        let (caller, caller_environment, caller_callable) = self.reached_caller(key);
        let Some(caller_callable) = caller_callable else {
            return QueryPoll::Ready(Ok(ScrAnswer::text("no-caller-callable")));
        };
        let span = match self.frame.body.span(NodeId(key.node)) {
            Ok(span) => span,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let caller_source = db.input(&crate::db::dependency::InputKey::DefinitionSource(key.body.definition));
        let caller_body = db.input(&crate::db::dependency::InputKey::DefinitionBody(key.body.definition));
        let site = match self.reached_source_site(&span, caller_source.as_deref(), caller_body.as_deref()) {
            Ok(Some(site)) => site,
            Ok(None) => return QueryPoll::Ready(Ok(ScrAnswer::text("no-source-site"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // Variables, scoped by the leaf's entry.
        let transaction = rows.transaction;
        let var = |ordinal: u32| InferVarKey { transaction, ordinal };
        let mut variables = BTreeMap::new();
        for (ordinal, family) in rows.vars.iter().enumerate() {
            variables.insert(
                var(ordinal as u32),
                match family {
                    RowFamily::Type => SourceVariableFamily::Type,
                    RowFamily::Integer => SourceVariableFamily::Integer,
                    RowFamily::Float => SourceVariableFamily::Float,
                },
            );
        }
        fn term(row: &RowTerm, var: &dyn Fn(u32) -> InferVarKey) -> TyTerm {
            match row {
                RowTerm::Var(ordinal) => TyTerm::Infer(var(*ordinal)),
                RowTerm::Closed(term) => term.clone(),
                RowTerm::Tuple(members) => TyTerm::Tuple(members.iter().map(|member| term(member, var)).collect()),
                RowTerm::Ref { mutable, inner } => {
                    TyTerm::Ref(RegionTerm::Static, *mutable, Box::new(term(inner, var)))
                }
            }
        }
        let relations: Vec<SourceCallRelation> = rows
            .equations
            .iter()
            .map(|row| SourceCallRelation {
                formal: term(&row.left, &var),
                actual: term(&row.right, &var),
                kind: SourceCallRelationKind::Equal,
                site,
            })
            .collect();
        let deferred: Vec<DeferredRelation> = rows
            .deferred
            .iter()
            .map(|row| match row {
                DeferredRow::Deref { operand, result, .. } => DeferredRelation {
                    kind: DeferredRelationKind::Deref,
                    first: term(operand, &var),
                    second: term(operand, &var),
                    result: term(result, &var),
                    site,
                },
                DeferredRow::Binary { kind, lhs, rhs, result, .. } => DeferredRelation {
                    kind: match kind {
                        BinaryRowKind::Arithmetic => DeferredRelationKind::Arithmetic,
                        BinaryRowKind::Comparison => DeferredRelationKind::Comparison,
                    },
                    first: term(lhs, &var),
                    second: term(rhs, &var),
                    result: term(result, &var),
                    site,
                },
            })
            .collect();
        let mut sites = Vec::new();
        for row in &rows.sites {
            let mut fingerprint = StableEncoder::new("lrr/v1/component-site");
            fingerprint.digest(row.callee);
            fingerprint.u32(row.node.0);
            sites.push(ComponentSiteInput {
                callee: row.callee,
                signature: fingerprint.finish(),
                parameters: row.generics.iter().map(|(key, ordinal)| (*key, TyTerm::Infer(var(*ordinal)))).collect(),
            });
        }
        let literals: Vec<InferVarKey> = rows.literals.iter().map(|(_, ordinal)| var(*ordinal)).collect();
        // The component's owner is the caller body: its own generics are rigid.
        let mut parameters = BTreeMap::new();
        if let Some(environment) = &caller_environment {
            for generic in &environment.definition().generics {
                if generic.kind != GenericKind::Type {
                    continue;
                }
                let parameter = ParamKey {
                    definition: environment.definition().definition,
                    ordinal: generic.ordinal,
                };
                parameters.insert(parameter, TyTerm::Param(parameter));
            }
        }
        let mut digest = StableEncoder::new("lrr/v1/component-rows");
        digest.record(key);
        digest.digest(rows.structure);
        for relation in &relations {
            digest.record(&relation.formal);
            digest.record(&relation.actual);
        }
        for row in &deferred {
            digest.u8(row.kind as u8);
            digest.record(&row.first);
            digest.record(&row.second);
            digest.record(&row.result);
        }
        for site in &sites {
            digest.digest(site.callee);
            digest.digest(site.signature);
        }
        let fingerprint = digest.finish();
        let extras = ComponentInput {
            deferred: &deferred,
            sites: &sites,
            literals: &literals,
            closed: rows.holes.is_empty(),
        };
        let packet = SourceRootCallInput {
            callee: key.body.definition,
            signature: fingerprint,
            parameters: &parameters,
            variables: &variables,
            relations: &relations,
            caller: &caller,
            component: Some(&extras),
        };
        let subject = ReachedSubject::Component(key.body.definition, fingerprint);
        let outcome = ready!(poll_reached_root(
            db,
            &inventory,
            &packet,
            caller_environment.as_ref(),
            &caller_callable,
            &subject,
        ));
        let leaf = var(rows.leaf);
        QueryPoll::Ready(Ok(match outcome {
            ReachedCallOutcome::Solved(response) => {
                let mut primitive = None;
                let answer = match response.values.get(&leaf) {
                    Some(TyTerm::Primitive(found)) => {
                        primitive = Some(*found);
                        format!("{found:?}")
                    }
                    Some(TyTerm::Infer(_)) => "open".to_owned(),
                    Some(other) => format!("{other:?}").chars().take(40).collect(),
                    None => "absent".to_owned(),
                };
                let literals = rows
                    .literals
                    .iter()
                    .map(|(node, ordinal)| {
                        let found = match response.values.get(&var(*ordinal)) {
                            Some(TyTerm::Primitive(found)) => Some(*found),
                            _ => None,
                        };
                        (*node, found)
                    })
                    .collect();
                ScrAnswer {
                    leaf: primitive,
                    literals,
                    text: format!("{answer} closed={}", response.closed),
                }
            }
            ReachedCallOutcome::Held(hold) => ScrAnswer::text(format!(
                "held {}: {}",
                hold.stage,
                hold.reason.chars().take(120).collect::<String>()
            )),
            ReachedCallOutcome::Failed(failures) => ScrAnswer::text(format!(
                "failed {}",
                failures.first().map(|failure| failure.predicate.chars().take(120).collect::<String>()).unwrap_or_default()
            )),
            ReachedCallOutcome::Overflow(_) => ScrAnswer::text("overflow"),
        }))
    }
}

/// The pinned owner's answer for a component's leaf literal.
struct ScrAnswer {
    leaf: Option<crate::ir::op::PrimitiveType>,
    /// Every unsuffixed literal of the component and its answer.
    literals: Vec<(NodeId, Option<crate::ir::op::PrimitiveType>)>,
    text: String,
}
impl ScrAnswer {
    fn text(text: impl Into<String>) -> Self {
        Self { leaf: None, literals: Vec::new(), text: text.into() }
    }
}

fn report(_mode: &str, spelling: &str, hint: &crate::ir::op::HintRecipe, rows: &ComponentRows, solved: Option<&str>) {
    let holes: Vec<&str> = {
        let mut why: Vec<&str> = rows.holes.iter().map(|hole| hole.why.as_str()).collect();
        why.sort_unstable();
        why.dedup();
        why
    };
    let legacy = match hint {
        crate::ir::op::HintRecipe::Primitive(primitive) => format!("{primitive:?}"),
        crate::ir::op::HintRecipe::RelatedNumeric(_) => "related".to_owned(),
        crate::ir::op::HintRecipe::None => "none".to_owned(),
        other => format!("{other:?}").chars().take(20).collect(),
    };
    eprintln!(
        "SCR-SHADOW node={} lit={spelling} legacy={legacy} vars={} eq={} deferred={} sites={} literals={} holes={}{}",
        rows.key.node,
        rows.vars.len(),
        rows.equations.len(),
        rows.deferred.len(),
        rows.sites.len(),
        rows.literals.len(),
        holes.len(),
        if holes.is_empty() { String::new() } else { format!(" [{}]", holes.join("; ")) },
    );
    if let Some(solved) = solved {
        eprintln!("SCR-SOLVE node={} lit={spelling} legacy={legacy} scr={solved}", rows.key.node);
    }
}
