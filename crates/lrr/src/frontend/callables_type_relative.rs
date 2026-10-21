//! Enum variants reached through a type-relative path: `Self::V` in an impl of
//! the enum, `Alias::V` / `m::Alias::V` for a type alias of an enum.
//!
//! rustc resolves a variant through any path whose prefix names the enum
//! type, including `Self` and (since 1.37) a type alias. The module graph
//! holds a variant only under its enum's own module-like prefix (`E::V`), so
//! this extends the one `CallResolver` lookup that call callees, value paths,
//! record constructors and patterns already share: the type prefix is
//! selected exactly as a written Type path is (the caller's module scope,
//! imports and edition rules included; `Self` as the caller's impl self
//! type), an alias is normalized to its nominal target by the ordinary alias
//! service, and the variant is then the enum-prefix graph lookup of the
//! selected enum. It selects a declaration only; constructor binding and
//! type arguments stay with the ordinary consumers.
use super::*;
use crate::{
    frontend::names::NameDefinition,
    syntax::outline::variant_identity,
    traits::term::TyTerm,
};

impl CallResolver<'_> {
    /// The enum a written type prefix names or aliases (`Self` as the
    /// caller's impl self type). `None` for any other type, a generic
    /// parameter, a primitive or an unresolved prefix.
    fn enumeration_of_prefix(
        &self,
        db: &mut Database,
        caller: Digest,
        scope: &ScopeKey,
        edition: u16,
        prefix: &str,
    ) -> QueryPoll<Option<Digest>> {
        let identity = if prefix == "Self" {
            let Some(members) = self.members else {
                return QueryPoll::Ready(Ok(None));
            };
            match members.owner_of(caller).and_then(|owner| owner.self_nominal) {
                Some(identity) => identity,
                None => return QueryPoll::Ready(Ok(None)),
            }
        } else {
            match self.lookup_written_type(scope, edition, prefix) {
                Ok(NameLookup::Defined(definition)) if definition.namespace == Namespace::Type => {
                    definition.identity
                }
                Ok(_) => return QueryPoll::Ready(Ok(None)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        };
        match self.nominal_of(db, identity) {
            QueryPoll::Ready(Ok(Some((target, ItemKind::Enum)))) => QueryPoll::Ready(Ok(Some(target))),
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }

    /// The nominal declaration a type name denotes and its kind: the name
    /// itself, or for a type alias the target the ordinary alias service
    /// normalizes it to. `None` for a primitive, parameter or other target.
    pub(crate) fn nominal_of(
        &self,
        db: &mut Database,
        identity: Digest,
    ) -> QueryPoll<Option<(Digest, ItemKind)>> {
        // A body-local declaration is selected by its entered lexical goal and
        // normalized by the body's own Type service, never by module items.
        if db
            .input(&InputKey::LocalDeclarationOwner(identity))
            .is_some()
        {
            return QueryPoll::Ready(Ok(None));
        }
        match selected_item_kind(db, self.sources.iter().copied(), identity) {
            Some(ItemKind::TypeAlias) => {
                let application = match crate::frontend::source_types::prepare_alias_application(
                    db,
                    self.graph,
                    self.sources,
                    identity,
                    &[],
                ) {
                    QueryPoll::Ready(Ok(Some(application))) => application,
                    QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                };
                let TyTerm::Nominal(target, _) = application.term else {
                    return QueryPoll::Ready(Ok(None));
                };
                QueryPoll::Ready(Ok(
                    selected_item_kind(db, self.sources.iter().copied(), target)
                        .map(|kind| (target, kind)),
                ))
            }
            Some(kind) => QueryPoll::Ready(Ok(Some((identity, kind)))),
            None => QueryPoll::Ready(Ok(None)),
        }
    }

    /// `Prefix::Variant` where `Prefix` names (or aliases) an enum and
    /// `Variant` is one of its variants, in `namespace` (unit and tuple
    /// variants are Value names, every variant a Type name). `None` when the
    /// spelling is not such a path: the caller then falls back to associated
    /// items.
    pub(crate) fn type_relative_variant(
        &self,
        db: &mut Database,
        caller: Digest,
        spelled: &str,
        namespace: Namespace,
    ) -> QueryPoll<Option<NameDefinition>> {
        let Some((prefix, name)) = spelled.rsplit_once("::") else {
            return QueryPoll::Ready(Ok(None));
        };
        // `<Type>::V` is the written type's variant too; `<Type as Trait>::V`
        // names a trait item.
        let prefix = match prefix.strip_prefix('<').and_then(|inner| inner.strip_suffix('>')) {
            Some(inner) if !inner.contains(" as ") => inner,
            Some(_) => return QueryPoll::Ready(Ok(None)),
            None => prefix,
        };
        if prefix.is_empty() || prefix.starts_with('<') || name.is_empty() || name.contains(['<', '>']) {
            return QueryPoll::Ready(Ok(None));
        }
        let (scope, edition) = match self.caller_context(db, caller) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let enumeration = match self.enumeration_of_prefix(db, caller, &scope, edition, prefix) {
            QueryPoll::Ready(Ok(Some(enumeration))) => enumeration,
            QueryPoll::Ready(Ok(None)) => return QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let Some(semantic) = crate::syntax::ast_adapter::semantic_name(name) else {
            return QueryPoll::Ready(Ok(None));
        };
        let variant = variant_identity(enumeration, &semantic);
        if self
            .graph
            .enum_variant(variant)
            .is_none_or(|member| member.enumeration != enumeration)
        {
            return QueryPoll::Ready(Ok(None));
        }
        // The variant is a member of its enum's module-like prefix: the same
        // graph lookup `Enum::Variant` takes, from the caller's module scope.
        let Some((_, item)) = self.item(enumeration) else {
            return QueryPoll::Ready(Ok(None));
        };
        let mut components: Vec<&str> = item.scope.module_path.iter().map(String::as_str).collect();
        components.push(&item.def_key.declared_name);
        components.push(name);
        let path = match NameNamespaces::path(
            item.scope.crate_key,
            &components,
            item.header.profile.edition,
        ) {
            Ok(path) => path,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        match self.graph.lookup(namespace, &path, &scope) {
            Ok(NameLookup::Defined(definition)) if definition.identity == variant => {
                QueryPoll::Ready(Ok(Some(definition)))
            }
            Ok(_) => QueryPoll::Ready(Ok(None)),
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}

impl CallResolver<'_> {
    /// `lookup_written` for a type-relative spelling: the variant, else absent.
    pub(super) fn type_relative_lookup(
        &self,
        db: &mut Database,
        caller: Digest,
        spelled: &str,
        namespace: Namespace,
    ) -> QueryPoll<NameLookup> {
        self.type_relative_variant(db, caller, spelled, namespace)
            .map(|found| found.map_or(NameLookup::Absent, NameLookup::Defined))
    }

    /// The call selection of a variant a type-relative callee path names.
    pub(super) fn select_type_relative_variant(
        &self,
        db: &mut Database,
        caller: Digest,
        spelled: &str,
    ) -> QueryPoll<Option<CallSelection>> {
        match self.type_relative_variant(db, caller, spelled, Namespace::Value) {
            QueryPoll::Ready(Ok(Some(definition))) => self
                .select(db, NameLookup::Defined(definition))
                .map(Some),
            QueryPoll::Ready(Ok(None)) => QueryPoll::Ready(Ok(None)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}

impl CallResolver<'_> {
    /// `Self` written alone: the caller's impl self type as a declaration of
    /// `namespace` (a struct is a constructor and a Type name at once).
    pub(super) fn self_type_lookup(
        &self,
        db: &mut Database,
        caller: Digest,
        namespace: Namespace,
    ) -> QueryPoll<NameLookup> {
        let Some(members) = self.members else {
            return QueryPoll::Ready(Ok(NameLookup::Absent));
        };
        let Some(identity) = members
            .owner_of(caller)
            .and_then(|owner| owner.self_nominal)
        else {
            return QueryPoll::Ready(Ok(NameLookup::Absent));
        };
        QueryPoll::Ready(Ok(match self.nominal_definition(db, identity, namespace) {
            Some(definition) => NameLookup::Defined(definition),
            None => NameLookup::Absent,
        }))
    }

    /// The declaration row (`namespace`, defining unit and edition) of a
    /// nominal type the source catalogue or published inputs hold.
    pub(crate) fn nominal_definition(
        &self,
        db: &mut Database,
        identity: Digest,
        namespace: Namespace,
    ) -> Option<NameDefinition> {
        self.member_definition(db, identity)
            .map(|definition| NameDefinition {
                namespace,
                ..definition
            })
    }

    /// `Self(..)` / unit-struct `Self`: the constructor of the impl self type.
    pub(super) fn select_self_constructor(
        &self,
        db: &mut Database,
        caller: Digest,
    ) -> QueryPoll<CallSelection> {
        match self.self_type_lookup(db, caller, Namespace::Value) {
            QueryPoll::Ready(Ok(found)) => self.select(db, found),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}

impl CallResolver<'_> {
    /// A member-index self type that names a type alias, normalized to the
    /// nominal declaration it aliases (the ordinary alias service); any other
    /// self type, and an alias of a non-nominal type, is unchanged.
    pub(super) fn alias_self_type(
        &self,
        db: &mut Database,
        self_type: Result<Digest, super::super::member_index::BuiltinSelf>,
    ) -> QueryPoll<Result<Digest, super::super::member_index::BuiltinSelf>> {
        let Ok(identity) = self_type else {
            return QueryPoll::Ready(Ok(self_type));
        };
        if selected_item_kind(db, self.sources.iter().copied(), identity) != Some(ItemKind::TypeAlias) {
            return QueryPoll::Ready(Ok(self_type));
        }
        match self.nominal_of(db, identity) {
            QueryPoll::Ready(Ok(Some((target, _)))) => QueryPoll::Ready(Ok(Ok(target))),
            QueryPoll::Ready(Ok(None)) => QueryPoll::Ready(Ok(self_type)),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}
