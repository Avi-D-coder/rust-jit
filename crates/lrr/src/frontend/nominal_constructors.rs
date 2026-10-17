//! Value-namespace nominal constructors: unit and tuple structs and enum
//! variants. `CallResolver` selects one through the ordinary Value namespace
//! (a struct's constructor shares its Type identity; a variant is a member of
//! its enum's module-like namespace), and this binds the selected variant from
//! the owning declaration's schema with the same binder record construction
//! uses. It performs only typed query demands: no guest allocation or effect.
use super::{
    names::{EnumVariantDefinition, NameNamespaces},
    types::{BoundNominalVariant, FieldShape, NominalBinding, NominalBindingDemand, SchemaNames},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    source::{
        changes::PreparedSourceView,
        manifest::{DefKey, ItemKind},
    },
    syntax::outline::ItemOutline,
};
use std::sync::Arc;

/// The selected owner declaration and, for an enum, the semantic variant name.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NominalConstructorTarget {
    owner: DefKey,
    identity: Digest,
    variant: Option<String>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-SCHEMA", message.into())
}
impl NominalConstructorTarget {
    pub(crate) fn structure(item: &ItemOutline) -> Self {
        Self {
            owner: item.def_key.clone(),
            identity: item.def_key.fingerprint(),
            variant: None,
        }
    }
    pub(crate) fn variant(
        sources: &[&PreparedSourceView],
        variant: &EnumVariantDefinition,
    ) -> Result<Self, Diagnostic> {
        let mut owners = sources.iter().flat_map(|source| {
            source
                .outline()
                .items
                .iter()
                .filter(|item| item.def_key.fingerprint() == variant.enumeration)
        });
        let owner = owners
            .next()
            .ok_or_else(|| fault("enum variant owner is outside the source catalogue"))?;
        if owners.next().is_some() || owner.def_key.item_kind != ItemKind::Enum {
            return Err(fault("enum variant owner declaration is not unique"));
        }
        Ok(Self {
            owner: owner.def_key.clone(),
            identity: variant.enumeration,
            variant: Some(variant.name.clone()),
        })
    }
    pub(crate) fn owner(&self) -> &DefKey {
        &self.owner
    }
    pub(crate) fn variant_name(&self) -> Option<&str> {
        self.variant.as_deref()
    }
}

/// A source-checked constructor: the bound variant witness and its written
/// constructor shape (unit, tuple or record).
#[derive(Debug, Clone)]
pub(crate) struct BoundConstructor {
    variant: Arc<BoundNominalVariant>,
    shape: FieldShape,
}
impl BoundConstructor {
    pub(crate) fn variant(&self) -> &Arc<BoundNominalVariant> {
        &self.variant
    }
    pub(crate) fn shape(&self) -> FieldShape {
        self.shape
    }
}

/// Bind the reached constructor's variant. Field Types are admitted by the
/// current declaration service; a generic or unresolved field remains a
/// typed refusal rather than a guessed descriptor.
pub(crate) fn bind_constructor(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    target: &NominalConstructorTarget,
) -> QueryPoll<BoundConstructor> {
    if !matches!(target.owner.item_kind, ItemKind::Struct | ItemKind::Enum)
        || (target.owner.item_kind == ItemKind::Enum) != target.variant.is_some()
    {
        return QueryPoll::Ready(Err(fault("constructor owner/variant selection differs")));
    }
    let schema = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(target.identity)) {
        QueryPoll::Ready(Ok(outcome)) => outcome,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("constructor schema outcome differs")));
    };
    let shape = match schema.variant(target.variant.as_deref()) {
        Ok(variant) => variant.shape,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let names = match SchemaNames::from_current_database(db, graph, sources, schema) {
        QueryPoll::Ready(Ok(names)) => names,
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    };
    match names.bind_variant_current(db, sources, target.variant.as_deref()) {
        QueryPoll::Ready(Ok(NominalBinding::Ready(variant))) => {
            QueryPoll::Ready(Ok(BoundConstructor {
                variant: Arc::new(variant),
                shape,
            }))
        }
        QueryPoll::Ready(Ok(NominalBinding::Deferred { demand, .. })) => {
            QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                match demand {
                    NominalBindingDemand::GenericContext => {
                        "generic nominal constructor requires its generic type context"
                    }
                    NominalBindingDemand::TypeRecipe => {
                        "constructor field type requires compound source type binding"
                    }
                    NominalBindingDemand::Name(_) => {
                        "constructor field type requires further source type binding"
                    }
                }
                .into(),
            )))
        }
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}

/// The generic nominal declaration a written generic application (`Opt<i32>`)
/// names, selected at its exact indexed Type occurrence by the owning body's
/// ordinary prefix/declaration kernel, plus the input closures it read.
pub(crate) struct GenericNominalBase {
    pub(crate) identity: Digest,
    pub(crate) definition: DefKey,
    pub(crate) schema: Digest,
    pub(crate) type_parameters: usize,
    pub(crate) guards: Vec<Arc<crate::db::memo::PreparedInputGuard>>,
}
pub(crate) fn generic_annotation_base(
    db: &mut Database,
    body: &crate::source::manifest::BodyKey,
    node: crate::ir::pc::NodeId,
) -> QueryPoll<GenericNominalBase> {
    use super::source_locals::owned_plan::type_declarations::{BodyTypeDemand, BodyTypeOutcome};
    macro_rules! ready {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let unsupported =
        |message: &str| QueryPoll::Ready(Err(Diagnostic::new("LRR-E-UNSUPPORTED", message.into())));
    let plan_key = QueryKey::SourceLocalPlan(body.clone());
    let owner = ready!(db.fetch(&SyntaxProvider, plan_key.clone()));
    let QueryValue::SourceLocalPlan(plan) = owner.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("generic annotation owning plan differs")));
    };
    let plan = Arc::clone(plan);
    let mut guards = Vec::new();
    guards.push(Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &plan_key,
        owner.fingerprint
    ))));
    let identity = match ready!(plan.generic_base_body_type(db, node)) {
        BodyTypeOutcome::Pending(BodyTypeDemand::GenericArguments(identity)) => identity,
        BodyTypeOutcome::Ready(_) => {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "type arguments written for a non-generic Type".into(),
            )));
        }
        BodyTypeOutcome::Pending(_) => {
            return unsupported("generic Type occurrence requires further source resolution");
        }
    };
    let Some(environment) = plan.environment() else {
        return QueryPoll::Ready(Err(fault("generic annotation environment absent")));
    };
    let completed = Arc::clone(environment.macros().source_environment());
    let views: Vec<&PreparedSourceView> = completed.sources().iter().collect();
    let definition = match super::source_types::nominal_definition(db, &views, identity) {
        Ok(definition) => definition,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if !matches!(definition.item_kind, ItemKind::Struct | ItemKind::Enum) {
        return unsupported("generic Type application requires a nominal declaration");
    }
    let schema_key = QueryKey::TypeSchema(identity);
    let schema = ready!(db.fetch(&SyntaxProvider, schema_key.clone()));
    let QueryValue::TypeSchema(captured) = schema.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("generic annotation schema outcome differs")));
    };
    if captured
        .generics
        .iter()
        .any(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Const)
    {
        return unsupported("const generic arguments require the pure constant evaluator");
    }
    let type_parameters = captured
        .generics
        .iter()
        .filter(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
        .count();
    let schema_identity = captured.fingerprint();
    guards.push(Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &schema_key,
        schema.fingerprint
    ))));
    QueryPoll::Ready(Ok(GenericNominalBase {
        identity,
        definition,
        schema: schema_identity,
        type_parameters,
        guards,
    }))
}

/// The principal trait a written `dyn Trait` annotation names, selected at
/// its exact indexed Type occurrence by the same prefix/declaration kernel as
/// any Type use. Only the object schema identity is retained: a trait-object
/// place keeps its value's concrete runtime Type.
pub(crate) struct DynPrincipal {
    pub(crate) identity: Digest,
    pub(crate) guards: Vec<Arc<crate::db::memo::PreparedInputGuard>>,
}
pub(crate) fn dyn_annotation_principal(
    db: &mut Database,
    body: &crate::source::manifest::BodyKey,
    node: crate::ir::pc::NodeId,
) -> QueryPoll<DynPrincipal> {
    use super::source_locals::owned_plan::type_declarations::{BodyTypeDemand, BodyTypeOutcome};
    macro_rules! ready {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let unsupported =
        |message: &str| QueryPoll::Ready(Err(Diagnostic::new("LRR-E-UNSUPPORTED", message.into())));
    let plan_key = QueryKey::SourceLocalPlan(body.clone());
    let owner = ready!(db.fetch(&SyntaxProvider, plan_key.clone()));
    let QueryValue::SourceLocalPlan(plan) = owner.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("trait object annotation owning plan differs")));
    };
    let plan = Arc::clone(plan);
    let guard = Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &plan_key,
        owner.fingerprint
    )));
    // A trait is not a Type declaration: the declaration kernel stops at its
    // Type-recipe demand with the selected declaration's identity.
    let identity = match ready!(plan.generic_base_body_type(db, node)) {
        BodyTypeOutcome::Pending(BodyTypeDemand::TypeRecipe(identity)) => identity,
        _ => return unsupported("trait object principal requires further source resolution"),
    };
    let Some(environment) = plan.environment() else {
        return QueryPoll::Ready(Err(fault("trait object annotation environment absent")));
    };
    let completed = Arc::clone(environment.macros().source_environment());
    let views: Vec<&PreparedSourceView> = completed.sources().iter().collect();
    let definition = match super::source_types::nominal_definition(db, &views, identity) {
        Ok(definition) => definition,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    if definition.item_kind != ItemKind::Trait {
        return unsupported("trait object principal is not a trait declaration");
    }
    QueryPoll::Ready(Ok(DynPrincipal {
        identity,
        guards: vec![guard],
    }))
}

#[cfg(test)]
#[path = "nominal_constructor_tests.rs"]
mod tests;
