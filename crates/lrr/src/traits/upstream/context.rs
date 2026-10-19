//! The shared IR context over one immutable, prepared source/service handle.
//! This generic core has no fake source-fact provider or semantic fallback.
use super::{
    facts::SourceFacts,
    identity::DefinitionId,
    model::*,
    parameters::SourceParameter,
    storage::{InternPool, Interned},
};
use rustc_index::bit_set::DenseBitSet;
use rustc_type_ir::lang_items::{SolverAdtLangItem, SolverProjectionLangItem, SolverTraitLangItem};
use rustc_type_ir::search_graph::RequiredDepth;
use rustc_type_ir::solve::{inspect, QueryResult};
use rustc_type_ir::{self as ty, inherent::*, *};
use std::{
    cell::RefCell,
    fmt::{self, Debug},
    hash::{Hash, Hasher},
};

#[derive(Clone, Copy)]
pub(crate) struct Context<'cx, F: SourceFacts<'cx>> {
    state: &'cx State<'cx, F>,
}
impl<'cx, F: SourceFacts<'cx>> PartialEq for Context<'cx, F> {
    fn eq(&self, b: &Self) -> bool {
        std::ptr::eq(self.state, b.state)
    }
}
impl<'cx, F: SourceFacts<'cx>> Eq for Context<'cx, F> {}
impl<'cx, F: SourceFacts<'cx>> Hash for Context<'cx, F> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        std::ptr::from_ref(self.state).hash(h)
    }
}
impl<'cx, F: SourceFacts<'cx>> Debug for Context<'cx, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LrrUpstreamContext")
    }
}

pub(crate) struct State<'cx, F: SourceFacts<'cx>> {
    services: F,
    global: RefCell<ty::search_graph::GlobalCache<Context<'cx, F>>>,
    canonical_env: RefCell<ty::CanonicalParamEnvCache<Context<'cx, F>>>,
    types: InternPool<'cx, WithCachedTypeInfo<TyKind<Context<'cx, F>>>>,
    args: InternPool<'cx, Vec<Arg<'cx, F>>>,
    type_lists: InternPool<'cx, Vec<Ty<'cx, F>>>,
    const_lists: InternPool<'cx, Vec<Const<Context<'cx, F>>>>,
    regions: InternPool<'cx, RegionKind<Context<'cx, F>>>,
    constants: InternPool<'cx, WithCachedTypeInfo<ConstKind<Context<'cx, F>>>>,
    patterns: InternPool<'cx, WithCachedTypeInfo<PatternKind<Context<'cx, F>>>>,
    pattern_lists: InternPool<'cx, Vec<Pattern<'cx, F>>>,
    trees: InternPool<'cx, ValTreeKind<Context<'cx, F>>>,
    predicates: InternPool<
        'cx,
        WithCachedTypeInfo<Binder<Context<'cx, F>, PredicateKind<Context<'cx, F>>>>,
    >,
    clause_lists: InternPool<'cx, Vec<Clause<'cx, F>>>,
    clauses: InternPool<'cx, WithCachedTypeInfo<List<'cx, Clause<'cx, F>>>>,
    existentials:
        InternPool<'cx, Vec<Binder<Context<'cx, F>, ExistentialPredicate<Context<'cx, F>>>>>,
    bound_kinds: InternPool<'cx, Vec<BoundVariableKind<Context<'cx, F>>>>,
    canonical_kinds: InternPool<'cx, Vec<CanonicalVarKind<Context<'cx, F>>>>,
    opaques: InternPool<'cx, Vec<(OpaqueTypeKey<Context<'cx, F>>, Ty<'cx, F>)>>,
    local_ids: InternPool<'cx, Vec<DefinitionId>>,
    assumptions: InternPool<'cx, Vec<OutlivesClause<Context<'cx, F>, Arg<'cx, F>>>>,
    external: InternPool<'cx, ty::solve::ExternalConstraintsData<Context<'cx, F>>>,
    canonical_inputs: InternPool<'cx, ty::solve::CanonicalInputData<Context<'cx, F>>>,
    generic_parameters: InternPool<'cx, Vec<GenericParameter<'cx, F>>>,
}
impl<'cx, F: SourceFacts<'cx>> State<'cx, F> {
    pub(crate) fn new(services: F) -> Self {
        Self {
            services,
            global: RefCell::new(Default::default()),
            canonical_env: RefCell::new(Default::default()),
            types: InternPool::default(),
            args: InternPool::default(),
            type_lists: InternPool::default(),
            const_lists: InternPool::default(),
            regions: InternPool::default(),
            constants: InternPool::default(),
            patterns: InternPool::default(),
            pattern_lists: InternPool::default(),
            trees: InternPool::default(),
            predicates: InternPool::default(),
            clause_lists: InternPool::default(),
            clauses: InternPool::default(),
            existentials: InternPool::default(),
            bound_kinds: InternPool::default(),
            canonical_kinds: InternPool::default(),
            opaques: InternPool::default(),
            local_ids: InternPool::default(),
            assumptions: InternPool::default(),
            external: InternPool::default(),
            canonical_inputs: InternPool::default(),
            generic_parameters: InternPool::default(),
        }
    }
    pub(crate) fn context(&'cx self) -> Context<'cx, F> {
        Context { state: self }
    }
}
impl<'cx, F: SourceFacts<'cx>> Context<'cx, F> {
    pub(crate) fn services(self) -> F {
        self.state.services
    }
    pub(crate) fn ty(self, kind: TyKind<Self>) -> Ty<'cx, F> {
        let flags = FlagComputation::for_kind(&kind);
        Ty(self.state.types.intern(WithCachedTypeInfo {
            internee: kind,
            flags: flags.flags,
            outer_exclusive_binder: flags.outer_exclusive_binder,
        }))
    }
    pub(crate) fn predicate(self, kind: Binder<Self, PredicateKind<Self>>) -> Predicate<'cx, F> {
        let flags = FlagComputation::for_predicate(kind);
        Predicate(self.state.predicates.intern(WithCachedTypeInfo {
            internee: kind,
            flags: flags.flags,
            outer_exclusive_binder: flags.outer_exclusive_binder,
        }))
    }
    pub(crate) fn clauses(self, items: List<'cx, Clause<'cx, F>>) -> Clauses<'cx, F> {
        let flags = FlagComputation::<Self>::for_clauses(items.items());
        Clauses(self.state.clauses.intern(WithCachedTypeInfo {
            internee: items,
            flags: flags.flags,
            outer_exclusive_binder: flags.outer_exclusive_binder,
        }))
    }
    pub(crate) fn valtree(self, kind: ValTreeKind<Self>) -> ValTree<'cx, F> {
        ValTree(self.state.trees.intern(kind))
    }
    pub(crate) fn pattern(self, kind: PatternKind<Self>) -> Pattern<'cx, F> {
        let mut flags = TypeFlags::empty();
        let mut outer = INNERMOST;
        match kind {
            PatternKind::Range { start, end } => {
                flags |= start.flags() | end.flags();
                outer = start
                    .outer_exclusive_binder()
                    .max(end.outer_exclusive_binder());
            }
            PatternKind::Or(list) => {
                for p in list.iter() {
                    flags |= p.flags();
                    outer = outer.max(p.outer_exclusive_binder());
                }
            }
            PatternKind::NotNull => {}
        }
        Pattern(self.state.patterns.intern(WithCachedTypeInfo {
            internee: kind,
            flags,
            outer_exclusive_binder: outer,
        }))
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, Arg<'cx, F>> for Context<'cx, F> {
    fn list(self, values: Vec<Arg<'cx, F>>) -> List<'cx, Arg<'cx, F>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.args.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, Ty<'cx, F>> for Context<'cx, F> {
    fn list(self, values: Vec<Ty<'cx, F>>) -> List<'cx, Ty<'cx, F>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.type_lists.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, Const<Context<'cx, F>>> for Context<'cx, F> {
    fn list(self, values: Vec<Const<Context<'cx, F>>>) -> List<'cx, Const<Context<'cx, F>>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.const_lists.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, Pattern<'cx, F>> for Context<'cx, F> {
    fn list(self, values: Vec<Pattern<'cx, F>>) -> List<'cx, Pattern<'cx, F>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.pattern_lists.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, Clause<'cx, F>> for Context<'cx, F> {
    fn list(self, values: Vec<Clause<'cx, F>>) -> List<'cx, Clause<'cx, F>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.clause_lists.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>>
    AllocateList<'cx, Binder<Context<'cx, F>, ExistentialPredicate<Context<'cx, F>>>>
    for Context<'cx, F>
{
    fn list(
        self,
        values: Vec<Binder<Context<'cx, F>, ExistentialPredicate<Context<'cx, F>>>>,
    ) -> List<'cx, Binder<Context<'cx, F>, ExistentialPredicate<Context<'cx, F>>>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.existentials.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, BoundVariableKind<Context<'cx, F>>>
    for Context<'cx, F>
{
    fn list(
        self,
        values: Vec<BoundVariableKind<Context<'cx, F>>>,
    ) -> List<'cx, BoundVariableKind<Context<'cx, F>>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.bound_kinds.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, CanonicalVarKind<Context<'cx, F>>>
    for Context<'cx, F>
{
    fn list(
        self,
        values: Vec<CanonicalVarKind<Context<'cx, F>>>,
    ) -> List<'cx, CanonicalVarKind<Context<'cx, F>>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.canonical_kinds.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, (OpaqueTypeKey<Context<'cx, F>>, Ty<'cx, F>)>
    for Context<'cx, F>
{
    fn list(
        self,
        values: Vec<(OpaqueTypeKey<Context<'cx, F>>, Ty<'cx, F>)>,
    ) -> List<'cx, (OpaqueTypeKey<Context<'cx, F>>, Ty<'cx, F>)> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.opaques.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, DefinitionId> for Context<'cx, F> {
    fn list(self, values: Vec<DefinitionId>) -> List<'cx, DefinitionId> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.local_ids.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, OutlivesClause<Context<'cx, F>, Arg<'cx, F>>>
    for Context<'cx, F>
{
    fn list(
        self,
        values: Vec<OutlivesClause<Context<'cx, F>, Arg<'cx, F>>>,
    ) -> List<'cx, OutlivesClause<Context<'cx, F>, Arg<'cx, F>>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.assumptions.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> AllocateList<'cx, GenericParameter<'cx, F>> for Context<'cx, F> {
    fn list(self, values: Vec<GenericParameter<'cx, F>>) -> List<'cx, GenericParameter<'cx, F>> {
        if values.is_empty() {
            List::default()
        } else {
            List(Some(self.state.generic_parameters.intern(values)))
        }
    }
}
impl<'cx, F: SourceFacts<'cx>> ty::Interner for Context<'cx, F> {
    type DefId = DefinitionId;
    type LocalDefId = DefinitionId;
    type TraitId = DefinitionId;
    type ForeignId = DefinitionId;
    type FunctionId = DefinitionId;
    type ClosureId = DefinitionId;
    type CoroutineClosureId = DefinitionId;
    type CoroutineId = DefinitionId;
    type AdtId = DefinitionId;
    type ImplId = DefinitionId;
    type AnonConstId = DefinitionId;
    type TraitAssocTyId = DefinitionId;
    type TraitAssocConstId = DefinitionId;
    type TraitAssocTermId = DefinitionId;
    type OpaqueTyId = DefinitionId;
    type LocalOpaqueTyId = DefinitionId;
    type FreeTyAliasId = DefinitionId;
    type FreeConstAliasId = DefinitionId;
    type FreeTermAliasId = DefinitionId;
    type ImplOrTraitAssocTyId = DefinitionId;
    type ImplOrTraitAssocConstId = DefinitionId;
    type ImplOrTraitAssocTermId = DefinitionId;
    type InherentAssocTyId = DefinitionId;
    type InherentAssocConstId = DefinitionId;
    type InherentAssocTermId = DefinitionId;
    type AllocId = DefinitionId;
    type Span = Span;
    type GenericArgs = Args<'cx, F>;
    type GenericArgsSlice = &'cx [Arg<'cx, F>];
    type GenericArg = Arg<'cx, F>;
    type Term = Term<'cx, F>;
    type BoundVarKinds = List<'cx, ty::BoundVariableKind<Context<'cx, F>>>;
    type PredefinedOpaques = List<'cx, (ty::OpaqueTypeKey<Context<'cx, F>>, Ty<'cx, F>)>;
    type LocalDefIds = List<'cx, DefinitionId>;
    type CanonicalVarKinds = List<'cx, ty::CanonicalVarKind<Context<'cx, F>>>;
    type ExternalConstraints = Interned<'cx, ty::solve::ExternalConstraintsData<Context<'cx, F>>>;
    type DepNodeIndex = ();
    type Ty = Ty<'cx, F>;
    type Tys = Types<'cx, F>;
    type FnInputTys = &'cx [Ty<'cx, F>];
    type ParamTy = SourceParameter;
    type Symbol = Symbol<'cx>;
    type ErrorGuaranteed = ErrorToken;
    type BoundExistentialPredicates = Existentials<'cx, F>;
    type Pat = Pattern<'cx, F>;
    type PatList = List<'cx, Pattern<'cx, F>>;
    type Safety = Safety;
    type Consts = List<'cx, ty::Const<Context<'cx, F>>>;
    type ParamConst = SourceParameter;
    type ValueConst = ValueConst<'cx, F>;
    type ExprConst = ExprConst<'cx, F>;
    type ValTree = ValTree<'cx, F>;
    type ScalarInt = ScalarInt;
    type EarlyParamRegion = EarlyRegion<'cx>;
    type LateParamRegionKind = LateRegion<'cx>;
    type InternedRegionKind = Interned<'cx, ty::RegionKind<Context<'cx, F>>>;
    type InternedConstKind = Interned<'cx, ty::WithCachedTypeInfo<ty::ConstKind<Context<'cx, F>>>>;
    type RegionAssumptions = Regions<'cx, F>;
    type ParamEnv = ParamEnv<'cx, F>;
    type Predicate = Predicate<'cx, F>;
    type Clause = Clause<'cx, F>;
    type Clauses = Clauses<'cx, F>;
    type GenericsOf = Generics<'cx, F>;
    type VariancesOf = &'cx [ty::Variance];
    type AdtDef = AdtDefinition<'cx, F>;
    type Features = FeatureSet<'cx>;
    type UnsizingParams = &'cx DenseBitSet<u32>;
    type Probe = Box<ty::solve::inspect::Probe<Context<'cx, F>>>;
    type CanonicalInput = Interned<'cx, ty::solve::CanonicalInputData<Context<'cx, F>>>;
    type Tracked<T: Debug + Clone> = T;
    fn expand_abstract_consts<T: TypeFoldable<Self>>(self, t: T) -> T {
        self.services().expand_abstract_consts(self, t)
    }
    fn generics_of(self, def_id: Self::DefId) -> Self::GenericsOf {
        self.services().generics_of(self, def_id)
    }
    fn variances_of(self, def_id: Self::DefId) -> Self::VariancesOf {
        self.services().variances_of(self, def_id)
    }
    fn opt_alias_variances(
        self,
        kind: impl Into<ty::AliasTermKind<Self>>,
    ) -> Option<Self::VariancesOf> {
        self.services().opt_alias_variances(self, kind)
    }
    fn type_of(self, def_id: Self::DefId) -> ty::EarlyBinder<Self, Self::Ty> {
        self.services().type_of(self, def_id)
    }
    fn type_of_opaque_hir_typeck(
        self,
        def_id: Self::LocalOpaqueTyId,
    ) -> ty::EarlyBinder<Self, Self::Ty> {
        self.services().type_of_opaque_hir_typeck(self, def_id)
    }
    fn is_direct_const(self, alias: ty::AliasConstKind<Self>) -> bool {
        self.services().is_direct_const(self, alias)
    }
    fn const_of_item(
        self,
        alias: ty::AliasConstKind<Self>,
    ) -> Option<ty::EarlyBinder<Self, Const<Self>>> {
        self.services().const_of_item(self, alias)
    }
    fn anon_const_kind(self, def_id: Self::DefId) -> ty::AnonConstKind {
        self.services().anon_const_kind(self, def_id)
    }
    fn def_span(self, def_id: Self::DefId) -> Self::Span {
        self.services().def_span(self, def_id)
    }
    fn trait_ref_and_own_args_for_alias(
        self,
        def_id: Self::TraitAssocTermId,
        args: Self::GenericArgs,
    ) -> (ty::TraitRef<Self>, Self::GenericArgsSlice) {
        self.services()
            .trait_ref_and_own_args_for_alias(self, def_id, args)
    }
    fn check_alias_term_args_compatible(
        self,
        term_kind: AliasTermKind<Self>,
        args: Self::GenericArgs,
    ) -> bool {
        self.services()
            .check_alias_term_args_compatible(self, term_kind, args)
    }
    fn debug_assert_args_compatible(self, def_id: Self::DefId, args: Self::GenericArgs) {
        self.services()
            .debug_assert_args_compatible(self, def_id, args)
    }
    fn debug_assert_alias_term_args_compatible(
        self,
        term_kind: AliasTermKind<Self>,
        args: Self::GenericArgs,
    ) {
        self.services()
            .debug_assert_alias_term_args_compatible(self, term_kind, args)
    }
    fn debug_assert_existential_args_compatible(
        self,
        def_id: Self::DefId,
        args: Self::GenericArgs,
    ) {
        self.services()
            .debug_assert_existential_args_compatible(self, def_id, args)
    }
    fn projection_parent(self, def_id: Self::TraitAssocTermId) -> Self::TraitId {
        self.services().projection_parent(self, def_id)
    }
    fn impl_or_trait_assoc_term_parent(self, def_id: Self::ImplOrTraitAssocTermId) -> Self::DefId {
        self.services()
            .impl_or_trait_assoc_term_parent(self, def_id)
    }
    fn inherent_alias_term_parent(self, def_id: Self::InherentAssocTermId) -> Self::ImplId {
        self.services().inherent_alias_term_parent(self, def_id)
    }
    fn recursion_limit(self) -> usize {
        self.services().recursion_limit()
    }
    fn features(self) -> Self::Features {
        self.services().features()
    }
    fn assumptions_on_binders(self) -> bool {
        self.services().assumptions_on_binders()
    }
    fn renormalize_rigid_aliases(self) -> bool {
        self.services().renormalize_rigid_aliases()
    }
    fn coroutine_hidden_types(
        self,
        def_id: Self::CoroutineId,
    ) -> ty::EarlyBinder<Self, ty::Binder<Self, ty::CoroutineWitnessTypes<Self>>> {
        self.services().coroutine_hidden_types(self, def_id)
    }
    fn fn_sig(
        self,
        def_id: Self::FunctionId,
    ) -> ty::EarlyBinder<Self, ty::Binder<Self, ty::FnSig<Self>>> {
        self.services().fn_sig(self, def_id)
    }
    fn coroutine_movability(self, def_id: Self::CoroutineId) -> Movability {
        self.services().coroutine_movability(self, def_id)
    }
    fn coroutine_for_closure(self, def_id: Self::CoroutineClosureId) -> Self::CoroutineId {
        self.services().coroutine_for_closure(self, def_id)
    }
    fn generics_require_sized_self(self, def_id: Self::DefId) -> bool {
        self.services().generics_require_sized_self(self, def_id)
    }
    fn item_bounds(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = Self::Clause>> {
        self.services().item_bounds(self, def_id)
    }
    fn item_self_bounds(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = Self::Clause>> {
        self.services().item_self_bounds(self, def_id)
    }
    fn item_non_self_bounds(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = Self::Clause>> {
        self.services().item_non_self_bounds(self, def_id)
    }
    fn clauses_of(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = Self::Clause>> {
        self.services().clauses_of(self, def_id)
    }
    fn own_clauses_of(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = Self::Clause>> {
        self.services().own_clauses_of(self, def_id)
    }
    fn explicit_super_clauses_of(
        self,
        def_id: Self::TraitId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = (Self::Clause, Self::Span)>> {
        self.services().explicit_super_clauses_of(self, def_id)
    }
    fn explicit_implied_clauses_of(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = (Self::Clause, Self::Span)>> {
        self.services().explicit_implied_clauses_of(self, def_id)
    }
    fn impl_super_outlives(
        self,
        impl_def_id: Self::ImplId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = Self::Clause>> {
        self.services().impl_super_outlives(self, impl_def_id)
    }
    fn supertrait_def_ids(
        self,
        trait_def_id: Self::TraitId,
    ) -> impl Iterator<Item = Self::TraitId> {
        self.services().supertrait_def_ids(self, trait_def_id)
    }
    fn impl_is_const(self, def_id: Self::ImplId) -> bool {
        self.services().impl_is_const(self, def_id)
    }
    fn fn_is_const(self, def_id: Self::FunctionId) -> bool {
        self.services().fn_is_const(self, def_id)
    }
    fn closure_is_const(self, def_id: Self::ClosureId) -> bool {
        self.services().closure_is_const(self, def_id)
    }
    fn alias_has_const_conditions(self, def_id: Self::DefId) -> bool {
        self.services().alias_has_const_conditions(self, def_id)
    }
    fn const_conditions(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = ty::Binder<Self, ty::TraitRef<Self>>>> {
        self.services().const_conditions(self, def_id)
    }
    fn explicit_implied_const_bounds(
        self,
        def_id: Self::DefId,
    ) -> ty::EarlyBinder<Self, impl IntoIterator<Item = ty::Binder<Self, ty::TraitRef<Self>>>> {
        self.services().explicit_implied_const_bounds(self, def_id)
    }
    fn impl_self_is_guaranteed_unsized(self, def_id: Self::ImplId) -> bool {
        self.services()
            .impl_self_is_guaranteed_unsized(self, def_id)
    }
    fn has_target_features(self, def_id: Self::FunctionId) -> bool {
        self.services().has_target_features(self, def_id)
    }
    fn require_projection_lang_item(
        self,
        lang_item: SolverProjectionLangItem,
    ) -> Self::TraitAssocTyId {
        self.services()
            .require_projection_lang_item(self, lang_item)
    }
    fn require_trait_lang_item(self, lang_item: SolverTraitLangItem) -> Self::TraitId {
        self.services().require_trait_lang_item(self, lang_item)
    }
    fn require_adt_lang_item(self, lang_item: SolverAdtLangItem) -> Self::AdtId {
        self.services().require_adt_lang_item(self, lang_item)
    }
    fn is_projection_lang_item(
        self,
        def_id: Self::TraitAssocTyId,
        lang_item: SolverProjectionLangItem,
    ) -> bool {
        self.services()
            .is_projection_lang_item(self, def_id, lang_item)
    }
    fn is_trait_lang_item(self, def_id: Self::TraitId, lang_item: SolverTraitLangItem) -> bool {
        self.services().is_trait_lang_item(self, def_id, lang_item)
    }
    fn is_adt_lang_item(self, def_id: Self::AdtId, lang_item: SolverAdtLangItem) -> bool {
        self.services().is_adt_lang_item(self, def_id, lang_item)
    }
    fn is_default_trait(self, def_id: Self::TraitId) -> bool {
        self.services().is_default_trait(self, def_id)
    }
    fn is_sizedness_trait(self, def_id: Self::TraitId) -> bool {
        self.services().is_sizedness_trait(self, def_id)
    }
    fn as_projection_lang_item(
        self,
        def_id: Self::TraitAssocTyId,
    ) -> Option<SolverProjectionLangItem> {
        self.services().as_projection_lang_item(self, def_id)
    }
    fn as_trait_lang_item(self, def_id: Self::TraitId) -> Option<SolverTraitLangItem> {
        self.services().as_trait_lang_item(self, def_id)
    }
    fn as_adt_lang_item(self, def_id: Self::AdtId) -> Option<SolverAdtLangItem> {
        self.services().as_adt_lang_item(self, def_id)
    }
    fn associated_type_def_ids(
        self,
        def_id: Self::TraitId,
    ) -> impl IntoIterator<Item = Self::DefId> {
        self.services().associated_type_def_ids(self, def_id)
    }
    fn for_each_relevant_impl<R: VisitorResult>(
        self,
        trait_ref: TraitRef<Self>,
        f: impl FnMut(Self::ImplId) -> R,
    ) -> R {
        self.services().for_each_relevant_impl(self, trait_ref, f)
    }
    fn for_each_blanket_impl<R: VisitorResult>(
        self,
        trait_def_id: Self::TraitId,
        f: impl FnMut(Self::ImplId) -> R,
    ) -> R {
        self.services().for_each_blanket_impl(self, trait_def_id, f)
    }
    fn has_item_definition(self, def_id: Self::ImplOrTraitAssocTermId) -> bool {
        self.services().has_item_definition(self, def_id)
    }
    fn impl_specializes(self, impl_def_id: Self::ImplId, victim_def_id: Self::ImplId) -> bool {
        self.services()
            .impl_specializes(self, impl_def_id, victim_def_id)
    }
    fn impl_is_default(self, impl_def_id: Self::ImplId) -> bool {
        self.services().impl_is_default(self, impl_def_id)
    }
    fn impl_trait_ref(
        self,
        impl_def_id: Self::ImplId,
    ) -> ty::EarlyBinder<Self, ty::TraitRef<Self>> {
        self.services().impl_trait_ref(self, impl_def_id)
    }
    fn impl_polarity(self, impl_def_id: Self::ImplId) -> ty::ImplPolarity {
        self.services().impl_polarity(self, impl_def_id)
    }
    fn is_fully_generic_for_reflection(self, impl_def_id: Self::ImplId) -> bool {
        self.services()
            .is_fully_generic_for_reflection(self, impl_def_id)
    }
    fn trait_is_auto(self, trait_def_id: Self::TraitId) -> bool {
        self.services().trait_is_auto(self, trait_def_id)
    }
    fn trait_is_marker(self, trait_def_id: Self::TraitId) -> bool {
        self.services().trait_is_marker(self, trait_def_id)
    }
    fn trait_is_coinductive(self, trait_def_id: Self::TraitId) -> bool {
        self.services().trait_is_coinductive(self, trait_def_id)
    }
    fn trait_is_alias(self, trait_def_id: Self::TraitId) -> bool {
        self.services().trait_is_alias(self, trait_def_id)
    }
    fn trait_is_dyn_compatible(self, trait_def_id: Self::TraitId) -> bool {
        self.services().trait_is_dyn_compatible(self, trait_def_id)
    }
    fn trait_is_fundamental(self, def_id: Self::TraitId) -> bool {
        self.services().trait_is_fundamental(self, def_id)
    }
    fn trait_is_unsafe(self, trait_def_id: Self::TraitId) -> bool {
        self.services().trait_is_unsafe(self, trait_def_id)
    }
    fn is_impl_trait_in_trait(self, def_id: Self::DefId) -> bool {
        self.services().is_impl_trait_in_trait(self, def_id)
    }
    fn delay_bug(self, msg: impl ToString) -> Self::ErrorGuaranteed {
        self.services().delay_bug(self, msg)
    }
    fn span_delayed_bug(self, span: Self::Span, msg: impl ToString) -> Self::ErrorGuaranteed {
        self.services().span_delayed_bug(self, span, msg)
    }
    fn is_general_coroutine(self, coroutine_def_id: Self::CoroutineId) -> bool {
        self.services().is_general_coroutine(self, coroutine_def_id)
    }
    fn coroutine_is_async(self, coroutine_def_id: Self::CoroutineId) -> bool {
        self.services().coroutine_is_async(self, coroutine_def_id)
    }
    fn coroutine_is_gen(self, coroutine_def_id: Self::CoroutineId) -> bool {
        self.services().coroutine_is_gen(self, coroutine_def_id)
    }
    fn coroutine_is_async_gen(self, coroutine_def_id: Self::CoroutineId) -> bool {
        self.services()
            .coroutine_is_async_gen(self, coroutine_def_id)
    }
    fn unsizing_params_for_adt(self, adt_def_id: Self::AdtId) -> Self::UnsizingParams {
        self.services().unsizing_params_for_adt(self, adt_def_id)
    }
    fn anonymize_bound_vars<T: TypeFoldable<Self>>(
        self,
        binder: ty::Binder<Self, T>,
    ) -> ty::Binder<Self, T> {
        self.services().anonymize_bound_vars(self, binder)
    }
    fn opaque_types_defined_by(self, defining_anchor: Self::LocalDefId) -> Self::LocalDefIds {
        self.services()
            .opaque_types_defined_by(self, defining_anchor)
    }
    fn opaque_types_and_coroutines_defined_by(
        self,
        defining_anchor: Self::LocalDefId,
    ) -> Self::LocalDefIds {
        self.services()
            .opaque_types_and_coroutines_defined_by(self, defining_anchor)
    }
    fn mk_probe(self, probe: inspect::Probe<Self>) -> Self::Probe {
        self.services().mk_probe(self, probe)
    }
    fn evaluate_root_goal_for_proof_tree_raw(
        self,
        canonical_goal: Self::CanonicalInput,
        root_depth: usize,
    ) -> (QueryResult<Self>, Self::Probe, RequiredDepth) {
        self.services()
            .evaluate_root_goal_for_proof_tree_raw(self, canonical_goal, root_depth)
    }
    fn item_name(self, item_index: Self::DefId) -> Self::Symbol {
        self.services().item_name(self, item_index)
    }
    fn get_anon_re_bounds_lifetime(self, idx: usize, var_idx: usize) -> Option<Region<Self>> {
        self.services()
            .get_anon_re_bounds_lifetime(self, idx, var_idx)
    }
    fn get_anon_re_canonical_bounds_lifetime(self, idx: usize) -> Option<Region<Self>> {
        self.services()
            .get_anon_re_canonical_bounds_lifetime(self, idx)
    }

    fn mk_predefined_opaques_in_body(
        self,
        data: &[(OpaqueTypeKey<Self>, Self::Ty)],
    ) -> Self::PredefinedOpaques {
        self.list(data.to_vec())
    }
    fn mk_canonical_var_kinds(self, data: &[CanonicalVarKind<Self>]) -> Self::CanonicalVarKinds {
        self.list(data.to_vec())
    }
    fn mk_external_constraints(
        self,
        data: ty::solve::ExternalConstraintsData<Self>,
    ) -> Self::ExternalConstraints {
        self.state.external.intern(data)
    }
    fn mk_tracked<T: Debug + Clone>(self, data: T, _: ()) -> T {
        data
    }
    fn get_tracked<T: Debug + Clone>(self, data: &T) -> T {
        data.clone()
    }
    fn with_cached_task<T>(self, f: impl FnOnce() -> T) -> (T, ()) {
        (f(), ())
    }
    fn with_global_cache<R>(
        self,
        f: impl FnOnce(&mut ty::search_graph::GlobalCache<Self>) -> R,
    ) -> R {
        f(&mut self.state.global.borrow_mut())
    }
    fn with_canonical_param_env_cache<R>(
        self,
        f: impl FnOnce(&mut ty::CanonicalParamEnvCache<Self>) -> R,
    ) -> R {
        f(&mut self.state.canonical_env.borrow_mut())
    }
    fn assert_evaluation_is_concurrent(&self) {
        panic!("unexpected concurrent cache replacement in one LRR context")
    }
    fn adt_def(self, definition: DefinitionId) -> Self::AdtDef {
        AdtDefinition {
            context: self,
            definition,
        }
    }
    fn mk_args(self, args: &[Self::GenericArg]) -> Self::GenericArgs {
        self.list(args.to_vec())
    }
    fn mk_args_from_iter<I, T>(self, args: I) -> T::Output
    where
        I: Iterator<Item = T>,
        T: CollectAndApply<Self::GenericArg, Self::GenericArgs>,
    {
        T::collect_and_apply(args, |args| self.mk_args(args))
    }
    fn mk_type_list_from_iter<I, T>(self, args: I) -> T::Output
    where
        I: Iterator<Item = T>,
        T: CollectAndApply<Self::Ty, Self::Tys>,
    {
        T::collect_and_apply(args, |args| self.list(args.to_vec()))
    }
    fn mk_ct_from_kind(self, kind: ConstKind<Self>) -> Const<Self> {
        let flags = FlagComputation::for_const_kind(&kind);
        Const(self.state.constants.intern(WithCachedTypeInfo {
            internee: kind,
            flags: flags.flags,
            outer_exclusive_binder: flags.outer_exclusive_binder,
        }))
    }
    fn get_re_static_lifetime(self) -> Region<Self> {
        self.intern_region(RegionKind::ReStatic)
    }
    fn intern_re_var(self, rv: RegionVid) -> Region<Self> {
        self.intern_region(RegionKind::ReVar(rv))
    }
    fn intern_region(self, kind: RegionKind<Self>) -> Region<Self> {
        Region(self.state.regions.intern(kind))
    }
    fn intern_bound_region(
        self,
        debruijn: DebruijnIndex,
        region: BoundRegion<Self>,
    ) -> Region<Self> {
        self.intern_region(RegionKind::ReBound(
            BoundVarIndexKind::Bound(debruijn),
            region,
        ))
    }
    fn intern_canonical_bound(self, var: BoundVar) -> Region<Self> {
        self.intern_region(RegionKind::ReBound(
            BoundVarIndexKind::Canonical,
            BoundRegion {
                var,
                kind: BoundRegionKind::Anon,
            },
        ))
    }
    fn mk_canonical_input(self, data: ty::solve::CanonicalInputData<Self>) -> Self::CanonicalInput {
        self.state.canonical_inputs.intern(data)
    }
}
impl<'cx, F: SourceFacts<'cx>>
    AllocateValue<'cx, ty::solve::ExternalConstraintsData<Context<'cx, F>>> for Context<'cx, F>
{
    fn value(
        self,
        value: ty::solve::ExternalConstraintsData<Context<'cx, F>>,
    ) -> Interned<'cx, ty::solve::ExternalConstraintsData<Context<'cx, F>>> {
        self.state.external.intern(value)
    }
}

use super::model::{
    Clause, Clauses, ExprConst, ParamEnv, Pattern, Predicate, Safety, Span, Symbol, Term, Ty,
    ValueConst,
};
use rustc_type_ir::inherent::{Clause as _, Predicate as _, Ty as _};
