//! Pinned next-solver inference storage client; MIT OR Apache-2.0.
//! Pin ae0fe511b15ab760d7e963b13389d1bc2ab42b18; originals/licenses in vendor/rustc-trait-solver.
//! Registered mechanical port: tools/sync_upstream_inference.py.
pub(crate) use super::{model::Span, prepared::PreparedProvider};
use ena::unify as ut;
use opaque_types::OpaqueTypeStorage;
use region_constraints::{RegionConstraintCollector, RegionConstraintStorage};
use rustc_type_ir::{FloatVid, IntVid, TyVid, UniverseIndex};
use std::{
    cell::{Cell, RefCell},
    ops::Range,
    sync::Arc,
};
use tracing::debug;
use unify_key::ConstVidKey;
pub(crate) type DbInterner<'db> = super::context::Context<'db, PreparedProvider<'db>>;
pub(crate) type Ty<'db> = super::model::Ty<'db, PreparedProvider<'db>>;
pub(crate) type Const<'db> = rustc_type_ir::Const<DbInterner<'db>>;
pub(crate) type Region<'db> = rustc_type_ir::Region<DbInterner<'db>>;
pub(crate) type Binder<'db, T> = rustc_type_ir::Binder<DbInterner<'db>, T>;
pub(crate) type AliasTy<'db> = rustc_type_ir::AliasTy<DbInterner<'db>>;
pub(crate) type PlaceholderType<'db> = rustc_type_ir::PlaceholderType<DbInterner<'db>>;
pub(crate) type ParamTy = super::parameters::SourceParameter;
pub(crate) type OpaqueTypeKey<'db> = rustc_type_ir::OpaqueTypeKey<DbInterner<'db>>;
pub(crate) type ArgOutlivesPredicate<'db> =
    rustc_type_ir::OutlivesClause<DbInterner<'db>, super::model::Arg<'db, PreparedProvider<'db>>>;
pub(crate) type FxIndexMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;
mod higher_ranked;
pub(crate) mod opaque_types;
pub(crate) mod region_constraints;
pub(crate) mod snapshot;
pub(crate) mod solver_region_constraints;
use solver_region_constraints::SolverRegionConstraintStorage;
mod span;
pub(crate) mod type_variable;
pub(crate) mod unify_key;
/// Mutable storage only; this does not implement InferCtxtLike or SolverDelegate.
pub(crate) struct InferenceStorage<'db> {
    pub(crate) inner: RefCell<InferCtxtInner<'db>>,
    universe: Cell<UniverseIndex>,
}
impl<'db> InferenceStorage<'db> {
    pub(crate) fn new() -> Self {
        Self {
            inner: RefCell::new(InferCtxtInner::new()),
            universe: Cell::new(UniverseIndex::ROOT),
        }
    }
}
pub(crate) use snapshot::undo_log::InferCtxtUndoLogs;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) type UnificationTable<'a, 'db, T> = ut::UnificationTable<
    ut::InPlace<T, &'a mut ut::UnificationStorage<T>, &'a mut InferCtxtUndoLogs<'db>>,
>;

fn iter_idx_range<T: From<u32> + Into<u32>>(range: Range<T>) -> impl Iterator<Item = T> {
    (range.start.into()..range.end.into()).map(Into::into)
}

#[derive(Clone)]
pub struct InferCtxtInner<'db> {
    pub(crate) undo_log: InferCtxtUndoLogs<'db>,

    /// We instantiate `UnificationTable` with `bounds<Ty>` because the types
    /// that might instantiate a general type variable have an order,
    /// represented by its upper and lower bounds.
    pub(crate) type_variable_storage: type_variable::TypeVariableStorage<'db>,

    /// Map from const parameter variable to the kind of const it represents.
    pub(crate) const_unification_storage: ut::UnificationTableStorage<ConstVidKey<'db>>,

    /// Map from integral variable to the kind of integer it represents.
    pub(crate) int_unification_storage: ut::UnificationTableStorage<IntVid>,

    /// Map from floating variable to the kind of float it represents.
    pub(crate) float_unification_storage: ut::UnificationTableStorage<FloatVid>,

    /// Tracks the set of region variables and the constraints between them.
    ///
    /// This is initially `Some(_)` but when
    /// `resolve_regions_and_report_errors` is invoked, this gets set to `None`
    /// -- further attempts to perform unification, etc., may fail if new
    /// region constraints would've been added.
    pub(crate) region_constraint_storage: Option<RegionConstraintStorage<'db>>,

    pub(crate) solver_region_constraint_storage: SolverRegionConstraintStorage<'db>,

    /// A set of constraints that regionck must validate.
    ///
    /// Each constraint has the form `T:'a`, meaning "some type `T` must
    /// outlive the lifetime 'a". These constraints derive from
    /// instantiated type parameters. So if you had a struct defined
    /// like the following:
    /// ```ignore (illustrative)
    /// struct Foo<T: 'static> { ... }
    /// ```
    /// In some expression `let x = Foo { ... }`, it will
    /// instantiate the type parameter `T` with a fresh type `$0`. At
    /// the same time, it will record a region obligation of
    /// `$0: 'static`. This will get checked later by regionck. (We
    /// can't generally check these things right away because we have
    /// to wait until types are resolved.)
    ///
    /// These are stored in a map keyed to the id of the innermost
    /// enclosing fn body / static initializer expression. This is
    /// because the location where the obligation was incurred can be
    /// relevant with respect to which sublifetime assumptions are in
    /// place. The reason that we store under the fn-id, and not
    /// something more fine-grained, is so that it is easier for
    /// regionck to be sure that it has found *all* the region
    /// obligations (otherwise, it's easy to fail to walk to a
    /// particular node-id).
    ///
    /// Before running `resolve_regions_and_report_errors`, the creator
    /// of the inference context is expected to invoke
    /// `InferCtxt::process_registered_region_obligations`
    /// for each body-id in this map, which will process the
    /// obligations within. This is expected to be done 'late enough'
    /// that all type inference variables have been bound and so forth.
    pub(crate) region_obligations: Vec<TypeOutlivesConstraint<'db>>,

    /// The outlives bounds that we assume must hold about placeholders that
    /// come from instantiating the binder of coroutine-witnesses. These bounds
    /// are deduced from the well-formedness of the witness's types, and are
    /// necessary because of the way we anonymize the regions in a coroutine,
    /// which may cause types to no longer be considered well-formed.
    region_assumptions: Vec<ArgOutlivesPredicate<'db>>,

    /// Caches for opaque type inference.
    pub(crate) opaque_type_storage: OpaqueTypeStorage<'db>,
}

impl<'db> InferCtxtInner<'db> {
    fn new() -> InferCtxtInner<'db> {
        InferCtxtInner {
            undo_log: InferCtxtUndoLogs::default(),

            type_variable_storage: Default::default(),
            const_unification_storage: Default::default(),
            int_unification_storage: Default::default(),
            float_unification_storage: Default::default(),
            region_constraint_storage: Some(Default::default()),
            solver_region_constraint_storage: SolverRegionConstraintStorage::new(),
            region_obligations: vec![],
            region_assumptions: Default::default(),
            opaque_type_storage: Default::default(),
        }
    }

    #[inline]
    pub fn region_obligations(&self) -> &[TypeOutlivesConstraint<'db>] {
        &self.region_obligations
    }

    #[inline]
    fn try_type_variables_probe_ref(
        &self,
        vid: TyVid,
    ) -> Option<&type_variable::TypeVariableValue<'db>> {
        // Uses a read-only view of the unification table, this way we don't
        // need an undo log.
        self.type_variable_storage
            .eq_relations_ref()
            .try_probe_value(vid)
    }

    #[inline]
    fn type_variables(&mut self) -> type_variable::TypeVariableTable<'_, 'db> {
        self.type_variable_storage.with_log(&mut self.undo_log)
    }

    #[inline]
    pub(crate) fn opaque_types(&mut self) -> opaque_types::OpaqueTypeTable<'_, 'db> {
        self.opaque_type_storage.with_log(&mut self.undo_log)
    }

    #[inline]
    pub(crate) fn int_unification_table(&mut self) -> UnificationTable<'_, 'db, IntVid> {
        tracing::debug!(?self.int_unification_storage);
        self.int_unification_storage.with_log(&mut self.undo_log)
    }

    #[inline]
    pub(crate) fn float_unification_table(&mut self) -> UnificationTable<'_, 'db, FloatVid> {
        self.float_unification_storage.with_log(&mut self.undo_log)
    }

    #[inline]
    fn const_unification_table(&mut self) -> UnificationTable<'_, 'db, ConstVidKey<'db>> {
        self.const_unification_storage.with_log(&mut self.undo_log)
    }

    #[inline]
    pub fn unwrap_region_constraints(&mut self) -> RegionConstraintCollector<'db, '_> {
        self.region_constraint_storage
            .as_mut()
            .expect("region constraints already solved")
            .with_log(&mut self.undo_log)
    }
}

#[derive(Clone, Debug)]
pub struct TypeOutlivesConstraint<'db> {
    pub sub_region: Region<'db>,
    pub sup_type: Ty<'db>,
    pub origin: Span,
}

impl<'db> InferenceStorage<'db> {
    pub fn universe(&self) -> UniverseIndex {
        self.universe.get()
    }
    pub fn create_next_universe(&self) -> UniverseIndex {
        let u = self.universe.get().next_universe();
        debug!("create_next_universe {u:?}");
        self.universe.set(u);
        u
    }
}

mod context;
pub(crate) use context::InferenceContext;
mod resolve;
mod variables;

#[cfg(test)]
mod solver_region_constraints_tests;

mod obligation_state;

mod client;
mod client_operations;
mod generalize;

pub(crate) mod canonical;

impl<'db> InferCtxtInner<'db> {
    pub fn region_assumptions(&self) -> &[ArgOutlivesPredicate<'db>] {
        &self.region_assumptions
    }
}
mod region_response;
