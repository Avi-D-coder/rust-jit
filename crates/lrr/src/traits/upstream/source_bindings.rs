//! Demanded source-definition/generic-Type closure, shared by IR construction.
use super::{identity::DefinitionId, prepared::{self, DefinitionReceipt}, source_terms::SourceTypePending};
use crate::{core::{diagnostic::Diagnostic, digest::{Digest, StableEncode, StableEncoder}},
    db::{dependency::InputValue, key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue},
    frontend::declaration_generic_types::DeclarationGenericTypesKey,
    traits::term::{TyTerm, GenericArg}};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum DefinitionKind { Nominal, Trait, Impl, Declaration }
#[derive(Debug)]
pub(super) struct SourceBindings {
    pub(super) catalog: Arc<InputValue>,
    pub(super) definitions: BTreeMap<Digest, DefinitionId>,
    pub(super) generics: BTreeMap<DefinitionId, Arc<QueryValue>>,
    receipts: Vec<DefinitionReceipt>,
    pub(super) pending: Option<SourceTypePending>,
}
impl SourceBindings {
    pub(super) fn empty(catalog: Arc<InputValue>) -> Self {
        Self { catalog, definitions: BTreeMap::new(), generics: BTreeMap::new(), receipts: Vec::new(), pending: None }
    }
    pub(super) fn prepare(db: &mut Database, catalog_input: Arc<InputValue>, mut required: BTreeMap<Digest, DefinitionKind>) -> QueryPoll<Self> {
        macro_rules! ready { ($value:expr) => { match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        } }; }
        let InputValue::DiscoverySeal(catalog) = catalog_input.as_ref() else { return QueryPoll::Ready(Err(fault("source binding catalog differs"))); };
        let world = match prepared::catalog_world(catalog) { Ok(world) => world, Err(error) => return QueryPoll::Ready(Err(error)) };
        let local = catalog.impl_catalog.as_ref().expect("checked source catalog").root().outline.crate_key;
        let key = catalog.seal.key();
        let mut result = Self::empty(Arc::clone(&catalog_input));
        while let Some((definition, kind)) = required.pop_first() {
            if result.definitions.contains_key(&definition) { continue; }
            let Some(receipt) = ready!(DefinitionReceipt::observe(db, definition)) else {
                result.pending = Some(SourceTypePending::SourceDefinition); return QueryPoll::Ready(Ok(result));
            };
            let valid = match kind { DefinitionKind::Trait => receipt.is_trait(), DefinitionKind::Impl => receipt.is_impl(),
                DefinitionKind::Declaration => true, DefinitionKind::Nominal => match receipt.is_nominal() {
                    Ok(valid) => valid, Err(error) => return QueryPoll::Ready(Err(error)),
                } };
            if !valid { result.pending = Some(SourceTypePending::SourceDefinition); return QueryPoll::Ready(Ok(result)); }
            let generics = ready!(db.fetch(&SyntaxProvider, QueryKey::DeclarationGenericTypes(Box::new(DeclarationGenericTypesKey {
                inventory: key.clone(), definition,
            })))).value;
            let QueryValue::DeclarationGenericTypes(actual) = generics.as_ref() else { return QueryPoll::Ready(Err(fault("source generic Type query differs"))); };
            if !receipt.matches_generics(actual.declaration()) || actual.pending().is_some() {
                result.pending = Some(if actual.pending() == Some(crate::frontend::declaration_generic_types::GenericTypesPending::Parameters) {
                    SourceTypePending::ParameterLayout
                } else { SourceTypePending::ConstTypeMetadata }); return QueryPoll::Ready(Ok(result));
            }
            let id = receipt.definition_id(world, local);
            result.definitions.insert(definition, id);
            for ty in actual.const_types().values() { require_type(&mut required, ty); }
            result.generics.insert(id, generics); result.receipts.push(receipt);
        }
        QueryPoll::Ready(Ok(result))
    }
}
pub(super) fn require_type(required: &mut BTreeMap<Digest, DefinitionKind>, ty: &TyTerm) {
    let mut terms = vec![ty];
    while let Some(ty) = terms.pop() {
        if let Some((_, pointee)) = crate::traits::pointer_terms::as_raw_pointer(ty) { terms.push(pointee); continue; }
        match ty {
            TyTerm::Nominal(id, args) => { required.insert(*id, DefinitionKind::Nominal);
                terms.extend(args.iter().filter_map(|arg| if let GenericArg::Type(ty) = arg { Some(ty) } else { None })); },
            TyTerm::Tuple(fields) => terms.extend(fields),
            TyTerm::Array(ty, _) | TyTerm::Slice(ty) | TyTerm::Ref(_, _, ty) => terms.push(ty),
            TyTerm::Function(signature) => { terms.extend(&signature.value.parameters); terms.push(&signature.value.result); },
            _ => {},
        }
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
impl StableEncode for SourceBindings {
    const DOMAIN: &'static str = "lrr/v1/demanded-source-type-bindings";
    fn encode_fields(&self, e: &mut StableEncoder) {
        let InputValue::DiscoverySeal(catalog) = self.catalog.as_ref() else { unreachable!() };
        e.digest(catalog.seal.fingerprint());
        e.map(&self.definitions, |e, definition, id| { e.digest(*definition); e.record(id); });
        e.map(&self.generics, |e, id, value| { e.record(id); let QueryValue::DeclarationGenericTypes(generics) = value.as_ref() else { unreachable!() }; e.record(generics.as_ref()); });
        e.option(&self.pending, |e, pending| e.u32(*pending as u32));
    }
}
