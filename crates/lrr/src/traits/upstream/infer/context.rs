//! Source-bound owner for actual upstream variable/term operations.
//! Actual inference-client ownership; full delegate/root readiness is separate.
use super::{DbInterner, InferenceStorage};
use crate::traits::upstream::{
    identity::DefinitionId,
    model::{Arg, Args, ErrorToken, GenericParameter, Span},
};
use rustc_type_ir::{self as ty, Interner, inherent::*};
use std::{
    cell::{Cell, RefCell},
    ops::Deref,
};

pub(crate) struct InferenceContext<'cx> {
    pub(crate) interner: DbInterner<'cx>,
    source_root: Cell<Option<crate::core::digest::Digest>>,
    storage: InferenceStorage<'cx>,
    pub(super) typing_mode: ty::TypingMode<DbInterner<'cx>>,
    pub(super) tainted_by_errors: Cell<Option<ErrorToken>>,
    pub(super) enable_next_solver_overflow_fcw: Cell<bool>,
    // Same ownership as pinned InferCtxt; upstream does not snapshot this map.
    pub(super) placeholder_assumptions_for_next_solver: RefCell<
        ty::data_structures::IndexMap<
            ty::UniverseIndex,
            ty::region_constraint::Assumptions<DbInterner<'cx>>,
        >,
    >,
}
impl<'cx> InferenceContext<'cx> {
    pub(crate) fn new(
        interner: DbInterner<'cx>,
        typing_mode: ty::TypingMode<DbInterner<'cx>>,
    ) -> Self {
        Self {
            interner,
            source_root: Cell::new(None),
            storage: InferenceStorage::new(),
            typing_mode,
            tainted_by_errors: Cell::new(None),
            enable_next_solver_overflow_fcw: Cell::new(
                interner.services().enable_next_solver_overflow_fcw(),
            ),
            placeholder_assumptions_for_next_solver: RefCell::default(),
        }
    }
    pub(in crate::traits::upstream) fn admit_source_root(
        &self,
        admission: &crate::traits::upstream::reached_call::SourceRootAdmission,
    ) {
        assert!(self.source_root.get().is_none());
        assert_eq!(
            self.interner.services().active_source_root(),
            Some(admission.identity())
        );
        self.source_root.set(Some(admission.identity()));
    }
    pub(super) fn inherit_canonical_source_root(&self) {
        self.source_root.set(Some(
            self.interner
                .services()
                .active_source_root()
                .expect("canonical descendant requires reached source root"),
        ));
    }
    pub(in crate::traits::upstream) fn require_source_root(&self) {
        let root = self
            .source_root
            .get()
            .expect("inference owner has no reached source root lineage");
        assert_eq!(
            self.interner.services().active_source_root(),
            Some(root),
            "inference source root scope ended or differs"
        );
    }
    /// The portable runtime type response has no region/opaque constraint
    /// transport yet. Preserve a lossless owned diagnostic and hold instead of
    /// certifying success while discarding any actual external constraint.
    pub(in crate::traits::upstream) fn source_root_external_constraints(&self) -> Option<String> {
        self.require_source_root();
        let regions = self.with_region_constraints(Clone::clone);
        let solver = self.get_solver_region_constraint();
        let opaques = self.clone_opaque_types();
        let inner = self.inner.borrow();
        let placeholders = self.placeholder_assumptions_for_next_solver.borrow();
        if regions.is_empty()
            && inner.region_obligations().is_empty()
            && inner.region_assumptions().is_empty()
            && solver.is_true()
            && opaques.is_empty()
            && placeholders.is_empty()
        {
            return None;
        }
        Some(format!(
            "regions={regions:?}; obligations={:?}; assumptions={:?}; solver={solver:?}; opaques={opaques:?}; placeholders={placeholders:?}",
            inner.region_obligations(),
            inner.region_assumptions()
        ))
    }
    /// Representation join: the actual table already retains parent-first rows.
    pub(crate) fn var_for_def(
        &self,
        parameter: GenericParameter<'cx, super::PreparedProvider<'cx>>,
        span: Span,
    ) -> Arg<'cx, super::PreparedProvider<'cx>> {
        match parameter {
            GenericParameter::Type(_) => self.next_ty_var(span).into(),
            GenericParameter::Region(_) => self.next_region_var(span).into(),
            GenericParameter::Const(_, _) => self.next_const_var(span).into(),
        }
    }
    pub(crate) fn fresh_args_for_item(
        &self,
        span: Span,
        definition: DefinitionId,
    ) -> Args<'cx, super::PreparedProvider<'cx>> {
        Args::for_item(self.interner, definition, |parameter, _| {
            self.var_for_def(parameter, span)
        })
    }
}
impl<'cx> Deref for InferenceContext<'cx> {
    type Target = InferenceStorage<'cx>;
    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}
