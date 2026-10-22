//! Block-level `impl` and `trait` declarations of an entered body, resolved for
//! the run's member index (E-R5-009). The headers are the body's own
//! declaration-header Type occurrences, entered through the same lexical Type
//! prefix as a local `fn` parameter or struct field, so an impl may name a
//! block-local type, an imported type or a module type. Names that no block
//! item, import or generic parameter binds resolve through the module graph
//! exactly as a module-level impl header does.
use super::*;
use crate::{
    frontend::{
        member_index::{AssociatedTypeAlias, BuiltinSelf, ImplMembers, LocalContainer},
        names::NameLookup,
    },
    syntax::imports::ImportSegment,
};
use ra_ap_syntax::ast::HasGenericParams;

/// What a written impl-header type names.
enum HeaderTarget {
    Definition(Digest),
    Builtin(BuiltinSelf),
    Unresolved,
}

impl DeclarationFieldTypes {
    fn impl_occurrence(
        &self,
        definition: Digest,
        written: SourceRange,
    ) -> Result<HeaderTypeSite<'_>, Diagnostic> {
        let item = self
            .items
            .get(&definition)
            .ok_or_else(|| error("local impl header occurrences are absent"))?;
        let field = item
            .fields
            .iter()
            .find(|field| field.parameter.is_none() && field.range == written)
            .ok_or_else(|| error("local impl header Type occurrence is absent"))?;
        Ok(HeaderTypeSite {
            owner: self.owner,
            item,
            field,
            leaf: None,
        })
    }
}

impl PreparedSourceLocalPlan {
    /// `Self` of a member of a block-level impl: the impl's resolved self type
    /// as a closed source term (a nominal declaration or a builtin), read
    /// from the member index the owning body published. `None` for any other
    /// declaration, and for self types this service does not yet name.
    pub(crate) fn local_self_term(
        &self,
        db: &mut Database,
        member: Digest,
    ) -> QueryPoll<Option<crate::traits::term::TyTerm>> {
        use crate::traits::term::TyTerm;
        let Some(members) = self
            .environment()
            .and_then(|environment| environment.macros().source_environment().member_index().ok())
        else {
            return QueryPoll::Ready(Ok(None));
        };
        if let Some(owner) = members.owner_of(member) {
            return QueryPoll::Ready(Ok(match (owner.self_nominal, owner.self_builtin) {
                (Some(nominal), _) => Some(TyTerm::Nominal(nominal, Vec::new())),
                (None, Some(BuiltinSelf::Primitive(primitive))) => {
                    Some(TyTerm::Primitive(primitive))
                }
                (None, Some(BuiltinSelf::Str)) => Some(TyTerm::Str),
                _ => None,
            }));
        }
        // A member of a block-level trait carries `Self` as its implicit
        // generic parameter, bound from the call.
        let signature = ready!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(member)));
        let QueryValue::FunctionSignature(signature) = signature.value.as_ref() else {
            return QueryPoll::Ready(Err(error("local member signature variant differs")));
        };
        QueryPoll::Ready(Ok(signature
            .generics
            .iter()
            .find(|parameter| {
                parameter.kind == crate::syntax::signatures::GenericKind::Type
                    && parameter.name == "Self"
            })
            .map(|parameter| {
                TyTerm::Param(crate::traits::term::ParamKey {
                    definition: member,
                    ordinal: parameter.ordinal,
                })
            })))
    }
    /// A receiver written `self`, `&self`, `&mut self` or `self: Self`,
    /// `&Self`, `&mut Self` (with an optional lifetime) of a member of a
    /// block-level impl, as its source term.
    pub(crate) fn local_receiver_term(
        &self,
        db: &mut Database,
        member: Digest,
        written: &[crate::syntax::ast_adapter::TokenRecipe],
    ) -> QueryPoll<Option<crate::traits::term::TyTerm>> {
        use crate::traits::term::{RegionTerm, TyTerm};
        let mut rest = written;
        let mut reference = None;
        if rest.first().is_some_and(|token| token.text == "&") {
            rest = &rest[1..];
            if rest.first().is_some_and(|token| token.text.starts_with('\'')) {
                rest = &rest[1..];
            }
            let mutable = rest.first().is_some_and(|token| token.text == "mut");
            if mutable {
                rest = &rest[1..];
            }
            reference = Some(mutable);
        }
        match rest {
            [token] if token.text == "Self" => {
                let Some(term) = ready!(self.local_self_term(db, member)) else {
                    return QueryPoll::Ready(Ok(None));
                };
                QueryPoll::Ready(Ok(Some(match reference {
                    Some(mutable) => TyTerm::Ref(RegionTerm::Static, mutable, Box::new(term)),
                    None => term,
                })))
            }
            _ => QueryPoll::Ready(Ok(None)),
        }
    }
    /// The block-level impls and traits of this body, resolved. Cold: called
    /// once when the owning body's environment publishes. An impl whose header
    /// this service cannot yet name (generic impls, qualified paths through a
    /// block-level module) is left out: its methods stay unselectable, which
    /// the call site reports as a missing method.
    pub(crate) fn local_containers(
        self: &Arc<Self>,
        db: &mut Database,
    ) -> QueryPoll<Vec<LocalContainer>> {
        ready!(self.require_current(db));
        let facts = Arc::clone(self.context.facts());
        let mut containers = Vec::new();
        for row in &facts.items {
            if row.attribute_input.is_some() {
                continue;
            }
            let identity = row.item.def_key.fingerprint();
            match row.item.def_key.item_kind {
                ItemKind::Trait => {
                    let mut declarations = BTreeMap::new();
                    let mut defaults = BTreeMap::new();
                    let mut consts = BTreeMap::new();
                    for member in &facts.members {
                        if member.item.scope.owner != Some(identity) {
                            continue;
                        }
                        let name = member.item.def_key.declared_name.clone();
                        let id = member.item.def_key.fingerprint();
                        match member.item.def_key.item_kind {
                            ItemKind::Function => {
                                declarations.insert(name.clone(), id);
                                if member.item.body.is_some() {
                                    defaults.insert(name, id);
                                }
                            }
                            ItemKind::Constant if member.item.body.is_some() => {
                                consts.insert(name, id);
                            }
                            _ => {}
                        }
                    }
                    containers.push(LocalContainer::Trait {
                        definition: identity,
                        declarations,
                        defaults,
                        consts,
                    });
                }
                ItemKind::Impl => {
                    if let Some(container) = ready!(self.local_impl(db, &facts, row)) {
                        containers.push(container);
                    }
                }
                _ => {}
            }
        }
        QueryPoll::Ready(Ok(containers))
    }

    fn local_impl(
        self: &Arc<Self>,
        db: &mut Database,
        facts: &crate::syntax::entered_macros::EnteredMacroFacts,
        row: &crate::syntax::body_items::BodyItem,
    ) -> QueryPoll<Option<LocalContainer>> {
        let Some(identity) = row.item.def_key.enclosing_impl else {
            return QueryPoll::Ready(Ok(None));
        };
        let definition = row.item.def_key.fingerprint();
        let parsed = self.context.source().parsed();
        let syntax = checked!(
            parsed
                .syntax_at(row.item.declaration_span, |node| ast::Impl::can_cast(node.kind()))
                .and_then(|node| node
                    .and_then(ast::Impl::cast)
                    .ok_or_else(|| error("local impl exact syntax absent")))
        );
        // A generic impl needs the owner generics of its header (a blanket
        // impl or a generic self type); that is a later demand.
        if syntax.generic_param_list().is_some() {
            return QueryPoll::Ready(Ok(None));
        }
        let mut trait_path = None;
        let mut trait_definition = None;
        if let Some(trait_) = syntax.trait_() {
            let ast::Type::PathType(path) = &trait_ else {
                return QueryPoll::Ready(Ok(None));
            };
            trait_path = path
                .path()
                .and_then(crate::frontend::member_index::path_segments)
                .map(|(segments, _)| segments);
            match ready!(self.resolve_header_target(db, definition, &trait_)) {
                HeaderTarget::Definition(trait_key) => trait_definition = Some(trait_key),
                _ => {}
            }
        }
        let Some(self_ty) = syntax.self_ty() else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut self_nominal = None;
        let mut self_builtin = None;
        let mut self_pointee = None;
        match &self_ty {
            ast::Type::RefType(reference) => {
                let mutable = reference.mut_token().is_some();
                match ready!(self.resolve_header_target(db, definition, &self_ty)) {
                    HeaderTarget::Definition(pointee) => self_pointee = Some(Ok(pointee)),
                    HeaderTarget::Builtin(pointee) => self_pointee = Some(Err(pointee)),
                    HeaderTarget::Unresolved => {}
                }
                self_builtin = Some(BuiltinSelf::Reference(mutable));
            }
            ast::Type::PathType(_) => {
                match ready!(self.resolve_header_target(db, definition, &self_ty)) {
                    HeaderTarget::Definition(nominal) => self_nominal = Some(nominal),
                    HeaderTarget::Builtin(builtin) => self_builtin = Some(builtin),
                    HeaderTarget::Unresolved => {}
                }
            }
            ast::Type::SliceType(_) => self_builtin = Some(BuiltinSelf::Slice),
            ast::Type::ArrayType(_) => self_builtin = Some(BuiltinSelf::Array),
            ast::Type::TupleType(tuple) => {
                self_builtin = u32::try_from(tuple.fields().count())
                    .ok()
                    .map(BuiltinSelf::Tuple);
            }
            _ => {}
        }
        let mut functions = BTreeMap::new();
        let mut types = BTreeMap::new();
        let mut consts = BTreeMap::new();
        let mut aliases = Vec::new();
        for member in &facts.members {
            if member.item.scope.owner != Some(identity) {
                continue;
            }
            let name = member.item.def_key.declared_name.clone();
            let id = member.item.def_key.fingerprint();
            match member.item.def_key.item_kind {
                ItemKind::Function => {
                    functions.insert(name, id);
                }
                ItemKind::Constant => {
                    consts.insert(name, id);
                }
                ItemKind::TypeAlias => {
                    types.insert(name, id);
                    aliases.push((
                        id,
                        AssociatedTypeAlias {
                            header: member.item.header.clone(),
                            scope: member.item.scope.clone(),
                            impl_generic_count: 0,
                        },
                    ));
                }
                _ => {}
            }
        }
        QueryPoll::Ready(Ok(Some(LocalContainer::Impl {
            members: ImplMembers {
                identity,
                self_nominal,
                self_builtin,
                self_pointee,
                trait_path,
                trait_definition,
                trait_arguments: None,
                self_shape: crate::frontend::member_index::ArgShape::Any,
                functions,
                types,
                consts,
                blanket: None,
            },
            aliases,
        })))
    }

    /// The declaration (or builtin type) a written impl-header type names.
    /// The leading name enters the lexical Type prefix; a name no block item,
    /// import or generic binds goes to the module graph with the whole path.
    fn resolve_header_target(
        self: &Arc<Self>,
        db: &mut Database,
        definition: Digest,
        written: &ast::Type,
    ) -> QueryPoll<HeaderTarget> {
        let path_type = match written {
            ast::Type::RefType(reference) => match reference.ty() {
                Some(ast::Type::PathType(inner)) => inner,
                _ => return QueryPoll::Ready(Ok(HeaderTarget::Unresolved)),
            },
            ast::Type::PathType(path) => path.clone(),
            _ => return QueryPoll::Ready(Ok(HeaderTarget::Unresolved)),
        };
        let Some((segments, absolute)) = path_type
            .path()
            .and_then(crate::frontend::member_index::path_segments)
        else {
            return QueryPoll::Ready(Ok(HeaderTarget::Unresolved));
        };
        let written_range = range(written.syntax().text_range());
        let header = checked!(self.field_types.impl_occurrence(definition, written_range));
        let Some(name) = header.field.name.as_ref() else {
            return QueryPoll::Ready(Ok(HeaderTarget::Unresolved));
        };
        if let Some(key) = self.missing_expansions.first() {
            return QueryPoll::Ready(Err(error(&format!(
                "local impl header awaits macro definition {key:?}"
            ))));
        }
        let site = crate::frontend::source_locals::OrdinaryNameSite {
            source: self.entered.parent.source(),
            body: self.context.source().index().body(),
            at: u64::from(name.range.start),
            name: name.name(),
            context: name.context(),
            namespace: Namespace::Type,
        };
        let found = checked!(self.adapter().lookup_prepared_field_type_prefix(
            &site,
            &self.type_names,
            name.edition(),
            &header
        ));
        let (selection, selector) = checked!(self.select_type_prefix(found, name.context()));
        let edition = name.edition();
        let leading = name.name().to_owned();
        let builtin = |segments: &[ImportSegment]| match segments {
            [ImportSegment::Name(only)] => BuiltinSelf::named(only),
            _ => None,
        };
        let through_module_graph = |plan: &Self| -> Result<HeaderTarget, Diagnostic> {
            let (Some(environment), Some(scope)) = (plan.environment(), plan.module_scope()) else {
                return Ok(HeaderTarget::Unresolved);
            };
            let completed = environment.macros().source_environment();
            Ok(
                match completed.names().lookup_source_path(
                    Namespace::Type,
                    &segments,
                    absolute,
                    scope,
                    edition,
                )? {
                    NameLookup::Defined(found) => HeaderTarget::Definition(found.identity),
                    _ => builtin(&segments)
                        .map(HeaderTarget::Builtin)
                        .unwrap_or(HeaderTarget::Unresolved),
                },
            )
        };
        match &selection {
            // Bound by a block item or import: only the bare name is a type.
            LocalTypePrefixSelection::Lookup(NameLookup::Defined(found)) => {
                QueryPoll::Ready(Ok(if segments.len() == 1 {
                    HeaderTarget::Definition(found.identity)
                } else {
                    HeaderTarget::Unresolved
                }))
            }
            LocalTypePrefixSelection::Lookup(NameLookup::Absent) => {
                QueryPoll::Ready(through_module_graph(self).or_else(|_| Ok(HeaderTarget::Unresolved)))
            }
            LocalTypePrefixSelection::ModuleTail => {
                let prefix = LocalTypePrefix {
                    plan: Arc::clone(self),
                    node: None,
                    header: None,
                    selection,
                    use_name: Some(LocalTypeUse {
                        name: leading,
                        range: name.range,
                        context: name.context,
                        edition,
                    }),
                    module_selector: selector,
                    fingerprint: Digest([0; 32]),
                };
                // Completion of the module-name observation is not needed to
                // read a defined name; an absent one falls to builtin names.
                // A path keyword (`crate::S`) roots the path in a module and
                // names no member to observe.
                if !matches!(prefix.use_name.as_ref().map(|used| used.name.as_str()), Some("crate" | "super" | "self")) {
                    let _ = ready!(prefix.module_name(db));
                }
                QueryPoll::Ready(through_module_graph(self).or_else(|_| Ok(HeaderTarget::Unresolved)))
            }
            LocalTypePrefixSelection::Parameter(_) => {
                QueryPoll::Ready(Ok(HeaderTarget::Unresolved))
            }
            other => QueryPoll::Ready(Err(error(&format!(
                "local impl header Type requires actual source authority: {}",
                match other {
                    LocalTypePrefixSelection::NeedImport(_) => "NeedImport",
                    LocalTypePrefixSelection::NeedDeclarationOccurrence(_) => {
                        "NeedDeclarationOccurrence"
                    }
                    LocalTypePrefixSelection::NeedModulePublication => "NeedModulePublication",
                    _ => "pending selection",
                }
            )))),
        }
    }
}
