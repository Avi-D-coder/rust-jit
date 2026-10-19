//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
use super::{Const, DbInterner, InferenceContext, PreparedProvider, Span, Ty};
use crate::traits::upstream::model::{Arg, Args, Clauses};
use rustc_index::IndexVec;
use rustc_type_ir::data_structures::DelayedMap;
use rustc_type_ir::inherent::{GenericArg as _, Ty as _};
use rustc_type_ir::{
    self as ty, Interner, PredicateProxy, TypeFlags, TypeFoldable, TypeFolder, TypeSuperFoldable,
    TypeSuperVisitable, TypeVisitable, TypeVisitableExt, TypeVisitor, inherent::*,
};
type Canonical<'db, V> = ty::Canonical<DbInterner<'db>, V>;
type CanonicalQueryInput<'db, V> = ty::CanonicalQueryInput<DbInterner<'db>, V>;
type CanonicalVarValues<'db> = ty::CanonicalVarValues<DbInterner<'db>>;
type CanonicalVarKind<'db> = ty::CanonicalVarKind<DbInterner<'db>>;
type GenericArg<'db> = Arg<'db, PreparedProvider<'db>>;
type CanonicalClauses<'db> = Clauses<'db, PreparedProvider<'db>>;
pub(crate) trait CanonicalExt<'db, V> {
    fn instantiate(&self, interner: DbInterner<'db>, var_values: &CanonicalVarValues<'db>) -> V
    where
        V: TypeFoldable<DbInterner<'db>>;
    fn instantiate_projected<T>(
        &self,
        interner: DbInterner<'db>,
        var_values: &CanonicalVarValues<'db>,
        projection_fn: impl FnOnce(&V) -> T,
    ) -> T
    where
        T: TypeFoldable<DbInterner<'db>>;
}
impl<'db> InferenceContext<'db> {
    /// Creates an instantiation S for the canonical value with fresh inference
    /// variables and placeholders then applies it to the canonical value.
    /// Returns both the instantiated result *and* the instantiation S.
    ///
    /// This can be invoked as part of constructing an
    /// inference context at the start of a query (see
    /// `InferCtxtBuilder::build_with_canonical`). It basically
    /// brings the canonical value "into scope" within your new infcx.
    ///
    /// At the end of processing, the instantiation S (once
    /// canonicalized) then represents the values that you computed
    /// for each of the canonical inputs to your query.
    pub fn instantiate_canonical<T>(
        &self,
        span: Span,
        canonical: &Canonical<'db, T>,
    ) -> (T, CanonicalVarValues<'db>)
    where
        T: TypeFoldable<DbInterner<'db>>,
    {
        // For each universe that is referred to in the incoming
        // query, create a universe in our local inference context. In
        // practice, as of this writing, all queries have no universes
        // in them, so this code has no effect, but it is looking
        // forward to the day when we *do* want to carry universes
        // through into queries.
        //
        // Instantiate the root-universe content into the current universe,
        // and create fresh universes for the higher universes.
        let universes: IndexVec<ty::UniverseIndex, _> = std::iter::once(self.universe())
            .chain((1..=canonical.max_universe.as_u32()).map(|_| self.create_next_universe()))
            .collect();

        let var_values = CanonicalVarValues::instantiate(
            self.interner,
            canonical.var_kinds,
            |var_values, info| {
                self.instantiate_canonical_var(span, info, &var_values, |ui| universes[ui])
            },
        );
        let result = canonical.instantiate(self.interner, &var_values);
        (result, var_values)
    }

    /// Given the "info" about a canonical variable, creates a fresh
    /// variable for it. If this is an existentially quantified
    /// variable, then you'll get a new inference variable; if it is a
    /// universally quantified variable, you get a placeholder.
    ///
    /// FIXME(-Znext-solver): This is public because it's used by the
    /// new trait solver which has a different canonicalization routine.
    /// We should somehow deduplicate all of this.
    pub fn instantiate_canonical_var(
        &self,
        span: Span,
        kind: CanonicalVarKind<'db>,
        previous_var_values: &[GenericArg<'db>],
        universe_map: impl Fn(ty::UniverseIndex) -> ty::UniverseIndex,
    ) -> GenericArg<'db> {
        match kind {
            CanonicalVarKind::Ty { ui, sub_root } => {
                let vid = self.next_ty_var_id_in_universe(universe_map(ui), span);
                // If this inference variable is related to an earlier variable
                // via subtyping, we need to add that info to the inference context.
                if let Some(prev) = previous_var_values.get(sub_root.as_usize()) {
                    if let ty::Infer(ty::TyVar(sub_root)) = prev.expect_ty().kind() {
                        self.sub_unify_ty_vids_raw(vid, sub_root);
                    } else {
                        unreachable!()
                    }
                }
                Ty::new_var(self.interner, vid).into()
            }

            CanonicalVarKind::Int => self.next_int_var().into(),

            CanonicalVarKind::Float => {
                // There is no HirId available to pass as a lint_id.
                self.next_float_var().into()
            }

            CanonicalVarKind::PlaceholderTy(ty::PlaceholderType {
                universe, bound, ..
            }) => {
                let universe_mapped = universe_map(universe);
                let placeholder_mapped = ty::PlaceholderType::new(universe_mapped, bound);
                Ty::new_placeholder(self.interner, placeholder_mapped).into()
            }

            CanonicalVarKind::Region(ui) => self
                .next_region_var_in_universe(universe_map(ui), span)
                .into(),

            CanonicalVarKind::PlaceholderRegion(ty::PlaceholderRegion {
                universe, bound, ..
            }) => {
                let universe_mapped = universe_map(universe);
                let placeholder_mapped = ty::PlaceholderRegion::new(universe_mapped, bound);
                ty::Region::new_placeholder(self.interner, placeholder_mapped).into()
            }

            CanonicalVarKind::Const(ui) => self
                .next_const_var_in_universe(universe_map(ui), span)
                .into(),
            CanonicalVarKind::PlaceholderConst(ty::PlaceholderConst {
                universe, bound, ..
            }) => {
                let universe_mapped = universe_map(universe);
                let placeholder_mapped = ty::PlaceholderConst::new(universe_mapped, bound);
                ty::Const::new_placeholder(self.interner, placeholder_mapped).into()
            }
        }
    }
}

impl<'db> InferenceContext<'db> {
    pub fn build_with_canonical<T>(
        interner: DbInterner<'db>,
        span: Span,
        input: &CanonicalQueryInput<'db, T>,
    ) -> (InferenceContext<'db>, T, CanonicalVarValues<'db>)
    where
        T: TypeFoldable<DbInterner<'db>>,
    {
        let infcx = Self::new(interner, input.typing_mode.0);
        let (value, args) = infcx.instantiate_canonical(span, &input.canonical);
        (infcx, value, args)
    }
}
impl<'db, V> CanonicalExt<'db, V> for Canonical<'db, V> {
    /// Instantiate the wrapped value, replacing each canonical value
    /// with the value given in `var_values`.
    fn instantiate(&self, interner: DbInterner<'db>, var_values: &CanonicalVarValues<'db>) -> V
    where
        V: TypeFoldable<DbInterner<'db>>,
    {
        self.instantiate_projected(interner, var_values, |value| value.clone())
    }

    /// Allows one to apply a instantiation to some subset of
    /// `self.value`. Invoke `projection_fn` with `self.value` to get
    /// a value V that is expressed in terms of the same canonical
    /// variables bound in `self` (usually this extracts from subset
    /// of `self`). Apply the instantiation `var_values` to this value
    /// V, replacing each of the canonical variables.
    fn instantiate_projected<T>(
        &self,
        interner: DbInterner<'db>,
        var_values: &CanonicalVarValues<'db>,
        projection_fn: impl FnOnce(&V) -> T,
    ) -> T
    where
        T: TypeFoldable<DbInterner<'db>>,
    {
        assert_eq!(self.var_kinds.len(), var_values.len());
        let value = projection_fn(&self.value);
        instantiate_value(interner, var_values, value)
    }
}

/// Instantiate the values from `var_values` into `value`. `var_values`
/// must be values for the set of canonical variables that appear in
/// `value`.
pub(super) fn instantiate_value<'db, T>(
    interner: DbInterner<'db>,
    var_values: &CanonicalVarValues<'db>,
    value: T,
) -> T
where
    T: TypeFoldable<DbInterner<'db>>,
{
    if var_values.var_values.is_empty() {
        return value;
    }

    value.fold_with(&mut CanonicalInstantiator {
        interner,
        var_values: var_values.var_values,
        cache: Default::default(),
    })
}

/// Replaces the bound vars in a canonical binder with var values.
struct CanonicalInstantiator<'db> {
    interner: DbInterner<'db>,

    // The values that the bound vars are are being instantiated with.
    var_values: crate::traits::upstream::model::Args<'db, PreparedProvider<'db>>,

    // Because we use `ty::BoundVarIndexKind::Canonical`, we can cache
    // based only on the entire ty, not worrying about a `DebruijnIndex`
    cache: DelayedMap<Ty<'db>, Ty<'db>>,
}

impl<'db> TypeFolder<DbInterner<'db>> for CanonicalInstantiator<'db> {
    fn cx(&self) -> DbInterner<'db> {
        self.interner
    }

    fn fold_ty(&mut self, t: Ty<'db>) -> Ty<'db> {
        match t.kind() {
            ty::Bound(ty::BoundVarIndexKind::Canonical, bound_ty) => {
                self.var_values.items()[bound_ty.var.as_usize()].expect_ty()
            }
            _ => {
                if !t.has_type_flags(TypeFlags::HAS_CANONICAL_BOUND) {
                    t
                } else if let Some(&t) = self.cache.get(&t) {
                    t
                } else {
                    let res = t.super_fold_with(self);
                    assert!(self.cache.insert(t, res));
                    res
                }
            }
        }
    }

    fn fold_region(&mut self, r: ty::Region<DbInterner<'db>>) -> ty::Region<DbInterner<'db>> {
        match r.kind() {
            ty::RegionKind::ReBound(ty::BoundVarIndexKind::Canonical, br) => {
                self.var_values.items()[br.var.as_usize()].expect_region()
            }
            _ => r,
        }
    }

    fn fold_const(&mut self, ct: Const<'db>) -> Const<'db> {
        match ct.kind() {
            ty::ConstKind::Bound(ty::BoundVarIndexKind::Canonical, bound_const) => {
                self.var_values.items()[bound_const.var.as_usize()].expect_const()
            }
            _ => ct.super_fold_with(self),
        }
    }

    fn fold_predicate<P: PredicateProxy<DbInterner<'db>>>(&mut self, p: P) -> P {
        if p.has_type_flags(TypeFlags::HAS_CANONICAL_BOUND) {
            p.super_fold_with(self)
        } else {
            p
        }
    }

    fn fold_clauses(&mut self, c: CanonicalClauses<'db>) -> CanonicalClauses<'db> {
        if !c.has_type_flags(TypeFlags::HAS_CANONICAL_BOUND) {
            return c;
        }

        // Our cache key is `(clauses, var_values)`, but we also don't care about
        // var values that aren't named in the clauses, since they can change without
        // affecting the output. Since `ParamEnv`s are cached first, we compute the
        // last var value that is mentioned in the clauses, and cut off the list so
        // that we have more hits in the cache.

        // We also cache the computation of "highest var named by clauses" since that
        // is both expensive (depending on the size of the clauses) and a pure function.
        let index = *self
            .interner
            .instantiation_caches()
            .highest_var_in_clauses_cache
            .borrow_mut()
            .entry(c)
            .or_insert_with(|| highest_var_in_clauses(c));
        let c_args = &self.var_values.items()[..=index];

        if let Some(c) = self
            .interner
            .instantiation_caches()
            .clauses_cache
            .borrow_mut()
            .get(&(c, c_args))
            .copied()
        {
            c
        } else {
            let folded = c.super_fold_with(self);
            self.interner
                .instantiation_caches()
                .clauses_cache
                .borrow_mut()
                .insert((c, c_args), folded);
            folded
        }
    }
}

fn highest_var_in_clauses<'db>(c: CanonicalClauses<'db>) -> usize {
    struct HighestVarInClauses {
        max_var: usize,
    }
    impl<'db> TypeVisitor<DbInterner<'db>> for HighestVarInClauses {
        type Result = ();
        fn visit_ty(&mut self, t: Ty<'db>) {
            if let ty::Bound(ty::BoundVarIndexKind::Canonical, bound_ty) = t.kind() {
                self.max_var = self.max_var.max(bound_ty.var.as_usize());
            } else if t.has_type_flags(TypeFlags::HAS_CANONICAL_BOUND) {
                t.super_visit_with(self);
            }
        }
        fn visit_region(&mut self, r: ty::Region<DbInterner<'db>>) {
            if let ty::RegionKind::ReBound(ty::BoundVarIndexKind::Canonical, bound_region) =
                r.kind()
            {
                self.max_var = self.max_var.max(bound_region.var.as_usize());
            }
        }
        fn visit_const(&mut self, ct: Const<'db>) {
            if let ty::ConstKind::Bound(ty::BoundVarIndexKind::Canonical, bound_const) = ct.kind() {
                self.max_var = self.max_var.max(bound_const.var.as_usize());
            } else if ct.has_type_flags(TypeFlags::HAS_CANONICAL_BOUND) {
                ct.super_visit_with(self);
            }
        }
    }
    let mut visitor = HighestVarInClauses { max_var: 0 };
    c.visit_with(&mut visitor);
    visitor.max_var
}
