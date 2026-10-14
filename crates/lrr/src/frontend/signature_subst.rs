//! Call-local type constraints retain rigid source templates and lazy bounds.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    syntax::signatures::{FunctionSignature, GenericKind},
    traits::{
        term::{GenericArg, InferVarKey, ParamKey, TyTerm},
        unify::Unifier,
    },
};
use std::collections::BTreeMap;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERIC-BINDING", message.into())
}
pub struct SignatureSubstitution {
    definition: Digest,
    variables: BTreeMap<ParamKey, InferVarKey>,
    parameters: Vec<Option<TyTerm>>,
    result: Option<TyTerm>,
    unifier: Unifier,
}
impl SignatureSubstitution {
    pub fn new(signature: &FunctionSignature, transaction: Digest) -> Result<Self, Diagnostic> {
        for term in signature
            .parameters
            .iter()
            .filter_map(|p| p.symbolic.as_ref())
            .chain(signature.result_symbolic.iter())
        {
            if term.heap_bytes().is_none() {
                return Err(Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "signature substitution input exceeds term budgets".into(),
                ));
            }
        }
        let mut variables = BTreeMap::new();
        for parameter in &signature.generics {
            if parameter.kind == GenericKind::Type {
                variables.insert(
                    ParamKey {
                        definition: signature.definition,
                        ordinal: parameter.ordinal,
                    },
                    InferVarKey {
                        transaction,
                        ordinal: parameter.ordinal,
                    },
                );
            }
        }
        let parameters = signature
            .parameters
            .iter()
            .map(|p| p.symbolic.clone())
            .collect();
        Ok(Self {
            definition: signature.definition,
            variables,
            parameters,
            result: signature.result_symbolic.clone(),
            unifier: Unifier::new(transaction),
        })
    }
    pub fn source_parameter(&self, index: usize) -> Option<&TyTerm> {
        self.parameters.get(index)?.as_ref()
    }
    pub fn source_result(&self) -> Option<&TyTerm> {
        self.result.as_ref()
    }
    pub fn binding(&self, ordinal: u32) -> Option<&TyTerm> {
        self.unifier.binding(*self.variables.get(&ParamKey {
            definition: self.definition,
            ordinal,
        })?)
    }
    pub fn bind_type_parameter(&mut self, ordinal: u32, actual: &TyTerm) -> Result<(), Diagnostic> {
        let variable = self
            .variables
            .get(&ParamKey {
                definition: self.definition,
                ordinal,
            })
            .ok_or_else(|| fault("no declared type parameter at this ordinal"))?;
        self.unifier.unify(&TyTerm::Infer(*variable), actual)
    }
    pub fn constrain_argument(&mut self, index: usize, actual: &TyTerm) -> Result<(), Diagnostic> {
        let template = self.source_parameter(index).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "argument contract requires source type resolution".into(),
            )
        })?;
        let instantiated = instantiate(template, &self.variables, 0)?;
        self.unifier.unify(&instantiated, actual)
    }
    pub fn constrain_result(&mut self, actual: &TyTerm) -> Result<(), Diagnostic> {
        let template = self.source_result().ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "result contract requires source type resolution".into(),
            )
        })?;
        let instantiated = instantiate(template, &self.variables, 0)?;
        self.unifier.unify(&instantiated, actual)
    }
}
fn instantiate(
    term: &TyTerm,
    variables: &BTreeMap<ParamKey, InferVarKey>,
    depth: usize,
) -> Result<TyTerm, Diagnostic> {
    if depth > 256 {
        return Err(fault("signature substitution nesting limit exceeded"));
    }
    let mut substitution = crate::traits::subst::Substitution::default();
    for (key, variable) in variables {
        substitution.insert(*key, GenericArg::Type(TyTerm::Infer(*variable)))?;
    }
    substitution.apply(term)
}
