//! Reached item values use the ordinary interpreter and run-long initializer
//! storage. Preparation-time pure constant queries are not a runtime fast path.
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
    pub(super) facade: Option<(
        crate::facade::functions::LibraryFunction,
        crate::core::digest::Digest,
    )>,
    /// `Root::member`: the written member, `identity`/`kind` naming the root
    /// the entered lexical prefix selected (an enum, whose variant it is).
    pub(super) rooted: Option<String>,
    /// A module the root selected (`use std::mem; mem::swap`): the module's
    /// path spelled from its defining crate's root, which the module route
    /// resolves like any written path.
    pub(super) module_root: Option<String>,
}

fn primitive_view(
    value: &crate::runtime::value::ValueRepr,
) -> Option<crate::ir::op::PrimitiveType> {
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
        17 => P::I128,
        18 => P::U128,
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
        explicit: Option<&[Option<crate::runtime::shape::TypeId>]>,
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
                return self
                    .prepare_lexical_constructor_value(bound, stated, explicit)
                    .map(|()| true);
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
                    return self
                        .prepare_lexical_constructor_value(bound, stated, explicit)
                        .map(|()| true);
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
            if explicit.is_some() {
                return QueryPoll::Ready(Err(unsupported("explicit Type arguments on a non-constructor value require their value contract integration".into())));
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
        if explicit.is_some() {
            return QueryPoll::Ready(Err(unsupported("explicit Type arguments on a non-constructor value require their value contract integration".into())));
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
        use crate::frontend::nominal_constructors::NominalConstructorTarget;
        let Some(_graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "value paths require a checked source catalogue",
            )));
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
                match NominalConstructorTarget::local(db, selected.identity, Some(member.as_str()))
                {
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
            return self.bind_selected_constructor(db, &target).map(Some);
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
        self.bind_selected_constructor(db, &target).map(Some)
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
                self.local_function_values
                    .insert(identity, Arc::from(proof));
                QueryPoll::Ready(Ok(()))
            }
            QueryPoll::Ready(Ok(EnteredLocalCall::Pending(demand))) => {
                QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-CALL-CONTEXT",
                    format!(
                        "local function value requires exact entered source context ({demand:?})"
                    ),
                )))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(call_fault(
                "local function value selection is not a body-local function",
            ))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn select_lexical_value(&mut self, db: &mut Database) -> QueryPoll<Option<LexicalSelection>> {
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
            // The declaring body is this body or an enclosing one (a nested
            // `fn` or an impl member names its parents' block items).
            Some(owner) => {
                let mut declaring = Some(goal.retained_plan().context());
                while let Some(context) = declaring {
                    if context.source().index().body() == owner {
                        break;
                    }
                    declaring = context
                        .source()
                        .local_parent()
                        .map(|parent| parent.context());
                }
                let Some(declaring) = declaring else {
                    return QueryPoll::Ready(Err(call_fault(
                        "lexical value declaration belongs to another body",
                    )));
                };
                declaring
                    .facts()
                    .items
                    .iter()
                    .find(|row| row.item.def_key.fingerprint() == identity)
                    .map(|row| row.item.def_key.item_kind)
            }
            // A variant has no outline row of its own: it is its enum's member.
            None if declaration.variant().is_some() => None,
            None => {
                crate::frontend::callables::selected_item_kind(db, self.sources.iter(), identity)
            }
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
                    let mut segments: Vec<&str> = item
                        .def_key
                        .module_path
                        .iter()
                        .map(String::as_str)
                        .collect();
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
    /// Only a genuine source-selected parameter may precede module and
    /// constructor lookup. Reuse its retained qualifier and ordinary item
    /// initializer route; spelling alone never establishes this precedence.
    pub(super) fn prepare_parameter_associated_value(
        &mut self,
        db: &mut Database,
        path: &str,
    ) -> QueryPoll<bool> {
        if !path.contains("::") {
            return QueryPoll::Ready(Ok(false));
        }
        // A genuine free/unowned signature with no Type binder proves this
        // cannot be a parameter-relative value. An owner-bearing retained
        // fragment may lack its actual binder and cannot prove its absence.
        let body = self.frame.pc().body.clone();
        let declared = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(body.definition)) {
            QueryPoll::Ready(Ok(declared)) => declared,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::FunctionSignature(signature) = declared.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("associated parameter signature query variant differs")));
        };
        if !declared.cacheable || signature.definition != body.definition
            || !matches!(db.input(&crate::db::dependency::InputKey::DefinitionBody(body.definition)).as_deref(),
                Some(crate::db::dependency::InputValue::DefinitionBody(actual)) if actual.key() == body)
        {
            return QueryPoll::Ready(Err(call_fault("associated parameter signature does not own the current body")));
        }
        if signature.scope.owner.is_none() && signature.header.context.is_empty()
            && !signature.generics.iter().any(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
        {
            return QueryPoll::Ready(Ok(false));
        }
        let selected = match self.associated_value_qualifier(db) {
            QueryPoll::Ready(Ok(selected)) => selected,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::AssociatedItemQualifier(qualifier) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("associated value qualifier query variant differs")));
        };
        if !matches!(qualifier.source(), Some(crate::frontend::explicit_type_arguments::ExplicitType::Parameter(_))) {
            return QueryPoll::Ready(Ok(false));
        }
        let identity = match self.associated_value_path_prepared(db, path, &selected) {
            QueryPoll::Ready(Ok(Some(identity))) => identity,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Err(unsupported(format!(
                "value path `{path}` has no unique accessible declaration"
            )))),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let kind = crate::frontend::callables::selected_item_kind(db, self.sources.iter(), identity);
        self.prepare_selected_item_value(db, path, identity, kind).map(|()| true)
    }
    pub(super) fn prepare_item_value_path(
        &mut self,
        db: &mut Database,
        path: &str,
    ) -> QueryPoll<()> {
        if self.frame.use_cached_item_value() {
            return QueryPoll::Ready(Ok(()));
        }
        match self.prepare_parameter_associated_value(db, path) {
            QueryPoll::Ready(Ok(true)) => return QueryPoll::Ready(Ok(())),
            QueryPoll::Ready(Ok(false)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
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
        let item_kind =
            crate::frontend::callables::selected_item_kind(db, self.sources.iter(), identity);
        self.prepare_selected_item_value(db, path, identity, item_kind)
    }
    /// The associated constant a type-relative written path selects through
    /// the run's shared member index (the same `CallResolver` route a
    /// `Type::function` callee uses).
    fn associated_value_qualifier(
        &self,
        db: &mut Database,
    ) -> QueryPoll<crate::db::memo::Outcome> {
        let pc = PcKey { body: self.frame.pc().body.clone(), node: self.frame.pc().node, phase: 0 };
        let selected = match db.fetch(&SyntaxProvider, QueryKey::AssociatedItemQualifier(pc.clone())) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::AssociatedItemQualifier(qualifier) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("associated value qualifier query variant differs")));
        };
        if !selected.cacheable || qualifier.key() != &pc {
            return QueryPoll::Ready(Err(call_fault("associated value qualifier is not its retained occurrence")));
        }
        QueryPoll::Ready(Ok(selected))
    }
    fn associated_value_path(
        &self,
        db: &mut Database,
        path: &str,
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        let selected = match self.associated_value_qualifier(db) {
            QueryPoll::Ready(Ok(selected)) => selected,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        self.associated_value_path_prepared(db, path, &selected)
    }
    fn associated_value_path_prepared(
        &self,
        db: &mut Database,
        path: &str,
        selected: &crate::db::memo::Outcome,
    ) -> QueryPoll<Option<crate::core::digest::Digest>> {
        let Some(graph) = &self.graph else {
            return QueryPoll::Ready(Err(call_fault(
                "value paths require a checked source catalogue",
            )));
        };
        let sources: Vec<_> = self.sources.iter().collect();
        let QueryValue::AssociatedItemQualifier(qualifier) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("associated value qualifier query variant differs")));
        };
        let key = QueryKey::AssociatedItemQualifier(qualifier.key().clone());
        let resolver = match CallResolver::new(graph, &sources) {
            Ok(resolver) => resolver.with_members(self.members.as_deref()),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if let Some(source) = qualifier.source() {
            let actual = if let crate::frontend::explicit_type_arguments::ExplicitType::Parameter(crate::traits::term::TyTerm::Param(parameter)) = source {
                match self.frame.parameter_hint(*parameter) {
                    Ok(Some(_)) => {},
                    Ok(None) => return QueryPoll::Ready(Err(call_fault("associated qualifier source parameter has no paired runtime binding"))),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
                let Some(actual) = self.frame.bound_parameter_source(source.source()) else {
                    return QueryPoll::Ready(Err(call_fault("associated qualifier parameter requires its actual closed source binding")));
                };
                actual
            } else {
                self.frame.bound_view(source.source(), &self.store)
            };
            // Source arguments are never dropped to pretend the declaration
            // alone identifies an initializer application. Its ordinary packet
            // and once-per-application storage transport is the next join.
            if matches!(&actual, crate::traits::term::TyTerm::Nominal(_, args) if !args.is_empty()) {
                return QueryPoll::Ready(Err(call_fault("associated constant owner application requires its generic argument packet")));
            }
            let Some(member) = qualifier.member() else { return QueryPoll::Ready(Err(call_fault("associated value member occurrence absent"))); };
            let Some(name) = crate::syntax::ast_adapter::semantic_name(member.text()) else { return QueryPoll::Ready(Err(call_fault("associated value member name differs"))); };
            let guard = match db.prepare_input_guard(&SyntaxProvider, &key, selected.fingerprint) {
                QueryPoll::Ready(Ok(guard)) => guard,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if let Err(error) = guard.require_current(db) { return QueryPoll::Ready(Err(error)); }
            let member_context = qualifier.member_traits();
            return QueryPoll::Ready(resolver.associated_value_for_source(&actual, &name, member_context).map(|selected| selected.map(|selected| selected.definition())));
        }
        if qualifier.pending().is_some() {
            return QueryPoll::Ready(Err(call_fault("associated constant qualifier requires its actual source Type application")));
        }
        resolver.associated_value(db, self.frame.pc().body.definition, path)
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
    /// A constant root under the same indexed shared-borrow context that
    /// temporary projection lowering uses, including a field/index base.
    fn path_is_shared_borrow_operand(&self, pc: &PcKey) -> bool {
        self.frame.body.shared_borrow_owner(NodeId(pc.node)).is_some()
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
            // Every reached constant uses the same initializer-frame route;
            // its prepared value is installed before this PC is retried.
            Some(ItemKind::Constant) => {
                // `&CONST` is rustc's promotable borrow of an rvalue; any other
                // use of the constant as a place (a `&mut`, a method autoref)
                // addresses a fresh temporary.
                let place = self.path_is_shared_borrow_operand(&pc);
                let promote = |value: ItemValue| {
                    if place {
                        ItemValue::Promotable(Box::new(value))
                    } else {
                        value
                    }
                };
                if let Some((ticket, view)) = self
                    .initialized_values
                    .remove(&(pc.clone(), definition.identity))
                {
                    self.frame
                        .prepare_item_value(promote(ItemValue::Ticket { ticket, view }));
                    return QueryPoll::Ready(Ok(()));
                }
                if let Some(stored) = self.const_cells.get(&definition.identity).cloned() {
                    promote(match stored.template {
                        Some(template) => ItemValue::ConstTemplate {
                            template,
                            view: stored.view,
                        },
                        None => ItemValue::ConstCell {
                            cell: stored.cell,
                            view: stored.view,
                        },
                    })
                } else {
                    self.demand_initializer(definition.identity, false);
                    return QueryPoll::Ready(Ok(()));
                }
            }
            // A static's VM-initialized storage has a stable run-long address.
            // Reads and places use that same location, including mutation.
            Some(ItemKind::Static) => {
                let place = match self.path_forms_place(db, &pc) {
                    QueryPoll::Ready(Ok(place)) => place,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let Some(cell) = self.static_cells.get(&definition.identity).copied() else {
                    self.demand_initializer(definition.identity, true);
                    return QueryPoll::Ready(Ok(()));
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
                    self.frame.cache_item_value(value);
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
                match self
                    .store
                    .intern_type(crate::runtime::shape::TypeKey::Callable {
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
        self.frame.cache_item_value(value);
        QueryPoll::Ready(Ok(()))
    }
}
