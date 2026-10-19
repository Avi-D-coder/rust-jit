//! Bounded demand expansion through immutable original and generated declaration contexts.
use super::{
    names::{NameLookup, NameNamespaces},
    signature_names::SignatureName,
    type_name_namespace::{SourceTypeOwner, lookup_declaration_name, resolve_declaration_name},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::{
        changes::PreparedSourceView,
        manifest::{DefKey, ItemKind},
    },
    syntax::{
        ast_adapter::TokenRecipe,
        outline::{HeaderRecipe, ScopeKey},
    },
    traits::term::TyTerm,
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AliasTarget {
    Type(TyTerm),
    Nominal(Digest),
    Str,
    NeedProducer(Digest),
    NeedOwnerContext(ScopeKey),
    Unresolved(NameLookup),
    Deferred {
        definition: Digest,
        target: Vec<TokenRecipe>,
    },
}
struct AliasDeclaration {
    local_lookup: Option<NameLookup>,
    definition: DefKey,
    scope: ScopeKey,
    header: HeaderRecipe,
}
pub struct AliasNormalizer<'a> {
    sources: &'a [&'a PreparedSourceView],
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
impl<'a> AliasNormalizer<'a> {
    pub fn new(
        graph: &'a NameNamespaces,
        sources: &'a [&'a PreparedSourceView],
    ) -> Result<Self, Diagnostic> {
        for source in sources {
            graph.require_view(source)?;
        }
        Ok(Self { sources })
    }
    fn declaration(&self, db: &mut Database, identity: Digest) -> QueryPoll<AliasDeclaration> {
        if let Some((source, item)) = self.sources.iter().find_map(|source| {
            source
                .outline_item(identity)
                .map(|item| (*source, item))
        }) {
            if !matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),Some(InputValue::DefinitionSource(key)) if key==&source.selection().outline.parse)
            {
                return QueryPoll::Ready(Err(fault(
                    "alias source catalogue/database revision mismatch",
                )));
            }
            return QueryPoll::Ready(Ok(AliasDeclaration {
                local_lookup: None,
                definition: item.def_key.clone(),
                scope: item.scope.clone(),
                header: item.header.clone(),
            }));
        }
        match super::source_locals::cold_declarations::declaration(db, identity) {
            QueryPoll::Ready(Ok(Some(local))) => return QueryPoll::Ready(Ok(AliasDeclaration {
                definition: local.definition, scope: local.scope, header: local.header,
                local_lookup: Some(local.lookup),
            })),
            QueryPoll::Ready(Ok(None)) => {},
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let Some(source) = db.input(&InputKey::DefinitionSource(identity)) else {
            return QueryPoll::Ready(Err(fault(
                "alias definition is absent from its source catalogue",
            )));
        };
        let InputValue::DefinitionSource(parse) = source.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated alias source variant differs")));
        };
        let Some(generated) = db.input(&InputKey::GeneratedSyntax(parse.content)) else {
            return QueryPoll::Ready(Err(fault("generated alias syntax absent")));
        };
        let InputValue::GeneratedSyntax(unit) = generated.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated alias syntax variant differs")));
        };
        if parse != &unit.key {
            return QueryPoll::Ready(Err(fault("generated alias parse identity differs")));
        }
        let Some(definition) = unit.definitions.get(&identity) else {
            return QueryPoll::Ready(Err(fault("generated alias canonical identity absent")));
        };
        // Even nominal completion must validate the owning discovery certificate.
        match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(_)) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let Some(header) = db.input(&InputKey::DefinitionHeader(identity)) else {
            return QueryPoll::Ready(Err(fault("generated alias header absent")));
        };
        let InputValue::DefinitionHeader(header) = header.as_ref() else {
            return QueryPoll::Ready(Err(fault("generated alias header variant differs")));
        };
        let Some(context) = db.input(&InputKey::DeclarationContext(identity)) else {
            return QueryPoll::Ready(Err(fault("generated alias declaration context absent")));
        };
        let InputValue::DeclarationContext(context) = context.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "generated alias declaration context variant differs",
            )));
        };
        if header.profile != unit.key.profile
            || definition.crate_key != context.scope.crate_key
            || definition.module_path != context.scope.module_path
            || context.scope.crate_key != unit.scope.crate_key
            || !context
                .scope
                .module_path
                .starts_with(&unit.scope.module_path)
        {
            return QueryPoll::Ready(Err(fault("generated alias declaration context differs")));
        }
        QueryPoll::Ready(Ok(AliasDeclaration {
            local_lookup: None,
            definition: definition.clone(),
            scope: context.scope.clone(),
            header: header.clone(),
        }))
    }
    pub fn normalize(&self, db: &mut Database, initial: Digest) -> QueryPoll<AliasTarget> {
        self.normalize_traced(db, initial, None)
    }
    /// Bind a successful return normalization to every reached alias and its
    /// captured namespace source world. This reads immutable input identities;
    /// it never enters bodies, expands producers or executes build tools.
    /// The sole runtime consumer additionally pins Database::revision/source_revision
    /// in NormalizedDeferredReturn, retaining conservative currentness for the
    /// actual Type receipt/origin lookups rather than treating this trace alone
    /// as a complete namespace publication certificate.
    pub(crate) fn normalize_with_dependencies(
        &self,
        db: &mut Database,
        initial: Digest,
    ) -> QueryPoll<(AliasTarget, Vec<DependencyStamp>)> {
        let mut trace = AliasDependencyTrace::default();
        for source in self.sources {
            let key = InputKey::CapturedSource(source.selection().location_identity());
            let Some(published) = db.input(&key) else {
                return QueryPoll::Ready(Err(fault("alias namespace source publication absent")));
            };
            let InputValue::CapturedSource(current) = published.as_ref() else {
                return QueryPoll::Ready(Err(fault(
                    "alias namespace source publication variant differs",
                )));
            };
            if current != source.selection() {
                return QueryPoll::Ready(Err(fault(
                    "alias namespace catalogue/database source mismatch",
                )));
            }
            if let Err(error) = trace.push(DependencyStamp::Input {
                key,
                fingerprint: published.fingerprint(),
            }) {
                return QueryPoll::Ready(Err(error));
            }
        }
        match self.normalize_traced(db, initial, Some(&mut trace)) {
            QueryPoll::Ready(Ok(target)) => QueryPoll::Ready(Ok((target, trace.rows))),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
    fn normalize_traced(
        &self,
        db: &mut Database,
        initial: Digest,
        mut trace: Option<&mut AliasDependencyTrace>,
    ) -> QueryPoll<AliasTarget> {
        let mut definition = initial;
        let mut seen = Vec::new();
        loop {
            if seen.contains(&definition) {
                seen.push(definition);
                return QueryPoll::Ready(Err(Diagnostic::new(
                    "LRR-E-TYPE-ALIAS-CYCLE",
                    format!(
                        "reached alias cycle: {}",
                        seen.iter()
                            .map(|d| d.hex())
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    ),
                )));
            }
            seen.push(definition);
            let item = match self.declaration(db, definition) {
                QueryPoll::Ready(Ok(item)) => item,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if item.scope.owner.is_some() && item.local_lookup.is_none() {
                return QueryPoll::Ready(Ok(AliasTarget::NeedOwnerContext(item.scope.clone())));
            }
            let selected = if let Some(lookup) = item.local_lookup { lookup } else { match lookup_declaration_name(
                db,
                SourceTypeOwner {
                    definition,
                    scope: &item.scope,
                    header: &item.header,
                },
                self.sources,
                &item.definition.declared_name,
            ) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            }};
            match selected {
                NameLookup::Defined(selected) if selected.identity == definition => {}
                NameLookup::NeedInput(producer) => {
                    return QueryPoll::Ready(Ok(AliasTarget::NeedProducer(producer)));
                }
                other => return QueryPoll::Ready(Ok(AliasTarget::Unresolved(other))),
            }
            if matches!(item.definition.item_kind, ItemKind::Struct | ItemKind::Enum) {
                return QueryPoll::Ready(Ok(AliasTarget::Nominal(definition)));
            }
            if item.definition.item_kind != ItemKind::TypeAlias {
                return QueryPoll::Ready(Err(fault(
                    "reached definition is not a supported alias or nominal type",
                )));
            }
            if let Some(trace) = trace.as_deref_mut() {
                for key in [
                    InputKey::DefinitionSource(definition),
                    InputKey::DefinitionHeader(definition),
                    InputKey::DeclarationContext(definition),
                    InputKey::CrateCfg(item.scope.crate_key),
                ] {
                    if let Err(error) = trace.input(db, key) {
                        return QueryPoll::Ready(Err(error));
                    }
                }
                if let Some(InputValue::DefinitionSource(parse)) =
                    db.input(&InputKey::DefinitionSource(definition)).as_deref()
                    && db
                        .input(&InputKey::GeneratedSyntax(parse.content))
                        .is_some()
                    && let Err(error) = trace.input(db, InputKey::GeneratedSyntax(parse.content))
                {
                    return QueryPoll::Ready(Err(error));
                }
            }
            let outcome = match db.fetch(&SyntaxProvider, QueryKey::TypeAlias(definition)) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            if let Some(trace) = trace.as_deref_mut()
                && let Err(error) = trace.push(DependencyStamp::Query {
                    key: QueryKey::TypeAlias(definition),
                    fingerprint: outcome.fingerprint,
                })
            {
                return QueryPoll::Ready(Err(error));
            }
            let QueryValue::TypeAlias(alias) = outcome.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("alias query result mismatch")));
            };
            if alias.scope != item.scope || alias.header != item.header {
                return QueryPoll::Ready(Err(fault("alias recipe/source context mismatch")));
            }
            if alias.scope.owner.is_some() {
                return QueryPoll::Ready(Ok(AliasTarget::NeedOwnerContext(alias.scope.clone())));
            }
            if !alias.generics.is_empty() {
                return QueryPoll::Ready(Ok(AliasTarget::Deferred {
                    definition,
                    target: alias.target.clone(),
                }));
            }
            if let Some(term) = &alias.symbolic {
                return QueryPoll::Ready(Ok(AliasTarget::Type(term.clone())));
            }
            if alias.target.len() != 1 {
                return QueryPoll::Ready(Ok(AliasTarget::Deferred {
                    definition,
                    target: alias.target.clone(),
                }));
            }
            let resolved = match resolve_declaration_name(
                db,
                SourceTypeOwner {
                    definition,
                    scope: &alias.scope,
                    header: &alias.header,
                },
                &alias.generics,
                self.sources,
                &alias.target[0].text,
            ) {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            };
            match resolved {
                SignatureName::Primitive(p) => {
                    return QueryPoll::Ready(Ok(AliasTarget::Type(TyTerm::Primitive(p))));
                }
                SignatureName::Str => return QueryPoll::Ready(Ok(AliasTarget::Str)),
                SignatureName::Source(source) => definition = source.identity,
                SignatureName::Unresolved(NameLookup::NeedInput(producer)) => {
                    return QueryPoll::Ready(Ok(AliasTarget::NeedProducer(producer)));
                }
                SignatureName::Unresolved(other) => {
                    return QueryPoll::Ready(Ok(AliasTarget::Unresolved(other)));
                }
                SignatureName::NeedOwnerContext(scope) => {
                    return QueryPoll::Ready(Ok(AliasTarget::NeedOwnerContext(scope)));
                }
                SignatureName::Parameter(_) => {
                    return QueryPoll::Ready(Ok(AliasTarget::Deferred {
                        definition,
                        target: alias.target.clone(),
                    }));
                }
            }
        }
    }
}

/// Fixed row/byte budgets also bound the conservative name/import world guard.
#[derive(Default)]
struct AliasDependencyTrace {
    rows: Vec<DependencyStamp>,
    bytes: usize,
}
impl AliasDependencyTrace {
    fn input(&mut self, db: &mut Database, key: InputKey) -> Result<(), Diagnostic> {
        let value = db
            .input(&key)
            .ok_or_else(|| fault("alias dependency publication absent"))?;
        self.push(DependencyStamp::Input {
            key,
            fingerprint: value.fingerprint(),
        })
    }
    fn push(&mut self, stamp: DependencyStamp) -> Result<(), Diagnostic> {
        let bytes = self
            .bytes
            .checked_add(std::mem::size_of::<DependencyStamp>())
            .and_then(|n| n.checked_add(stamp.heap_bytes()))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "alias dependency trace budget exceeded".into(),
                )
            })?;
        self.rows
            .try_reserve_exact(1)
            .map_err(|_| crate::runtime::arena::resource())?;
        self.rows.push(stamp);
        self.bytes = bytes;
        Ok(())
    }
}

