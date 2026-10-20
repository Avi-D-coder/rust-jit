//! Nominal constructor values selected through the ordinary Value namespace.
//!
//! A unit struct or unit variant read as a value (`Op::ValuePath`) and a tuple
//! struct or tuple variant called as a constructor function both construct a
//! source-checked nominal aggregate: the type's run-local `TypeKey::Nominal`
//! descriptor, the selected variant ordinal, and the declaration-order fields
//! of that variant (none for a unit constructor). This is the same aggregate
//! representation record constructors commit, so field projection, pattern
//! tests (`GuestStore::aggregate_variant`) and declaration-order cleanup inspect
//! every nominal value identically.
//!
//! The dispatcher binds the variant witness before dispatch; the VM never
//! resolves a name or demands a schema.
use super::{Frame, invalid};
use crate::{
    core::diagnostic::Diagnostic,
    frontend::types::BoundNominalVariant,
    ir::pc::{PcKey, SlotId},
    runtime::{
        cell::{GuestStore, TicketId},
        shape::TypeId,
    },
    traits::term::TyTerm,
};
use std::{collections::BTreeMap, sync::Arc};

/// Run-local facts owned by one activation.
#[derive(Default)]
pub(crate) struct NominalFacts {
    /// Constructor calls selected in this activation, keyed like facade calls.
    calls: BTreeMap<super::super::call::CallIdentity, SelectedNominalCall>,
    /// The sealed constructor call (with its type arguments) awaiting its
    /// Call exit.
    sealed: Option<(Arc<BoundNominalVariant>, Vec<TypeId>, u32)>,
    /// Dispatcher-bound unit constructor for the value path at this exact PC.
    value: Option<(PcKey, Arc<BoundNominalVariant>, TypeId, Vec<TypeId>)>,
}

struct SelectedNominalCall {
    bound: Arc<BoundNominalVariant>,
    arguments: Option<Arc<[Option<TypeId>]>>,
}

impl Frame {
    /// The type arguments a generic nominal's result context fixes: the
    /// slot's reached descriptor hint naming the same declaration.
    pub(crate) fn nominal_hint_args(
        &self,
        slot: SlotId,
        bound: &BoundNominalVariant,
        store: &GuestStore,
    ) -> Result<Option<Vec<TypeId>>, Diagnostic> {
        if bound.type_parameters() == 0 {
            return Ok(Some(Vec::new()));
        }
        let Some(hint) = self.hint(slot)? else {
            return Ok(None);
        };
        Self::nominal_descriptor_args(hint, bound, store)
    }
    /// The type arguments of `descriptor` when it is the bound declaration's
    /// nominal Type.
    fn nominal_descriptor_args(
        descriptor: TypeId,
        bound: &BoundNominalVariant,
        store: &GuestStore,
    ) -> Result<Option<Vec<TypeId>>, Diagnostic> {
        Ok(match store.type_descriptor(descriptor)?.key.as_ref() {
            crate::runtime::shape::TypeKey::Nominal { identity, args, .. }
                if *identity == bound.identity()
                    && args.len() == bound.type_parameters() as usize =>
            {
                // A bare `dyn Trait` argument is a coercion target
                // (`&Fat<dyn T>` from `&Fat { ptr: Bar }`): the arguments are
                // left to the constructed fields.
                for arg in args.iter() {
                    if matches!(
                        store.type_descriptor(*arg)?.key.as_ref(),
                        crate::runtime::shape::TypeKey::Dyn { .. }
                    ) {
                        return Ok(None);
                    }
                }
                Some(args.to_vec())
            }
            _ => None,
        })
    }
    /// The source view of a constructed nominal value: its descriptor's closed
    /// term (`W<i32>`), or the bare declaration when it has no arguments.
    pub(crate) fn constructed_source(&self, ty: TypeId, store: &GuestStore) -> Option<TyTerm> {
        store
            .descriptor_source_term(ty)
            .filter(|term| Self::supported_source(term))
    }
    /// Install the bound unit constructor for the `ValuePath` at this PC. The
    /// descriptor is interned here so the opcode preflight is store-immutable.
    /// A generic unit variant takes its type arguments from its context.
    pub(crate) fn prepare_nominal_value(
        &mut self,
        bound: Arc<BoundNominalVariant>,
        stated: Option<&crate::ir::written_type::WrittenType>,
        explicit: Option<&[Option<TypeId>]>,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if !bound.fields().is_empty() {
            return Err(invalid("value path constructor must be fieldless"));
        }
        let result = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .and_then(|node| node.value)
            .ok_or_else(|| invalid("value path result slot absent"))?;
        let args = self.nominal_type_arguments(result, &bound, stated, explicit, store)?
            .into_iter().collect::<Option<Vec<_>>>()
            .ok_or_else(|| Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "generic unit variant requires its type context".into(),
            ))?;
        let ty = store.intern_nominal_with(&bound, &args)?;
        self.nominal.value = Some((self.pc.clone(), bound, ty, args));
        Ok(())
    }
    pub(crate) fn preflight_nominal_value(
        &self,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let Some((pc, bound, ty, _)) = &self.nominal.value else {
            return Err(invalid("value path lacks its dispatcher selection"));
        };
        if pc != &self.pc || !bound.fields().is_empty() {
            return Err(invalid("value path selection belongs to another PC"));
        }
        self.preflight_write_type(destination, *ty, store)?;
        if let Some(cell) = self.slot(destination)?.cell {
            store.validate_constructor_destination(cell, *ty)?;
        }
        Ok(())
    }
    /// Construct the fieldless nominal value and publish its source identity.
    pub(crate) fn write_nominal_value(
        &mut self,
        destination: SlotId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_nominal_value(destination, store)?;
        let (_, bound, ty, args) = self
            .nominal
            .value
            .take()
            .ok_or_else(|| invalid("value path selection consumed"))?;
        let cell = self.prepare_write(destination, store)?;
        store.validate_constructor_destination(cell, ty)?;
        let (constructed, guard) = store.begin_nominal_construction_with(&bound, &args)?;
        if constructed != ty {
            return Err(invalid("value path descriptor changed after selection"));
        }
        let ticket = store.commit_construction(guard, ty)?;
        store.initialize(cell, ticket)?;
        let source = self.constructed_source(ty, store);
        self.set_source_view(destination, source)
    }
    /// One seed operation for nominal values, argument hints and call sealing.
    /// Source selection is already complete; only activation descriptors live here.
    fn nominal_type_arguments(
        &self,
        result: SlotId,
        bound: &BoundNominalVariant,
        stated: Option<&crate::ir::written_type::WrittenType>,
        explicit: Option<&[Option<TypeId>]>,
        store: &mut GuestStore,
    ) -> Result<Vec<Option<TypeId>>, Diagnostic> {
        let context = match self.nominal_hint_args(result, bound, store)? {
            Some(args) => Some(args),
            None => match stated {
                Some(written) => match self.materialize_written_hint(written, store)? {
                    Some(descriptor) => Self::nominal_descriptor_args(descriptor, bound, store)?,
                    None => None,
                },
                None => None,
            },
        };
        let count = bound.type_parameters() as usize;
        let mut fixed = match context {
            Some(args) => args.into_iter().map(Some).collect(),
            None => self
                .partial_nominal_args(result, bound, store)?
                .unwrap_or_else(|| vec![None; count]),
        };
        if let Some(explicit) = explicit {
            if explicit.len() > count {
                return Err(Diagnostic::new(
                    "LRR-E-GENERIC-BINDING",
                    "constructor Type argument count differs from the declaration".into(),
                ));
            }
            for (slot, argument) in fixed.iter_mut().zip(explicit) {
                if let Some(argument) = argument {
                    *slot = Some(*argument);
                }
            }
        }
        Ok(fixed)
    }
    pub(crate) fn select_constructor_call(
        &mut self,
        site: u32,
        bound: Arc<BoundNominalVariant>,
        arguments: Option<Arc<[Option<TypeId>]>>,
    ) {
        let identity = self.call_identity(site);
        self.nominal
            .calls
            .insert(identity, SelectedNominalCall { bound, arguments });
    }
    pub(crate) fn selected_constructor_call(&self, site: u32) -> Option<&Arc<BoundNominalVariant>> {
        self.nominal
            .calls
            .get(&self.call_identity(site))
            .map(|selected| &selected.bound)
    }
    pub(crate) fn constructor_type_arguments(
        &self,
        site: u32,
        store: &mut GuestStore,
    ) -> Result<Vec<Option<TypeId>>, Diagnostic> {
        let selected = self
            .nominal
            .calls
            .get(&self.call_identity(site))
            .ok_or_else(|| invalid("constructor Type arguments lack their selection"))?;
        let result = self
            .body
            .nodes()
            .get(site as usize)
            .and_then(|node| node.value)
            .ok_or_else(|| invalid("constructor call result slot absent"))?;
        self.nominal_type_arguments(
            result,
            &selected.bound,
            None,
            selected.arguments.as_deref(),
            store,
        )
    }
    /// Seal the evaluated arguments of a selected constructor call. Concrete
    /// fields require their bound Types; a generic declaration's parameter is
    /// fixed by its context, else by the first argument at that parameter (an
    /// unconstrained literal taking rustc's integer/float fallback).
    pub(crate) fn seal_constructor_call(
        &mut self,
        site: u32,
        arguments: &[SlotId],
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        use crate::frontend::types::NominalFieldType;
        let bound = self
            .selected_constructor_call(site)
            .cloned()
            .ok_or_else(|| invalid("constructor call seal lacks its selection"))?;
        if arguments.len() != bound.fields().len() {
            return Err(Diagnostic::new(
                "LRR-E-CALL-ARITY",
                "evaluated written argument count differs from the constructor fields".into(),
            ));
        }
        let mut args = self.constructor_type_arguments(site, store)?;
        let mut cells = Vec::new();
        let mut types = Vec::new();
        cells
            .try_reserve_exact(arguments.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        types
            .try_reserve_exact(arguments.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for (slot, field) in arguments.iter().zip(bound.fields()) {
            let cell = self.cell(*slot)?;
            let value = store.admit_move(cell)?;
            let actual = store.effective_type(value)?;
            let expected = match field {
                NominalFieldType::Concrete(source) => source.materialize(store)?,
                // A compound field Type fixes the parameters it mentions from
                // the argument's reached Type, which must have its structure.
                NominalFieldType::Compound(_) => match actual {
                    Some(ty) if field.bind(ty, store, &mut args)? => ty,
                    Some(_) => {
                        return Err(Diagnostic::new(
                            "LRR-E-CALL-ARGUMENT-TYPE",
                            "constructor argument differs from its declared field type".into(),
                        ));
                    }
                    None => {
                        return Err(Diagnostic::new(
                            "LRR-E-TYPE-CONTEXT",
                            "compound constructor argument requires its reached concrete type"
                                .into(),
                        ));
                    }
                },
                NominalFieldType::Parameter(index) => {
                    let parameter = args.get_mut(*index as usize).ok_or_else(|| {
                        invalid("generic field parameter outside its declaration")
                    })?;
                    match parameter {
                        Some(ty) => *ty,
                        None => {
                            let ty = match actual {
                                Some(ty) => ty,
                                None if value.kind == 11 => TypeId(6),
                                None if value.kind == 12 => TypeId(15),
                                None => {
                                    return Err(Diagnostic::new(
                                        "LRR-E-TYPE-CONTEXT",
                                        "generic constructor argument requires its reached concrete type"
                                            .into(),
                                    ));
                                }
                            };
                            *parameter = Some(ty);
                            ty
                        }
                    }
                }
            };
            match actual {
                Some(actual)
                    if actual != expected
                        && !store.contract_admits(Some(expected), Some(actual)) =>
                {
                    return Err(Diagnostic::new(
                        "LRR-E-CALL-ARGUMENT-TYPE",
                        "constructor argument differs from its declared field type".into(),
                    ));
                }
                Some(_) => {}
                // An unconstrained literal at a parameter takes the Type that
                // parameter was fixed to, through the ordinary checked
                // literal materialization.
                None => {
                    let alias = store.copy_from(cell)?;
                    let result = store.materialize_ticket(alias, expected)?;
                    store.discard(result)?;
                    store.discard(alias)?;
                }
            }
            cells.push(cell);
            types.push(expected);
        }
        let args = args
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "generic type argument is not determined by any constructor argument".into(),
                )
            })?;
        self.call_types = types;
        self.call_cells = cells;
        self.call_ready = true;
        self.nominal.sealed = Some((bound, args, site));
        Ok(())
    }
    pub(crate) fn constructor_call_sealed(&self) -> bool {
        self.nominal.sealed.is_some()
    }
    /// The sealed constructor call's selection and type arguments, consumed
    /// by its Call exit.
    pub(crate) fn take_sealed_constructor_call(
        &mut self,
        site: u32,
    ) -> Result<(Arc<BoundNominalVariant>, Vec<TypeId>), Diagnostic> {
        let (bound, args, sealed) = self
            .nominal
            .sealed
            .take()
            .ok_or_else(|| invalid("constructor Call exit has no sealed constructor call"))?;
        if sealed != site || site != self.pc.node {
            return Err(invalid("constructor Call site changed after sealing"));
        }
        let identity = self.call_identity(site);
        self.nominal.calls.remove(&identity);
        Ok((bound, args))
    }
    /// Publish the nominal source Type of a resumed constructor result.
    pub(crate) fn publish_constructor_result(
        &mut self,
        result: SlotId,
        ty: TypeId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let source = self.constructed_source(ty, store);
        self.set_source_view(result, source)
    }
}

/// Commit the sealed argument tickets as the bound variant's fields, in
/// declaration order, under the sealed type arguments.
pub(crate) fn construct_from_arguments(
    bound: &BoundNominalVariant,
    args: &[TypeId],
    tickets: &[TicketId],
    store: &mut GuestStore,
) -> Result<TicketId, Diagnostic> {
    if tickets.len() != bound.fields().len() {
        return Err(Diagnostic::new(
            "LRR-E-CALL-ARITY",
            "sealed argument count differs from the constructor fields".into(),
        ));
    }
    let (ty, guard) = store.begin_nominal_construction_with(bound, args)?;
    for (field, ticket) in tickets.iter().enumerate() {
        let field = u32::try_from(field).map_err(|_| crate::runtime::arena::resource())?;
        store.construction_push(guard, field, *ticket)?;
    }
    store.commit_construction(guard, ty)
}
