//! Coordinator-only current source/query preparation; no guest execution here.
use super::*;
use crate::{
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        memo::Outcome,
        seals::SealDomain,
    },
    frontend::{
        constant_construction::CurrentConstantConstruction,
        constant_path_selection::{ConstantPathOutcome, ConstantPathSelection},
        constant_result_type::CurrentConstantResultType,
        constant_source_admission::ConstantSourceAdmission,
        constants::{
            ConstLimits, query::fetch_constant_preparing_macros, query_value::ConstantQueryValue,
            wire::ConstantWireType,
        },
        names::NameLookup,
    },
    ir::{
        recipe::{Origin, RecipeDraft, RecipeExit, VerifiedRecipe},
        source_constant::SourceConstantInstruction,
    },
    source::manifest::{ItemKind, Namespace},
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        spans::SourceRange,
    },
};
const LIMIT: usize = 32 << 20;
pub(super) struct SourceConstantPlan {
    caller: usize,
    target: usize,
    path: ConstantPathSelection,
    result: CurrentConstantResultType,
    child: Option<(ConstantSourceAdmission, Outcome)>,
    outcome: Outcome,
    recipe: VerifiedRecipe,
    bytes: usize,
}
fn current_value(outcome: &Outcome) -> Result<&ConstantQueryValue, Diagnostic> {
    match outcome.value.as_ref() {
        QueryValue::Constant(v) => Ok(v),
        QueryValue::ConstantFailure(v) => Err(v.diagnostic().clone()),
        QueryValue::Error(e) => Err(e.clone()),
        _ => Err(call_fault("constant coordinator query variant differs")),
    }
}
impl SourceConstantPlan {
    #[cfg(test)]
    pub(super) fn query_key(&self) -> &crate::db::key::ConstantKey {
        match self.outcome.value.as_ref() {
            QueryValue::Constant(value) => value.key(),
            _ => panic!("actual prepared source constant has no successful parent memo"),
        }
    }
    pub(super) fn construction<'a>(
        &'a self,
        db: &'a Database,
        sources: &'a [PreparedSourceView],
    ) -> Result<CurrentConstantConstruction<'a>, Diagnostic> {
        let source = sources
            .get(self.target)
            .ok_or_else(|| call_fault("constant target source index absent"))?;
        let caller = sources
            .get(self.caller)
            .ok_or_else(|| call_fault("constant caller source index absent"))?;
        let views = sources.iter().collect::<Vec<_>>();
        self.path.revalidate(db, caller, &views)?;
        CurrentConstantConstruction::capture_current(
            db,
            source,
            &self.result,
            current_value(&self.outcome)?,
            self.child
                .as_ref()
                .map(|(proof, outcome)| current_value(outcome).map(|value| (proof, value)))
                .transpose()?,
            LIMIT,
        )
    }
    pub(super) fn proof<'a>(
        &'a self,
        construction: &'a CurrentConstantConstruction<'a>,
        frame: &Frame,
        store: &mut GuestStore,
        sources: &'a [PreparedSourceView],
        views: &'a [&'a PreparedSourceView],
    ) -> Result<crate::engine::constant_array_publication::AdmittedConstantUse<'a>, Diagnostic>
    {
        let binding = construction.bind_guest_descriptor(store)?;
        crate::engine::constant_array_publication::AdmittedConstantUse::capture_source_for_dispatch(
            frame,
            self.recipe
                .recipe()
                .operations
                .iter()
                .find_map(|op| match op {
                    Op::SourceConstant { destination, .. } => Some(*destination),
                    _ => None,
                })
                .ok_or_else(|| call_fault("constant instruction absent"))?,
            &sources[self.caller],
            views,
            &self.path,
            binding,
            store,
        )
    }
}
impl RunMachine {
    pub(super) fn fail_source_constant(&mut self, error: Diagnostic) -> RunPoll {
        if let Err(error) = self.frame.abort_source_constant_op(&mut self.store, error) {
            self.terminal = true;
            return RunPoll::Fault(error);
        }
        loop {
            match self.frame.drain_source_constant_op(&mut self.store) {
                Ok(false) => continue,
                Ok(true) => {
                    self.terminal = true;
                    return RunPoll::Fault(call_fault("constant drain lost its failure"));
                }
                Err(error) => {
                    self.terminal = true;
                    return RunPoll::Fault(error);
                }
            }
        }
    }
    /// Demand a Type seal only from the exact actual Value seal's selected world.
    /// Never synthesize a world identity or execute preparation inside a provider.
    fn required_constant_type(
        db: &Database,
        target: &PreparedSourceView,
        path: &ConstantPathSelection,
    ) -> Result<Option<InputRequest>, Diagnostic> {
        let input = db.source_view_scope_certificate(target.selection(), &path.context().scope)?;
        let InputValue::DiscoverySeal(value) = input.as_ref() else {
            return Err(call_fault(
                "constant target Value certificate variant differs",
            ));
        };
        let mut key = value.seal.key().clone();
        key.domain = SealDomain::Scope {
            id: path.context().scope.fingerprint(),
            namespace: Namespace::Type,
        };
        Ok(db
            .published_input(&InputKey::DiscoverySeal(key.clone()))
            .is_none()
            .then_some(InputRequest::Discover { key }))
    }
    pub(super) fn prepare_source_constant_recipe(
        &mut self,
        db: &mut Database,
    ) -> QueryPoll<Option<VerifiedRecipe>> {
        let pc = self.frame.pc().clone();
        if pc.phase != Phase::Entry.code().unwrap_or(u16::MAX)
            || !self
                .frame
                .body
                .nodes()
                .get(pc.node as usize)
                .is_some_and(|n| n.role == NodeRole::Expression(ExpressionKind::Path))
        {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(graph) = self.graph.as_ref() else {
            return QueryPoll::Ready(Ok(None));
        };
        let owners = self
            .sources
            .iter()
            .enumerate()
            .filter_map(|(ordinal, source)| {
                source
                    .outline()
                    .items
                    .iter()
                    .filter_map(|item| item.body.as_ref())
                    .find(|body| body.key() == pc.body)
                    .map(|body| (ordinal, body))
            })
            .collect::<Vec<_>>();
        let [(caller, body)] = owners.as_slice() else {
            return if owners.is_empty() {
                QueryPoll::Ready(Ok(None))
            } else {
                QueryPoll::Ready(Err(call_fault("constant caller source body is duplicated")))
            };
        };
        let caller = *caller;
        let prepare = || -> Result<Option<ConstantPathSelection>, Diagnostic> {
            let source = &self.sources[caller];
            let parsed = db
                .prepared_parse(&source.selection().outline.parse)
                .ok_or_else(|| call_fault("constant source parse is not actually prepared"))?;
            let span = self.frame.body.span(NodeId(pc.node))?;
            let range = SourceRange {
                start: body
                    .range
                    .start
                    .checked_add(span.relative_start)
                    .ok_or_else(|| call_fault("constant source start overflow"))?,
                end: body
                    .range
                    .start
                    .checked_add(span.relative_end)
                    .ok_or_else(|| call_fault("constant source end overflow"))?,
            };
            let Some(site) = parsed.constant_path_route(source, body, range)? else {
                return Ok(None);
            };
            // Routing only. Real source/local/namespace admission follows below;
            // this graph never grants execution or overrides a local shadow.
            if !matches!(graph.lookup_source_path(Namespace::Value,site.segments(),site.absolute(),&site.context().scope,
                source.selection().outline.parse.profile.edition)?,NameLookup::Defined(ref def) if self.sources.iter().any(|v|v.outline().items.iter().any(|item|item.def_key.fingerprint()==def.identity && item.def_key.item_kind==ItemKind::Constant)))
            {
                return Ok(None);
            }
            // The constant preparation bound belongs to this reached candidate,
            // not to unrelated execution in a large captured source world.
            if self.sources.len() > 256 {
                return Err(call_fault("constant source set exceeds bounded profile"));
            }
            let views = self.sources.iter().collect::<Vec<_>>();
            match ConstantPathSelection::capture_current(db, source, body, range, &views, LIMIT)? {
                ConstantPathOutcome::Selected(path) => Ok(Some(*path)),
                ConstantPathOutcome::LocalBinding(_) | ConstantPathOutcome::OtherValueMember(_) => {
                    Ok(None)
                }
                other => Err(other.prerequisite_error()),
            }
        };
        let path = match prepare() {
            Ok(Some(path)) => path,
            Ok(None) => return QueryPoll::Ready(Ok(None)),
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let Some(target) = self
            .sources
            .iter()
            .position(|view| view.selection() == path.target())
        else {
            return QueryPoll::Ready(Err(call_fault("selected constant target absent")));
        };
        match Self::required_constant_type(db, &self.sources[target], &path) {
            Ok(Some(input)) => return QueryPoll::NeedInput(input),
            Ok(None) => {}
            Err(e) => return QueryPoll::Ready(Err(e)),
        }
        let views = self.sources.iter().collect::<Vec<_>>();
        let result = match ConstantSourceAdmission::capture_current_in_sources(
            db,
            &self.sources[target],
            path.definition(),
            path.request(),
            LIMIT,
            &views,
        )
        .and_then(|a| {
            CurrentConstantResultType::capture_current(db, &self.sources[target], a, LIMIT)
        }) {
            Ok(result) => result,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let mut child = None;
        let wire = if let Some(scalar) = result.scalar() {
            scalar.wire_type()
        } else if let Some((element, demand)) = result.array() {
            let proof = match result.admission().reselect_array_length_current(
                db,
                &self.sources[target],
                demand,
                LIMIT,
            ) {
                Ok(proof) => proof,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            let (proof, outcome) = match fetch_constant_preparing_macros(
                db,
                &self.sources[target],
                proof,
                &demand.expected_wire(),
                ConstLimits::default(),
                1 << 20,
            ) {
                QueryPoll::Ready(Ok(outcome)) => outcome,
                QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
            };
            let length = match current_value(&outcome)
                .and_then(|v| v.payload().decode_value())
                .and_then(|v| {
                    v.scalar()
                        .ok_or_else(|| call_fault("constant source length is not scalar"))
                }) {
                Ok(v) if v.kind == 5 && v.type_id == 13 => v.payload,
                Ok(_) => {
                    return QueryPoll::Ready(Err(call_fault(
                        "constant source length is not usize",
                    )));
                }
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            child = Some((proof, outcome));
            ConstantWireType::Array {
                element: Box::new(element.wire_type()),
                length,
            }
        } else {
            return QueryPoll::Ready(Err(call_fault("constant source result readiness absent")));
        };
        let (admission, outcome) = match fetch_constant_preparing_macros(
            db,
            &self.sources[target],
            result.into_admission(),
            &wire,
            ConstLimits::default(),
            1 << 20,
        ) {
            QueryPoll::Ready(Ok(o)) => o,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        };
        let result = match CurrentConstantResultType::capture_current(
            db,
            &self.sources[target],
            admission,
            LIMIT,
        ) {
            Ok(result) => result,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let finish = || -> Result<SourceConstantPlan, Diagnostic> {
            let parent = current_value(&outcome)?;
            let joined = CurrentConstantConstruction::capture_current(
                db,
                &self.sources[target],
                &result,
                parent,
                child
                    .as_ref()
                    .map(|(p, o)| current_value(o).map(|v| (p, v)))
                    .transpose()?,
                LIMIT,
            )?;
            path.revalidate(db, &self.sources[caller], &views)?;
            let value = parent.payload().decode_value()?;
            let query = QueryKey::Constant(parent.key().clone());
            let memo = db
                .memo(&query)
                .ok_or_else(|| call_fault("prepared constant actual memo absent"))?;
            let capacity = memo
                .dependencies
                .len()
                .checked_add(path.dependencies().len())
                .and_then(|n| n.checked_add(1))
                .filter(|n| *n <= 100_000)
                .ok_or_else(crate::runtime::arena::resource)?;
            let mut dependencies = Vec::new();
            dependencies
                .try_reserve_exact(capacity)
                .map_err(|_| crate::runtime::arena::resource())?;
            dependencies.extend(memo.dependencies.iter().cloned());
            // The constant memo describes evaluation; selecting its source path
            // also used the actual caller namespace and publication inputs.
            // Retain those current facts as data, never execution permission.
            dependencies.extend(path.dependencies().iter().map(|(key, fingerprint)| {
                DependencyStamp::Input {
                    key: key.clone(),
                    fingerprint: *fingerprint,
                }
            }));
            dependencies.push(DependencyStamp::Query {
                key: query,
                fingerprint: memo.fingerprint,
            });
            dependencies.sort_unstable();
            dependencies.dedup();

            let instruction = SourceConstantInstruction::capture_data(
                &self.frame.body,
                &pc,
                parent.key(),
                path.identity(),
                joined.wire(),
                &value,
                dependencies,
            )?;
            let node = &self.frame.body.nodes()[pc.node as usize];
            let mut operations = Vec::new();
            if let Some(scope) = node.boolean_scope.or(node.statement_scope) {
                operations.push(Op::ScopeEnter { scope });
            }
            let destination = instruction.destination();
            let next = instruction.next().clone();
            let dependencies = instruction.dependencies().to_vec();
            operations.push(Op::SourceConstant {
                constant: Box::new(instruction),
                destination,
            });
            let recipe = crate::ir::validate::verify(
                RecipeDraft {
                    key: pc.clone(),
                    origin: Origin {
                        span: self.frame.body.span(node.id)?,
                        expansion_ancestry: vec![],
                    },
                    operations,
                    exits: vec![RecipeExit::Next(next)],
                    dependencies,
                },
                &self.frame.body,
            )?;
            let bytes = path
                .metadata_bytes()
                .checked_add(result.metadata_bytes())
                .and_then(|n| n.checked_add(parent.metadata_bytes().ok()?))
                .and_then(|n| n.checked_add(recipe.metadata_bytes()))
                .and_then(|n| {
                    n.checked_add(child.as_ref().map_or(Some(0), |(p, o)| {
                        p.metadata_bytes()
                            .checked_add(current_value(o).ok()?.metadata_bytes().ok()?)
                    })?)
                })
                .and_then(|n| n.checked_add(std::mem::size_of::<SourceConstantPlan>() + 256))
                .filter(|n| *n <= LIMIT)
                .ok_or_else(crate::runtime::arena::resource)?;
            drop(joined);
            Ok(SourceConstantPlan {
                caller,
                target,
                path,
                result,
                child,
                outcome,
                recipe,
                bytes,
            })
        };
        let plan = match finish() {
            Ok(plan) => plan,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let bytes = match self
            .constant_plan_bytes
            .checked_add(plan.bytes)
            .filter(|n| *n <= LIMIT && self.constant_plans.len() < 4096)
        {
            Some(b) => b,
            None => return QueryPoll::Ready(Err(crate::runtime::arena::resource())),
        };
        let recipe = plan.recipe.clone();
        self.constant_plans.insert(pc, plan);
        self.constant_plan_bytes = bytes;
        QueryPoll::Ready(Ok(Some(recipe)))
    }
}
