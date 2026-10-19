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
/// Inspect symbolic Type parameter identity, never identifier spelling.
pub(crate) fn references_parameters(term: &TyTerm, needed: &[ParamKey]) -> bool {
    let mut terms = vec![term];
    while let Some(term) = terms.pop() {
        fn arguments(args: &[GenericArg]) -> impl Iterator<Item = &TyTerm> {
            args.iter().filter_map(|argument| match argument {
                GenericArg::Type(term) => Some(term),
                _ => None,
            })
        }
        match term {
            TyTerm::Param(key) if needed.contains(key) => return true,
            TyTerm::Ref(_, _, inner) | TyTerm::Slice(inner) | TyTerm::Array(inner, _) => {
                terms.push(inner)
            }
            TyTerm::Tuple(fields) => terms.extend(fields),
            TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => terms.extend(arguments(args)),
            TyTerm::Projection(key) => {
                terms.push(&key.trait_ref.self_ty);
                terms.extend(arguments(&key.trait_ref.args));
                terms.extend(arguments(&key.item_args));
            }
            TyTerm::Function(function) => {
                terms.extend(&function.value.parameters);
                terms.push(&function.value.result);
            }
            _ => {}
        }
    }
    false
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
    /// Relate a genuine declared equation in the same call transaction.
    pub(crate) fn constrain_equation(
        &mut self,
        template: &TyTerm,
        actual: &TyTerm,
    ) -> Result<(), Diagnostic> {
        let instantiated = instantiate(template, &self.variables, 0)?;
        self.unifier.unify(&instantiated, actual)
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
    /// Structural argument constraint with regions erased on both sides: the
    /// interpreter binds generic parameters from reached argument types after
    /// rustc's own region checking.
    pub fn constrain_argument_erased(
        &mut self,
        index: usize,
        actual: &TyTerm,
    ) -> Result<(), Diagnostic> {
        let template = self.source_parameter(index).ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "argument contract requires source type resolution".into(),
            )
        })?;
        let instantiated = erase_regions(&instantiate(template, &self.variables, 0)?);
        self.unifier.unify(&instantiated, &erase_regions(actual))
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
/// Every region becomes `'static`; types and mutability are kept.
pub(crate) fn erase_regions(term: &TyTerm) -> TyTerm {
    match term {
        TyTerm::Ref(_, mutable, target) => TyTerm::Ref(
            crate::traits::term::RegionTerm::Static,
            *mutable,
            Box::new(erase_regions(target)),
        ),
        TyTerm::Tuple(fields) => TyTerm::Tuple(fields.iter().map(erase_regions).collect()),
        TyTerm::Array(element, len) => TyTerm::Array(Box::new(erase_regions(element)), len.clone()),
        TyTerm::Slice(element) => TyTerm::Slice(Box::new(erase_regions(element))),
        TyTerm::Nominal(identity, args) => TyTerm::Nominal(
            *identity,
            args.iter()
                .map(|arg| match arg {
                    GenericArg::Type(ty) => GenericArg::Type(erase_regions(ty)),
                    GenericArg::Region(_) => {
                        GenericArg::Region(crate::traits::term::RegionTerm::Static)
                    }
                    other => other.clone(),
                })
                .collect(),
        ),
        other => other.clone(),
    }
}
fn instantiate(
    term: &TyTerm,
    variables: &BTreeMap<ParamKey, InferVarKey>,
    _depth: usize,
) -> Result<TyTerm, Diagnostic> {
    let mut substitution = crate::traits::subst::Substitution::default();
    for (key, variable) in variables {
        substitution.insert(*key, GenericArg::Type(TyTerm::Infer(*variable)))?;
    }
    substitution.apply(term)
}
