//! Owned definition assumptions and separate call substitutions; no goal execution.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    frontend::{
        call_contracts::{NormalizedDeferredReturn, PreparedCallContracts, primitive_runtime_id},
        callables::SourceCallable,
    },
    runtime::shape::TypeId,
    syntax::signatures::{FunctionSignature, GenericKind},
    traits::term::{ParamKey, TyTerm},
};
use std::collections::BTreeMap;
#[derive(Debug)]
pub struct GenericEnv {
    definition: Box<FunctionSignature>,
    signature_key: Digest,
    symbolic: Box<[(ParamKey, TyTerm)]>,
    runtime: Box<[(ParamKey, TypeId)]>,
    bytes: usize,
    normalized_return: Option<Box<NormalizedDeferredReturn>>,
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CALL-CONTEXT", message.into())
}
/// Input maps iterate in canonical key order. The retained owner is immutable,
/// so boxed sorted entries provide lookup without hidden tree-node storage.
fn own_table<K: Copy + Ord, V: Clone>(
    values: &BTreeMap<K, V>,
) -> Result<Box<[(K, V)]>, Diagnostic> {
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(values.len())
        .map_err(|_| crate::runtime::arena::resource())?;
    entries.extend(values.iter().map(|(key, value)| (*key, value.clone())));
    Ok(entries.into_boxed_slice())
}
impl GenericEnv {
    /// Check identity and retained sizes before allocating the owning copy.
    pub fn checked_bytes(
        signature: &FunctionSignature,
        contracts: &PreparedCallContracts,
    ) -> Result<usize, Diagnostic> {
        // The declaration owner remains byte-exact. A deferred written
        // result may have a separately normalized primitive contract, just
        // as named alias parameters have resolved source templates.
        let result_matches = match signature.result_symbolic.as_ref() {
            Some(source) => contracts.source_result.as_ref() == Some(source),
            None => match (
                contracts.source_result.as_ref(),
                contracts.result_descriptor,
            ) {
                (None, None) => true,
                (Some(TyTerm::Primitive(primitive)), Some(descriptor)) => {
                    primitive_runtime_id(*primitive)? == descriptor
                        && contracts
                            .normalized_deferred_return
                            .as_ref()
                            .is_some_and(|proof| {
                                proof.validate_packet(signature, contracts).is_ok()
                            })
                }
                _ => false,
            },
        };
        if signature.definition != contracts.definition
            || signature.parameters.len() != contracts.source_parameters.len()
            || signature.parameters.len() != contracts.parameter_descriptors.len()
            || contracts
                .parameter_descriptors
                .iter()
                .any(|descriptor| descriptor.0 == 0)
            || !result_matches
            || signature.generics.len() > 100_000
            || contracts.runtime_substitution.len() > 100_000
            || contracts.source_substitution.len() != contracts.runtime_substitution.len()
        {
            return Err(invalid(
                "definition/call environment identity or arity mismatch",
            ));
        }
        if let Some(proof) = &contracts.normalized_deferred_return {
            proof.validate_packet(signature, contracts)?;
        }
        // The immutable declaration is also copied into the frame. Validate
        // its symbolic views as well as the resolved contract templates before
        // allocating that owner; identity agreement alone is not admission.
        for source in signature
            .parameters
            .iter()
            .filter_map(|parameter| parameter.symbolic.as_ref())
            .chain(signature.result_symbolic.as_ref())
            .chain(contracts.source_result.as_ref())
            .chain(&contracts.source_parameters)
        {
            source.validate_opaque_shape().map_err(|_| {
                invalid("retained source template has invalid opaque shape or exceeds structural limits")
            })?;
            if crate::frontend::call_contracts::contains_transaction_term(source) {
                return Err(invalid(
                    "retained source template exceeds checked frame limits",
                ));
            }
        }
        let mut ordinals = BTreeMap::new();
        for parameter in &signature.generics {
            if parameter.ordinal >= 100_000
                || ordinals.insert(parameter.ordinal, parameter.kind).is_some()
            {
                return Err(invalid("definition environment parameter ordinal mismatch"));
            }
        }
        let mut bytes = signature
            .estimated_bytes()
            .checked_add(std::mem::size_of::<Self>())
            .and_then(|bytes| {
                bytes.checked_add(
                    contracts
                        .source_substitution
                        .len()
                        .checked_mul(std::mem::size_of::<(ParamKey, TyTerm)>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    contracts
                        .runtime_substitution
                        .len()
                        .checked_mul(std::mem::size_of::<(ParamKey, TypeId)>())?,
                )
            })
            .ok_or_else(|| invalid("definition environment size overflow"))?;
        for (key, descriptor) in &contracts.runtime_substitution {
            if key.definition != signature.definition
                || descriptor.0 == 0
                || ordinals.get(&key.ordinal) != Some(&GenericKind::Type)
            {
                return Err(invalid(
                    "runtime binding belongs to another definition parameter",
                ));
            }
            let source = contracts
                .source_substitution
                .get(key)
                .ok_or_else(|| invalid("runtime binding lacks a symbolic substitution"))?;
            let source_bytes = source
                .heap_bytes()
                .ok_or_else(|| invalid("substitution term exceeds checked structural limits"))?;
            source.validate_opaque_shape().map_err(|_| {
                invalid(
                    "symbolic substitution has invalid opaque shape or exceeds structural limits",
                )
            })?;
            if crate::frontend::call_contracts::contains_transaction_term(source) {
                return Err(invalid(
                    "transaction inference or placeholder cannot enter a frame environment",
                ));
            }
            if let TyTerm::Primitive(primitive) = source
                && primitive_runtime_id(*primitive)? != *descriptor
            {
                return Err(invalid(
                    "symbolic primitive and runtime substitution disagree",
                ));
            }
            bytes = bytes
                .checked_add(source_bytes)
                .ok_or_else(|| invalid("substitution environment size overflow"))?;
        }
        if let Some(proof) = &contracts.normalized_deferred_return {
            bytes = bytes
                .checked_add(
                    proof
                        .metadata_bytes()
                        .ok_or_else(|| invalid("normalized return certificate size overflow"))?,
                )
                .ok_or_else(|| invalid("normalized return retained size overflow"))?;
        }
        if bytes > 32 * 1024 * 1024 {
            return Err(crate::runtime::arena::resource());
        }
        Ok(bytes)
    }
    pub fn capture(
        signature: &FunctionSignature,
        contracts: &PreparedCallContracts,
    ) -> Result<Self, Diagnostic> {
        if contracts.normalized_deferred_return.is_some() {
            return Err(invalid(
                "normalized deferred return requires current source admission",
            ));
        }
        Self::capture_owned(signature, contracts)
    }
    pub fn capture_current(
        db: &mut crate::db::memo::Database,
        callee: &SourceCallable,
        contracts: &PreparedCallContracts,
    ) -> Result<Self, Diagnostic> {
        callee.revalidate(db)?;
        if let Some(proof) = &contracts.normalized_deferred_return {
            proof.validate_packet(callee.signature(), contracts)?;
            proof.require_current(db, callee)?;
        }
        Self::capture_owned(callee.signature(), contracts)
    }
    fn capture_owned(
        signature: &FunctionSignature,
        contracts: &PreparedCallContracts,
    ) -> Result<Self, Diagnostic> {
        let bytes = Self::checked_bytes(signature, contracts)?;
        Ok(Self {
            definition: Box::new(signature.clone()),
            signature_key: signature.fingerprint(),
            symbolic: own_table(&contracts.source_substitution)?,
            runtime: own_table(&contracts.runtime_substitution)?,
            bytes,
            normalized_return: contracts.normalized_deferred_return.clone(),
        })
    }
    pub fn definition(&self) -> &FunctionSignature {
        &self.definition
    }
    pub fn signature_key(&self) -> Digest {
        self.signature_key
    }
    pub fn symbolic_binding(&self, key: ParamKey) -> Option<&TyTerm> {
        self.symbolic
            .binary_search_by_key(&key, |(parameter, _)| *parameter)
            .ok()
            .and_then(|index| self.symbolic.get(index))
            .map(|(_, source)| source)
    }
    pub fn runtime_binding(&self, key: ParamKey) -> Option<TypeId> {
        self.runtime
            .binary_search_by_key(&key, |(parameter, _)| *parameter)
            .ok()
            .and_then(|index| self.runtime.get(index))
            .map(|(_, descriptor)| *descriptor)
    }
    pub(crate) fn normalized_root_hint(
        &self,
        hint: &crate::ir::op::HintRecipe,
    ) -> Result<Option<(crate::ir::op::PrimitiveType, TypeId)>, Diagnostic> {
        match &self.normalized_return {
            Some(proof) => proof.for_written_hint(hint),
            None => Ok(None),
        }
    }
    pub(crate) fn normalized_result(&self) -> Option<(crate::ir::op::PrimitiveType, TypeId)> {
        self.normalized_return
            .as_ref()
            .map(|proof| proof.resolved_result())
    }
    pub fn retained_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
#[path = "context_return_tests.rs"]
mod return_tests;
