//! Value-namespace items read through an ordinary `ValuePath`.
//!
//! A written value path that selects no unit constructor is resolved once per
//! PC through the run's completed namespace graph (`CallResolver` Value
//! lookup, shared with pattern names). A constant is evaluated by the pure
//! constant evaluator through the same admission constant patterns use; the
//! VM receives only the evaluated value (`Frame::prepare_item_value`).
use super::*;
use crate::{
    frontend::names::NameLookup,
    source::manifest::{ItemKind, Namespace},
    vm::frame::item_values_frame::ItemValue,
};

fn unsupported(message: String) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message)
}

fn primitive_view(value: &crate::runtime::value::ValueRepr) -> Option<crate::ir::op::PrimitiveType> {
    use crate::ir::op::PrimitiveType as P;
    Some(match value.type_id {
        1 => P::Unit,
        2 => P::Bool,
        3 => P::Char,
        4 => P::I8,
        5 => P::I16,
        6 => P::I32,
        7 => P::I64,
        8 => P::Isize,
        9 => P::U8,
        10 => P::U16,
        11 => P::U32,
        12 => P::U64,
        13 => P::Usize,
        14 => P::F32,
        15 => P::F64,
        _ => return None,
    })
}

impl RunMachine {
    /// A single-name `ValuePath` that a body-local item or `use` may bind is
    /// selected through the ordinary entered lexical Value goal, the same
    /// hygienic prefix selection direct calls use: block items are visible
    /// throughout their block and shadow module items. `false`: no lexical
    /// item can bind it; the module route selects it.
    pub(super) fn prepare_lexical_value_path(
        &mut self,
        db: &mut Database,
        path: &str,
    ) -> QueryPoll<bool> {
        if path.contains("::") {
            return QueryPoll::Ready(Ok(false));
        }
        let pc = self.frame.pc().clone();
        let selected = match self.lexical_values.get(&pc) {
            Some(selected) => *selected,
            None => {
                let selected = match self.select_lexical_value(db) {
                    QueryPoll::Ready(Ok(selected)) => selected,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                self.lexical_values.insert(pc, selected);
                selected
            }
        };
        let Some((identity, kind)) = selected else {
            return QueryPoll::Ready(Ok(false));
        };
        if kind == Some(ItemKind::Function) && !self.local_function_values.contains_key(&identity) {
            let module_item = self
                .sources
                .iter()
                .any(|source| source.outline_item(identity).is_some());
            if !module_item {
                match self.admit_local_function_value(db, identity) {
                    QueryPoll::Ready(Ok(())) => {}
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
        }
        self.prepare_selected_item_value(db, path, identity, kind)
            .map(|()| true)
    }
    /// A static read by value at an already selected PC: only the cell's
    /// current state is read (no name selection or source work).
    pub(super) fn read_static_value(
        &mut self,
        cell: crate::runtime::cell::CellId,
    ) -> Result<(), Diagnostic> {
        let value = match self.store.state(cell)? {
            crate::runtime::cell::CellState::Live(value) if (1..=7).contains(&value.kind) => {
                ItemValue::Scalar {
                    view: primitive_view(&value),
                    value,
                }
            }
            _ => {
                return Err(unsupported(
                    "static read by value requires an aggregate copy of its storage".into(),
                ));
            }
        };
        self.frame.prepare_item_value(value);
        Ok(())
    }
    /// A body-local function item used as a value: admit it through the
    /// entered Value selection of this path, exactly as a direct call of the
    /// path is admitted, so a later call through the value selects it.
    fn admit_local_function_value(
        &mut self,
        db: &mut Database,
        identity: crate::core::digest::Digest,
    ) -> QueryPoll<()> {
        use crate::frontend::callables::{
            EnteredLocalCall, ordinary_local::prepare_entered_local_function_value,
        };
        let pc = self.frame.pc();
        let site = match Phase::Entry.code() {
            Ok(phase) => PcKey {
                body: pc.body.clone(),
                node: pc.node,
                phase,
            },
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match prepare_entered_local_function_value(db, &site) {
            QueryPoll::Ready(Ok(EnteredLocalCall::Local(proof))) => {
                if proof.item().def_key.fingerprint() != identity {
                    return QueryPoll::Ready(Err(call_fault(
                        "local function value admission selects another declaration",
                    )));
                }
                self.local_function_values.insert(identity, Arc::from(proof));
                QueryPoll::Ready(Ok(()))
            }
            QueryPoll::Ready(Ok(EnteredLocalCall::Pending(demand))) => {
                QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    format!("local function value requires exact entered source context ({demand:?})"),
                )))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(call_fault(
                "local function value selection is not a body-local function",
            ))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn select_lexical_value(
        &mut self,
        db: &mut Database,
    ) -> QueryPoll<Option<(crate::core::digest::Digest, Option<ItemKind>)>> {
        use crate::db::dependency::{InputKey, InputValue};
        let pc = self.frame.pc().clone();
        let definition = pc.body.definition;
        let source = match db.input(&InputKey::DefinitionSource(definition)).as_deref() {
            Some(InputValue::DefinitionSource(source)) => source.clone(),
            _ => return QueryPoll::Ready(Err(call_fault("reached value path source absent"))),
        };
        let handle = match db.input(&InputKey::DefinitionBody(definition)).as_deref() {
            Some(InputValue::DefinitionBody(handle)) if handle.key() == pc.body => handle.clone(),
            _ => return QueryPoll::Ready(Err(call_fault("reached value path body differs"))),
        };
        let parsed = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(source)) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("reached value path parse kind differs")));
        };
        match parsed.entered_value_requires_lexical_context(&handle, &self.frame.body, &pc) {
            Ok(true) => {}
            Ok(false) => return QueryPoll::Ready(Ok(None)),
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
        let key = PcKey {
            body: pc.body.clone(),
            node: pc.node,
            phase: 0,
        };
        self.entered_value_declaration(db, key)
    }
    /// The declaration the entered lexical Value goal at `key` selects
    /// (block items shadow module items), or `None` for a local binding.
    pub(super) fn entered_value_declaration(
        &mut self,
        db: &mut Database,
        key: PcKey,
    ) -> QueryPoll<Option<(crate::core::digest::Digest, Option<ItemKind>)>> {
        let pc = key.clone();
        let goal = match db.fetch(&SyntaxProvider, QueryKey::EnteredValue(key)) {
            QueryPoll::Ready(Ok(goal)) => goal,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::EnteredValue(goal) = goal.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("lexical value goal variant differs")));
        };
        if goal.body() != &pc.body || goal.node().0 != pc.node {
            return QueryPoll::Ready(Err(call_fault("lexical value goal site differs")));
        }
        if let Some(demand) = goal.pending() {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                format!("lexical value path requires exact entered source context ({demand:?})"),
            )));
        }
        let Some(declaration) = goal.declaration() else {
            if goal.local_binding().is_some() {
                return QueryPoll::Ready(Ok(None));
            }
            return QueryPoll::Ready(Err(unsupported(
                "lexical value path selects no source declaration".into(),
            )));
        };
        let identity = declaration.identity();
        let kind = match declaration.owner() {
            // The owning body's actual entered item row, published with it.
            Some(owner) if owner == &pc.body => goal
                .retained_plan()
                .context()
                .facts()
                .items
                .iter()
                .find(|row| row.item.def_key.fingerprint() == identity)
                .map(|row| row.item.def_key.item_kind),
            Some(_) => {
                return QueryPoll::Ready(Err(call_fault(
                    "lexical value declaration belongs to another body",
                )));
            }
            None => self
                .sources
                .iter()
                .find_map(|source| source.outline_item(identity))
                .map(|item| item.def_key.item_kind),
        };
        QueryPoll::Ready(Ok(Some((identity, kind))))
    }
    pub(super) fn prepare_item_value_path(&mut self, db: &mut Database, path: &str) -> QueryPoll<()> {
        let pc = self.frame.pc().clone();
        if let Some(value) = self.item_values.get(&pc) {
            self.frame.prepare_item_value(value.clone());
            return QueryPoll::Ready(Ok(()));
        }
        let found = match self.lookup_value_name(db, path, Namespace::Value) {
            QueryPoll::Ready(Ok(found)) => found,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let identity = match found {
            NameLookup::Defined(definition) => definition.identity,
            NameLookup::NeedInput(_) => {
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-CATALOG-INCOMPLETE",
                    format!("value path `{path}` awaits unresolved namespace producers"),
                )));
            }
            // A type-relative path names an associated constant of the
            // type's impls (or its trait's provided default).
            NameLookup::Absent if path.contains("::") => {
                match self.associated_value_path(db, path) {
                    QueryPoll::Ready(Ok(Some(constant))) => constant,
                    QueryPoll::Ready(Ok(None)) => {
                        return QueryPoll::Ready(Err(unsupported(format!(
                            "value path `{path}` has no unique accessible declaration"
                        ))));
                    }
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            _ => {
                return QueryPoll::Ready(Err(unsupported(format!(
                    "value path `{path}` has no unique accessible declaration"
                ))));
            }
        };
        let item_kind = self
            .sources
            .iter()
            .find_map(|source| source.outline_item(identity))
            .map(|item| item.def_key.item_kind)
            .or_else(|| generated_item_kind(db, identity));
        self.prepare_selected_item_value(db, path, identity, item_kind)
    }
    /// The associated constant a type-relative written path selects through
    /// the run's shared member index (the same `CallResolver` route a
    /// `Type::function` callee uses).
    fn associated_value_path(
        &self,
        db: &mut Database,
        path: &str,
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "value paths require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver
                .with_members(self.members.as_deref())
                .associated_value(db, self.frame.pc().body.definition, path),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
    /// The value of the Value item `identity` (a module item, or a body-local
    /// item selected by the entered lexical scope) read by this `ValuePath`.
    pub(super) fn prepare_selected_item_value(
        &mut self,
        db: &mut Database,
        path: &str,
        identity: crate::core::digest::Digest,
        item_kind: Option<ItemKind>,
    ) -> QueryPoll<()> {
        let pc = self.frame.pc().clone();
        let definition = crate::frontend::names::NameDefinition {
            namespace: Namespace::Value,
            identity,
            defining_unit: Default::default(),
            definition_edition: 0,
        };
        let value = match item_kind {
            Some(ItemKind::Constant) => match self.constant_item(db, definition.identity) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            },
            // An immutable static read by value observes its initializer's
            // const-evaluated value. Its place (a borrow, projection or a
            // `static mut` access) has an address identity this copy lacks.
            Some(ItemKind::Static) => {
                // The path's own Exit recipe addresses its value as a place
                // exactly when the written path is in place context.
                let exit = match Phase::Exit.code() {
                    Ok(phase) => PcKey {
                        body: pc.body.clone(),
                        node: pc.node,
                        phase,
                    },
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                let place = match db.fetch(&SyntaxProvider, QueryKey::Thunk(exit)) {
                    QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
                        QueryValue::Thunk(recipe) => recipe
                            .recipe()
                            .operations
                            .iter()
                            .any(|op| matches!(op, Op::ValuePlace { .. })),
                        _ => return QueryPoll::Ready(Err(call_fault("static path exit recipe differs"))),
                    },
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let cell = match self.static_cell(db, definition.identity) {
                    QueryPoll::Ready(Ok(cell)) => cell,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                if place {
                    let view = match self.store.state(cell) {
                        Ok(crate::runtime::cell::CellState::Live(value)) => {
                            primitive_view(&value).map(crate::traits::term::TyTerm::Primitive)
                        }
                        Ok(_) => None,
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    };
                    let value = ItemValue::Static { cell, view };
                    self.item_values.insert(pc, value.clone());
                    self.frame.prepare_item_value(value);
                    return QueryPoll::Ready(Ok(()));
                }
                // A value read observes the static's current contents (a
                // `static mut` may have been written): the selected cell is
                // cached per PC, and each execution reads only its state.
                self.static_reads.insert(pc, cell);
                return QueryPoll::Ready(self.read_static_value(cell));
            }
            // A function item is a zero-sized value of its own callable
            // type; calling it selects the same source function a direct
            // call of its path selects.
            Some(ItemKind::Function) => {
                match self.store.intern_type(crate::runtime::shape::TypeKey::Callable {
                    identity: definition.identity,
                    captures: Box::new([]),
                    borrowed: Box::new([]),
                }) {
                    Ok(ty) => ItemValue::Function { ty },
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            }
            _ => {
                return QueryPoll::Ready(Err(unsupported(format!(
                    "value path `{path}` names a Value item that is not a constant, static, function or constructor"
                ))));
            }
        };
        self.item_values.insert(pc, value.clone());
        self.frame.prepare_item_value(value);
        QueryPoll::Ready(Ok(()))
    }
}

/// The guest layout and source view of an evaluated constant's wire type.
fn constant_layout(
    store: &mut crate::runtime::cell::GuestStore,
    wire: &crate::ir::constant_data::wire::ConstantWireType,
) -> Result<(crate::runtime::shape::TypeId, crate::traits::term::TyTerm), Diagnostic> {
    use crate::{
        ir::constant_data::wire::ConstantWireType,
        runtime::shape::{PrimitiveIdentity, TypeKey},
        traits::term::{ConstTerm, TyTerm},
    };
    match wire {
        ConstantWireType::Scalar(primitive) => Ok((
            PrimitiveIdentity::from_recipe(*primitive)?.id(),
            TyTerm::Primitive(*primitive),
        )),
        ConstantWireType::Array { element, length } => {
            let (element_ty, element_view) = constant_layout(store, element)?;
            let len = u32::try_from(*length).map_err(|_| {
                Diagnostic::new("LRR-E-CONST-INSTANTIATION", "constant array length overflow".into())
            })?;
            let ty = store.intern_type(TypeKey::Array {
                element: element_ty,
                len,
            })?;
            Ok((ty, TyTerm::Array(Box::new(element_view), ConstTerm::Value(*length))))
        }
    }
}

impl RunMachine {
    /// A constant item read by value: a scalar, or an aggregate instantiated
    /// afresh at each read from its one evaluated immutable value.
    fn constant_item(
        &mut self,
        db: &mut Database,
        identity: crate::core::digest::Digest,
    ) -> QueryPoll<ItemValue> {
        let (value, wire) = match self.constant_item_data(db, identity) {
            QueryPoll::Ready(Ok(data)) => data,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        if let Some(scalar) = value.scalar() {
            return QueryPoll::Ready(Ok(ItemValue::Scalar {
                view: primitive_view(&scalar),
                value: scalar,
            }));
        }
        QueryPoll::Ready(constant_layout(&mut self.store, &wire).map(|(ty, view)| {
            ItemValue::Aggregate {
                value,
                wire,
                ty,
                view: Some(view),
            }
        }))
    }
    /// The run-global cell of static `identity`, allocated and initialized
    /// from its const-evaluated initializer on first use.
    fn static_cell(
        &mut self,
        db: &mut Database,
        identity: crate::core::digest::Digest,
    ) -> QueryPoll<crate::runtime::cell::CellId> {
        if let Some(cell) = self.static_cells.get(&identity) {
            return QueryPoll::Ready(Ok(*cell));
        }
        let (value, wire) = match self.constant_item_data(db, identity) {
            QueryPoll::Ready(Ok(data)) => data,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        // The published header of the module or body-local static.
        let writable = matches!(
            db.input(&crate::db::dependency::InputKey::DefinitionHeader(identity)).as_deref(),
            Some(crate::db::dependency::InputValue::DefinitionHeader(header))
                if header
                    .tokens
                    .windows(2)
                    .any(|pair| pair[0].text == "static" && pair[1].text == "mut")
        );
        let result = (|| -> Result<crate::runtime::cell::CellId, Diagnostic> {
            let anchor = match self.static_anchor {
                Some(anchor) => anchor,
                None => {
                    let anchor = self.store.new_frame_anchor()?;
                    self.static_anchor = Some(anchor);
                    anchor
                }
            };
            let slot = u32::try_from(self.static_cells.len())
                .map_err(|_| Diagnostic::new("LRR-E-INTERNAL-IR", "static slot overflow".into()))?;
            let (ty, _) = constant_layout(&mut self.store, &wire)?;
            let cell = self.store.new_cell(
                crate::runtime::cell::StorageContract::Exact(ty),
                writable,
                crate::runtime::cell::CellOwner::FrameSlot {
                    frame: anchor,
                    slot,
                },
            )?;
            // A scalar initializer is its value; an aggregate one is
            // instantiated once into the static's own storage.
            let ticket = match value.scalar() {
                Some(scalar) => self.store.ticket(scalar)?,
                None => {
                    let plan = self.store.prepare_constant_instantiation(&value, &wire, ty)?;
                    self.store
                        .instantiate_constant_value(plan, &mut self.frame.fuel)?
                }
            };
            self.store.initialize(cell, ticket)?;
            Ok(cell)
        })();
        match result {
            Ok(cell) => {
                self.static_cells.insert(identity, cell);
                QueryPoll::Ready(Ok(cell))
            }
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
/// A macro-generated declaration's kind, from its canonical owner (its
/// published unit, or the retained admission of an associated item).
fn generated_item_kind(
    db: &mut Database,
    identity: crate::core::digest::Digest,
) -> Option<ItemKind> {
    use crate::db::dependency::{InputKey, InputValue};
    let parse = match db.input(&InputKey::DefinitionSource(identity)).as_deref() {
        Some(InputValue::DefinitionSource(parse)) => parse.clone(),
        _ => return None,
    };
    db.generated_declaration_owner(&parse, identity)
        .map(|owner| owner.def_key.item_kind)
}
