//! Coordinator-only current source/query preparation; no guest execution here.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        memo::Outcome,
        seals::SealDomain,
    },
    frontend::{
        constant_construction::CurrentConstantConstruction,
        constant_path_selection::ConstantPathSelection,
        constant_result_type::CurrentConstantResultType,
        constant_source_admission::ConstantSourceAdmission,
        constants::{
            ConstLimits, query::fetch_constant_preparing_macros, query_value::ConstantQueryValue,
        },
    },
    ir::recipe::VerifiedRecipe,
    source::manifest::{ItemKind, Namespace},
};
// Only the superseded `SourceConstant` preparation (regression models) uses these.
#[cfg(test)]
use crate::{
    db::dependency::DependencyStamp,
    frontend::{
        constant_path_selection::ConstantPathOutcome, constants::wire::ConstantWireType,
        names::NameLookup,
    },
    ir::{
        recipe::{Origin, RecipeDraft, RecipeExit},
        source_constant::SourceConstantInstruction,
    },
    syntax::{
        body_index::{ExpressionKind, NodeRole},
        spans::SourceRange,
    },
};
// E-R5-006: only satisfies byte-budget parameters still required by the
// frontend constant APIs; remove with those parameters. No run-local cap.
const LIMIT: usize = usize::MAX;
pub(super) struct SourceConstantPlan {
    caller: usize,
    target: usize,
    path: ConstantPathSelection,
    result: CurrentConstantResultType,
    child: Option<(ConstantSourceAdmission, Outcome)>,
    outcome: Outcome,
    recipe: VerifiedRecipe,
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
    #[cfg(test)]
    fn required_constant_type(
        db: &Database,
        target: &PreparedSourceView,
        path: &ConstantPathSelection,
    ) -> Result<Option<InputRequest>, Diagnostic> {
        if db
            .source_view_type_scope_certificate(target.selection(), &path.context().scope)?
            .is_some()
        {
            return Ok(None);
        }
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
    /// Superseded legacy route, retained only for its historical regression
    /// models: production dispatch reads every constant path as an ordinary
    /// `ValuePath` (`constant_item_data`), whatever body it is written in.
    #[cfg(test)]
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
                usize::MAX,
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
            usize::MAX,
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
            drop(joined);
            Ok(SourceConstantPlan {
                caller,
                target,
                path,
                result,
                child,
                outcome,
                recipe,
            })
        };
        let plan = match finish() {
            Ok(plan) => plan,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let recipe = plan.recipe.clone();
        self.constant_plans.insert(pc, plan);
        QueryPoll::Ready(Ok(Some(recipe)))
    }
}

impl RunMachine {
    /// The evaluated scalar value of a constant item named by a pattern
    /// (constant patterns compare scalars only).
    pub(super) fn constant_item_value(
        &mut self,
        db: &mut Database,
        definition: crate::core::digest::Digest,
    ) -> QueryPoll<crate::runtime::value::ValueRepr> {
        match self.constant_item_data(db, definition) {
            QueryPoll::Ready(Ok((value, _))) => QueryPoll::Ready(value.scalar().ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "non-scalar constant pattern requires structural constant integration".into(),
                )
            })),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// The evaluated value of a constant (or a static's initializer) and its
    /// selected wire layout, through the one pure constant admission and
    /// evaluation route. Value paths, statics and patterns share it; the
    /// dispatcher prepares it once per PC (statics once per run).
    pub(super) fn constant_item_data(
        &mut self,
        db: &mut Database,
        definition: crate::core::digest::Digest,
    ) -> QueryPoll<(
        crate::ir::constant_data::ConstValue,
        crate::ir::constant_data::wire::ConstantWireType,
    )> {
        let unsupported = |message: &str| {
            QueryPoll::Ready(Err(Diagnostic::new("LRR-E-UNSUPPORTED", message.into())))
        };
        // A module item of a catalogued source, or a body-local item whose
        // owning body the coordinator published with its reached plan.
        let mut local = None;
        let target = match self.sources.iter().position(|view| {
            view.outline().items.iter().any(|item| {
                item.def_key.fingerprint() == definition
                    && matches!(item.def_key.item_kind, ItemKind::Constant | ItemKind::Static)
            })
        }) {
            Some(target) => target,
            None => {
                let owner = match db
                    .input(&InputKey::LocalDeclarationOwner(definition))
                    .as_deref()
                {
                    Some(InputValue::LocalDeclarationOwner(owner)) => owner.clone(),
                    _ => {
                        return unsupported(
                            "constant item target is outside the source catalogue",
                        );
                    }
                };
                let parse = match db.input(&InputKey::DefinitionSource(definition)).as_deref() {
                    Some(InputValue::DefinitionSource(parse)) => parse.clone(),
                    _ => return QueryPoll::Ready(Err(call_fault("local constant source absent"))),
                };
                let plan = match db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(owner.clone()))
                {
                    QueryPoll::Ready(Ok(plan)) => plan,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let QueryValue::SourceLocalPlan(plan) = plan.value.as_ref() else {
                    return QueryPoll::Ready(Err(call_fault("local constant owner plan differs")));
                };
                let plan = Arc::clone(plan);
                match plan.require_current(db) {
                    QueryPoll::Ready(Ok(())) => {}
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
                let owning = plan.context().source();
                if owning.index().body() != &owner || owning.parsed().key() != parse {
                    return QueryPoll::Ready(Err(call_fault("local constant owner plan differs")));
                }
                // The original source the owning body (or its retained
                // assembly) belongs to: its seals certify module names.
                let Some(module_source) = plan.context().module_source() else {
                    return unsupported("local constant owning module source is absent");
                };
                let Some(target) = self
                    .sources
                    .iter()
                    .position(|view| view.selection() == module_source.selection())
                else {
                    return unsupported("local constant owning source is outside the catalogue");
                };
                local = Some(crate::frontend::constant_source_admission::LocalConstantScope::new(
                    owner,
                    Arc::clone(owning.parsed()),
                    Arc::clone(plan.context().facts()),
                ));
                target
            }
        };
        let scope = match db
            .input(&InputKey::DeclarationContext(definition))
            .as_deref()
        {
            // A block item's names beyond its blocks are its module's.
            Some(InputValue::DeclarationContext(context)) => crate::syntax::outline::ScopeKey {
                crate_key: context.scope.crate_key,
                module_path: context.scope.module_path.clone(),
                owner: None,
            },
            _ => return QueryPoll::Ready(Err(call_fault("constant pattern context absent"))),
        };
        let required = (|| -> Result<Option<InputRequest>, Diagnostic> {
            // A current primary or contained Type certificate of the owning
            // scope (a nested module's facet of the same fixed point).
            if db
                .source_view_type_scope_certificate(self.sources[target].selection(), &scope)?
                .is_some()
            {
                return Ok(None);
            }
            let input =
                db.source_view_scope_certificate(self.sources[target].selection(), &scope)?;
            let InputValue::DiscoverySeal(value) = input.as_ref() else {
                return Err(call_fault(
                    "constant target Value certificate variant differs",
                ));
            };
            let mut key = value.seal.key().clone();
            key.domain = SealDomain::Scope {
                id: scope.fingerprint(),
                namespace: Namespace::Type,
            };
            Ok(db
                .published_input(&InputKey::DiscoverySeal(key.clone()))
                .is_none()
                .then_some(InputRequest::Discover { key }))
        })();
        match required {
            Ok(Some(input)) => return QueryPoll::NeedInput(input),
            Ok(None) => {}
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
        let views = self.sources.iter().collect::<Vec<_>>();
        let request = crate::frontend::constant_source_admission::ConstantRequest::Constant;
        let admission = match local {
            Some(local) => ConstantSourceAdmission::capture_current_local_in_sources(
                db,
                &self.sources[target],
                definition,
                request,
                LIMIT,
                &views,
                local,
            ),
            None => ConstantSourceAdmission::capture_current_in_sources(
                db,
                &self.sources[target],
                definition,
                request,
                LIMIT,
                &views,
            ),
        };
        let result = match admission.and_then(|admission| {
            CurrentConstantResultType::capture_current(db, &self.sources[target], admission, LIMIT)
        }) {
            Ok(result) => result,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let wire = if let Some(scalar) = result.scalar() {
            scalar.wire_type()
        } else if let Some((element, demand)) = result.array() {
            // The array length is its own pure constant demand, selected
            // from the declaration's written type.
            let proof = match result.admission().reselect_array_length_current(
                db,
                &self.sources[target],
                demand,
                LIMIT,
            ) {
                Ok(proof) => proof,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let (_, outcome) = match fetch_constant_preparing_macros(
                db,
                &self.sources[target],
                proof,
                &demand.expected_wire(),
                ConstLimits::default(),
                usize::MAX,
            ) {
                QueryPoll::Ready(Ok(outcome)) => outcome,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let length = match current_value(&outcome)
                .and_then(|value| value.payload().decode_value())
                .and_then(|value| {
                    value
                        .scalar()
                        .ok_or_else(|| call_fault("constant array length is not scalar"))
                }) {
                Ok(value) if value.kind == 5 && value.type_id == 13 => value.payload,
                Ok(_) => return QueryPoll::Ready(Err(call_fault("constant array length is not usize"))),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            crate::ir::constant_data::wire::ConstantWireType::Array {
                element: Box::new(element.wire_type()),
                length,
            }
        } else {
            return unsupported("constant result type requires structural constant integration");
        };
        let (_, outcome) = match fetch_constant_preparing_macros(
            db,
            &self.sources[target],
            result.into_admission(),
            &wire,
            ConstLimits::default(),
            usize::MAX,
        ) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(
            current_value(&outcome)
                .and_then(|value| value.payload().decode_value())
                .map(|value| (value, wire)),
        )
    }
}
