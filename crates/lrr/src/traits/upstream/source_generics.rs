//! Source-derived argument namespace layouts available before typed fact freezing.
//! These do not authorize solver evaluation or replace actual typed generics.
use super::identity::DefinitionId;
use crate::{core::diagnostic::Diagnostic, db::{dependency::InputValue, value::QueryValue},
    frontend::declaration_generics::DeclarationGenerics};
use std::{collections::BTreeMap, sync::Arc};

pub(super) struct SourceGenericLayouts {
    catalog: Arc<InputValue>,
    declarations: BTreeMap<DefinitionId, Arc<QueryValue>>,
}
impl SourceGenericLayouts {
    pub(super) fn from_plans(catalog: Arc<InputValue>, plans: &[Arc<QueryValue>]) -> Result<Self, Diagnostic> {
        let mut result = Self { catalog, declarations: BTreeMap::new() };
        for value in plans {
            let QueryValue::ImplSourceTypes(plan) = value.as_ref() else { return Err(fault("constructor metadata needs actual source Type plans")); };
            if !Arc::ptr_eq(plan.catalog_input(), &result.catalog) { return Err(fault("constructor metadata belongs to another catalog")); }
            for (id, generics) in plan.source_generics() {
                let QueryValue::DeclarationGenerics(actual) = generics.as_ref() else { return Err(fault("actual constructor generic query differs")); };
                if actual.definition() != id.source() || actual.argument_kinds().is_none() {
                    return Err(fault("actual constructor generic metadata is pending"));
                }
                if let Some(previous) = result.declarations.insert(*id, Arc::clone(generics)) {
                    if !Arc::ptr_eq(&previous, generics) { return Err(fault("constructor metadata changed between actual plans")); }
                }
            }
        }
        Ok(result)
    }
    pub(super) fn matches_catalog(&self, catalog: &Arc<InputValue>) -> bool { Arc::ptr_eq(&self.catalog, catalog) }
    pub(super) fn get(&self, id: DefinitionId) -> Option<&DeclarationGenerics> {
        match self.declarations.get(&id)?.as_ref() {
            QueryValue::DeclarationGenerics(actual) => Some(actual), _ => unreachable!("captured generic query"),
        }
    }
}
fn fault(message: &str) -> Diagnostic { Diagnostic::new("LRR-E-UPSTREAM-SOLVER-INPUT", message.into()) }
