//! Reached field selection from retained source identity, before guest mutation.
use super::*;
use crate::{
    frontend::types::{NominalFieldBinding, SchemaNames},
    source::manifest::ItemKind,
};
fn field_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}

#[path = "run_field_source.rs"]
mod source;

impl RunMachine {
    #[cfg(test)]
    pub(crate) fn current_field_source_for_test(
        db: &mut Database,
        sources: &[PreparedSourceView],
        index: &crate::syntax::body_index::BodyIndex,
        pc: &PcKey,
    ) -> QueryPoll<(String, crate::syntax::outline::ScopeKey)> {
        let views = sources.iter().collect::<Vec<_>>();
        match source::capture(db, &views, index, pc) {
            QueryPoll::Ready(Ok(proof)) => QueryPoll::Ready(
                proof
                    .scope()
                    .map(|scope| (proof.member().to_owned(), scope.clone())),
            ),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }

    pub(super) fn prepare_field(
        &mut self,
        db: &mut Database,
        base: SlotId,
        destination: SlotId,
    ) -> QueryPoll<()> {
        match self.prepare_field_inner(db, base, destination) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }

    fn prepare_field_inner(
        &mut self,
        db: &mut Database,
        base: SlotId,
        destination: SlotId,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        // Field access autoderefs through boxes to the record (rustc's
        // autoderef of the base); a box itself has no named fields.
        self.frame
            .autoderef_box_member_base(base, &self.store)
            .map_err(|failure| match failure {
                crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                crate::runtime::place::PlaceFailure::GuestPanic(panic) => {
                    Diagnostic::new("LRR-E-INTERNAL-IR", panic.message())
                }
            })?;
        // The definition comes from the executed expression's retained source
        // view. A runtime descriptor or a same-spelled type cannot select it.
        let definition = self.frame.nominal_place_source(base)?;
        if matches!(self.graph, Some(RunNames::Legacy(_))) {
            return self.prepare_legacy_field(db, base, destination, definition);
        }
        Ok(self.prepare_selected_field(
            db,
            base,
            destination,
            definition,
            crate::frontend::nominal_member_goals::NominalMemberSelection::Named,
        ))
    }
    // Held legacy source-catalogue adapter. It keeps the pre-existing argument-
    // free shared SchemaNames binder and VM projection unchanged. Removal needs
    // actual BodyEnvironment publication, never a fabricated discovery seal.
    fn prepare_legacy_field(
        &mut self,
        db: &mut Database,
        base: SlotId,
        destination: SlotId,
        definition: crate::core::digest::Digest,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        let graph = self
            .graph
            .as_ref()
            .ok_or_else(|| field_fault("field requires a checked source catalogue"))?;
        for view in &self.sources {
            graph.require_view(view)?;
        }
        self.sources
            .len()
            .checked_mul(std::mem::size_of::<&PreparedSourceView>())
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut sources = Vec::new();
        sources
            .try_reserve_exact(self.sources.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        sources.extend(self.sources.iter());
        let caller = match source::capture(db, &sources, &self.frame.body, self.frame.pc()) {
            QueryPoll::Ready(Ok(caller)) => caller,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(definition)) {
            QueryPoll::Ready(Ok(captured)) => captured,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return Err(field_fault("selected field schema query outcome differs"));
        };
        let names = match SchemaNames::from_current_database(db, graph, &sources, schema) {
            QueryPoll::Ready(Ok(names)) => names,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        if names.definition().item_kind != ItemKind::Struct {
            return Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "field expression requires an entered enum variant or other source member view"
                    .into(),
            ));
        }
        match names.bind_named_field_current(db, &sources, None, caller.member(), caller.scope()?) {
            QueryPoll::Ready(Ok(NominalFieldBinding::Ready(binding))) => Ok(QueryPoll::Ready(
                self.frame.prepare_source_field(base, destination, binding),
            )),
            QueryPoll::Ready(Ok(NominalFieldBinding::Deferred { .. })) => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "field expression requires further selected source type or generic binding".into(),
            )),
            QueryPoll::Ready(Err(error)) => Err(error),
            QueryPoll::NeedInput(request) => Ok(QueryPoll::NeedInput(request)),
        }
    }
}

/// What the next place operation needs of its base place.
enum PlaceNeed {
    /// `*place`: the operand's own type dereferences (not a reference or box).
    Explicit,
    /// `base.name`
    Named,
    /// `base.n`
    Positional(u32),
    /// `base[i]`
    Index,
    /// The hint of `base[i]`'s operand, before the operand runs.
    IndexOperand,
}

impl RunMachine {
    /// rustc's overloaded autoderef of a place expression: when the next
    /// place operation's base place is (a reference or box to) a nominal
    /// value that cannot serve the operation itself -- `*rc`, `rc.field`,
    /// `rc.0`, `guard[i]` -- but implements `Deref`, the base takes that
    /// step first. The operation then suspends for the trait method, as an
    /// overloaded operator or a method receiver does, and runs over the
    /// returned reference's referent. `DerefMut` is used when the type has
    /// it and the place is used mutably (assigned to, `&mut`-borrowed or a
    /// method receiver).
    pub(super) fn prepare_place_deref(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> QueryPoll<()> {
        match self.prepare_place_deref_inner(db, id) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    fn prepare_place_deref_inner(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> Result<QueryPoll<()>, Diagnostic> {
        use crate::runtime::shape::TypeKey;
        if self.frame.has_place_deref_demand() || self.frame.has_operator_activation() {
            return Ok(QueryPoll::Ready(Ok(())));
        }
        let resume = self.frame.resume_operation_index();
        let operation = {
            let recipe = self.installed.get_current(db, id)?;
            recipe
                .recipe()
                .operations
                .iter()
                .skip(resume)
                .find_map(|op| match op {
                    Op::DerefPlace { reference, .. } => Some((*reference, PlaceNeed::Explicit)),
                    Op::ProjectField { base, .. } => Some((*base, PlaceNeed::Named)),
                    Op::ProjectTuple { base, field, .. } => {
                        Some((*base, PlaceNeed::Positional(*field)))
                    }
                    Op::ProjectIndex { base, .. } => Some((*base, PlaceNeed::Index)),
                    Op::SetHint {
                        hint: crate::ir::op::HintRecipe::IndexOperand { base },
                        ..
                    } => Some((*base, PlaceNeed::IndexOperand)),
                    _ => None,
                })
        };
        let Some((base, need)) = operation else {
            return Ok(QueryPoll::Ready(Ok(())));
        };
        let Some(mut ty) = self.frame.place_type_if_executed(base, &self.store)? else {
            return Ok(QueryPoll::Ready(Ok(())));
        };
        if matches!(need, PlaceNeed::Explicit) {
            if self.store.box_contents_type(ty)?.is_some() {
                return Ok(QueryPoll::Ready(Ok(())));
            }
        } else {
            // The builtin steps first: references and boxes to the value.
            loop {
                match self.store.type_descriptor(ty)?.key.as_ref() {
                    TypeKey::Ref { target, .. } => ty = *target,
                    _ => match self.store.box_contents_type(ty)? {
                        Some(contents) => ty = contents,
                        None => break,
                    },
                }
            }
        }
        let TypeKey::Nominal { identity, .. } = self.store.type_descriptor(ty)?.key.as_ref()
        else {
            return Ok(QueryPoll::Ready(Ok(())));
        };
        let identity = *identity;
        if !matches!(need, PlaceNeed::Explicit) && self.store.is_vec_type(ty)? {
            if matches!(need, PlaceNeed::Index | PlaceNeed::IndexOperand) {
                return Ok(QueryPoll::Ready(Ok(())));
            }
        }
        let Some(members) = self.members.clone() else {
            return Ok(QueryPoll::Ready(Ok(())));
        };
        // Whether the nominal value serves the operation itself.
        let serves = match need {
            PlaceNeed::Explicit => false,
            PlaceNeed::Index | PlaceNeed::IndexOperand => self.store.is_vec_type(ty)?,
            PlaceNeed::Named | PlaceNeed::Positional(_) => {
                let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(identity)) {
                    QueryPoll::Ready(Ok(captured)) => captured,
                    QueryPoll::Ready(Err(error)) => return Err(error),
                    QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
                };
                let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
                    return Err(field_fault("place dereference schema outcome differs"));
                };
                let Ok(variant) = schema.variant(None) else {
                    return Ok(QueryPoll::Ready(Ok(())));
                };
                match need {
                    PlaceNeed::Positional(position) => {
                        variant.shape == crate::frontend::types::FieldShape::Tuple
                            && (position as usize) < variant.fields.len()
                    }
                    _ => {
                        let graph = self
                            .graph
                            .as_ref()
                            .ok_or_else(|| field_fault("field requires a checked source catalogue"))?;
                        for view in &self.sources {
                            graph.require_view(view)?;
                        }
                        let sources: Vec<_> = self.sources.iter().collect();
                        let member =
                            match source::capture(db, &sources, &self.frame.body, self.frame.pc()) {
                                QueryPoll::Ready(Ok(caller)) => caller.member().to_owned(),
                                QueryPoll::Ready(Err(error)) => return Err(error),
                                QueryPoll::NeedInput(request) => {
                                    return Ok(QueryPoll::NeedInput(request));
                                }
                            };
                        variant
                            .fields
                            .iter()
                            .any(|field| field.name.as_deref() == Some(member.as_str()))
                    }
                }
            }
        };
        if serves {
            return Ok(QueryPoll::Ready(Ok(())));
        }
        // rustc indexes the first autoderef step whose type has an
        // `Index<Idx>` impl: the base's own, before any `Deref` step.
        if matches!(need, PlaceNeed::Index | PlaceNeed::IndexOperand) {
            let index_trait = match self.operator_trait(db, "ops", "Index") {
                QueryPoll::Ready(Ok(index_trait)) => Some(index_trait),
                QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => None,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            };
            if index_trait
                .is_some_and(|index_trait| !members.trait_method(Ok(identity), index_trait, "index").is_empty())
            {
                if matches!(need, PlaceNeed::IndexOperand) {
                    let parameter = match self.single_index_parameter(db, ty, index_trait) {
                        QueryPoll::Ready(Ok(parameter)) => parameter,
                        QueryPoll::Ready(Err(error)) => return Err(error),
                        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
                    };
                    self.frame.note_index_operand_overloaded(base, parameter);
                    return Ok(QueryPoll::Ready(Ok(())));
                }
                let index_mut = match self.operator_trait(db, "ops", "IndexMut") {
                    QueryPoll::Ready(Ok(index_mut)) => Some(index_mut),
                    QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => None,
                    QueryPoll::Ready(Err(error)) => return Err(error),
                    QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
                };
                let mutable_capable = index_mut.is_some_and(|index_mut| {
                    !members.trait_method(Ok(identity), index_mut, "index_mut").is_empty()
                });
                let mutable = if mutable_capable {
                    match self.place_used_mutably(db) {
                        QueryPoll::Ready(Ok(mutable)) => mutable,
                        QueryPoll::Ready(Err(error)) => return Err(error),
                        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
                    }
                } else {
                    false
                };
                self.frame
                    .peel_builtin_derefs(base, &self.store)
                    .map_err(|failure| match failure {
                        crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                        crate::runtime::place::PlaceFailure::GuestPanic(panic) => {
                            Diagnostic::new("LRR-E-INTERNAL-IR", panic.message())
                        }
                    })?;
                self.frame.demand_place_index(base, mutable);
                return Ok(QueryPoll::Ready(Ok(())));
            }
        }
        if matches!(need, PlaceNeed::IndexOperand) {
            return Ok(QueryPoll::Ready(Ok(())));
        }
        let deref = match self.operator_trait(db, "ops", "Deref") {
            QueryPoll::Ready(Ok(deref)) => deref,
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => {
                return Ok(QueryPoll::Ready(Ok(())));
            }
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
        };
        if members.trait_method(Ok(identity), deref, "deref").is_empty() {
            return Ok(QueryPoll::Ready(Ok(())));
        }
        let deref_mut = match self.operator_trait(db, "ops", "DerefMut") {
            QueryPoll::Ready(Ok(deref_mut)) => Some(deref_mut),
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => None,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
        };
        let mutable_capable = deref_mut
            .is_some_and(|deref_mut| !members.trait_method(Ok(identity), deref_mut, "deref_mut").is_empty());
        let mutable = if mutable_capable {
            match self.place_used_mutably(db) {
                QueryPoll::Ready(Ok(mutable)) => mutable,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            }
        } else {
            false
        };
        if !matches!(need, PlaceNeed::Explicit) {
            self.frame
                .peel_builtin_derefs(base, &self.store)
                .map_err(|failure| match failure {
                    crate::runtime::place::PlaceFailure::RuntimeFault(error) => error,
                    crate::runtime::place::PlaceFailure::GuestPanic(panic) => {
                        Diagnostic::new("LRR-E-INTERNAL-IR", panic.message())
                    }
                })?;
        }
        self.frame.demand_place_deref(base, mutable);
        Ok(QueryPoll::Ready(Ok(())))
    }
    /// The closed type of the `index` method's operand when the nominal `ty`
    /// has one `Index` impl (as an ordinary call's argument is typed by its
    /// callee's parameter); with several impls the evaluated operand selects.
    fn single_index_parameter(
        &mut self,
        db: &mut Database,
        ty: crate::runtime::shape::TypeId,
        index_trait: Option<crate::core::digest::Digest>,
    ) -> QueryPoll<Option<crate::runtime::shape::TypeId>> {
        let (Some(index_trait), Some(members)) = (index_trait, self.members.clone()) else {
            return QueryPoll::Ready(Ok(None));
        };
        let self_type = match self.reached_self_type(ty) {
            Ok(self_type) => self_type,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut functions = Vec::new();
        for (_, function) in members.trait_method(self_type, index_trait, "index") {
            if !functions.contains(&function) {
                functions.push(function);
            }
        }
        let [function] = functions.as_slice() else {
            return QueryPoll::Ready(Ok(None));
        };
        let callee = match self.member_callee(db, *function) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let hints = match self.operator_parameter_hints(db, &callee) {
            QueryPoll::Ready(Ok(hints)) => hints,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let Some(Some(hint)) = hints.get(1) else {
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(match hint.descriptor {
            Some(descriptor) => Ok(Some(descriptor)),
            None => self.store.intern_structural_term(&hint.source),
        })
    }
    /// Whether a pattern matched against the scrutinee of the `match`, `let`
    /// statement or `let` clause at `node` has a `ref mut` binding. Cold: read
    /// from the retained syntax only when an overloaded dereference needs the
    /// answer.
    fn patterns_bind_ref_mut(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<bool> {
        use crate::db::dependency::{InputKey, InputValue};
        use ra_ap_syntax::{AstNode, ast};
        let definition = self.frame.pc().body.definition;
        let source = match db.input(&InputKey::DefinitionSource(definition)).as_deref() {
            Some(InputValue::DefinitionSource(source)) => source.clone(),
            _ => return QueryPoll::Ready(Ok(false)),
        };
        let handle = match db.input(&InputKey::DefinitionBody(definition)).as_deref() {
            Some(InputValue::DefinitionBody(handle)) if handle.key() == self.frame.pc().body => {
                handle.clone()
            }
            _ => return QueryPoll::Ready(Ok(false)),
        };
        let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(source)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(field_fault("place use parse kind differs")));
        };
        let syntax = match parsed.entered_node_syntax(&handle, &self.frame.body, node) {
            Ok(Some(syntax)) => syntax,
            Ok(None) => return QueryPoll::Ready(Ok(false)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let binds = |pattern: Option<ast::Pat>| {
            pattern.is_some_and(|pattern| {
                pattern.syntax().descendants().any(|inner| {
                    ast::IdentPat::cast(inner)
                        .is_some_and(|ident| ident.ref_token().is_some() && ident.mut_token().is_some())
                })
            })
        };
        let result = if let Some(matched) = ast::MatchExpr::cast(syntax.clone()) {
            matched
                .match_arm_list()
                .into_iter()
                .flat_map(|arms| arms.arms())
                .any(|arm| binds(arm.pat()))
        } else if let Some(statement) = ast::LetStmt::cast(syntax.clone()) {
            binds(statement.pat())
        } else if let Some(clause) = ast::LetExpr::cast(syntax) {
            binds(clause.pat())
        } else {
            false
        };
        QueryPoll::Ready(Ok(result))
    }
    /// Whether `Deref` steps from the nominal Type `ty` reach `expected`
    /// (the pointee a borrow's result expects), each target normalized by the
    /// shared associated-type normalizer over the impl index; for a mutable
    /// borrow every step must also be a `DerefMut` impl. Repeated normalized
    /// source Types terminate an impossible chain without an arbitrary cap.
    pub(super) fn deref_chain_reaches(
        &mut self,
        db: &mut Database,
        ty: crate::runtime::shape::TypeId,
        expected: &crate::vm::frame::CoercionTarget,
        mutable: bool,
    ) -> QueryPoll<bool> {
        use crate::traits::term::{ProjectionKey, TraitRef, TyTerm};
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Ok(false));
        };
        let deref = match self.operator_trait(db, "ops", "Deref") {
            QueryPoll::Ready(Ok(deref)) => deref,
            QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => {
                return QueryPoll::Ready(Ok(false));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let deref_mut = if mutable {
            match self.operator_trait(db, "ops", "DerefMut") {
                QueryPoll::Ready(Ok(deref_mut)) => Some(deref_mut),
                QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => {
                    return QueryPoll::Ready(Ok(false));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        } else {
            None
        };
        let Some(mut term) = self.store.descriptor_source_term(ty) else {
            return QueryPoll::Ready(Ok(false));
        };
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Ok(false));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let mut visited = std::collections::BTreeSet::new();
        while visited.insert(term.clone()) {
            let TyTerm::Nominal(head, _) = &term else {
                return QueryPoll::Ready(Ok(false));
            };
            if members.trait_method(Ok(*head), deref, "deref").is_empty()
                || deref_mut.is_some_and(|deref_mut| {
                    members.trait_method(Ok(*head), deref_mut, "deref_mut").is_empty()
                })
            {
                return QueryPoll::Ready(Ok(false));
            }
            let projection = TyTerm::Projection(Box::new(ProjectionKey {
                trait_ref: TraitRef {
                    trait_key: deref,
                    self_ty: term.clone(),
                    args: Vec::new(),
                },
                item: crate::frontend::member_index::associated_name_key("Target"),
                item_args: Vec::new(),
            }));
            let target = match crate::frontend::associated_types::normalize(
                db, graph, &sources, &members, &projection,
            ) {
                QueryPoll::Ready(Ok(Some(target))) => target,
                QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(false)),
                QueryPoll::Ready(Err(error)) if error.code == "LRR-E-NAME-MISSING" => {
                    return QueryPoll::Ready(Ok(false));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            match expected.admits_source(&target, &self.store) {
                Ok(true) => return QueryPoll::Ready(Ok(true)),
                Ok(false) => {},
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
            term = target;
        }
        QueryPoll::Ready(Ok(false))
    }
    /// Whether the place expression at the current node is used mutably:
    /// assigned to, compound-assigned, `&mut`-borrowed or a method receiver,
    /// through any chain of fields, indexing, parentheses and dereferences
    /// (rustc's `convert_place_derefs_to_mutable`).
    fn place_used_mutably(&mut self, db: &mut Database) -> QueryPoll<bool> {
        use crate::syntax::source_constraints::{FactKind, SourceOperator};
        let outcome = match db.fetch(
            &SyntaxProvider,
            QueryKey::SourceConstraints(self.frame.body.body().clone()),
        ) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::SourceConstraints(plan) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(field_fault("place use plan outcome differs")));
        };
        let mut node = NodeId(self.frame.pc().node);
        // Every step climbs to a parent expression.
        for _ in 0..plan.facts_len() {
            let mut next = None;
            for edge in plan.uses(node) {
                let Some(parent) = plan.fact(edge.parent) else {
                    continue;
                };
                match parent.kind() {
                    FactKind::Binary
                        if edge.ordinal == 0
                            && matches!(
                                parent.operation(),
                                Some(
                                    SourceOperator::Assign
                                        | SourceOperator::CompoundArithmetic
                                        | SourceOperator::CompoundShift
                                )
                            ) =>
                    {
                        return QueryPoll::Ready(Ok(true));
                    }
                    FactKind::Borrow => return QueryPoll::Ready(Ok(parent.flag())),
                    // A `ref mut` binding of the patterns matched against this
                    // place borrows it mutably (rustc picks `DerefMut` when
                    // any binding is `ref mut`).
                    FactKind::Match if edge.ordinal == 0 => {
                        return self.patterns_bind_ref_mut(db, edge.parent);
                    }
                    FactKind::Let if edge.ordinal == 1 => {
                        return self.patterns_bind_ref_mut(db, edge.parent);
                    }
                    // A receiver may take `&mut self`: the method is selected
                    // only after the place, so a `DerefMut` type is assumed
                    // to be used through it mutably.
                    FactKind::MethodCall if edge.ordinal == 0 => {
                        return QueryPoll::Ready(Ok(true));
                    }
                    FactKind::Field | FactKind::Index | FactKind::IndexRange | FactKind::Parenthesis
                        if edge.ordinal == 0 =>
                    {
                        next = Some(edge.parent);
                    }
                    FactKind::Prefix if parent.operation() == Some(SourceOperator::Deref) => {
                        next = Some(edge.parent);
                    }
                    _ => {}
                }
            }
            match next {
                Some(parent) => node = parent,
                None => return QueryPoll::Ready(Ok(false)),
            }
        }
        QueryPoll::Ready(Ok(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::digest::Digest,
        db::dependency::{InputKey, InputValue},
        ir::op::PrimitiveType,
        syntax::place_fixture_facts,
        traits::term::TyTerm,
    };
    const STOCK: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/borrowck/borrowck-assign-comp.rs"
    );

    fn first_field() -> (
        Database,
        RunMachine,
        crate::db::memo::Outcome,
        SlotId,
        SlotId,
    ) {
        // Enter the original d body; do not rewrite the licensed compile-fail
        // source or claim that its rustc borrow-checker oracle passes under LRR.
        let (mut db, mut run) = super::super::borrows::tests::start_stock_body(STOCK, "d");
        for _ in 0..100 {
            let outcome = super::super::borrows::tests::next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock named field thunk absent")
            };
            if let Some((base, destination)) = recipe.recipe().operations.iter().find_map(|op| {
                if let Op::ProjectField { base, destination } = op {
                    Some((*base, *destination))
                } else {
                    None
                }
            }) {
                return (db, run, outcome, base, destination);
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        panic!("original &p.y did not reach named projection");
    }

    fn guest_snapshot(run: &RunMachine) -> (u64, u32, usize, u64) {
        (
            run.frame.fuel_snapshot().used,
            run.store.budget().handles(),
            run.store.budget().bytes(),
            run.frame.counters.operations_started,
        )
    }

    fn borrowed_member_at_commit() -> (
        Database,
        RunMachine,
        crate::db::memo::Outcome,
        SlotId,
        SlotId,
    ) {
        let (mut db, mut run) = super::super::borrows::tests::start_stock_body(STOCK, "a");
        for _ in 0..120 {
            let outcome = super::super::borrows::tests::next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("original borrowed-member continuation absent")
            };
            for op in &recipe.recipe().operations {
                if let Op::ProjectField { base, destination } = op {
                    if matches!(
                        run.frame.source_view(*base).unwrap(),
                        Some(TyTerm::Ref(_, _, _))
                    ) {
                        let slots = (*base, *destination);
                        return (db, run, outcome, slots.0, slots.1);
                    }
                    run.prepare_field(&mut db, *base, *destination)
                        .into_ready()
                        .unwrap()
                        .unwrap();
                }
            }
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
        }
        panic!("original q.x through &Point did not reach named projection");
    }

    #[test]
    fn rustc_original_borrowed_nominal_field_reads_owner_assignment_without_copying_reference() {
        let (mut db, mut run, outcome, base, _) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed named field recipe absent")
        };
        let Some(TyTerm::Ref(_, false, target)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared &Point source view was lost")
        };
        assert!(matches!(target.as_ref(), TyTerm::Nominal(_, args) if args.is_empty()));
        let (base, destination) = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| {
                if let Op::ProjectField { base, destination } = op {
                    Some((*base, *destination))
                } else {
                    None
                }
            })
            .unwrap();
        let read = recipe
            .recipe()
            .operations
            .iter()
            .find_map(|op| {
                if let Op::ReadPlace { destination, .. } = op {
                    Some(*destination)
                } else {
                    None
                }
            })
            .unwrap();
        let reference_cell = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .filter_map(|node| run.frame.binding_cell_for_test(node.id).ok())
            .find(|cell| run.store.reference_from_cell(*cell).is_ok())
            .expect("original q reference binding absent");
        let reference = run.store.reference_from_cell(reference_cell).unwrap();
        let before = guest_snapshot(&run);
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        let value = run
            .store
            .preview_scalar(run.frame.cell(read).unwrap(), None)
            .unwrap();
        let (member, expected) = place_fixture_facts::assignment(STOCK, "a");
        assert_eq!(member, "p.x");
        assert_eq!(
            value.type_id,
            crate::frontend::call_contracts::primitive_runtime_id(PrimitiveType::Isize)
                .unwrap()
                .0
        );
        assert_eq!(value.payload as i64, expected);
        assert_eq!(
            run.store.reference_from_cell(reference_cell).unwrap(),
            reference
        );
        assert_eq!(run.frame.counters.started_by_opcode[31], 1);
        assert_eq!(run.frame.counters.started_by_opcode[37], 2);
        assert_eq!(run.frame.counters.started_by_opcode[34], 1);
        assert_eq!(run.frame.counters.started_by_opcode[33], 1);
        assert_eq!(run.frame.counters.started_by_opcode[8], 0);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_borrowed_nominal_field_shared_lease_refuses_writes_before_effects() {
        let (mut db, mut run, outcome, base, destination) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed field recipe absent")
        };
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        let projection = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let before = guest_snapshot(&run);
        let prepared = run
            .frame
            .prepare_projection(projection, &run.store)
            .unwrap()
            .unwrap();
        let source = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .find_map(|node| run.frame.binding_cell_for_test(node.id).ok())
            .unwrap();
        let error = run
            .store
            .preview_intrinsic_place_assignment(prepared.place_for_test(), source)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-REFERENCE-ACCESS")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(
            run.store
                .read_place(prepared.place_for_test())
                .unwrap()
                .payload as i64,
            place_fixture_facts::assignment(STOCK, "a").1
        );
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_borrowed_field_reference_source_access_mismatch_refuses_before_fuel() {
        let (mut db, mut run, outcome, base, destination) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed field recipe absent")
        };
        let Some(TyTerm::Ref(region, false, target)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared source absent")
        };
        run.frame
            .set_source_view(base, Some(TyTerm::Ref(region, true, target)))
            .unwrap();
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        let before = guest_snapshot(&run);
        let projection = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let error = run
            .frame
            .prepare_projection(projection, &run.store)
            .err()
            .expect("source/access mismatch admitted");
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-OWNERSHIP")
        );
        assert_eq!(guest_snapshot(&run), before);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_borrowed_nominal_field_replaced_owner_invalidates_before_projection() {
        let (mut db, mut run, outcome, base, destination) = borrowed_member_at_commit();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("borrowed field recipe absent")
        };
        let owner = run.frame.body.nodes().iter().filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .find_map(|node| run.frame.binding_cell_for_test(node.id).ok().filter(|cell|
                matches!(run.store.state(*cell), Ok(crate::runtime::cell::CellState::Live(value)) if value.kind == 8)))
            .unwrap();
        let held = run.store.move_from(owner).unwrap();
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        let before = guest_snapshot(&run);
        let projection = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let error = run
            .frame
            .prepare_projection(projection, &run.store)
            .err()
            .expect("stale owner lease admitted");
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-STALE-HANDLE")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.store.peek_ticket(held).unwrap().kind, 8);
        db.finish_run().unwrap();
    }

    fn original_nominal_owners(
        run: &RunMachine,
    ) -> (crate::runtime::cell::CellId, crate::runtime::cell::CellId) {
        let cells: Vec<_> = run
            .frame
            .body
            .nodes()
            .iter()
            .filter(|node| node.role == crate::syntax::body_index::NodeRole::Pattern)
            .filter_map(|node| run.frame.binding_cell_for_test(node.id).ok())
            .collect();
        let reference = *cells
            .iter()
            .find(|cell| run.store.reference_from_cell(**cell).is_ok())
            .unwrap();
        let target = *cells
            .iter()
            .find(|cell| {
                matches!(run.store.state(**cell),
            Ok(crate::runtime::cell::CellState::Live(value)) if value.kind == 8)
            })
            .unwrap();
        (reference, target)
    }

    #[test]
    fn rustc_nominal_reference_deref_retains_source_owner_and_shared_access() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(_, false, pointee)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared nominal source absent")
        };
        assert!(matches!(pointee.as_ref(), TyTerm::Nominal(_, args) if args.is_empty()));
        let (reference_cell, target_cell) = original_nominal_owners(&run);
        let reference = run.store.reference_from_cell(reference_cell).unwrap();
        let reference_owner = run.store.state(reference_cell).unwrap();
        let target_owner = run.store.state(target_cell).unwrap();
        let epoch = run.store.location_epoch(target_cell).unwrap();
        let reference_type = run
            .store
            .place_type_id(run.frame.executed_place_for_test(base).unwrap())
            .unwrap();
        let crate::runtime::shape::TypeKey::Ref { target, .. } = run
            .store
            .type_descriptor(reference_type)
            .unwrap()
            .key
            .as_ref()
        else {
            panic!("original reference descriptor absent")
        };
        let target = *target;
        let before = guest_snapshot(&run);
        run.frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        // LRR frame adaptation of the original q/p lease, not execution of a
        // rewritten source PrefixExpr or completion of the borrowck oracle.
        run.frame
            .deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.frame.source_view(destination).unwrap(), Some(*pointee));
        let place = run.frame.executed_place_for_test(destination).unwrap();
        assert_eq!(run.store.place_type_id(place).unwrap(), target);
        let error = run
            .store
            .preview_intrinsic_place_assignment(place, reference_cell)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-REFERENCE-ACCESS")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.store.state(reference_cell).unwrap(), reference_owner);
        assert_eq!(run.store.state(target_cell).unwrap(), target_owner);
        assert_eq!(run.store.location_epoch(target_cell).unwrap(), epoch);
        assert_eq!(
            run.store.reference_from_cell(reference_cell).unwrap(),
            reference
        );
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_deref_destination_source_mismatch_preserves_constraint_and_owner() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(_, _, pointee)) = run.frame.source_view(base).unwrap() else {
            panic!("original nominal source absent")
        };
        let TyTerm::Nominal(actual, args) = *pointee else {
            panic!("original nominal definition absent")
        };
        let foreign = Digest::of_bytes(b"foreign-nominal-deref-destination");
        assert_ne!(foreign, actual);
        let expected = TyTerm::Nominal(foreign, args);
        run.frame
            .set_source_view(destination, Some(expected.clone()))
            .unwrap();
        let (reference, target) = original_nominal_owners(&run);
        let owners = (
            run.store.state(reference).unwrap(),
            run.store.state(target).unwrap(),
        );
        let epoch = run.store.location_epoch(target).unwrap();
        let before = guest_snapshot(&run);
        let error = run
            .frame
            .deref_place(base, destination, &run.store)
            .unwrap_err();
        assert!(matches!(error,
            crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
                if error.code == "LRR-E-OWNERSHIP"));
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.frame.source_view(destination).unwrap(), Some(expected));
        assert!(run.frame.executed_place_for_test(destination).is_err());
        assert!(run.frame.executed_place_for_test(base).is_ok());
        assert_eq!(
            (
                run.store.state(reference).unwrap(),
                run.store.state(target).unwrap()
            ),
            owners
        );
        assert_eq!(run.store.location_epoch(target).unwrap(), epoch);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_deref_matching_destination_source_preserves_original_lease() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(_, false, pointee)) = run.frame.source_view(base).unwrap() else {
            panic!("original shared nominal source absent")
        };
        let expected = *pointee;
        run.frame
            .set_source_view(destination, Some(expected.clone()))
            .unwrap();
        let (reference, target) = original_nominal_owners(&run);
        let lease = run.store.reference_from_cell(reference).unwrap();
        let owners = (
            run.store.state(reference).unwrap(),
            run.store.state(target).unwrap(),
        );
        let epoch = run.store.location_epoch(target).unwrap();
        let before = guest_snapshot(&run);
        run.frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        run.frame
            .deref_place(base, destination, &run.store)
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.frame.source_view(destination).unwrap(), Some(expected));
        assert!(run.frame.executed_place_for_test(destination).is_ok());
        assert!(run.frame.executed_place_for_test(base).is_err());
        assert_eq!(run.store.reference_from_cell(reference).unwrap(), lease);
        assert_eq!(
            (
                run.store.state(reference).unwrap(),
                run.store.state(target).unwrap()
            ),
            owners
        );
        assert_eq!(run.store.location_epoch(target).unwrap(), epoch);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_reference_deref_foreign_source_refuses_before_publication() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let Some(TyTerm::Ref(region, mutable, pointee)) = run.frame.source_view(base).unwrap()
        else {
            panic!("original nominal source absent")
        };
        let TyTerm::Nominal(actual, args) = *pointee else {
            panic!("original nominal definition absent")
        };
        let foreign = Digest::of_bytes(b"foreign-nominal-dereference-source");
        assert_ne!(foreign, actual);
        run.frame
            .set_source_view(
                base,
                Some(TyTerm::Ref(
                    region,
                    mutable,
                    Box::new(TyTerm::Nominal(foreign, args)),
                )),
            )
            .unwrap();
        let (reference, target) = original_nominal_owners(&run);
        let owners = (
            run.store.state(reference).unwrap(),
            run.store.state(target).unwrap(),
        );
        let before = guest_snapshot(&run);
        let error = run
            .frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-OWNERSHIP")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(
            (
                run.store.state(reference).unwrap(),
                run.store.state(target).unwrap()
            ),
            owners
        );
        assert!(run.frame.source_view(destination).unwrap().is_none());
        assert!(run.frame.executed_place_for_test(destination).is_err());
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_reference_deref_without_source_view_uses_closed_descriptor() {
        // The reached reference descriptor is closed; a cleared source view
        // takes the same auto-deref as a retained one.
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        run.frame.set_source_view(base, None).unwrap();
        run.frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap();
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_nominal_reference_deref_moved_owner_refuses_stale_lease() {
        let (mut db, mut run, _, base, destination) = borrowed_member_at_commit();
        let (reference, target) = original_nominal_owners(&run);
        let owner = run.store.state(reference).unwrap();
        let held = run.store.move_from(target).unwrap();
        let before = guest_snapshot(&run);
        let error = run
            .frame
            .preflight_deref_place(base, destination, &run.store)
            .unwrap_err();
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-STALE-HANDLE")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert_eq!(run.store.state(reference).unwrap(), owner);
        assert_eq!(
            run.store.state(target).unwrap(),
            crate::runtime::cell::CellState::Moved
        );
        assert_eq!(run.store.peek_ticket(held).unwrap().kind, 8);
        assert!(run.frame.source_view(destination).unwrap().is_none());
        assert!(run.frame.executed_place_for_test(destination).is_err());
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_original_named_field_borrow_and_assignment_use_source_selected_member() {
        let (mut db, mut run, first, base, destination) = first_field();
        let source_definition = run.frame.nominal_place_source(base).unwrap();
        let schema = run.sources[0]
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.declared_name == "Point")
            .unwrap();
        assert_eq!(source_definition, schema.def_key.fingerprint());
        let before = guest_snapshot(&run);
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        let QueryValue::Thunk(recipe) = first.value.as_ref() else {
            panic!("first named projection absent")
        };
        assert!(matches!(
            step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
            StepOutcome::Next(_)
        ));
        assert!(matches!(
            run.frame.source_view(destination).unwrap(),
            Some(TyTerm::Primitive(PrimitiveType::Isize))
        ));
        let mut reference = None;
        let mut result = None;
        for _ in 0..100 {
            let outcome = super::super::borrows::tests::next(&mut db, &mut run);
            let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
                panic!("stock field continuation absent")
            };
            for op in &recipe.recipe().operations {
                if let Op::ProjectField { base, destination } = op {
                    run.prepare_field(&mut db, *base, *destination)
                        .into_ready()
                        .unwrap()
                        .unwrap();
                }
            }
            let borrow = recipe.recipe().operations.iter().find_map(|op| {
                if let Op::Borrow { destination, .. } = op {
                    Some(*destination)
                } else {
                    None
                }
            });
            let read = recipe.recipe().operations.iter().find_map(|op| {
                if let Op::ReadPlace { destination, .. } = op {
                    Some(*destination)
                } else {
                    None
                }
            });
            assert!(matches!(
                step::step(&mut run.frame, &mut run.store, recipe).unwrap(),
                StepOutcome::Next(_)
            ));
            if let Some(destination) = borrow {
                reference = Some(
                    run.store
                        .reference_from_cell(run.frame.cell(destination).unwrap())
                        .unwrap(),
                );
                assert_eq!(run.frame.counters.started_by_opcode[33], 0);
            }
            if let Some(destination) = read {
                result = Some(
                    run.store
                        .preview_scalar(run.frame.cell(destination).unwrap(), None)
                        .unwrap(),
                );
                break;
            }
        }
        let (target, expected) = place_fixture_facts::assignment(STOCK, "d");
        assert_eq!(target, "p.y");
        let actual = result.expect("original *q was not reached");
        assert_eq!(
            actual.type_id,
            crate::frontend::call_contracts::primitive_runtime_id(PrimitiveType::Isize)
                .unwrap()
                .0
        );
        assert_eq!(actual.payload as i64, expected);
        assert_eq!(
            run.store.read_reference(reference.unwrap()).unwrap(),
            actual
        );
        assert_eq!(run.frame.counters.started_by_opcode[37], 2);
        assert_eq!(run.frame.counters.started_by_opcode[34], 1);
        assert_eq!(run.frame.counters.started_by_opcode[33], 1);
        assert_eq!(db.trace.counts.get("TypeSchema:ProviderRun"), Some(&1));
        assert!(!db.trace.counts.keys().any(|key| key.starts_with("Goal:")));
        // Ends at the original dereference. Aggregate scope cleanup and the
        // upstream compile-fail diagnostics are not exercised by this control.
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_named_field_without_dispatcher_binding_refuses_before_fuel_or_projection() {
        let (mut db, mut run, outcome, base, destination) = first_field();
        let QueryValue::Thunk(recipe) = outcome.value.as_ref() else {
            panic!("named field recipe absent")
        };
        let before = guest_snapshot(&run);
        let selected = recipe
            .recipe()
            .operations
            .iter()
            .find(|op| matches!(op, Op::ProjectField { .. }))
            .unwrap();
        let error = run
            .frame
            .prepare_projection(selected, &run.store)
            .err()
            .expect("unbound field projection was admitted");
        assert!(
            matches!(error, crate::runtime::place::PlaceFailure::RuntimeFault(ref error)
            if error.code == "LRR-E-INTERNAL-IR")
        );
        assert_eq!(guest_snapshot(&run), before);
        assert!(!run.frame.has_source_field_for_test());
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert!(
            run.frame
                .prepare_projection(selected, &run.store)
                .unwrap()
                .is_some()
        );
        assert_eq!(guest_snapshot(&run), before);
        db.finish_run().unwrap();
    }

    #[test]
    fn rustc_named_field_changed_caller_header_refuses_before_guest_effects() {
        let (mut db, mut run, _, base, destination) = first_field();
        let caller = run.frame.body.body().definition;
        let original = db.input(&InputKey::DefinitionHeader(caller)).unwrap();
        let InputValue::DefinitionHeader(header) = original.as_ref() else {
            panic!("stock caller header absent")
        };
        let mut changed = header.clone();
        changed.visibility = crate::syntax::outline::Visibility::Public;
        let mut inputs = run.sources[0].prepared_inputs().clone();
        inputs.insert(
            InputKey::DefinitionHeader(caller),
            InputValue::DefinitionHeader(changed),
        );
        db.finish_run().unwrap();
        db.publish_inputs(inputs).unwrap();
        db.begin_run().unwrap();
        let before = guest_snapshot(&run);
        let error = run
            .prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap_err();
        assert_eq!(error.code, "LRR-E-TYPE-SCHEMA");
        assert_eq!(guest_snapshot(&run), before);
        assert!(!run.frame.has_source_field_for_test());
        db.finish_run().unwrap();
        db.publish_inputs(run.sources[0].prepared_inputs().clone())
            .unwrap();
        db.begin_run().unwrap();
        run.prepare_field(&mut db, base, destination)
            .into_ready()
            .unwrap()
            .unwrap();
        assert_eq!(guest_snapshot(&run), before);
        db.finish_run().unwrap();
    }
}
