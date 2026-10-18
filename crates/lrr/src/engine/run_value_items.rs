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

/// The declaration an entered lexical Value goal selected (block items and
/// block-level imports shadow module items).
#[derive(Debug, Clone)]
pub(super) struct LexicalSelection {
    /// The item's identity; an enum variant's own identity for a variant.
    pub(super) identity: crate::core::digest::Digest,
    pub(super) kind: Option<ItemKind>,
    /// The enum variant a named import or glob of its variants selected.
    pub(super) variant: Option<crate::frontend::names::EnumVariantDefinition>,
    /// Declared in the executing body (not a module item reached by an import).
    pub(super) local: bool,
    /// A closed library facade function a block-level import brought into
    /// scope, with its definition identity.
    pub(super) facade: Option<(crate::facade::functions::LibraryFunction, crate::core::digest::Digest)>,
    /// `Root::member`: the written member, `identity`/`kind` naming the root
    /// the entered lexical prefix selected (an enum, whose variant it is).
    pub(super) rooted: Option<String>,
    /// A module the root selected (`use std::mem; mem::swap`): the module's
    /// path spelled from its defining crate's root, which the module route
    /// resolves like any written path.
    pub(super) module_root: Option<String>,
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
        stated: Option<&crate::ir::written_type::WrittenType>,
    ) -> QueryPoll<bool> {
        // A single name, or `Root::member` whose root a block item or import
        // may bind; longer or anchored paths are the module route's.
        if path.starts_with("::") || path.matches("::").count() > 1 {
            return QueryPoll::Ready(Ok(false));
        }
        let pc = self.frame.pc().clone();
        let selected = match self.lexical_values.get(&pc) {
            Some(selected) => selected.clone(),
            None => {
                let selected = match self.select_lexical_value(db) {
                    QueryPoll::Ready(Ok(selected)) => selected,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                self.lexical_values.insert(pc.clone(), selected.clone());
                selected
            }
        };
        let Some(selected) = selected else {
            return QueryPoll::Ready(Ok(false));
        };
        // A unit struct or unit variant is a constructor value, bound by the
        // same service module-level constructor paths use.
        match self.lexical_constructor(db, &selected) {
            QueryPoll::Ready(Ok(Some(bound))) => {
                return self.prepare_lexical_constructor_value(bound, stated).map(|()| true);
            }
            QueryPoll::Ready(Ok(None)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        // `Module::item` through an import of the module: the item's path
        // spelled from the module's defining crate, selected by the ordinary
        // module route (constructors, then constants, statics and functions).
        if let (Some(root), Some(member)) = (&selected.module_root, &selected.rooted) {
            let rewritten = format!("{root}::{member}");
            match self.select_nominal_constructor(db, &rewritten) {
                QueryPoll::Ready(Ok(Some(bound))) => {
                    return self.prepare_lexical_constructor_value(bound, stated).map(|()| true);
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
            return self.prepare_item_value_path(db, &rewritten).map(|()| true);
        }
        // The root of `Root::member` was selected lexically but names no
        // enum variant: an associated item of a block-scoped or imported
        // type, resolved through the member index, is not selected here and
        // must not fall back to a same-named module item.
        if selected.rooted.is_some() {
            return QueryPoll::Ready(Err(unsupported(format!(
                "value path `{path}` is an associated item of a block-scoped or imported type; its member selection awaits integration"
            ))));
        }
        let LexicalSelection { identity, kind, .. } = selected;
        if kind == Some(ItemKind::Function)
            && selected.local
            && !self.local_function_values.contains_key(&identity)
        {
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
    /// The bound constructor a lexical selection names, if it names one: a
    /// struct (local or reached through an import) or an enum variant.
    pub(super) fn lexical_constructor(
        &self,
        db: &mut Database,
        selected: &LexicalSelection,
    ) -> QueryPoll<Option<crate::frontend::nominal_constructors::BoundConstructor>> {
        use crate::frontend::nominal_constructors::{NominalConstructorTarget, bind_constructor};
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault("value paths require a checked source catalogue")));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        // `Enum::Variant`: the root selected an enum declared in this body
        // or imported; only a member naming a declared variant is a
        // constructor (an associated function of the enum is not).
        if let Some(member) = &selected.rooted {
            if selected.kind != Some(ItemKind::Enum) {
                return QueryPoll::Ready(Ok(None));
            }
            let schema = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(selected.identity)) {
                QueryPoll::Ready(Ok(schema)) => schema,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
                return QueryPoll::Ready(Err(call_fault("lexical enum schema outcome differs")));
            };
            if schema.variant(Some(member.as_str())).is_err() {
                return QueryPoll::Ready(Ok(None));
            }
            let target = if selected.local {
                match NominalConstructorTarget::local(db, selected.identity, Some(member.as_str())) {
                    QueryPoll::Ready(Ok(Some(target))) => target,
                    QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            } else {
                let variant = crate::frontend::names::EnumVariantDefinition {
                    enumeration: selected.identity,
                    name: member.clone(),
                };
                match NominalConstructorTarget::variant(&sources, &variant) {
                    Ok(target) => target,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
            return bind_constructor(db, graph, &sources, &target).map(Some);
        }
        let target = match (&selected.variant, selected.kind) {
            (Some(variant), _) => match NominalConstructorTarget::variant(&sources, variant) {
                Ok(target) => target,
                Err(error) => return QueryPoll::Ready(Err(error)),
            },
            (None, Some(ItemKind::Struct)) if selected.local => {
                match NominalConstructorTarget::local(db, selected.identity, None) {
                    QueryPoll::Ready(Ok(Some(target))) => target,
                    QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            (None, Some(ItemKind::Struct)) => {
                match sources
                    .iter()
                    .find_map(|source| source.outline_item(selected.identity))
                {
                    Some(item) => NominalConstructorTarget::structure(item),
                    None => return QueryPoll::Ready(Ok(None)),
                }
            }
            _ => return QueryPoll::Ready(Ok(None)),
        };
        bind_constructor(db, graph, &sources, &target).map(Some)
    }
    /// A static read by value at an already selected PC: only the cell's
    /// current state is read (no name selection or source work). A Copy
    /// aggregate is copied out of the static's storage.
    pub(super) fn read_static_value(
        &mut self,
        cell: crate::runtime::cell::CellId,
        view: Option<crate::traits::term::TyTerm>,
    ) -> Result<(), Diagnostic> {
        let value = match self.store.state(cell)? {
            crate::runtime::cell::CellState::Live(value) if (1..=7).contains(&value.kind) => {
                ItemValue::Scalar {
                    view: primitive_view(&value),
                    value,
                }
            }
            _ => ItemValue::Ticket {
                ticket: self.store.read_owning(cell)?,
                view,
            },
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
    ) -> QueryPoll<Option<LexicalSelection>> {
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
    ) -> QueryPoll<Option<LexicalSelection>> {
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
        // A qualified path whose root no block item or import binds belongs
        // to the module route.
        if goal.is_qualified_path() && goal.rooted().is_none() {
            return QueryPoll::Ready(Ok(None));
        }
        if let Some(demand) = goal.pending() {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                format!("lexical value path requires exact entered source context ({demand:?})"),
            )));
        }
        if let (Some(function), Some(identity)) = (goal.facade(), goal.facade_identity()) {
            return QueryPoll::Ready(Ok(Some(LexicalSelection {
                identity,
                kind: None,
                variant: None,
                local: false,
                facade: Some((function, identity)),
                rooted: None,
                module_root: None,
            })));
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
        let local = declaration.owner().is_some();
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
            // A variant has no outline row of its own: it is its enum's member.
            None if declaration.variant().is_some() => None,
            None => self
                .sources
                .iter()
                .find_map(|source| source.outline_item(identity))
                .map(|item| item.def_key.item_kind),
        };
        // A module-level root (a module, or a type or trait an import brings
        // into scope): its path spelled from the defining crate, so `Root::item`
        // resolves through the module route (associated items included).
        let module_root = match (kind, goal.rooted(), local) {
            (Some(_), Some(_), false) => self
                .sources
                .iter()
                .find_map(|source| source.outline_item(identity))
                .map(|item| {
                    let mut segments: Vec<&str> =
                        item.def_key.module_path.iter().map(String::as_str).collect();
                    segments.push(&item.def_key.declared_name);
                    format!(
                        "{}::{}",
                        crate::frontend::callables::dollar_crate_segment(item.def_key.crate_key),
                        segments.join("::")
                    )
                }),
            _ => None,
        };
        QueryPoll::Ready(Ok(Some(LexicalSelection {
            identity,
            kind,
            variant: declaration.variant().cloned(),
            local,
            facade: None,
            rooted: goal.rooted().map(str::to_owned),
            module_root,
        })))
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
    /// Whether the written path at `pc` is in place context: its own Exit
    /// recipe addresses its value as a place (`ValuePlace`), as a borrow,
    /// projection or method autoref of the path does.
    fn path_forms_place(&self, db: &mut Database, pc: &PcKey) -> QueryPoll<bool> {
        let exit = match Phase::Exit.code() {
            Ok(phase) => PcKey {
                body: pc.body.clone(),
                node: pc.node,
                phase,
            },
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match db.fetch(&SyntaxProvider, QueryKey::Thunk(exit)) {
            QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
                QueryValue::Thunk(recipe) => QueryPoll::Ready(Ok(recipe
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| matches!(op, Op::ValuePlace { .. })))),
                _ => QueryPoll::Ready(Err(call_fault("value path exit recipe differs"))),
            },
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    /// Whether the written path at `pc` is the operand of an explicit shared
    /// borrow (`&PATH`): its parent's operand recipe forms `Borrow` with
    /// shared access directly from the path's place.
    fn path_is_shared_borrow_operand(&self, db: &mut Database, pc: &PcKey) -> QueryPoll<bool> {
        let body = &self.frame.body;
        let Some(parent) = body.nodes()[pc.node as usize].parent else {
            return QueryPoll::Ready(Ok(false));
        };
        if body.nodes()[parent.0 as usize].role
            != crate::syntax::body_index::NodeRole::Expression(
                crate::syntax::body_index::ExpressionKind::Borrow,
            )
        {
            return QueryPoll::Ready(Ok(false));
        }
        let key = match body.pc(parent, Phase::Operand(0)) {
            Ok(key) => key,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match db.fetch(&SyntaxProvider, QueryKey::Thunk(key)) {
            QueryPoll::Ready(Ok(outcome)) => match outcome.value.as_ref() {
                QueryValue::Thunk(recipe) => QueryPoll::Ready(Ok(recipe
                    .recipe()
                    .operations
                    .iter()
                    .any(|op| {
                        matches!(
                            op,
                            Op::Borrow {
                                access: crate::ir::op::BorrowAccess::Shared,
                                ..
                            }
                        )
                    }))),
                _ => QueryPoll::Ready(Err(call_fault("borrow operand recipe differs"))),
            },
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
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
            // E-R5-007: a constant the pure evaluator does not produce runs
            // its initializer through the interpreter (demanded here, entered
            // by the dispatcher, then prepared again at this PC).
            Some(ItemKind::Constant) => {
                // `&CONST` is rustc's promotable borrow of an rvalue; any other
                // use of the constant as a place (a `&mut`, a method autoref)
                // addresses a fresh temporary.
                let place = match self.path_is_shared_borrow_operand(db, &pc) {
                    QueryPoll::Ready(Ok(place)) => place,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let promote = |value: ItemValue| {
                    if place {
                        ItemValue::Promotable(Box::new(value))
                    } else {
                        value
                    }
                };
                if let Some((ticket, view)) = self.initialized_values.remove(&pc) {
                    self.frame
                        .prepare_item_value(promote(ItemValue::Ticket { ticket, view }));
                    return QueryPoll::Ready(Ok(()));
                }
                if let Some((cell, view)) = self.const_cells.get(&definition.identity).cloned() {
                    promote(ItemValue::ConstCell { cell, view })
                } else {
                    match self.constant_item(db, definition.identity) {
                        QueryPoll::Ready(Ok(value)) => promote(value),
                        QueryPoll::Ready(Err(_)) => {
                            self.demand_initializer(definition.identity, false);
                            return QueryPoll::Ready(Ok(()));
                        }
                        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                    }
                }
            }
            // An immutable static read by value observes its initializer's
            // const-evaluated value. Its place (a borrow, projection or a
            // `static mut` access) has an address identity this copy lacks.
            Some(ItemKind::Static) => {
                let place = match self.path_forms_place(db, &pc) {
                    QueryPoll::Ready(Ok(place)) => place,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let cell = match self.static_cell(db, definition.identity) {
                    QueryPoll::Ready(Ok(cell)) => cell,
                    // E-R5-007: its initializer runs through the interpreter.
                    QueryPoll::Ready(Err(_)) => {
                        self.demand_initializer(definition.identity, true);
                        return QueryPoll::Ready(Ok(()));
                    }
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                if place {
                    let view = match self.store.state(cell) {
                        Ok(crate::runtime::cell::CellState::Live(value)) => self
                            .static_views
                            .get(&definition.identity)
                            .cloned()
                            .or_else(|| {
                                primitive_view(&value).map(crate::traits::term::TyTerm::Primitive)
                            }),
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
                let view = self.static_views.get(&definition.identity).cloned();
                self.static_reads.insert(pc, (cell, view.clone()));
                return QueryPoll::Ready(self.read_static_value(cell, view));
            }
            // A function item is a zero-sized value of its own callable
            // type; calling it selects the same source function a direct
            // call of its path selects.
            Some(ItemKind::Function) => {
                // The erased fn-pointer descriptor a fn-pointer parameter or
                // location of this value names (E-R5-008).
                if let Err(error) = self
                    .store
                    .intern_type(crate::runtime::shape::TypeKey::erased_function_pointer())
                {
                    return QueryPoll::Ready(Err(error));
                }
                // Its code address, so the item casts and compares as a fn
                // pointer.
                self.store.function_address(definition.identity);
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
            let slot = self.next_static_slot()?;
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
