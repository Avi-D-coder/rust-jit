//! Source-bound owner for actual upstream variable/term operations.
//! Full InferCtxtLike/delegate readiness is intentionally not claimed here.
use super::{DbInterner, InferenceStorage};
use crate::traits::upstream::{model::{ErrorToken, GenericParameter, Span, Arg, Args}, identity::DefinitionId};
use rustc_type_ir::{self as ty, Interner, inherent::*};
use std::{cell::{Cell, RefCell}, ops::Deref};

pub(crate) struct InferenceContext<'cx> {
    pub(crate) interner: DbInterner<'cx>,
    storage: InferenceStorage<'cx>,
    pub(super) typing_mode: ty::TypingMode<DbInterner<'cx>>,
    pub(super) tainted_by_errors: Cell<Option<ErrorToken>>,
    // Same ownership as pinned InferCtxt; upstream does not snapshot this map.
    pub(super) placeholder_assumptions_for_next_solver: RefCell<ty::data_structures::IndexMap<
        ty::UniverseIndex, ty::region_constraint::Assumptions<DbInterner<'cx>>>>,
}
impl<'cx> InferenceContext<'cx> {
    pub(crate) fn new(interner: DbInterner<'cx>, typing_mode: ty::TypingMode<DbInterner<'cx>>) -> Self {
        Self { interner, storage: InferenceStorage::new(), typing_mode, tainted_by_errors: Cell::new(None),
            placeholder_assumptions_for_next_solver: RefCell::default() }
    }
    /// Representation join: the actual table already retains parent-first rows.
    pub(crate) fn var_for_def(&self, parameter: GenericParameter<'cx, super::PreparedProvider<'cx>>, span: Span) -> Arg<'cx, super::PreparedProvider<'cx>> {
        match parameter {
            GenericParameter::Type(_) => self.next_ty_var(span).into(),
            GenericParameter::Region(_) => self.next_region_var(span).into(),
            GenericParameter::Const(_, _) => self.next_const_var(span).into(),
        }
    }
    pub(crate) fn fresh_args_for_item(&self, span: Span, definition: DefinitionId) -> Args<'cx, super::PreparedProvider<'cx>> {
        Args::for_item(self.interner, definition, |parameter, _| self.var_for_def(parameter, span))
    }
}
impl<'cx> Deref for InferenceContext<'cx> {
    type Target = InferenceStorage<'cx>;
    fn deref(&self) -> &Self::Target { &self.storage }
}
