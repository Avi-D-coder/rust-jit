//! Required prepared source facts and upstream client hooks. There is no
//! default/empty provider. Implementations must finish all necessary immutable
//! source/header preparation before solver entry; no callback IO, guest code,
//! macro execution or implicit negative answer for unavailable facts is allowed.
use super::{
    context::Context, identity::DefinitionId, model::*, parameters::SourceParameter,
    storage::Interned,
};
use rustc_index::bit_set::DenseBitSet;
use rustc_type_ir::lang_items::{SolverAdtLangItem, SolverProjectionLangItem, SolverTraitLangItem};
use rustc_type_ir::relate::{RelateResult, TypeRelation};
use rustc_type_ir::search_graph::RequiredDepth;
use rustc_type_ir::solve::{inspect, QueryResult};
use rustc_type_ir::{self as ty, *};
use std::fmt::Debug;

pub(crate) trait SourceFacts<'cx>:
    Sized + Copy + Debug + Eq + std::hash::Hash + 'cx
{
    fn expand_abstract_consts<T: TypeFoldable<Context<'cx, Self>>>(
        self,
        cx: Context<'cx, Self>,
        t: T,
    ) -> T;
    fn generics_of(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> Generics<'cx, Self>;
    fn variances_of(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> &'cx [ty::Variance];
    fn opt_alias_variances(
        self,
        cx: Context<'cx, Self>,
        kind: impl Into<ty::AliasTermKind<Context<'cx, Self>>>,
    ) -> Option<&'cx [ty::Variance]>;
    fn type_of(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, Ty<'cx, Self>>;
    fn type_of_opaque_hir_typeck(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, Ty<'cx, Self>>;
    fn is_direct_const(
        self,
        cx: Context<'cx, Self>,
        alias: ty::AliasConstKind<Context<'cx, Self>>,
    ) -> bool;
    fn const_of_item(
        self,
        cx: Context<'cx, Self>,
        alias: ty::AliasConstKind<Context<'cx, Self>>,
    ) -> Option<ty::EarlyBinder<Context<'cx, Self>, Const<Context<'cx, Self>>>>;
    fn anon_const_kind(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> ty::AnonConstKind;
    fn def_span(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> Span;
    fn trait_ref_and_own_args_for_alias(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
        args: Args<'cx, Self>,
    ) -> (ty::TraitRef<Context<'cx, Self>>, &'cx [Arg<'cx, Self>]);
    fn check_alias_term_args_compatible(
        self,
        cx: Context<'cx, Self>,
        term_kind: AliasTermKind<Context<'cx, Self>>,
        args: Args<'cx, Self>,
    ) -> bool;
    fn debug_assert_args_compatible(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
        args: Args<'cx, Self>,
    );
    fn debug_assert_alias_term_args_compatible(
        self,
        cx: Context<'cx, Self>,
        term_kind: AliasTermKind<Context<'cx, Self>>,
        args: Args<'cx, Self>,
    );
    fn debug_assert_existential_args_compatible(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
        args: Args<'cx, Self>,
    );
    fn projection_parent(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> DefinitionId;
    fn impl_or_trait_assoc_term_parent(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> DefinitionId;
    fn inherent_alias_term_parent(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> DefinitionId;
    fn recursion_limit(self) -> usize;
    fn features(self) -> FeatureSet<'cx>;
    fn assumptions_on_binders(self) -> bool;
    fn renormalize_rigid_aliases(self) -> bool;
    fn coroutine_hidden_types(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<
        Context<'cx, Self>,
        ty::Binder<Context<'cx, Self>, ty::CoroutineWitnessTypes<Context<'cx, Self>>>,
    >;
    fn fn_sig(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<
        Context<'cx, Self>,
        ty::Binder<Context<'cx, Self>, ty::FnSig<Context<'cx, Self>>>,
    >;
    fn coroutine_movability(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> Movability;
    fn coroutine_for_closure(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> DefinitionId;
    fn generics_require_sized_self(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn item_bounds(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = Clause<'cx, Self>>>;
    fn item_self_bounds(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = Clause<'cx, Self>>>;
    fn item_non_self_bounds(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = Clause<'cx, Self>>>;
    fn clauses_of(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = Clause<'cx, Self>>>;
    fn own_clauses_of(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = Clause<'cx, Self>>>;
    fn explicit_super_clauses_of(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = (Clause<'cx, Self>, Span)>>;
    fn explicit_implied_clauses_of(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = (Clause<'cx, Self>, Span)>>;
    fn impl_super_outlives(
        self,
        cx: Context<'cx, Self>,
        impl_def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, impl IntoIterator<Item = Clause<'cx, Self>>>;
    fn supertrait_def_ids(
        self,
        cx: Context<'cx, Self>,
        trait_def_id: DefinitionId,
    ) -> impl Iterator<Item = DefinitionId>;
    fn impl_is_const(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn fn_is_const(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn closure_is_const(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn alias_has_const_conditions(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn const_conditions(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<
        Context<'cx, Self>,
        impl IntoIterator<Item = ty::Binder<Context<'cx, Self>, ty::TraitRef<Context<'cx, Self>>>>,
    >;
    fn explicit_implied_const_bounds(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> ty::EarlyBinder<
        Context<'cx, Self>,
        impl IntoIterator<Item = ty::Binder<Context<'cx, Self>, ty::TraitRef<Context<'cx, Self>>>>,
    >;
    fn impl_self_is_guaranteed_unsized(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn has_target_features(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn require_projection_lang_item(
        self,
        cx: Context<'cx, Self>,
        lang_item: SolverProjectionLangItem,
    ) -> DefinitionId;
    fn require_trait_lang_item(
        self,
        cx: Context<'cx, Self>,
        lang_item: SolverTraitLangItem,
    ) -> DefinitionId;
    fn require_adt_lang_item(
        self,
        cx: Context<'cx, Self>,
        lang_item: SolverAdtLangItem,
    ) -> DefinitionId;
    fn is_projection_lang_item(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
        lang_item: SolverProjectionLangItem,
    ) -> bool;
    fn is_trait_lang_item(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
        lang_item: SolverTraitLangItem,
    ) -> bool;
    fn is_adt_lang_item(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
        lang_item: SolverAdtLangItem,
    ) -> bool;
    fn is_default_trait(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn is_sizedness_trait(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn as_projection_lang_item(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> Option<SolverProjectionLangItem>;
    fn as_trait_lang_item(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> Option<SolverTraitLangItem>;
    fn as_adt_lang_item(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> Option<SolverAdtLangItem>;
    fn associated_type_def_ids(
        self,
        cx: Context<'cx, Self>,
        def_id: DefinitionId,
    ) -> impl IntoIterator<Item = DefinitionId>;
    fn for_each_relevant_impl<R: VisitorResult>(
        self,
        cx: Context<'cx, Self>,
        trait_ref: TraitRef<Context<'cx, Self>>,
        f: impl FnMut(DefinitionId) -> R,
    ) -> R;
    fn for_each_blanket_impl<R: VisitorResult>(
        self,
        cx: Context<'cx, Self>,
        trait_def_id: DefinitionId,
        f: impl FnMut(DefinitionId) -> R,
    ) -> R;
    fn has_item_definition(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn impl_specializes(
        self,
        cx: Context<'cx, Self>,
        impl_def_id: DefinitionId,
        victim_def_id: DefinitionId,
    ) -> bool;
    fn impl_is_default(self, cx: Context<'cx, Self>, impl_def_id: DefinitionId) -> bool;
    fn impl_trait_ref(
        self,
        cx: Context<'cx, Self>,
        impl_def_id: DefinitionId,
    ) -> ty::EarlyBinder<Context<'cx, Self>, ty::TraitRef<Context<'cx, Self>>>;
    fn impl_polarity(self, cx: Context<'cx, Self>, impl_def_id: DefinitionId) -> ty::ImplPolarity;
    fn is_fully_generic_for_reflection(
        self,
        cx: Context<'cx, Self>,
        impl_def_id: DefinitionId,
    ) -> bool;
    fn trait_is_auto(self, cx: Context<'cx, Self>, trait_def_id: DefinitionId) -> bool;
    fn trait_is_marker(self, cx: Context<'cx, Self>, trait_def_id: DefinitionId) -> bool;
    fn trait_is_coinductive(self, cx: Context<'cx, Self>, trait_def_id: DefinitionId) -> bool;
    fn trait_is_alias(self, cx: Context<'cx, Self>, trait_def_id: DefinitionId) -> bool;
    fn trait_is_dyn_compatible(self, cx: Context<'cx, Self>, trait_def_id: DefinitionId) -> bool;
    fn trait_is_fundamental(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn trait_is_unsafe(self, cx: Context<'cx, Self>, trait_def_id: DefinitionId) -> bool;
    fn is_impl_trait_in_trait(self, cx: Context<'cx, Self>, def_id: DefinitionId) -> bool;
    fn delay_bug(self, cx: Context<'cx, Self>, msg: impl ToString) -> ErrorToken;
    fn span_delayed_bug(self, cx: Context<'cx, Self>, span: Span, msg: impl ToString)
        -> ErrorToken;
    fn is_general_coroutine(self, cx: Context<'cx, Self>, coroutine_def_id: DefinitionId) -> bool;
    fn coroutine_is_async(self, cx: Context<'cx, Self>, coroutine_def_id: DefinitionId) -> bool;
    fn coroutine_is_gen(self, cx: Context<'cx, Self>, coroutine_def_id: DefinitionId) -> bool;
    fn coroutine_is_async_gen(self, cx: Context<'cx, Self>, coroutine_def_id: DefinitionId)
        -> bool;
    fn unsizing_params_for_adt(
        self,
        cx: Context<'cx, Self>,
        adt_def_id: DefinitionId,
    ) -> &'cx DenseBitSet<u32>;
    fn opaque_types_defined_by(
        self,
        cx: Context<'cx, Self>,
        defining_anchor: DefinitionId,
    ) -> List<'cx, DefinitionId>;
    fn opaque_types_and_coroutines_defined_by(
        self,
        cx: Context<'cx, Self>,
        defining_anchor: DefinitionId,
    ) -> List<'cx, DefinitionId>;
    fn mk_probe(
        self,
        cx: Context<'cx, Self>,
        probe: inspect::Probe<Context<'cx, Self>>,
    ) -> Box<ty::solve::inspect::Probe<Context<'cx, Self>>>;
    fn evaluate_root_goal_for_proof_tree_raw(
        self,
        cx: Context<'cx, Self>,
        canonical_goal: Interned<'cx, ty::solve::CanonicalInputData<Context<'cx, Self>>>,
        root_depth: usize,
    ) -> (
        QueryResult<Context<'cx, Self>>,
        Box<ty::solve::inspect::Probe<Context<'cx, Self>>>,
        RequiredDepth,
    );
    fn item_name(self, cx: Context<'cx, Self>, item_index: DefinitionId) -> Symbol<'cx>;
    fn get_anon_re_bounds_lifetime(
        self,
        cx: Context<'cx, Self>,
        idx: usize,
        var_idx: usize,
    ) -> Option<Region<Context<'cx, Self>>>;
    fn get_anon_re_canonical_bounds_lifetime(
        self,
        cx: Context<'cx, Self>,
        idx: usize,
    ) -> Option<Region<Context<'cx, Self>>>;

    fn parameter_definition(
        self,
        cx: Context<'cx, Self>,
        parameter: SourceParameter,
    ) -> DefinitionId;
    fn adt_info(self, cx: Context<'cx, Self>, definition: DefinitionId) -> &'cx AdtInfo<'cx, Self>;
    fn field_representing_type_info(
        self,
        cx: Context<'cx, Self>,
        definition: DefinitionId,
        args: Args<'cx, Self>,
    ) -> Option<FieldInfo<Context<'cx, Self>>>;
    fn discriminant_type(self, cx: Context<'cx, Self>, ty: Ty<'cx, Self>) -> Ty<'cx, Self>;
    fn relate_const_expression<R: TypeRelation<Context<'cx, Self>>>(
        self,
        cx: Context<'cx, Self>,
        relation: &mut R,
        a: ExprConst<'cx, Self>,
        b: ExprConst<'cx, Self>,
    ) -> RelateResult<Context<'cx, Self>, ExprConst<'cx, Self>>;
    fn relate_type_pattern<R: TypeRelation<Context<'cx, Self>>>(
        self,
        cx: Context<'cx, Self>,
        relation: &mut R,
        a: Pattern<'cx, Self>,
        b: Pattern<'cx, Self>,
    ) -> RelateResult<Context<'cx, Self>, Pattern<'cx, Self>>;
    fn relate_existentials<R: TypeRelation<Context<'cx, Self>>>(
        self,
        cx: Context<'cx, Self>,
        relation: &mut R,
        a: Existentials<'cx, Self>,
        b: Existentials<'cx, Self>,
    ) -> RelateResult<Context<'cx, Self>, Existentials<'cx, Self>>;
}
