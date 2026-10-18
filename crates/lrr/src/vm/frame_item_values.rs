//! Value-namespace items read as values through the ordinary `ValuePath` op.
//!
//! The dispatcher resolves the written path once per PC through the run's
//! namespace graph and the pure constant evaluator (the same route constant
//! patterns use) and installs the evaluated value here before dispatch. The
//! VM never resolves a name, evaluates an initializer or demands a schema; a
//! `ValuePath` whose PC selected a constructor keeps the nominal route.
use super::{Frame, invalid};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::{PcKey, SlotId},
    runtime::{cell::GuestStore, shape::TypeId, value::ValueRepr},
    traits::term::TyTerm,
};

/// The evaluated item value selected for one `ValuePath` PC.
#[derive(Debug, Clone)]
pub(crate) enum ItemValue {
    /// A scalar constant (or immutable static) value with its declared
    /// primitive source view.
    Scalar {
        value: ValueRepr,
        view: Option<crate::ir::op::PrimitiveType>,
    },
    /// A function item value: a fieldless `TypeKey::Callable` aggregate
    /// whose identity is the selected function definition. Its descriptor is
    /// interned by the dispatcher so the preflight is store-immutable.
    Function { ty: TypeId },
    /// A static addressed as a place: its run-global cell. The path's value
    /// slot stays unwritten; its `ValuePlace` roots at this cell.
    Static {
        cell: crate::runtime::cell::CellId,
        view: Option<TyTerm>,
    },
    /// A constant whose initializer ran through the interpreter and whose
    /// value is copied intrinsically (a scalar or shared reference): its one
    /// run-long cell, copied at each read (E-R5-007).
    ConstCell {
        cell: crate::runtime::cell::CellId,
        view: Option<TyTerm>,
    },
    /// A constant value its initializer produced for this one read.
    Ticket {
        ticket: crate::runtime::cell::TicketId,
        view: Option<TyTerm>,
    },
    /// A constant read in place context (`&CONST`): rustc promotes a borrow
    /// of plain-data constants to a `'static` location, one per site, and
    /// gives any other (interior-mutable, dropping) constant a fresh
    /// temporary per use. The inner value is that fresh use.
    Promotable(Box<ItemValue>),
    /// An aggregate (array) constant: its immutable evaluated value and
    /// selected layout, instantiated into a fresh value at each read.
    Aggregate {
        value: crate::ir::constant_data::ConstValue,
        wire: crate::ir::constant_data::wire::ConstantWireType,
        ty: TypeId,
        view: Option<TyTerm>,
    },
}

#[derive(Default)]
pub(crate) struct ItemValueFacts {
    value: Option<(PcKey, ItemValue)>,
    /// A static selected by a `ValuePath` whose value slot forms its place.
    static_place: Option<(SlotId, crate::runtime::cell::CellId, Option<TyTerm>)>,
}

impl Frame {
    pub(crate) fn prepare_item_value(&mut self, value: ItemValue) {
        self.item_values.value = Some((self.pc.clone(), value));
    }
    fn selected_item_value(&self) -> Option<&ItemValue> {
        self.item_values
            .value
            .as_ref()
            .filter(|(pc, _)| pc == &self.pc)
            .map(|(_, value)| value)
    }
    /// `ValuePath` preflight: an item value selected at this PC, otherwise
    /// the nominal constructor route.
    pub(crate) fn preflight_value_path(
        &self,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_item_value(self.selected_item_value(), destination, store)
    }
    fn preflight_item_value(
        &self,
        value: Option<&ItemValue>,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        match value {
            Some(ItemValue::Promotable(inner)) => {
                self.preflight_item_value(Some(inner), destination, store)
            }
            Some(ItemValue::Scalar { value, .. }) => self.preflight_literal(
                destination,
                Some(TypeId(value.type_id)),
                self.hint(destination)?,
                store,
            ),
            Some(ItemValue::Function { ty }) => {
                // A fn-pointer expectation coerces the item (E-R5-008): the
                // destination is admitted at its own pointer descriptor.
                match self.function_pointer_expectation(destination, store)? {
                    Some(pointer) => self.preflight_write_type(destination, pointer, store)?,
                    None => {
                        self.preflight_write_type(destination, *ty, store)?;
                        if let Some(cell) = self.slot(destination)?.cell {
                            store.validate_constructor_destination(cell, *ty)?;
                        }
                    }
                }
                Ok(())
            }
            Some(ItemValue::Static { cell, .. }) => {
                store.state(*cell)?;
                Ok(())
            }
            Some(ItemValue::Aggregate { ty, view, .. }) => {
                self.preflight_source_view(destination, view.as_ref())?;
                self.preflight_write_type(destination, *ty, store)
            }
            Some(ItemValue::ConstCell { cell, view }) => {
                let crate::runtime::cell::CellState::Live(value) = store.state(*cell)? else {
                    return Err(invalid("evaluated constant storage is not live"));
                };
                self.preflight_source_view(destination, view.as_ref())?;
                self.preflight_write_type(destination, TypeId(value.type_id), store)
            }
            Some(ItemValue::Ticket { ticket, view }) => {
                let value = store.peek_ticket(*ticket)?;
                self.preflight_source_view(destination, view.as_ref())?;
                self.preflight_write_type(destination, TypeId(value.type_id), store)
            }
            None => self.preflight_nominal_value(destination, store),
        }
    }
    /// The static place a `ValuePath` selected for value slot `source`.
    pub(crate) fn static_place_for(
        &self,
        source: SlotId,
    ) -> Option<(crate::runtime::cell::CellId, Option<&TyTerm>)> {
        self.item_values
            .static_place
            .as_ref()
            .filter(|(slot, ..)| *slot == source)
            .map(|(_, cell, view)| (*cell, view.as_ref()))
    }
    pub(crate) fn take_static_place(
        &mut self,
        source: SlotId,
    ) -> Option<(crate::runtime::cell::CellId, Option<TyTerm>)> {
        if self.static_place_for(source).is_none() {
            return None;
        }
        self.item_values
            .static_place
            .take()
            .map(|(_, cell, view)| (cell, view))
    }
    pub(crate) fn write_value_path(
        &mut self,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.selected_item_value().is_none() {
            return self.write_nominal_value(destination, store);
        }
        self.preflight_value_path(destination, store)?;
        let (_, value) = self
            .item_values
            .value
            .take()
            .ok_or_else(|| invalid("value path item selection consumed"))?;
        match value {
            ItemValue::Scalar { value, view } => {
                let ticket = store.ticket(value)?;
                self.write(destination, ticket, store)?;
                if let Some(primitive) = view {
                    self.set_source_view(destination, Some(TyTerm::Primitive(primitive)))?;
                }
                Ok(())
            }
            ItemValue::Function { ty } => {
                let pointer = self.function_pointer_expectation(destination, store)?.is_some();
                let cell = self.prepare_write(destination, store)?;
                if !pointer {
                    store.validate_constructor_destination(cell, ty)?;
                }
                let guard = store.begin_construction(&[])?;
                let ticket = store.commit_construction(guard, ty)?;
                if pointer {
                    // The function item meets its fn-pointer expectation by
                    // the shared pointer coercion.
                    let value = store.peek_ticket(ticket)?;
                    let expected = self
                        .write_unsizing(destination, value, store)?
                        .ok_or_else(|| invalid("function item does not coerce to its fn pointer"))?;
                    store.coerce_unsized_ticket(ticket, expected)?;
                }
                store.initialize(cell, ticket)
            }
            ItemValue::Static { cell, view } => {
                self.item_values.static_place = Some((destination, cell, view));
                Ok(())
            }
            ItemValue::ConstCell { cell, view } => {
                let ticket = store.copy_from(cell)?;
                self.write(destination, ticket, store)?;
                self.set_source_view(destination, view)
            }
            ItemValue::Ticket { ticket, view } => {
                self.write(destination, ticket, store)?;
                self.set_source_view(destination, view)
            }
            ItemValue::Promotable(inner) => {
                let (ticket, view) = match *inner {
                    ItemValue::Scalar { value, view } => {
                        (store.ticket(value)?, view.map(TyTerm::Primitive))
                    }
                    ItemValue::ConstCell { cell, view } => (store.copy_from(cell)?, view),
                    ItemValue::Ticket { ticket, view } => (ticket, view),
                    ItemValue::Aggregate {
                        value,
                        wire,
                        ty,
                        view,
                    } => {
                        let plan = store.prepare_constant_instantiation(&value, &wire, ty)?;
                        (store.instantiate_constant_value(plan, &mut self.fuel)?, view)
                    }
                    other => {
                        self.item_values.value = Some((self.pc.clone(), other));
                        return self.write_value_path(destination, store);
                    }
                };
                let value = store.peek_ticket(ticket)?;
                let ty = if store.value_promotable(value)? {
                    store.effective_type(value)?
                } else {
                    None
                };
                let Some(ty) = ty else {
                    // A fresh temporary for this use.
                    self.write(destination, ticket, store)?;
                    return self.set_source_view(destination, view);
                };
                let site = crate::core::digest::StableEncode::fingerprint(&self.pc);
                let promoted = match store.promoted_cell(site, ty) {
                    Some(promoted) => {
                        // Later executions address the installed location; the
                        // equal fresh value is an ordinary temporary.
                        self.write(destination, ticket, store)?;
                        self.set_source_view(destination, view.clone())?;
                        promoted
                    }
                    None => store.install_promoted(site, ty, ticket)?,
                };
                self.item_values.static_place = Some((destination, promoted, view));
                Ok(())
            }
            ItemValue::Aggregate {
                value,
                wire,
                ty,
                view,
            } => {
                let plan = store.prepare_constant_instantiation(&value, &wire, ty)?;
                let cell = self.prepare_write(destination, store)?;
                let ticket = store.instantiate_constant_value(plan, &mut self.fuel)?;
                store.initialize(cell, ticket)?;
                self.set_source_view(destination, view)
            }
        }
    }
}
