//! Owned definition assumptions and separate call substitutions; no goal execution.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
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
    /// Worker-local value/code identity; runtime descriptor IDs are not portable keys.
    application: Option<Digest>,
    symbolic: Box<[(ParamKey, TyTerm)]>,
    runtime: Box<[(ParamKey, TypeId)]>,
    bytes: usize,
    normalized_return: Option<Box<NormalizedDeferredReturn>>,
    closed_references: Option<Box<ClosedReferenceContracts>>,
    /// A closed structural (tuple/array) declared result.
    structural_result: Option<TyTerm>,
    structural_references: bool,
    /// The admitted closed source result and its exact run-local descriptor.
    /// Cold call preparation instantiates the declared written template with
    /// actual runtime generic bindings (including anonymous callable Types).
    written_result: Option<(TyTerm, TypeId)>,
    /// Actual source-selected result structure, including coercion sites.
    written_result_template: Option<std::sync::Arc<crate::ir::written_type::WrittenType>>,
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
    /// Exact emptiness of the retained source/runtime application tables.
    /// A missing application digest alone is not evidence of no bindings.
    pub(crate) fn has_empty_bindings(&self) -> bool {
        self.symbolic.is_empty() && self.runtime.is_empty()
    }

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
                    // A written nominal/tuple/array result: closed, or an open
                    // template over this call's own generic positions
                    // (`Empty<T>`, `Vec<T>`, `Result<F, <F as FromStr>::Err>`)
                    // when the reached arguments and result context bound none
                    // (or only some) of them, or a projection has no selectable
                    // impl yet. An open template is admitted structurally:
                    // every free position must be one of this definition's own
                    // generics. The returned value, not this identity check,
                    // then supplies the open positions.
                    (
                        Some(
                            source @ (TyTerm::Nominal(..) | TyTerm::Tuple(..) | TyTerm::Array(..)),
                        ),
                        None,
                    ) => Self::declared_result_admitted(signature, source),
                    // A closed `&[T]`/`&mut [T]` result: a slice reference
                    // carries its own element window; the return boundary
                    // checks the returned value against the source Type.
                    (Some(source @ TyTerm::Ref(_, _, pointee)), None)
                        if matches!(pointee.as_ref(), TyTerm::Slice(_) | TyTerm::Str) =>
                    {
                        Self::declared_result_admitted(signature, source)
                    }
                    // A closed `&'static T` result names no parameter
                    // region, so no closed-reference proof relates it to
                    // an argument; the return boundary checks the value.
                    (
                        Some(source @ TyTerm::Ref(crate::traits::term::RegionTerm::Static, _, _)),
                        None,
                    ) => Self::declared_result_admitted(signature, source),
                    // A closed fn-pointer result (E-R5-008); the return
                    // boundary checks the returned value against it.
                    (Some(source @ TyTerm::Function(_)), None) => {
                        Self::declared_result_admitted(signature, source)
                    }
                    // A written associated-type or opaque result that no
                    // impl normalized at the call: the declared projection
                    // template (over this definition's own generics) stays
                    // open and the returned value supplies it.
                    (Some(source @ (TyTerm::Projection(_) | TyTerm::Opaque(..))), None) => {
                        Self::declared_result_admitted(signature, source)
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
    /// A declared written result whose source term is closed, or whose free
    /// generic positions are all this definition's own: type and const
    /// parameters (including those inside projection and opaque arguments and
    /// fn-pointer binders), never another declaration's parameter, an
    /// inference variable or a placeholder from a transaction. Regions are
    /// metadata, so lifetime positions only need an owning definition or a
    /// local/static identity.
    fn declared_result_admitted<'a>(signature: &FunctionSignature, source: &'a TyTerm) -> bool {
        use crate::traits::term::{ConstTerm, GenericArg, RegionTerm};
        if crate::frontend::call_contracts::closed_term(source) {
            return true;
        }
        let owns = |key: &ParamKey, kind: GenericKind| {
            key.definition == signature.definition
                && signature
                    .generics
                    .iter()
                    .any(|parameter| parameter.ordinal == key.ordinal && parameter.kind == kind)
        };
        let region_ok = |region: &RegionTerm, binders: usize| match region {
            RegionTerm::Static | RegionTerm::Local { .. } => true,
            RegionTerm::Param(key) => key.definition == signature.definition,
            RegionTerm::Bound { depth, .. } => (*depth as usize) < binders,
            RegionTerm::Placeholder(_) => false,
        };
        let const_ok = |constant: &ConstTerm, binders: usize| match constant {
            ConstTerm::Value(_) => true,
            ConstTerm::Param(key) => owns(key, GenericKind::Const),
            ConstTerm::Bound { depth, .. } => (*depth as usize) < binders,
            ConstTerm::Placeholder(_) | ConstTerm::Infer(_) => false,
        };
        let mut work = vec![(source, 0usize)];
        while let Some((term, binders)) = work.pop() {
            let arguments = |args: &'a [GenericArg], work: &mut Vec<(&'a TyTerm, usize)>| -> bool {
                for arg in args {
                    match arg {
                        GenericArg::Type(ty) => work.push((ty, binders)),
                        GenericArg::Region(region) => {
                            if !region_ok(region, binders) {
                                return false;
                            }
                        }
                        GenericArg::Const(constant) => {
                            if !const_ok(constant, binders) {
                                return false;
                            }
                        }
                    }
                }
                true
            };
            match term {
                TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Never | TyTerm::Dyn(_) => {}
                TyTerm::Param(key) => {
                    if !owns(key, GenericKind::Type) {
                        return false;
                    }
                }
                TyTerm::Bound { depth, .. } => {
                    if (*depth as usize) >= binders {
                        return false;
                    }
                }
                TyTerm::Infer(_) | TyTerm::Placeholder(_) => return false,
                TyTerm::Ref(region, _, target) => {
                    if !region_ok(region, binders) {
                        return false;
                    }
                    work.push((target, binders));
                }
                TyTerm::Slice(element) => work.push((element, binders)),
                TyTerm::Array(element, length) => {
                    if !const_ok(length, binders) {
                        return false;
                    }
                    work.push((element, binders));
                }
                TyTerm::Tuple(fields) => work.extend(fields.iter().map(|field| (field, binders))),
                TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => {
                    if !arguments(args, &mut work) {
                        return false;
                    }
                }
                TyTerm::Projection(key) => {
                    work.push((&key.trait_ref.self_ty, binders));
                    if !arguments(&key.trait_ref.args, &mut work)
                        || !arguments(&key.item_args, &mut work)
                    {
                        return false;
                    }
                }
                TyTerm::Function(function) => {
                    let inner = binders + 1;
                    work.extend(function.value.parameters.iter().map(|ty| (ty, inner)));
                    work.push((function.value.result.as_ref(), inner));
                }
            }
        }
        true
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
        let symbolic = own_table(&contracts.source_substitution)?;
        let runtime = own_table(&contracts.runtime_substitution)?;
        let application = if symbolic.is_empty() && runtime.is_empty() {
            None
        } else {
            let mut encoder = StableEncoder::new("lrr/v1/worker-generic-application");
            encoder.digest(signature.fingerprint());
            encoder.sequence(&symbolic, |e, (key, source)| {
                e.record(key);
                e.record(source);
            });
            encoder.sequence(&runtime, |e, (key, descriptor)| {
                e.record(key);
                e.u32(descriptor.0);
            });
            Some(encoder.finish())
        };
        Ok(Self {
            definition: Box::new(signature.clone()),
            signature_key: signature.fingerprint(),
            application,
            symbolic,
            runtime,
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
            written_result_template: None,
        })
    }
    /// Attach the descriptor of the source-selected declared result after its
    /// written template is instantiated from this call's runtime bindings.
    pub(crate) fn with_written_result(mut self, source: TyTerm, descriptor: TypeId) -> Self {
        self.written_result = Some((source, descriptor));
        self
    }
    pub(crate) fn with_written_result_template(
        mut self,
        written: std::sync::Arc<crate::ir::written_type::WrittenType>,
    ) -> Result<Self, Diagnostic> {
        if self.written_result_template.is_some() {
            return Err(invalid("declared result template already retained"));
        }
        self.bytes = self
            .bytes
            .checked_add(std::mem::size_of::<crate::ir::written_type::WrittenType>())
            .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
            .and_then(|bytes| bytes.checked_add(written.heap_bytes()))
            .ok_or_else(|| invalid("declared result template metadata overflow"))?;
        self.written_result_template = Some(written);
        Ok(self)
    }
    pub(crate) fn written_result_template(
        &self,
    ) -> Option<&std::sync::Arc<crate::ir::written_type::WrittenType>> {
        self.written_result_template.as_ref()
    }
    pub(crate) fn written_result_descriptor(&self) -> Option<TypeId> {
        self.written_result
            .as_ref()
            .map(|(_, descriptor)| *descriptor)
    }
    pub(crate) fn written_result_source(&self) -> Option<&TyTerm> {
        self.written_result.as_ref().map(|(source, _)| source)
    }
    pub fn definition(&self) -> &FunctionSignature {
        &self.definition
    }
    pub fn signature_key(&self) -> Digest {
        self.signature_key
    }
    pub(crate) fn application_identity(&self) -> Option<Digest> {
        self.application
    }
    /// An inherited lexical environment remains tied to its actual creating
    /// declaration and the existing source-selected proof dependencies.
    pub(crate) fn require_current(
        &self,
        db: &mut crate::db::memo::Database,
        owner: &SourceCallable,
    ) -> Result<(), Diagnostic> {
        if self.signature_key != owner.signature().fingerprint() {
            return Err(invalid("lexical generic environment declaration changed"));
        }
        if let Some(proof) = &self.normalized_return {
            proof.require_current_inherited(db, owner)?;
        }
        if let Some(proof) = &self.closed_references {
            proof.require_current(db, owner)?;
        }
        Ok(())
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
        // Declaration-selected results use their own exact source/descriptor
        // service. They are not printed primitive syntax to be remapped.
        // Frame's DefinitionResult/WrittenResult consumers check the current
        // definition/root and the same retained call environment.
        if matches!(hint, crate::ir::op::HintRecipe::DefinitionResult)
            || (matches!(hint, crate::ir::op::HintRecipe::DefinitionWrittenResult)
                && self.written_result.is_some())
        {
            return Ok(None);
        }
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
