//! Static receiver Types for the source numeric constraint service: method
//! calls select their method with the dispatcher's own probe rule
//! (`MemberIndex::select_method`) from the receiver's Type as the ordinary
//! services state it (written annotations, declared parameters and results,
//! selected constructors and fields), and index operands take the builtin
//! array/slice index Type. An expression whose Type those services do not
//! state yields no selection, which stays a missing goal.
use super::{Component, ConstraintDemand, Step, entry, fault};
use crate::{
    core::digest::Digest,
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    facade::functions::LibraryFunction,
    frontend::member_index::BuiltinSelf,
    ir::{op::PrimitiveType, pc::NodeId},
    source::manifest::ItemKind,
    syntax::source_constraints::{FactKind, PatternStep, SourceOperator},
    traits::term::{ConstTerm, InferVarKey, PlaceholderKey, RegionTerm, TyTerm},
};

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// The written Type of a constant/static header: the tokens after its
/// top-level `:` up to its `=` (or a declaration-only `;`).
fn declared_type_tokens(
    tokens: &[crate::syntax::ast_adapter::TokenRecipe],
) -> Option<Vec<crate::syntax::ast_adapter::TokenRecipe>> {
    let mut depth = 0i32;
    let mut start = None;
    for (index, token) in tokens.iter().enumerate() {
        match token.text.as_str() {
            "<" | "(" | "[" => depth += 1,
            ">" | ")" | "]" => depth -= 1,
            ":" if depth == 0 && start.is_none() => start = Some(index + 1),
            "=" | ";" if depth == 0 => {
                let start = start?;
                return (index > start).then(|| tokens[start..index].to_vec());
            }
            _ => {}
        }
    }
    let start = start?;
    (tokens.len() > start).then(|| tokens[start..].to_vec())
}
/// A part of a static Type that only inference decides.
pub(super) fn open() -> TyTerm {
    TyTerm::Infer(InferVarKey {
        transaction: Digest::default(),
        ordinal: 0,
    })
}
/// A shared borrow's Type; regions and mutability do not affect selection.
fn borrowed(inner: TyTerm) -> TyTerm {
    TyTerm::Ref(RegionTerm::Static, false, Box::new(inner))
}
/// Whether a declared Type term mentions a generic parameter.
fn mentions_parameter(term: &TyTerm) -> bool {
    use crate::traits::term::GenericArg;
    match term {
        TyTerm::Param(_) | TyTerm::Projection(_) | TyTerm::Opaque(..) => true,
        TyTerm::Tuple(members) => members.iter().any(mentions_parameter),
        TyTerm::Array(inner, _) | TyTerm::Slice(inner) | TyTerm::Ref(_, _, inner) => {
            mentions_parameter(inner)
        }
        TyTerm::Nominal(_, args) => args.iter().any(|arg| match arg {
            GenericArg::Type(term) => mentions_parameter(term),
            _ => false,
        }),
        TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Never => false,
        _ => true,
    }
}
/// The autoderef'd receiver self type the probe selects from.
fn probe_self(mut term: &TyTerm) -> Option<Result<Digest, BuiltinSelf>> {
    while let TyTerm::Ref(_, _, inner) = term {
        term = inner;
    }
    Some(match term {
        TyTerm::Nominal(identity, _) => Ok(*identity),
        TyTerm::Primitive(primitive) => Err(BuiltinSelf::Primitive(*primitive)),
        TyTerm::Str => Err(BuiltinSelf::Str),
        TyTerm::Slice(_) => Err(BuiltinSelf::Slice),
        TyTerm::Array(..) => Err(BuiltinSelf::Array),
        _ => return None,
    })
}

impl<'a> Component<'a> {
    /// The static Type of an expression, without recording any relation or
    /// pending goal of this component.
    pub(super) fn expression_type(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        if let Some(known) = self.types.get(&node) {
            return QueryPoll::Ready(Ok(known.clone()));
        }
        // A cycle through a binding's own initializer answers nothing.
        self.types.insert(node, None);
        let saved = (self.pending, self.unrecorded, self.via.clone());
        let result = self.expression_type_inner(db, node);
        (self.pending, self.unrecorded, self.via) = saved;
        let found = ready!(result);
        self.types.insert(node, found.clone());
        QueryPoll::Ready(Ok(found))
    }
    fn expression_type_inner(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        QueryPoll::Ready(Ok(match fact.kind() {
            FactKind::Parenthesis | FactKind::Block => match fact.operand(0) {
                Some(inner) => ready!(self.expression_type(db, inner)),
                None => None,
            },
            FactKind::Borrow => match fact.operand(0) {
                Some(inner) => ready!(self.expression_type(db, inner)).map(borrowed),
                None => None,
            },
            FactKind::Prefix => {
                let inner = match fact.operand(0) {
                    Some(inner) => ready!(self.expression_type(db, inner)),
                    None => None,
                };
                match (fact.operation(), inner) {
                    (Some(SourceOperator::Deref), Some(TyTerm::Ref(_, _, inner))) => Some(*inner),
                    (
                        Some(SourceOperator::Negate | SourceOperator::Not),
                        Some(term @ (TyTerm::Primitive(_) | TyTerm::Infer(_))),
                    ) => Some(term),
                    _ => None,
                }
            }
            FactKind::Binary => match fact.operation() {
                Some(SourceOperator::Comparison | SourceOperator::Lazy) => {
                    Some(TyTerm::Primitive(PrimitiveType::Bool))
                }
                Some(SourceOperator::Assign | SourceOperator::CompoundArithmetic)
                | Some(SourceOperator::CompoundShift) => {
                    Some(TyTerm::Primitive(PrimitiveType::Unit))
                }
                _ => None,
            },
            FactKind::Integer | FactKind::Float => Some(open()),
            FactKind::SuffixedInteger | FactKind::SuffixedFloat => {
                fact.suffix().map(TyTerm::Primitive)
            }
            FactKind::Path => ready!(self.path_type(db, node)),
            FactKind::Record => ready!(self.constructed_type(db, node)),
            FactKind::Call => match ready!(self.constructed_type(db, node)) {
                Some(term) => Some(term),
                None => match ready!(self.callee(db, node)) {
                    Some(super::Callee::Source(definition)) => {
                        ready!(self.result_term(db, node, definition))
                    }
                    Some(super::Callee::Facade(LibraryFunction::MustUse)) => {
                        match fact.operand(1) {
                            Some(argument) => ready!(self.expression_type(db, argument)),
                            None => None,
                        }
                    }
                    _ => None,
                },
            },
            FactKind::MethodCall => match ready!(self.method_function(db, node)) {
                Some(function) => ready!(self.result_term(db, node, function)),
                None => None,
            },
            FactKind::Field => {
                let step = match (fact.field(), fact.member()) {
                    (Some(position), _) => Step::Field(position),
                    (None, Some(row)) => Step::Named(row),
                    (None, None) => return QueryPoll::Ready(Ok(None)),
                };
                match fact.operand(0) {
                    Some(base) => match ready!(self.expression_type(db, base)) {
                        Some(base) => {
                            ready!(self.project_term(db, &base, &[Step::AutoDeref, step]))
                        }
                        None => None,
                    },
                    None => None,
                }
            }
            FactKind::Index if fact.flag() => match fact.operand(0) {
                Some(base) => match ready!(self.expression_type(db, base)) {
                    Some(base) => {
                        ready!(self.project_term(db, &base, &[Step::AutoDeref, Step::Element]))
                    }
                    None => None,
                },
                None => None,
            },
            FactKind::Array => {
                let element = match fact.operand(0) {
                    Some(first) => ready!(self.expression_type(db, first)).unwrap_or_else(open),
                    None => open(),
                };
                // A repeat expression's count is a constant only its
                // evaluation states; an element list is its own length.
                let length = match u64::try_from(fact.operands().len()) {
                    Ok(length) if !fact.flag() => ConstTerm::Value(length),
                    _ => ConstTerm::Placeholder(PlaceholderKey {
                        transaction: Digest::default(),
                        universe: 0,
                        index: 0,
                    }),
                };
                Some(TyTerm::Array(Box::new(element), length))
            }
            FactKind::Tuple => {
                let mut members = Vec::with_capacity(fact.operands().len());
                for member in fact.operands().to_vec() {
                    members.push(match member {
                        Some(member) => {
                            ready!(self.expression_type(db, member)).unwrap_or_else(open)
                        }
                        None => open(),
                    });
                }
                Some(if members.is_empty() {
                    TyTerm::Primitive(PrimitiveType::Unit)
                } else {
                    TyTerm::Tuple(members)
                })
            }
            FactKind::Cast => ready!(self.annotation_term(db, node)),
            _ => None,
        }))
    }
    /// The written annotation of a fact as its Type term.
    pub(super) fn annotation_term(
        &mut self,
        db: &mut Database,
        owner: NodeId,
    ) -> QueryPoll<Option<TyTerm>> {
        let annotation = match self.fact(owner) {
            Ok(fact) => fact.annotation_node(),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(annotation) = annotation else {
            return QueryPoll::Ready(Ok(None));
        };
        QueryPoll::Ready(Ok(Some(ready!(self.written_term(db, annotation)))))
    }
    /// A record expression's or constructor call's selected nominal Type
    /// (its generic arguments are left to inference).
    fn constructed_type(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<Option<TyTerm>> {
        let constructed =
            ready!(self.constructor(db, super::nominal::ConstructorSite::Expression(node)));
        QueryPoll::Ready(Ok(constructed.map(|constructed| constructed.nominal_type())))
    }
    /// A path's selected local binding Type or own parameter Type.
    fn path_type(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<Option<TyTerm>> {
        let observed = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredValue(entry(self.key, node))
        ));
        let QueryValue::EnteredValue(observed) = observed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("binding use query variant differs")));
        };
        if observed.pending().is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(identity) = observed.local_binding() else {
            return match observed.declaration() {
                Some(declaration) => {
                    let identity = declaration.identity();
                    self.declared_item_type(db, observed, identity)
                }
                None => QueryPoll::Ready(Ok(None)),
            };
        };
        match ready!(self.binding_pattern(db, node, identity)) {
            Some(pattern) => self.binding_type(db, pattern),
            None => self.own_parameter_term(db, node),
        }
    }
    /// A constant's or static's declared Type: the written Type of its
    /// header (`const NAME: Type = ..`, `static NAME: Type = ..`, an
    /// associated `const NAME: Type`), resolved in its declaration scope as
    /// any written Type is. A module item is read from its source outline, a
    /// block-local one from the entered plan's item rows.
    pub(super) fn declared_item_type(
        &mut self,
        db: &mut Database,
        goal: &crate::frontend::source_value_goals::EnteredValueGoal,
        identity: Digest,
    ) -> QueryPoll<Option<TyTerm>> {
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let item = sources
            .iter()
            .find_map(|source| source.outline_item(identity))
            .cloned()
            .or_else(|| {
                goal.retained_plan()
                    .context()
                    .facts()
                    .items
                    .iter()
                    .find(|row| {
                        crate::core::digest::StableEncode::fingerprint(&row.item.def_key)
                            == identity
                    })
                    .map(|row| row.item.clone())
            });
        let Some(item) = item else {
            return QueryPoll::Ready(Ok(None));
        };
        if !matches!(
            item.def_key.item_kind,
            ItemKind::Constant | ItemKind::Static
        ) {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(written) = declared_type_tokens(&item.header.tokens) else {
            return QueryPoll::Ready(Ok(None));
        };
        let signature = crate::syntax::signatures::FunctionSignature {
            definition: identity,
            scope: item.scope.clone(),
            header: item.header.clone(),
            generics: Vec::new(),
            parameters: Vec::new(),
            result: Vec::new(),
            result_symbolic: None,
            predicates: Vec::new(),
        };
        match crate::frontend::source_types::resolve_written_type(
            db,
            environment.names(),
            &sources,
            &signature,
            &written,
        ) {
            QueryPoll::Ready(Ok(term)) => QueryPoll::Ready(Ok(term)),
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// A local's Type: its owner pattern's matched Type (a `let`
    /// annotation or initializer, or a `match` scrutinee) projected to the
    /// binding's position.
    fn binding_type(&mut self, db: &mut Database, pattern: NodeId) -> QueryPoll<Option<TyTerm>> {
        let plan = self.plan;
        let Some(occurrence) = plan.occurrence(pattern) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut matched = None;
        for edge in plan.uses(occurrence.root) {
            let Some(parent) = plan.fact(edge.parent) else {
                continue;
            };
            matched = match parent.kind() {
                FactKind::Let if edge.ordinal == 0 => {
                    match ready!(self.annotation_term(db, edge.parent)) {
                        Some(term) => Some(term),
                        None if parent.annotation().is_some() => None,
                        None => match parent.operand(1) {
                            Some(initializer) => ready!(self.expression_type(db, initializer)),
                            None => None,
                        },
                    }
                }
                FactKind::Match if edge.ordinal % 2 == 1 => match parent.operand(0) {
                    Some(scrutinee) => ready!(self.expression_type(db, scrutinee)),
                    None => None,
                },
                _ => None,
            };
            break;
        }
        let Some(matched) = matched else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut steps = Vec::with_capacity(occurrence.steps.len());
        for step in &occurrence.steps {
            steps.push(match *step {
                PatternStep::Deref => Step::Deref,
                PatternStep::Field(position) => Step::Field(position),
                PatternStep::Element => Step::Element,
                PatternStep::Constructor { slot, written } => match ready!(
                    self.occurrence_constructor_step(db, occurrence.root, slot, written)
                ) {
                    Some(step) => step,
                    None => return QueryPoll::Ready(Ok(None)),
                },
            });
        }
        let projected = ready!(self.project_term(db, &matched, &steps));
        QueryPoll::Ready(Ok(projected.map(|term| {
            if occurrence.by_reference {
                borrowed(term)
            } else {
                term
            }
        })))
    }
    /// The method a method call selects: the dispatcher's probe rule over
    /// the receiver's static Type.
    pub(super) fn method_function(
        &mut self,
        db: &mut Database,
        call: NodeId,
    ) -> QueryPoll<Option<Digest>> {
        let fact = match self.fact(call) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let (Some(receiver), Some(name)) = (
            fact.operand(0),
            fact.member().and_then(|row| self.plan.member(row)),
        ) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(receiver) = ready!(self.expression_type(db, receiver)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(self_type) = probe_self(&receiver) else {
            // A receiver of the function's own type parameter: its bounds'
            // declared method.
            let mut parameter = &receiver;
            while let TyTerm::Ref(_, _, inner) = parameter {
                parameter = inner;
            }
            if let TyTerm::Param(key) = parameter {
                let key = *key;
                let name = name.to_owned();
                return self.bound_method(db, &key, &name);
            }
            return QueryPoll::Ready(Ok(None));
        };
        let Some(environment) = ready!(self.environment(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // No candidate on the static receiver Type itself (an autoderef or
        // unindexed trait method) leaves the selection open; an ambiguity is
        // the dispatcher's own refusal and propagates.
        match members.select_method(self_type, name) {
            Ok((function, _)) => QueryPoll::Ready(Ok(Some(function))),
            Err(error) if super::not_closed(&error) => QueryPoll::Ready(Ok(None)),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    /// The selected method's declared signature.
    fn method_signature(
        &mut self,
        db: &mut Database,
        function: Digest,
    ) -> QueryPoll<Option<std::sync::Arc<QueryValue>>> {
        match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(function)) {
            QueryPoll::Ready(Ok(value)) => QueryPoll::Ready(Ok(Some(value.value))),
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// A method call's value is the selected method's declared result.
    pub(super) fn method_inward(&mut self, db: &mut Database, call: NodeId) -> QueryPoll<()> {
        match ready!(self.method_function(db, call)) {
            Some(function) => self.result(db, call, function),
            None => {
                self.defer(call, ConstraintDemand::MethodSelection);
                QueryPoll::Ready(Ok(()))
            }
        }
    }
    /// A method call operand: an argument takes its declared parameter; a
    /// receiver holding the variable inside its structure is related only
    /// when the selected method's other parameters or result mention the
    /// impl's type parameters (`[T]::len` relates nothing).
    pub(super) fn method_outward(
        &mut self,
        db: &mut Database,
        call: NodeId,
        ordinal: u32,
    ) -> QueryPoll<()> {
        if ordinal == 0 && self.via.is_scalar() {
            // The method selected depends on the variable's own Type: rustc's
            // trait candidates for a numeric inference variable.
            return self.numeric_receiver(db, call);
        }
        let Some(function) = ready!(self.method_function(db, call)) else {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        };
        let Some(signature) = ready!(self.method_signature(db, function)) else {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        };
        let QueryValue::FunctionSignature(signature) = signature.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint method signature differs")));
        };
        let receiver = signature
            .parameters
            .first()
            .is_some_and(|parameter| parameter.receiver);
        if !receiver {
            self.defer(call, ConstraintDemand::MethodSelection);
            return QueryPoll::Ready(Ok(()));
        }
        if ordinal > 0 {
            return self.declared_parameter(db, call, function, ordinal);
        }
        let parameters = u32::try_from(signature.parameters.len()).unwrap_or(u32::MAX);
        // The selected impl's self type states the receiver's own parts
        // (`impl GenericFoo<u32> { fn bar(self) }`): its declared receiver
        // Type, under the receiver expression's auto-ref/deref.
        let saved = (self.pending, self.unrecorded);
        let declared = ready!(self.parameter_term(db, call, function, 0));
        (self.pending, self.unrecorded) = saved;
        if let Some(mut declared) = declared {
            while let TyTerm::Ref(_, _, inner) = declared {
                declared = *inner;
            }
            let mut steps: &[Step] = &self.via.steps;
            while let Some((Step::Deref, rest)) = steps.split_first() {
                steps = rest;
            }
            let steps = steps.to_vec();
            if let super::generics::TermProjection::Leaf(TyTerm::Primitive(primitive)) =
                ready!(self.project_parametric(db, &declared, &steps))
            {
                return QueryPoll::Ready(self.constrain(call, primitive));
            }
        }
        // The declared goals, read without recording this component's own
        // pending demands (a missing goal is a relation not stated).
        let saved = (self.pending, self.unrecorded);
        let mut others_relate = false;
        for ordinal in 1..parameters {
            match ready!(self.parameter_term(db, call, function, ordinal)) {
                Some(term) if !mentions_parameter(&term) => {}
                _ => others_relate = true,
            }
        }
        if !others_relate {
            match ready!(self.result_term(db, call, function)) {
                Some(term) if !mentions_parameter(&term) => {}
                _ => others_relate = true,
            }
        }
        (self.pending, self.unrecorded) = saved;
        if others_relate {
            self.defer(call, ConstraintDemand::MethodSelection);
        }
        QueryPoll::Ready(Ok(()))
    }
    /// An index operand: rustc's builtin array/slice indexing takes `usize`.
    /// Another base (a `Vec`, a map, a custom `Index` impl) keeps the
    /// existing held route.
    pub(super) fn index_operand(&mut self, db: &mut Database, index: NodeId) -> QueryPoll<()> {
        let base = match self.fact(index) {
            Ok(fact) => fact.operand(0),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let base = match base {
            Some(base) => ready!(self.expression_type(db, base)),
            None => None,
        };
        let mut base = base.as_ref();
        while let Some(TyTerm::Ref(_, _, inner)) = base {
            base = Some(inner);
        }
        match base {
            Some(TyTerm::Array(..) | TyTerm::Slice(_)) if self.via.is_scalar() => {
                ready!(QueryPoll::Ready(
                    self.constrain(index, PrimitiveType::Usize)
                ));
            }
            _ => self.unrecorded = true,
        }
        QueryPoll::Ready(Ok(()))
    }
}
