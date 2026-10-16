//! Common declaration-context named Type preparation; never infer from a value.
use super::{
    names::NameNamespaces,
    signature_names::{SignatureName, SignatureNames},
    type_aliases::{AliasNormalizer, AliasTarget},
};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    runtime::{
        cell::GuestStore,
        shape::{PrimitiveIdentity, TypeId, TypeKey},
    },
    source::{
        changes::PreparedSourceView,
        manifest::{DefKey, ItemKind, Namespace},
    },
    syntax::{
        closed_reference_signatures::ClosedReferenceTypeSyntax, signatures::FunctionSignature,
    },
    traits::term::TyTerm,
};

/// Selected source type/schema from the current owning Type lookup. Call-boundary
/// currentness remains a separate proof; this does not authorize an arbitrary
/// graph or a public runtime descriptor as a source Type.
#[derive(Debug, Clone)]
pub(crate) struct SourceType {
    source: TyTerm,
    key: TypeKey,
}
pub(crate) enum NamedTypePreparation {
    Ready(SourceType),
    Parameter(crate::traits::term::ParamKey),
    Alias(AliasTarget),
    Name(SignatureName),
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
fn metadata(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-METADATA-LIMIT", message.into())
}
impl SourceType {
    pub(crate) fn source(&self) -> &TyTerm {
        &self.source
    }
    pub(crate) fn same_type(&self, other: &Self) -> bool {
        self.source == other.source && self.key == other.key
    }
    pub(crate) fn find_descriptor(&self, store: &GuestStore) -> Option<TypeId> {
        store.find_type(&self.key)
    }
    pub(crate) fn materialize(&self, store: &mut GuestStore) -> Result<TypeId, Diagnostic> {
        if let Some(id) = self.find_descriptor(store) {
            return Ok(id);
        }
        store.intern_type(self.key.clone())
    }
    pub(crate) fn require_descriptor(
        &self,
        store: &GuestStore,
        actual: TypeId,
    ) -> Result<(), Diagnostic> {
        if store.type_descriptor(actual)?.key.as_ref() != &self.key {
            return Err(fault(
                "selected source Type/schema differs from actual descriptor",
            ));
        }
        Ok(())
    }
    pub(crate) fn heap_bytes(&self) -> Result<usize, Diagnostic> {
        self.allocated_bytes()?
            .checked_sub(std::mem::size_of::<Self>())
            .ok_or_else(|| metadata("source Type size underflow"))
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(
                self.source
                    .heap_bytes()
                    .ok_or_else(|| metadata("source Type size overflow"))?,
            )
            .ok_or_else(|| metadata("source Type size overflow"))?;
        if let TypeKey::Nominal { definition, .. } = &self.key {
            bytes = definition.module_path.iter().try_fold(
                bytes
                    .checked_add(definition.declared_name.capacity())
                    .and_then(|n| {
                        definition
                            .module_path
                            .capacity()
                            .checked_mul(std::mem::size_of::<String>())
                            .and_then(|m| n.checked_add(m))
                    })
                    .ok_or_else(|| metadata("source nominal Type size overflow"))?,
                |bytes, part| {
                    bytes
                        .checked_add(part.capacity())
                        .ok_or_else(|| metadata("source nominal Type size overflow"))
                },
            )?;
        }
        if bytes > 32 * 1024 * 1024 {
            return Err(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "source Type metadata budget".into(),
            ));
        }
        Ok(bytes)
    }
}

/// Shared by ordinary named parameters and named reference pointees. This uses
/// exactly the existing Type name/alias/schema services; it does not parse text,
/// force executable code, solve a trait goal or inspect a runtime descriptor.
pub(crate) fn prepare_named(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
) -> QueryPoll<NamedTypePreparation> {
    prepare_named_at(db, graph, sources, signature, name, None)
}

/// The same Type service with the exact parser-owned source occurrence. This
/// narrows origin replay; it does not change classification or Type semantics.
pub(crate) fn prepare_reference_pointee(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    syntax: &ClosedReferenceTypeSyntax,
) -> QueryPoll<NamedTypePreparation> {
    if let Err(error) = syntax.require(signature) {
        return QueryPoll::Ready(Err(error));
    }
    prepare_named_at(db, graph, sources, signature, syntax.target(), Some(syntax))
}
fn prepare_named_at(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
    reference: Option<&ClosedReferenceTypeSyntax>,
) -> QueryPoll<NamedTypePreparation> {
    let selected = match resolve_named_at(db, graph, sources, signature, name, reference) {
        QueryPoll::Ready(Ok(selected)) => selected,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
    };
    prepare_selected(db, graph, sources, selected)
}

/// The common current Type lookup/classification used before optional alias or
/// nominal schema preparation. A result header can retain its own alias trace.
pub(crate) fn resolve_named(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
) -> QueryPoll<SignatureName> {
    resolve_named_at(db, graph, sources, signature, name, None)
}
fn resolve_named_at(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    signature: &FunctionSignature,
    name: &str,
    reference: Option<&ClosedReferenceTypeSyntax>,
) -> QueryPoll<SignatureName> {
    // A completed receipt supplements the actual source catalogue. It does
    // not authorize substituting an unrelated graph/source universe.
    if let Err(error) = AliasNormalizer::new(graph, sources) {
        return QueryPoll::Ready(Err(error));
    }
    let names = match SignatureNames::from_database(db, graph, signature) {
        Ok(names) => names,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    match names.unqualified_local(name) {
        Ok(Some(local)) => return QueryPoll::Ready(Ok(local)),
        Ok(None) => {}
        Err(error) => return QueryPoll::Ready(Err(error)),
    }
    let Some(source) = db.input(&InputKey::DefinitionSource(signature.definition)) else {
        return QueryPoll::Ready(Err(fault("named Type declaration source absent")));
    };
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        return QueryPoll::Ready(Err(fault("named Type declaration source variant differs")));
    };
    if reference.is_some_and(|reference| reference.parse() != parse) {
        return QueryPoll::Ready(Err(fault("named Type source occurrence is not current")));
    }
    let lookup = if db
        .published_input(&InputKey::GeneratedSyntax(parse.content))
        .is_some()
    {
        if let Some(reference) = reference {
            super::call_contracts::generated_result_namespace::lookup_name(
                db,
                signature,
                parse,
                name,
                Some(reference),
            )
        } else {
            match super::call_contracts::generated_result_namespace::lookup_prepared_name(
                db, signature, parse, name,
            ) {
                QueryPoll::Ready(result) => result,
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            }
        }
    } else {
        let stamps = match db.lexical_namespace_stamps() {
            Ok(stamps) => stamps,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        super::type_name_namespace::lookup_name(
            db,
            super::type_name_namespace::SourceTypeOwner {
                definition: signature.definition,
                scope: &signature.scope,
                header: &signature.header,
            },
            sources,
            &stamps,
            name,
        )
    };
    match lookup {
        Ok(lookup) => QueryPoll::Ready(names.unqualified_with_lookup(name, lookup)),
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
/// Resolve the selected result of the owning source Type lookup above. This
/// internal helper cannot serve as an alternate public namespace admission.
fn prepare_selected(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    selected: SignatureName,
) -> QueryPoll<NamedTypePreparation> {
    let proof = match selected {
        SignatureName::Primitive(p) => {
            let primitive = match PrimitiveIdentity::from_recipe(p) {
                Ok(p) => p,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            SourceType {
                source: TyTerm::Primitive(p),
                key: TypeKey::Primitive(primitive),
            }
        }
        SignatureName::Parameter(key) => {
            return QueryPoll::Ready(Ok(NamedTypePreparation::Parameter(key)));
        }
        SignatureName::Source(selected) => {
            let normalizer = match AliasNormalizer::new(graph, sources) {
                Ok(normalizer) => normalizer,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let target = match normalizer.normalize(db, selected.identity) {
                QueryPoll::Ready(Ok(target)) => target,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            };
            let AliasTarget::Nominal(identity) = target else {
                return QueryPoll::Ready(Ok(NamedTypePreparation::Alias(target)));
            };
            let definition = match nominal_definition(db, sources, identity) {
                Ok(definition) => definition,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let captured = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(identity)) {
                QueryPoll::Ready(Ok(captured)) => captured,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
            };
            let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("selected nominal schema query differs")));
            };
            if schema.definition != identity
                || definition.namespace != Namespace::Type
                || !matches!(definition.item_kind, ItemKind::Struct | ItemKind::Enum)
                || !matches!(db.input(&InputKey::DefinitionHeader(identity)).as_deref(),Some(InputValue::DefinitionHeader(header)) if header==&schema.header)
                || !matches!(db.input(&InputKey::DeclarationContext(identity)).as_deref(),Some(InputValue::DeclarationContext(context)) if context.scope==schema.scope)
                || !matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),Some(InputValue::DefinitionSource(parse)) if parse.profile==schema.header.profile)
            {
                return QueryPoll::Ready(Err(fault(
                    "selected nominal source/schema publication differs",
                )));
            }
            if !schema.generics.is_empty() {
                return QueryPoll::Ready(Ok(NamedTypePreparation::Alias(AliasTarget::Nominal(
                    identity,
                ))));
            }
            SourceType {
                source: TyTerm::Nominal(identity, vec![]),
                key: TypeKey::Nominal {
                    identity,
                    definition,
                    schema: schema.fingerprint(),
                    args: Box::new([]),
                    const_args: Box::new([]),
                },
            }
        }
        other => return QueryPoll::Ready(Ok(NamedTypePreparation::Name(other))),
    };
    if let Err(error) = proof.allocated_bytes() {
        return QueryPoll::Ready(Err(error));
    }
    QueryPoll::Ready(Ok(NamedTypePreparation::Ready(proof)))
}
pub(super) fn nominal_definition(
    db: &mut Database,
    sources: &[&PreparedSourceView],
    identity: crate::core::digest::Digest,
) -> Result<DefKey, Diagnostic> {
    for source in sources {
        if let Some(item) = source
            .outline()
            .items
            .iter()
            .find(|item| item.def_key.fingerprint() == identity)
        {
            if !matches!(db.input(&InputKey::DefinitionSource(identity)).as_deref(),Some(InputValue::DefinitionSource(parse)) if parse==&source.selection().outline.parse)
            {
                return Err(fault(
                    "selected nominal source differs from actual catalogue",
                ));
            }
            return Ok(item.def_key.clone());
        }
    }
    let source = db
        .input(&InputKey::DefinitionSource(identity))
        .ok_or_else(|| fault("selected nominal source absent"))?;
    let InputValue::DefinitionSource(parse) = source.as_ref() else {
        return Err(fault("selected nominal source variant differs"));
    };
    let generated = db
        .input(&InputKey::GeneratedSyntax(parse.content))
        .ok_or_else(|| fault("selected generated nominal syntax absent"))?;
    let InputValue::GeneratedSyntax(unit) = generated.as_ref() else {
        return Err(fault("selected nominal generated variant differs"));
    };
    if unit.key != *parse {
        return Err(fault("selected generated nominal source key differs"));
    }
    // Generated declaration identity is the actual GeneratedItemKey, not the
    // independently retained parsed DefKey. The current canonical map joins
    // both; callers have already required the exact selected Type receipt and
    // nominal schema/header/context. Original catalogue selection above still
    // requires the DefKey fingerprint itself to equal identity.
    unit.definitions
        .get(&identity)
        .cloned()
        .ok_or_else(|| fault("selected generated nominal identity absent"))
}
