//! Owned definition assumptions and separate call substitutions; no goal execution.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    frontend::{
        call_contracts::{
            ClosedReferenceContracts, NormalizedDeferredReturn, PreparedCallContracts,
            primitive_runtime_id,
        },
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
    closed_references: Option<Box<ClosedReferenceContracts>>,
    /// A closed structural (tuple/array) declared result.
    structural_result: Option<TyTerm>,
    structural_references: bool,
    /// A closed declared result naming a nominal Type (`-> Option<u32>`),
    /// as the written Type its declarations select.
    written_result: Option<std::sync::Arc<crate::ir::written_type::WrittenType>>,
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
        let reference_result = contracts
            .closed_references
            .as_ref()
            .filter(|proof| proof.has_result());
        let result_matches = if let Some(proof) = reference_result {
            proof.validate_result_packet(signature, contracts)?;
            true
        } else {
            match signature.result_symbolic.as_ref() {
                Some(source) => {
                    contracts.source_result.as_ref() == Some(source)
                        || Self::instantiates(source, contracts)
                }
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
                    // A written name resolved to a closed nominal/tuple
                    // declaration; the return boundary checks the value.
                    // An opaque (`impl Trait`) or trait-object result keeps
                    // the returned value's own type.
                    (Some(source), None)
                        if crate::frontend::call_contracts::contains_object(source) =>
                    {
                        true
                    }
                    (
                        Some(
                            source @ (TyTerm::Nominal(..) | TyTerm::Tuple(..) | TyTerm::Array(..)),
                        ),
                        None,
                    ) => crate::frontend::call_contracts::closed_term(source),
                    // A closed `&[T]`/`&mut [T]` result: a slice reference
                    // carries its own element window; the return boundary
                    // checks the returned value against the source Type.
                    (Some(source @ TyTerm::Ref(_, _, pointee)), None)
                        if matches!(pointee.as_ref(), TyTerm::Slice(_) | TyTerm::Str) =>
                    {
                        crate::frontend::call_contracts::closed_term(source)
                    }
                    // A closed `&'static T` result names no parameter
                    // region, so no closed-reference proof relates it to
                    // an argument; the return boundary checks the value.
                    (
                        Some(source @ TyTerm::Ref(crate::traits::term::RegionTerm::Static, _, _)),
                        None,
                    ) => crate::frontend::call_contracts::closed_term(source),
                    // A closed fn-pointer result (E-R5-008); the return
                    // boundary checks the returned value against it.
                    (Some(source @ TyTerm::Function(_)), None) => {
                        crate::frontend::call_contracts::closed_term(source)
                    }
                    // A normalized associated-type result; the return
                    // boundary checks the returned value against it.
                    (Some(TyTerm::Primitive(_)), None) => {
                        signature.result.len() > 1 && contracts.normalized_deferred_return.is_none()
                    }
                    _ => false,
                },
            }
        };
        if signature.definition != contracts.definition
            || signature.parameters.len() != contracts.source_parameters.len()
            || signature.parameters.len() != contracts.parameter_descriptors.len()
            || contracts
                .parameter_descriptors
                .iter()
                .any(|descriptor| descriptor.0 == 0)
            || !result_matches
            || contracts.source_substitution.len() != contracts.runtime_substitution.len()
        {
            return Err(invalid(&format!(
                "definition/call environment identity or arity mismatch (definition {}, parameters {}/{}/{}, result {} [{:?} vs {:?}], substitution {}/{})",
                signature.definition == contracts.definition,
                signature.parameters.len(),
                contracts.source_parameters.len(),
                contracts.parameter_descriptors.len(),
                result_matches,
                contracts.source_result,
                signature.result_symbolic,
                contracts.source_substitution.len(),
                contracts.runtime_substitution.len(),
            )));
        }
        if let Some(proof) = &contracts.normalized_deferred_return {
            proof.validate_packet(signature, contracts)?;
        }
        if let Some(proof) = &contracts.closed_references {
            proof.validate_packet(signature, contracts)?;
        } else if !contracts.structural_references
            && contracts
                .source_parameters
                .iter()
                .any(|source| matches!(source, TyTerm::Ref(..)))
        {
            return Err(invalid(
                "closed reference parameter requires its current source certificate",
            ));
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
            if ordinals.insert(parameter.ordinal, parameter.kind).is_some() {
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
        if let Some(proof) = &contracts.closed_references {
            bytes = bytes
                .checked_add(proof.allocated_bytes()?)
                .ok_or_else(|| invalid("closed reference retained size overflow"))?;
        }
        Ok(bytes)
    }
    /// Whether the call's source result is the declared symbolic result
    /// under the call's source bindings (`-> &mut T` with `T = S`).
    fn instantiates(declared: &TyTerm, contracts: &PreparedCallContracts) -> bool {
        let mut substitution = crate::traits::subst::Substitution::default();
        for (key, bound) in &contracts.source_substitution {
            if substitution
                .insert(*key, crate::traits::term::GenericArg::Type(bound.clone()))
                .is_err()
            {
                return false;
            }
        }
        substitution
            .apply(declared)
            .ok()
            .is_some_and(|applied| contracts.source_result.as_ref() == Some(&applied))
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
        if contracts.closed_references.is_some() {
            return Err(invalid(
                "closed reference parameters require current source admission",
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
        if let Some(proof) = &contracts.closed_references {
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
            closed_references: contracts.closed_references.clone(),
            structural_result: contracts.source_result.clone().filter(|term| {
                (matches!(term, TyTerm::Tuple(_) | TyTerm::Array(..))
                        // A reference bound structurally from generic arguments
                        // (`-> &'a mut V` with `V` the reached value type).
                        || (contracts.structural_references && matches!(term, TyTerm::Ref(..)))
                        || matches!(term, TyTerm::Ref(_, _, pointee)
                            if matches!(pointee.as_ref(), TyTerm::Slice(_) | TyTerm::Str))
                        || matches!(
                            term,
                            TyTerm::Ref(crate::traits::term::RegionTerm::Static, _, _)
                        ))
                    && crate::frontend::call_contracts::closed_term(term)
            }),
            structural_references: contracts.structural_references,
            written_result: None,
        })
    }
    /// Attach the written Type the caller selected for this definition's
    /// closed nominal result.
    pub(crate) fn with_written_result(
        mut self,
        written: Option<std::sync::Arc<crate::ir::written_type::WrittenType>>,
    ) -> Self {
        self.written_result = written;
        self
    }
    pub(crate) fn written_result(&self) -> Option<&crate::ir::written_type::WrittenType> {
        self.written_result.as_deref()
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
    /// Readonly retained authority; no mutable table or source constructor.
    pub fn closed_reference_contracts(&self) -> Option<&ClosedReferenceContracts> {
        self.closed_references.as_deref()
    }
    /// Source-selected result, captured with the current declaration and actual
    /// input reference target. No query or runtime descriptor invents this view.
    pub(crate) fn reference_result_source(&self) -> Result<&TyTerm, Diagnostic> {
        self.closed_references
            .as_ref()
            .filter(|proof| proof.has_result())
            .and_then(|proof| proof.result_source())
            .ok_or_else(|| invalid("reference result requires its admitted source certificate"))
    }
    /// The runtime descriptor of the declared result when it is built from
    /// this call's generic bindings and structural constructors (`&mut V`
    /// with `V` bound), if the store already has it.
    pub(crate) fn bound_result_descriptor_find(
        &self,
        store: &crate::runtime::cell::GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        // Source-resolved member results may not have a symbolic template
        // on the original signature. The admitted structural result retains
        // the resolved source shape for this same call.
        Ok(self
            .definition
            .result_symbolic
            .iter()
            .chain(self.structural_result.iter())
            .find_map(|term| self.bound_find(term, store)))
    }
    /// `bound_result_descriptor_find`, interning the descriptor.
    pub(crate) fn bound_result_descriptor_intern(
        &self,
        store: &mut crate::runtime::cell::GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        for term in self
            .definition
            .result_symbolic
            .iter()
            .chain(self.structural_result.iter())
        {
            if let Some(descriptor) = self.bound_intern(term, store)? {
                return Ok(Some(descriptor));
            }
        }
        Ok(None)
    }
    /// The actual descriptor paired with an admitted source substitution.
    /// Closed nominal/closure leaves cannot be reconstructed from a source
    /// name: preserve the reached descriptor already bound by this call.
    fn bound_source_descriptor(&self, term: &TyTerm) -> Option<TypeId> {
        let mut found = None;
        for (key, source) in self.symbolic.iter() {
            if source != term {
                continue;
            }
            let descriptor = self.runtime_binding(*key)?;
            // Source views can erase dynamic/capture structure. Never pick
            // an arbitrary runtime representation from equal source views.
            if found.is_some_and(|prior| prior != descriptor) {
                return None;
            }
            found = Some(descriptor);
        }
        found
    }

    fn bound_find(
        &self,
        term: &TyTerm,
        store: &crate::runtime::cell::GuestStore,
    ) -> Option<TypeId> {
        use crate::runtime::shape::TypeKey;
        if let Some(descriptor) = self.bound_source_descriptor(term) {
            return Some(descriptor);
        }
        match term {
            TyTerm::Param(key) => self.runtime_binding(*key),
            TyTerm::Ref(_, mutable, inner) => {
                let target = self.bound_find(inner, store)?;
                store.find_type(&TypeKey::Ref {
                    mutable: *mutable,
                    target,
                })
            }
            TyTerm::Slice(element) => {
                let element = self.bound_find(element, store)?;
                store.find_type(&TypeKey::Slice { element })
            }
            TyTerm::Tuple(fields) if !fields.is_empty() => {
                let ids = fields
                    .iter()
                    .map(|field| self.bound_find(field, store))
                    .collect::<Option<Vec<_>>>()?;
                store.find_type(&TypeKey::Tuple(ids.into_boxed_slice()))
            }
            other => store.find_structural_term(other),
        }
    }
    fn bound_intern(
        &self,
        term: &TyTerm,
        store: &mut crate::runtime::cell::GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        use crate::runtime::shape::TypeKey;
        if let Some(descriptor) = self.bound_source_descriptor(term) {
            return Ok(Some(descriptor));
        }
        Ok(match term {
            TyTerm::Param(key) => self.runtime_binding(*key),
            TyTerm::Ref(_, mutable, inner) => {
                let Some(target) = self.bound_intern(inner, store)? else {
                    return Ok(None);
                };
                Some(store.intern_type(TypeKey::Ref {
                    mutable: *mutable,
                    target,
                })?)
            }
            TyTerm::Slice(element) => {
                let Some(element) = self.bound_intern(element, store)? else {
                    return Ok(None);
                };
                Some(store.intern_type(TypeKey::Slice { element })?)
            }
            TyTerm::Tuple(fields) if !fields.is_empty() => {
                let mut ids = Vec::with_capacity(fields.len());
                for field in fields {
                    let Some(id) = self.bound_intern(field, store)? else {
                        return Ok(None);
                    };
                    ids.push(id);
                }
                Some(store.intern_type(TypeKey::Tuple(ids.into_boxed_slice()))?)
            }
            other => store.intern_structural_term(other)?,
        })
    }
    /// Parameters bound structurally from reached arguments.
    pub(crate) fn structural_references(&self) -> bool {
        self.structural_references
    }
    /// The closed tuple/array declared result, when there is one.
    pub(crate) fn structural_result(&self) -> Option<&TyTerm> {
        self.structural_result.as_ref()
    }
    pub(crate) fn reference_parameter_source(&self, ordinal: usize) -> Option<&TyTerm> {
        self.closed_references
            .as_ref()
            .and_then(|proof| proof.parameter_source(ordinal))
    }
    pub(crate) fn require_reference_parameter_descriptor(
        &self,
        ordinal: usize,
        store: &crate::runtime::cell::GuestStore,
        descriptor: TypeId,
    ) -> Result<(), Diagnostic> {
        // Structurally bound parameters were matched against the reached
        // argument descriptor when the contracts were prepared.
        if self.structural_references
            && self
                .closed_references
                .as_ref()
                .and_then(|proof| proof.parameter_source(ordinal))
                .is_none()
        {
            let _ = (store, descriptor);
            return Ok(());
        }
        self.closed_references
            .as_ref()
            .ok_or_else(|| invalid("reference parameter source certificate absent"))?
            .require_parameter_descriptor(ordinal, store, descriptor)
    }
    pub(crate) fn reference_result_descriptor(
        &self,
        store: &crate::runtime::cell::GuestStore,
    ) -> Result<Option<TypeId>, Diagnostic> {
        self.closed_references
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?
            .result_hint_descriptor(store)
    }
    pub(crate) fn materialize_reference_result_descriptor(
        &self,
        store: &mut crate::runtime::cell::GuestStore,
    ) -> Result<TypeId, Diagnostic> {
        self.closed_references
            .as_ref()
            .ok_or_else(|| invalid("reference result source certificate absent"))?
            .materialize_result_hint(store)
    }
    pub fn retained_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(test)]
#[path = "context_return_tests.rs"]
mod return_tests;

#[cfg(test)]
#[path = "context_reference_tests.rs"]
mod reference_tests;
