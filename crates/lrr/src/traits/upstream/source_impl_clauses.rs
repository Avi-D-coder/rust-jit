//! Bounded compiler clauses_of(Impl), from retained ordinary source inputs.
//! This is collection/IR transport; no applicability or root authorization.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    frontend::declaration_source_predicates::DeclarationSourcePredicatesKey,
    syntax::signatures::GenericKind,
    traits::{
        predicate::{PredicateResult, SourcePredicateRecipe},
        term::{BoundKind, ParamKey, TyTerm},
        upstream::{
            configuration::PreparedSolverConfiguration,
            identity::DefinitionId,
            model::{Clause, Span},
            parameters::{DeclarationParameters, DeclaredParameterPreparation},
            prepared::{Cx, PreparedProvider},
            source_bindings::SourceBindings,
            source_declaration_clauses::{
                PreparedSourceDeclarationClauses, SourceDeclarationClausesPending,
            },
            source_terms::{self, SourceTypePending},
        },
    },
};
use ra_ap_syntax::{AstNode, ast};
use ra_ap_syntax::ast::HasTypeBounds;
use rustc_type_ir::{self as ty, inherent::IntoKind};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceImplClausesPending {
    Gate(super::ImplSemanticsPending),
    Declaration(SourceDeclarationClausesPending),
    Source(SourceTypePending),
    /// Q80 has no row for the compiler's empty-where WellFormed producer.
    EmptyWhere,
    RecipeOrder,
    ClauseFamily,
}

#[derive(Debug)]
pub(super) struct PreparedSourceImplClauses {
    own: Arc<QueryValue>,
    implicit: BTreeMap<u32, Vec<usize>>,
    written: Vec<usize>,
    pending: Option<SourceImplClausesPending>,
}
impl PreparedSourceImplClauses {
    fn own(&self) -> &PreparedSourceDeclarationClauses {
        let QueryValue::SourceDeclarationClauses(own) = self.own.as_ref() else {
            unreachable!("captured actual own source clauses")
        };
        own
    }
    pub(super) fn pending(&self) -> Option<SourceImplClausesPending> {
        self.pending
    }
    pub(super) fn bindings(&self) -> Result<&SourceBindings, SourceImplClausesPending> {
        if let Some(why) = self.pending {
            return Err(why);
        }
        self.own()
            .bindings()
            .map_err(SourceImplClausesPending::Declaration)
    }
    pub(super) fn source(
        &self,
    ) -> &crate::frontend::declaration_source_predicates::PreparedDeclarationSourcePredicates {
        self.own().source()
    }
    pub(super) fn prepare(
        db: &mut Database,
        key: DeclarationSourcePredicatesKey,
    ) -> QueryPoll<Self> {
        let own = match db.fetch(
            &SyntaxProvider,
            QueryKey::SourceDeclarationClauses(Box::new(key.clone())),
        ) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let mut result = Self {
            own,
            implicit: BTreeMap::new(),
            written: Vec::new(),
            pending: None,
        };
        macro_rules! pending {
            ($why:expr) => {{
                result.pending = Some($why);
                return QueryPoll::Ready(Ok(result));
            }};
        }
        if result.own().key() != &key {
            return QueryPoll::Ready(Err(fault("actual own clause key differs")));
        }
        if let Some(why) = result.own().pending() {
            pending!(SourceImplClausesPending::Declaration(why));
        }
        let source_root = Arc::clone(&result.own);
        let QueryValue::SourceDeclarationClauses(own) = source_root.as_ref() else {
            unreachable!()
        };
        let source = own.source();
        let declaration = source.declaration();
        let Ok(DeclaredParameterPreparation::Ready(parameters)) =
            DeclarationParameters::prepare(declaration)
        else {
            pending!(SourceImplClausesPending::Source(
                SourceTypePending::ParameterLayout
            ));
        };
        if declaration.definition() != key.definition
            || source.parent_definition().is_some()
            || parameters.parent_count() != 0
        {
            pending!(SourceImplClausesPending::Source(
                SourceTypePending::ParameterLayout
            ));
        }
        let Some((_, _, context, parsed)) = declaration.source_parts() else {
            pending!(SourceImplClausesPending::Source(
                SourceTypePending::SourceDefinition
            ));
        };
        let syntax = match parsed.syntax_at(context.range, |node| ast::Impl::can_cast(node.kind()))
        {
            Ok(Some(node)) => node,
            Ok(None) => pending!(SourceImplClausesPending::Source(
                SourceTypePending::SourceDefinition
            )),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if syntax
            .children()
            .find_map(ast::WhereClause::cast)
            .is_some_and(|clause| {
                clause.predicates().any(|predicate| {
                    predicate
                        .type_bound_list()
                        .is_none_or(|list| list.bounds().next().is_none())
                })
            })
        {
            pending!(SourceImplClausesPending::EmptyWhere);
        }
        let recipes = match source.recipes() {
            Ok(recipes) => recipes,
            Err(why) => pending!(SourceImplClausesPending::Declaration(
                SourceDeclarationClausesPending::Source(why)
            )),
        };
        let mut written = false;
        for (ordinal, recipe) in recipes.iter().enumerate() {
            // Q84's actual TypeParameter application uses the whole parameter
            // source site. Written bounds retain their strictly nested bound
            // site. Q80 places all such defaults before the written recipes.
            let implicit_parameter = match recipe {
                SourcePredicateRecipe::Explicit {
                    source: site,
                    result: PredicateResult::Implemented(reference),
                } if matches!(reference.self_ty, TyTerm::Param(parameter)
                        if source.parameter_range(parameter) == Some(*site)) =>
                {
                    let TyTerm::Param(parameter) = reference.self_ty else {
                        unreachable!()
                    };
                    Some(parameter)
                }
                SourcePredicateRecipe::ResolvedSizedDefault(reference) => {
                    let TyTerm::Param(parameter) = reference.self_ty else {
                        pending!(SourceImplClausesPending::RecipeOrder);
                    };
                    Some(parameter)
                }
                _ => None,
            };
            if let Some(parameter) = implicit_parameter {
                if written || parameters.from_source(parameter, BoundKind::Type).is_err() {
                    pending!(SourceImplClausesPending::RecipeOrder);
                }
                result
                    .implicit
                    .entry(parameter.ordinal)
                    .or_default()
                    .push(ordinal);
                continue;
            }
            match recipe {
                SourcePredicateRecipe::Explicit { .. } => {
                    written = true;
                    result.written.push(ordinal);
                }
                _ => pending!(SourceImplClausesPending::RecipeOrder),
            }
        }
        QueryPoll::Ready(Ok(result))
    }
    /// Pinned gather_explicit_clauses_of: HIR parameter order, then written
    /// predicates; IndexSet dedup includes the actual source span. No Projection
    /// clause is admitted until its pinned constraining-order helper is ported.
    pub(super) fn encode<'cx>(
        &self,
        cx: Cx<'cx>,
        configuration: &PreparedSolverConfiguration,
    ) -> Result<
        (
            DefinitionId,
            Vec<(Clause<'cx, PreparedProvider<'cx>>, Span)>,
        ),
        SourceImplClausesPending,
    > {
        let bindings = self.bindings()?;
        self.own()
            .require_encoding_context(cx, configuration)
            .map_err(SourceImplClausesPending::Declaration)?;
        let source = self.source();
        let declaration = source.declaration();
        let implementation = *bindings.definitions.get(&declaration.definition()).ok_or(
            SourceImplClausesPending::Source(SourceTypePending::SourceDefinition),
        )?;
        let parameters = match DeclarationParameters::prepare(declaration) {
            Ok(DeclaredParameterPreparation::Ready(parameters)) => parameters,
            _ => {
                return Err(SourceImplClausesPending::Source(
                    SourceTypePending::ParameterLayout,
                ));
            }
        };
        let (parse, _, _, _) =
            declaration
                .source_parts()
                .ok_or(SourceImplClausesPending::Source(
                    SourceTypePending::SourceDefinition,
                ))?;
        let mut clauses = Vec::new();
        for parameter in declaration.parameters() {
            let key = ParamKey {
                definition: declaration.definition(),
                ordinal: parameter.ordinal,
            };
            match parameter.kind {
                GenericKind::Lifetime => {}
                GenericKind::Type => {
                    for ordinal in self.implicit.get(&parameter.ordinal).into_iter().flatten() {
                        if let Some(row) = self
                            .own()
                            .encode_recipe_at(cx, configuration, *ordinal)
                            .map_err(SourceImplClausesPending::Declaration)?
                        {
                            clauses.push(row);
                        }
                    }
                }
                GenericKind::Const => {
                    let ty = source
                        .generic_types()
                        .const_types()
                        .get(&parameter.ordinal)
                        .ok_or(SourceImplClausesPending::Source(
                            SourceTypePending::ConstTypeMetadata,
                        ))?;
                    let ty = source_terms::encode_declared(
                        cx,
                        configuration,
                        bindings.type_bindings(),
                        ty,
                    )
                    .map_err(SourceImplClausesPending::Source)?;
                    let constant = parameters.from_source(key, BoundKind::Const).map_err(|_| {
                        SourceImplClausesPending::Source(SourceTypePending::ParameterLayout)
                    })?;
                    if cx.services().source_parameter(key, BoundKind::Const) != Some(constant) {
                        return Err(SourceImplClausesPending::Source(
                            SourceTypePending::ParameterLayout,
                        ));
                    }
                    let constant = ty::Const::new(cx, ty::ConstKind::Param(constant));
                    let site =
                        source
                            .parameter_range(key)
                            .ok_or(SourceImplClausesPending::Source(
                                SourceTypePending::ParameterLayout,
                            ))?;
                    clauses.push((
                        Clause::new(
                            cx,
                            ty::Binder::dummy(ty::ClauseKind::ConstArgHasType(constant, ty)),
                        ),
                        Span {
                            origin: parse.content,
                            start: site.start,
                            end: site.end,
                        },
                    ));
                }
            }
        }
        for ordinal in &self.written {
            if let Some(row) = self
                .own()
                .encode_recipe_at(cx, configuration, *ordinal)
                .map_err(SourceImplClausesPending::Declaration)?
            {
                clauses.push(row);
            }
        }
        if clauses.iter().any(|(clause, _)| {
            !matches!(
                clause.kind().skip_binder(),
                ty::ClauseKind::Trait(_)
                    | ty::ClauseKind::RegionOutlives(_)
                    | ty::ClauseKind::TypeOutlives(_)
                    | ty::ClauseKind::ConstArgHasType(..)
            )
        }) {
            return Err(SourceImplClausesPending::ClauseFamily);
        }
        let mut seen = HashSet::new();
        clauses.retain(|row| seen.insert(*row));
        Ok((implementation, clauses))
    }
}
impl StableEncode for PreparedSourceImplClauses {
    const DOMAIN: &'static str = "lrr/v1/source-full-impl-clauses";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(self.own());
        e.u64(self.implicit.len() as u64);
        for (parameter, ordinals) in &self.implicit {
            e.u32(*parameter);
            e.u64(ordinals.len() as u64);
            for ordinal in ordinals {
                e.u64(*ordinal as u64);
            }
        }
        e.u64(self.written.len() as u64);
        for ordinal in &self.written {
            e.u64(*ordinal as u64);
        }
        // All holds are consequences of the retained query/source inputs; no
        // fabricated empty value participates in the memo's semantic payload.
        e.bool(self.pending.is_some());
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-IMPL-CLAUSES", message.into())
}

/// Observations of actual encoded rows, never replacement query inputs.
#[cfg(test)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum ImplClauseKindObservation {
    Trait(crate::core::digest::Digest),
    RegionOutlives,
    TypeOutlives,
    ConstArgHasType {
        parameter: ParamKey,
        declared_type: TyTerm,
    },
}
#[cfg(test)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct ImplClauseRowObservation {
    pub(crate) kind: ImplClauseKindObservation,
    pub(crate) parameters: Vec<ParamKey>,
    pub(crate) span: Span,
}
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ImplClausesObservation {
    pub(crate) implementation: crate::core::digest::Digest,
    pub(crate) source: crate::core::digest::Digest,
    pub(crate) rows: Vec<ImplClauseRowObservation>,
}

#[cfg(test)]
pub(crate) fn check_actual_full_impl_clauses(
    db: &mut Database,
    inventory: &crate::db::seals::SealKey,
    canonical: crate::core::digest::Digest,
) -> Result<ImplClausesObservation, Diagnostic> {
    use crate::{
        db::dependency::InputValue,
        traits::upstream::{
            configuration::SolverConfigurationKey, source_generics::SourceGenericLayouts,
        },
    };
    use rustc_type_ir::TypeVisitable;
    macro_rules! ready {
        ($query:expr) => {
            match $query {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(_) => {
                    return Err(fault("actual full impl clause control requires input"))
                }
            }
        };
    }
    let headers = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::PreparedScopeImplHeaders(inventory.clone())
    ))
    .value;
    let QueryValue::PreparedScopeImplHeaders(headers) = headers.as_ref() else {
        return Err(fault("actual impl inventory differs"));
    };
    let raw = headers
        .header_by_owner(canonical)
        .ok_or_else(|| fault("actual canonical impl header absent or ambiguous"))?
        .identity();
    let current = ready!(super::CurrentSourceImplSemantics::capture(
        db,
        &super::SourceImplSemanticsKey {
            inventory: inventory.clone(),
            implementation: raw,
        }
    ));
    current.require_current(db)?;
    let plan = current.plan();
    let bindings = plan
        .full_clause_bindings()
        .map_err(|why| fault(&format!("actual full impl bindings pending: {why:?}")))?;
    let input = Arc::clone(&bindings.catalog);
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        return Err(fault("actual impl clause catalog differs"));
    };
    let configuration = ready!(
        db.fetch(
            &SyntaxProvider,
            QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(
                catalog
                    .impl_catalog
                    .as_ref()
                    .ok_or_else(|| fault("actual impl clause catalog root absent"))?
                    .root(),
            )))
        )
    )
    .value;
    let QueryValue::SolverConfiguration(configuration) = configuration.as_ref() else {
        return Err(fault("actual impl clause configuration differs"));
    };
    let layouts = SourceGenericLayouts::from_bindings(Arc::clone(&input), [bindings])?;
    let before = db.trace.events.len();
    let observation = configuration.with_source_context(
        db,
        input,
        Some(&layouts),
        |cx, _| -> Result<ImplClausesObservation, Diagnostic> {
            let (owner, clauses) = plan
                .encode_complete_clauses(cx, configuration)
                .map_err(|why| fault(&format!("actual full impl encoding pending: {why:?}")))?;
            assert_eq!(owner.source(), canonical);
            assert_eq!(owner.world(), cx.services().source_world());
            let full = plan
                .full_clauses
                .as_ref()
                .expect("encoded retained full plan");
            let source = full.source();
            let (parse, _, _, _) = source
                .declaration()
                .source_parts()
                .expect("actual impl source");
            let mut rows = Vec::new();
            for (clause, span) in clauses {
                assert_eq!(span.origin, parse.content);
                let kind = match clause.kind().skip_binder() {
                    ty::ClauseKind::Trait(reference) => {
                        ImplClauseKindObservation::Trait(reference.trait_ref.def_id.source())
                    }
                    ty::ClauseKind::RegionOutlives(_) => ImplClauseKindObservation::RegionOutlives,
                    ty::ClauseKind::TypeOutlives(_) => ImplClauseKindObservation::TypeOutlives,
                    ty::ClauseKind::ConstArgHasType(constant, actual_type) => {
                        let ty::ConstKind::Param(parameter) = constant.kind() else {
                            panic!("actual declared Const clause lost its Param");
                        };
                        let key = parameter.source();
                        assert_eq!(key.definition, canonical);
                        let declared_type = source
                            .generic_types()
                            .const_types()
                            .get(&key.ordinal)
                            .expect("actual declared Const Type");
                        assert_eq!(
                            actual_type,
                            source_terms::encode_declared(
                                cx,
                                configuration,
                                bindings.type_bindings(),
                                declared_type
                            )
                            .unwrap()
                        );
                        let site = source
                            .parameter_range(key)
                            .expect("actual declared Const site");
                        assert_eq!((span.start, span.end), (site.start, site.end));
                        ImplClauseKindObservation::ConstArgHasType {
                            parameter: key,
                            declared_type: declared_type.clone(),
                        }
                    }
                    other => panic!("full impl output exceeded admitted families: {other:?}"),
                };
                let mut visitor = ClauseParameters {
                    parameters: Vec::new(),
                };
                clause.visit_with(&mut visitor);
                rows.push(ImplClauseRowObservation {
                    kind,
                    parameters: visitor.parameters,
                    span,
                });
            }
            Ok(ImplClausesObservation {
                implementation: canonical,
                source: plan.fingerprint(),
                rows,
            })
        },
    )??;
    assert_eq!(
        before,
        db.trace.events.len(),
        "full impl encoding must not query source"
    );
    current.require_current(db)?;
    Ok(observation)
}

#[cfg(test)]
struct ClauseParameters {
    parameters: Vec<ParamKey>,
}
#[cfg(test)]
impl ClauseParameters {
    fn retain(&mut self, key: ParamKey) {
        if !self.parameters.contains(&key) {
            self.parameters.push(key);
        }
    }
}
#[cfg(test)]
impl<'cx> rustc_type_ir::TypeVisitor<Cx<'cx>> for ClauseParameters {
    type Result = ();
    fn visit_ty(&mut self, value: crate::traits::upstream::model::Ty<'cx, PreparedProvider<'cx>>) {
        use rustc_type_ir::TypeSuperVisitable;
        if let ty::TyKind::Param(parameter) = value.kind() {
            self.retain(parameter.source());
        }
        value.super_visit_with(self);
    }
    fn visit_region(&mut self, value: ty::Region<Cx<'cx>>) {
        if let ty::RegionKind::ReEarlyParam(parameter) = value.kind() {
            self.retain(parameter.parameter.source());
        }
    }
    fn visit_const(&mut self, value: ty::Const<Cx<'cx>>) {
        use rustc_type_ir::TypeSuperVisitable;
        if let ty::ConstKind::Param(parameter) = value.kind() {
            self.retain(parameter.source());
        }
        value.super_visit_with(self);
    }
}
