//! Settlement of a record constructor whose explicit Type argument holds a `_`
//! (`MaybeItem::<Iter<_>> { elem: .. }`). The hole is a real variable of the
//! one pinned inference owner: the constructor's nominal declaration, its
//! explicit application and its reached field operands go to the same reached
//! source root a selected call uses (`poll_reached_root`), with the actual
//! declared field Types as formals. Nothing here relates, normalizes or selects:
//! the answer is the owner's own response, applied through the ordinary
//! constructor start (`Frame::settle_open_constructor`).
use super::*;
use super::trait_calls::SelectionArgument;
use crate::{
    core::digest::StableEncoder,
    frontend::{explicit_type_arguments::ExplicitType, nominal_member_goals::{NominalMemberKey, NominalMemberSelection}},
    traits::{
        term::{GenericArg, InferVarKey, ParamKey, TyTerm},
        upstream::{
            ReachedCallOutcome, ReachedSubject, poll_reached_root,
            root_input::{SourceCallRelation, SourceCallRelationKind, SourceRootCallInput, SourceVariableFamily},
            source_adt_fields::{AdtSourceFieldsKey, CurrentAdtSourceFields},
        },
    },
};

fn unsupported(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
/// A variable of one constructor expression, scoped by its entry PC.
fn scoped_variable(domain: &'static str, entry: &PcKey, ordinal: u32) -> InferVarKey {
    let mut scope = StableEncoder::new(domain);
    scope.record(entry);
    InferVarKey { transaction: scope.finish(), ordinal }
}

impl RunMachine {
    /// Before the first `ConstructField` (or `ConstructFinish`) of an open
    /// record constructor runs, settle it. A closed constructor is untouched.
    pub(super) fn settle_open_constructor(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> QueryPoll<()> {
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        let operation = match self.installed.get_current(db, id) {
            Ok(recipe) => recipe
                .recipe()
                .operations
                .iter()
                .skip(self.frame.resume_operation_index())
                .find_map(|op| match op {
                    Op::ConstructField { guard, field, source } => Some((*guard, Some((*field, *source)))),
                    Op::ConstructRest { guard, .. } => Some((*guard, None)),
                    Op::ConstructFinish { guard, .. } => Some((*guard, None)),
                    _ => None,
                }),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some((guard, current)) = operation else {
            return QueryPoll::Ready(Ok(()));
        };
        let Some(source) = checked!(self.frame.open_constructor(guard)) else {
            return QueryPoll::Ready(Ok(()));
        };
        let prepared = source.prepared();
        let entry = prepared.key().clone();
        let definition = prepared.mapping().definition();
        let Some(inventory) = self.impl_catalog.clone() else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CATALOG-INCOMPLETE",
                "reached generic constructor requires its coordinator-owned source catalog locator".into(),
            )));
        };
        let (caller, caller_environment, caller_callable) = self.reached_caller(&entry);
        let Some(caller_callable) = caller_callable else {
            return QueryPoll::Ready(Err(call_fault("reached caller source callable is absent")));
        };
        // The declaration's actual source field Types authenticate every formal.
        let fields = Arc::new(ready!(CurrentAdtSourceFields::capture(
            db,
            &AdtSourceFieldsKey { inventory: inventory.clone(), definition },
        )));
        if let Some(why) = fields.plan().pending() {
            return QueryPoll::Ready(Err(unsupported(format!(
                "record constructor declaration fields are not prepared: {why:?}"
            ))));
        }
        let variant = match prepared.pending_target() {
            Some((variant, _)) => variant,
            None => checked!(source.ready_bound()).variant().variant(),
        };
        // Parameters: the explicit application, each `_` a source-owned variable.
        let mut parameters = BTreeMap::new();
        let mut variables = BTreeMap::new();
        let mut opens = Vec::new();
        for (index, ordinal) in fields.plan().type_parameter_ordinals().into_iter().enumerate() {
            let parameter = ParamKey { definition, ordinal };
            match prepared.arguments().get(index) {
                Some(Some(ExplicitType::Open(open))) => {
                    for hole in open.holes.iter() {
                        variables.insert(hole.key, SourceVariableFamily::Type);
                    }
                    parameters.insert(parameter, open.term.clone());
                    opens.push((ordinal, Arc::clone(open)));
                }
                Some(Some(argument)) => {
                    parameters.insert(parameter, self.frame.bound_view(argument.source(), &self.store));
                }
                _ => {
                    let key = scoped_variable("lrr/v1/constructor-argument", &entry, ordinal);
                    variables.insert(key, SourceVariableFamily::Type);
                    parameters.insert(parameter, TyTerm::Infer(key));
                }
            }
        }
        // The source site of the record expression.
        let span = checked!(self.frame.body.span(NodeId(entry.node)));
        let caller_source = db.input(&crate::db::dependency::InputKey::DefinitionSource(entry.body.definition));
        let caller_body = db.input(&crate::db::dependency::InputKey::DefinitionBody(entry.body.definition));
        let Some(site) = checked!(self.reached_source_site(&span, caller_source.as_deref(), caller_body.as_deref())) else {
            return QueryPoll::Ready(Err(unsupported("reached constructor source site is not attached")));
        };
        let mut relations = Vec::new();
        // The field operand that has executed: its reached Type coerces to the
        // declared field Type (rustc's `check_expr_coercible_to_type`).
        if let Some((written, operand)) = current {
            let Some(ordinal) = prepared.mapping().written_to_declaration().get(written as usize).copied() else {
                return QueryPoll::Ready(Err(call_fault("written constructor field absent from the bound source map")));
            };
            let Some(formal) = fields.plan().field_term(variant, ordinal) else {
                return QueryPoll::Ready(Err(unsupported("record constructor field has no source Type")));
            };
            let actual = match checked!(self.selection_argument(operand)) {
                SelectionArgument::Type(actual) => actual,
                _ => {
                    return QueryPoll::Ready(Err(unsupported(
                        "record Type argument hole requires the field operand's reached Type",
                    )));
                }
            };
            relations.push(SourceCallRelation {
                formal: formal.clone(),
                actual,
                kind: SourceCallRelationKind::Coerce,
                site,
            });
        }
        // The declared field Types the binder cannot normalize itself ask the
        // same owner for their normalization under these arguments.
        let mut queries = Vec::new();
        for field in 0..fields.plan().field_count(variant) {
            let Some(term) = fields.plan().field_term(variant, field) else { continue };
            if !crate::frontend::associated_types::contains_projection(term) {
                continue;
            }
            let key = scoped_variable("lrr/v1/constructor-field-type", &entry, field);
            variables.insert(key, SourceVariableFamily::Type);
            relations.push(SourceCallRelation {
                formal: term.clone(),
                actual: TyTerm::Infer(key),
                kind: SourceCallRelationKind::Equal,
                site,
            });
            queries.push((field, key));
        }
        let packet = SourceRootCallInput {
            callee: definition,
            signature: fields.fingerprint(),
            parameters: &parameters,
            variables: &variables,
            relations: &relations,
            caller: &caller,
            component: None,
        };
        let subject = ReachedSubject::Adt(Arc::clone(&fields), variant);
        let outcome = ready!(poll_reached_root(
            db,
            &inventory,
            &packet,
            caller_environment.as_ref(),
            &caller_callable,
            &subject,
        ));
        let response = match outcome {
            ReachedCallOutcome::Solved(response) => response,
            other => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-TRAIT-INFERENCE",
                    format!("reached constructor inference did not settle: {other:?}"),
                )));
            }
        };
        checked!(response.require_current(db));
        let closed = |key: &InferVarKey| -> Option<TyTerm> {
            response.values.get(key).filter(|term| crate::frontend::call_contracts::closed_term(term)).cloned()
        };
        // Closed arguments, in declaration order.
        let mut terms = Vec::new();
        for (index, ordinal) in fields.plan().type_parameter_ordinals().into_iter().enumerate() {
            let term = match prepared.arguments().get(index) {
                Some(Some(ExplicitType::Open(open))) => open.close(&closed),
                Some(Some(argument)) => Some(self.frame.bound_view(argument.source(), &self.store)),
                _ => closed(&scoped_variable("lrr/v1/constructor-argument", &entry, ordinal)),
            };
            let Some(term) = term else {
                return QueryPoll::Ready(Err(unsupported(
                    "record Type argument hole is not determined by the first field operand",
                )));
            };
            terms.push(term);
        }
        let mut normalized = Vec::new();
        for (field, key) in &queries {
            let Some(term) = closed(key) else {
                return QueryPoll::Ready(Err(unsupported(
                    "record field Type is not normalized by pinned inference",
                )));
            };
            normalized.push((variant, *field, term));
        }
        // Descriptors of the closed arguments.
        let sources: Vec<_> = self.sources.iter().collect();
        let mut arguments = Vec::new();
        for term in &terms {
            let Some(written) = ready!(crate::frontend::written_terms::closed_written_type(db, &sources, term)) else {
                return QueryPoll::Ready(Err(unsupported(
                    "settled record Type argument requires its source application",
                )));
            };
            let Some(descriptor) = checked!(written.materialize_in(&mut self.store, &[])) else {
                return QueryPoll::Ready(Err(unsupported("settled record Type argument names no single runtime Type")));
            };
            arguments.push((descriptor, term.clone()));
        }
        // The record's binding under the closed arguments.
        let settled = if prepared.bound().is_some() {
            source.clone()
        } else {
            let Some((_, name)) = prepared.pending_target() else {
                return QueryPoll::Ready(Err(call_fault("record pending source target absent")));
            };
            let key = NominalMemberKey {
                pc: PcKey { phase: 0, ..entry.clone() },
                definition,
                arguments: terms.iter().cloned().map(GenericArg::Type).collect(),
                selection: NominalMemberSelection::Variant(name.map(str::to_owned)),
                normalized: (!normalized.is_empty()).then_some(normalized),
            };
            let (member, member_guard) = ready!(self.nominal_member(db, key, None));
            let QueryValue::NominalMember(member) = member.as_ref() else {
                return QueryPoll::Ready(Err(call_fault("record template specialization outcome differs")));
            };
            let Some(bound) = member.variant() else {
                return QueryPoll::Ready(Err(call_fault("record template specialization variant absent")));
            };
            checked!(source.clone().specialize(bound, member_guard))
        };
        QueryPoll::Ready(self.frame.settle_open_constructor(guard, settled, &arguments, &mut self.store))
    }

    /// The declared field Types of one nominal under closed arguments, as the
    /// pinned owner normalizes them: the same reached root as constructor
    /// settlement, with no operand. `None` while a field is not fully answered.
    pub(super) fn normalized_member_fields(
        &mut self,
        db: &mut Database,
        key: &NominalMemberKey,
        every_field: bool,
    ) -> QueryPoll<Option<Vec<(u32, u32, TyTerm)>>> {
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        let cache_key = (key.definition, key.arguments.clone(), key.selection.clone(), every_field);
        if let Some((fields, response)) = self.nominal.normalized.get(&cache_key) {
            // A response whose inputs a later root superseded is recomputed.
            if response.require_current(db).is_ok() {
                return QueryPoll::Ready(Ok(Some(fields.clone())));
            }
            self.nominal.normalized.remove(&cache_key);
        }
        let Some(inventory) = self.impl_catalog.clone() else {
            return QueryPoll::Ready(Ok(None));
        };
        let definition = key.definition;
        let (caller, caller_environment, caller_callable) = self.reached_caller(&key.pc);
        let Some(caller_callable) = caller_callable else {
            return QueryPoll::Ready(Ok(None));
        };
        let fields = Arc::new(ready!(CurrentAdtSourceFields::capture(
            db,
            &AdtSourceFieldsKey { inventory: inventory.clone(), definition },
        )));
        if fields.plan().pending().is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        let variant = match &key.selection {
            NominalMemberSelection::Variant(name) => {
                match fields.plan().variant_index(name.as_deref()) {
                    Some(variant) => variant,
                    None => return QueryPoll::Ready(Ok(None)),
                }
            }
            _ => 0,
        };
        let mut parameters = BTreeMap::new();
        let mut arguments = key.arguments.iter().filter_map(|argument| match argument {
            GenericArg::Type(term) => Some(term),
            _ => None,
        });
        for ordinal in fields.plan().type_parameter_ordinals() {
            let Some(term) = arguments.next() else { return QueryPoll::Ready(Ok(None)); };
            if !crate::frontend::call_contracts::closed_term(term) {
                return QueryPoll::Ready(Ok(None));
            }
            parameters.insert(ParamKey { definition, ordinal }, term.clone());
        }
        let span = checked!(self.frame.body.span(NodeId(key.pc.node)));
        let caller_source = db.input(&crate::db::dependency::InputKey::DefinitionSource(key.pc.body.definition));
        let caller_body = db.input(&crate::db::dependency::InputKey::DefinitionBody(key.pc.body.definition));
        let Some(site) = checked!(self.reached_source_site(&span, caller_source.as_deref(), caller_body.as_deref())) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut variables = BTreeMap::new();
        let mut relations = Vec::new();
        let mut queries = Vec::new();
        for field in 0..fields.plan().field_count(variant) {
            let Some(term) = fields.plan().field_term(variant, field) else { continue };
            if !every_field && !crate::frontend::associated_types::contains_projection(term) {
                continue;
            }
            let variable = scoped_variable("lrr/v1/member-field-type", &key.pc, field);
            variables.insert(variable, SourceVariableFamily::Type);
            relations.push(SourceCallRelation {
                formal: term.clone(),
                actual: TyTerm::Infer(variable),
                kind: SourceCallRelationKind::Equal,
                site,
            });
            queries.push((field, variable));
        }
        let packet = SourceRootCallInput {
            callee: definition,
            signature: fields.fingerprint(),
            parameters: &parameters,
            variables: &variables,
            relations: &relations,
            caller: &caller,
            component: None,
        };
        let subject = ReachedSubject::Adt(Arc::clone(&fields), variant);
        let response = match ready!(poll_reached_root(
            db, &inventory, &packet, caller_environment.as_ref(), &caller_callable, &subject,
        )) {
            ReachedCallOutcome::Solved(response) => response,
            // An unsettled root leaves the field Types to the caller's own
            // typed refusal; it is never read as a normalization.
            _ => return QueryPoll::Ready(Ok(None)),
        };
        let mut normalized = Vec::new();
        for (field, variable) in queries {
            let Some(term) = response
                .values
                .get(&variable)
                .filter(|term| crate::frontend::call_contracts::closed_term(term))
            else {
                return QueryPoll::Ready(Ok(None));
            };
            normalized.push((variant, field, term.clone()));
        }
        self.nominal.normalized.insert(cache_key, (normalized.clone(), Arc::new(response)));
        QueryPoll::Ready(Ok(Some(normalized)))
    }
}
