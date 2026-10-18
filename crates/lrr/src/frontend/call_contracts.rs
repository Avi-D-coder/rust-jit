//! Reached call boundaries keep source templates separate from concrete descriptors.
use super::{
    callables::SourceCallable,
    names::NameNamespaces,
    signature_names::SignatureName,
    signature_subst::SignatureSubstitution,
    type_aliases::{AliasNormalizer, AliasTarget},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::op::PrimitiveType,
    runtime::shape::TypeId,
    source::changes::PreparedSourceView,
    syntax::ast_adapter::TokenRecipe,
    traits::term::{ParamKey, TyTerm},
};
use std::{collections::BTreeMap, sync::Arc};
#[path = "closed_reference_calls.rs"]
mod closed_reference_calls;
#[path = "generated_result_namespace.rs"]
pub(super) mod generated_result_namespace;
pub(crate) use closed_reference_calls::{ClosedReferenceArgument, ReferenceCallArguments};
pub use closed_reference_calls::{ClosedReferenceContracts, ClosedReferenceParameter};
#[derive(Debug, Clone)]
pub struct ArgumentType {
    pub source: TyTerm,
    pub descriptor: TypeId,
}
pub struct PreparedCallContracts {
    pub closed_references: Option<Box<ClosedReferenceContracts>>,
    pub(crate) inferred_equations: Option<Arc<PreparedInputGuard>>,
    pub definition: Digest,
    pub source_parameters: Vec<TyTerm>,
    pub parameter_descriptors: Vec<TypeId>,
    pub source_substitution: BTreeMap<ParamKey, TyTerm>,
    pub runtime_substitution: BTreeMap<ParamKey, TypeId>,
    pub source_result: Option<TyTerm>,
    pub result_descriptor: Option<TypeId>,
    /// Only the frontend can mint this; public packet fields are checked against it.
    pub normalized_deferred_return: Option<Box<NormalizedDeferredReturn>>,
    /// Generic parameters were bound structurally from reached arguments
    /// (`&T`, `W<T>`); their reference parameters carry no closed certificate.
    pub structural_references: bool,
}

/// Successful owning source normalization. No public constructor or mutable fields.
#[derive(Debug, Clone)]
pub struct NormalizedDeferredReturn {
    definition: Digest,
    signature: Digest,
    origin: DeferredReturnOrigin,
    source_revision: u64,
    query_revision: u64,
    source: PrimitiveType,
    descriptor: TypeId,
    written_hint: Option<PrimitiveType>,
    dependencies: Box<[DependencyStamp]>,
}
#[derive(Debug, Clone)]
enum DeferredReturnOrigin {
    DeclaredGoal {
        observation: Digest,
        guard: Arc<PreparedInputGuard>,
    },
    PrimitiveNamespace,
    Alias {
        resolution: Digest,
        alias: Digest,
    },
    Lexical {
        plan: Digest,
        parameters: Box<[PrimitiveType]>,
    },
}
impl NormalizedDeferredReturn {
    fn mint(
        db: &mut Database,
        callee: &SourceCallable,
        resolution: &super::names::NameDefinition,
        source: PrimitiveType,
        trace: Vec<DependencyStamp>,
    ) -> Result<Self, Diagnostic> {
        let signature = callee.signature();
        if signature.result_symbolic.is_some() || signature.result.len() != 1 {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "normalized return requires its deferred declared source",
            ));
        }
        let descriptor = primitive_runtime_id(source)?;
        let mut dependencies = Vec::new();
        let rows = trace.len().checked_add(9).ok_or_else(|| {
            fault(
                "LRR-E-METADATA-LIMIT",
                "normalized return dependency row budget exceeded",
            )
        })?;
        dependencies
            .try_reserve_exact(rows)
            .map_err(|_| crate::runtime::arena::resource())?;
        for key in [
            InputKey::DefinitionSource(signature.definition),
            InputKey::DefinitionHeader(signature.definition),
            InputKey::DeclarationContext(signature.definition),
            InputKey::DefinitionSource(resolution.identity),
            InputKey::DefinitionHeader(resolution.identity),
            InputKey::DeclarationContext(resolution.identity),
            InputKey::SourceDependencies,
            InputKey::CrateCfg(signature.scope.crate_key),
        ] {
            let value = db.input(&key).ok_or_else(|| {
                fault(
                    "LRR-E-CALL-CONTEXT",
                    "normalized return source publication absent",
                )
            })?;
            dependencies.push(DependencyStamp::Input {
                key,
                fingerprint: value.fingerprint(),
            });
        }
        let key = QueryKey::TypeAlias(resolution.identity);
        let outcome = match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return Err(e),
            QueryPoll::NeedInput(_) => {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "normalized return alias certificate is not current",
                ));
            }
        };
        dependencies.push(DependencyStamp::Query {
            key,
            fingerprint: outcome.fingerprint,
        });
        dependencies.extend(trace);
        dependencies.sort_unstable();
        for pair in dependencies.windows(2) {
            let same_key = match (&pair[0], &pair[1]) {
                (DependencyStamp::Input { key: a, .. }, DependencyStamp::Input { key: b, .. }) => {
                    a == b
                }
                (DependencyStamp::Query { key: a, .. }, DependencyStamp::Query { key: b, .. }) => {
                    a == b
                }
                _ => false,
            };
            if same_key && pair[0] != pair[1] {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "normalized return dependency fingerprints conflict",
                ));
            }
        }
        dependencies.dedup();
        let _bytes = dependencies
            .iter()
            .try_fold(std::mem::size_of::<Self>(), |bytes, stamp| {
                bytes
                    .checked_add(std::mem::size_of::<DependencyStamp>())?
                    .checked_add(stamp.heap_bytes())
            });
        let written_hint = printed_primitive_hint(&signature.result[0].text);
        Ok(Self {
            definition: signature.definition,
            signature: signature.fingerprint(),
            origin: DeferredReturnOrigin::Alias {
                resolution: resolution.fingerprint(),
                alias: resolution.identity,
            },
            source_revision: db.source_revision(),
            query_revision: db.revision(),
            source,
            descriptor,
            written_hint,
            dependencies: dependencies.into_boxed_slice(),
        })
    }
    /// Retain the same owning primitive namespace decision used by ordinary
    /// call admission, so the VM never reconstructs source type from a layout.
    fn mint_primitive(
        db: &mut Database,
        callee: &SourceCallable,
        graph: &NameNamespaces,
        sources: &[&PreparedSourceView],
        source: PrimitiveType,
    ) -> Result<Self, Diagnostic> {
        callee.revalidate(db)?;
        let signature = callee.signature();
        if signature.result_symbolic.is_some()
            || signature.result.len() != 1
            || match super::source_types::resolve_named(
                db,
                graph,
                sources,
                signature,
                &signature.result[0].text,
            ) {
                QueryPoll::Ready(Ok(selected)) => selected != SignatureName::Primitive(source),
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(_) => {
                    return Err(fault(
                        "LRR-E-CALL-CONTEXT",
                        "primitive result source certificate is not current",
                    ));
                }
            }
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "primitive result namespace certificate differs",
            ));
        }
        let stamps = db.source_view_namespace_stamps()?;
        // The coordinator already checks the full input census at100k rows and
        //32MiB.4096 below bounds captured sources, not their derived input rows:
        // one valid module contributes several independent source/scope facts.
        // Keep every fingerprint; do not truncate the namespace to fit a source count.
        if stamps.is_empty() {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "primitive result namespace dependency count",
            ));
        }
        Self::require_primitive_namespace(db, signature, sources, &stamps)?;
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(stamps.len())
            .map_err(|_| crate::runtime::arena::resource())?;
        for (key, fingerprint) in stamps {
            dependencies.push(DependencyStamp::Input { key, fingerprint });
        }
        dependencies.sort_unstable();
        let proof = Self {
            definition: signature.definition,
            signature: signature.fingerprint(),
            origin: DeferredReturnOrigin::PrimitiveNamespace,
            source_revision: db.source_revision(),
            query_revision: db.revision(),
            source,
            descriptor: primitive_runtime_id(source)?,
            written_hint: printed_primitive_hint(&signature.result[0].text),
            dependencies: dependencies.into_boxed_slice(),
        };
        Ok(proof)
    }
    /// Join the supplied graph's primitive-name absence to current owning
    /// namespace evidence. Input fingerprints alone cannot certify an unrelated
    /// graph; a published scope or exact original-source rebuild must agree.
    fn require_primitive_namespace(
        db: &mut Database,
        signature: &crate::syntax::signatures::FunctionSignature,
        sources: &[&PreparedSourceView],
        stamps: &BTreeMap<InputKey, Digest>,
    ) -> Result<(), Diagnostic> {
        Self::require_primitive_name_namespace(db, signature, sources, stamps, None)
    }
    fn require_primitive_name_namespace(
        db: &mut Database,
        signature: &crate::syntax::signatures::FunctionSignature,
        sources: &[&PreparedSourceView],
        stamps: &BTreeMap<InputKey, Digest>,
        written_name: Option<&str>,
    ) -> Result<(), Diagnostic> {
        // Names in an impl's functions resolve in the enclosing module (see
        // source_types::impl_owned_name); there is no owner Type namespace.
        if signature.scope.owner.is_some() {
            return Ok(());
        }
        let missing = || {
            fault(
                "LRR-E-CALL-CONTEXT",
                "primitive result requires its current owning Type namespace",
            )
        };
        let Some((InputValue::DefinitionSource(parse), _)) =
            db.published_input(&InputKey::DefinitionSource(signature.definition))
        else {
            return Err(missing());
        };
        let parse = parse.clone();
        if db.locates_generated_declaration(&parse) {
            if written_name.is_some() {
                return Err(fault(
                    "LRR-E-UNSUPPORTED",
                    "generated reference parameter Type authority requires its source producer",
                ));
            }
            return generated_result_namespace::require(db, signature, &parse);
        }
        let name = match written_name {
            Some(name) => name,
            None => signature
                .result
                .first()
                .map(|token| token.text.as_str())
                .ok_or_else(missing)?,
        };
        super::type_name_namespace::require_primitive_name(
            db,
            super::type_name_namespace::SourceTypeOwner {
                definition: signature.definition,
                scope: &signature.scope,
                header: &signature.header,
            },
            sources,
            stamps,
            name,
        )
    }
    fn mint_declared(
        callee: &SourceCallable,
        observation: &super::source_return_goals::ReturnGoalObservation,
        query_fingerprint: Digest,
        guard: Arc<PreparedInputGuard>,
        source: PrimitiveType,
    ) -> Result<Self, Diagnostic> {
        let signature = callee.signature();
        let wrapped_signature =
            QueryValue::FunctionSignature(Box::new(signature.clone())).fingerprint();
        if callee.ordinary_local_contract().is_none()
            || signature.result_symbolic.is_some()
            || signature.result.len() != 1
            || !observation.matches(callee.identity())
            || observation.signature_identity() != wrapped_signature
            || !matches!(observation.goal(), super::source_return_goals::ReturnTypeGoal::Ready(proof)
                if proof.require(callee.identity(), wrapped_signature).is_ok()
                && proof.source() == &TyTerm::Primitive(source))
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "declared return source proof differs from selected callable",
            ));
        }
        let certificate = Self {
            definition: signature.definition,
            signature: signature.fingerprint(),
            origin: DeferredReturnOrigin::DeclaredGoal {
                observation: query_fingerprint,
                guard,
            },
            // This origin uses its relevant retained dependency closure, not a
            // process-wide source or publication revision as a runtime guard.
            source_revision: 0,
            query_revision: 0,
            source,
            descriptor: primitive_runtime_id(source)?,
            written_hint: printed_primitive_hint(&signature.result[0].text),
            dependencies: Box::new([]),
        };
        Ok(certificate)
    }
    fn mint_lexical(db: &mut Database, callee: &SourceCallable) -> Result<Self, Diagnostic> {
        callee.revalidate(db)?;
        let proof = callee
            .lexical_contract()
            .ok_or_else(|| fault("LRR-E-CALL-CONTEXT", "lexical source certificate absent"))?;
        proof.revalidate(db)?;
        let signature = callee.signature();
        if signature.result_symbolic.is_some()
            || signature.result.len() != 1
            || !signature.generics.is_empty()
            || !signature.predicates.is_empty()
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "lexical deferred result declaration shape differs",
            ));
        }
        let source = proof.plan().primitive_result();
        let mut dependencies = Vec::new();
        let count = proof
            .namespace_dependencies()
            .len()
            .checked_add(8)
            .ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "lexical contract dependencies exceed budget",
                )
            })?;
        dependencies
            .try_reserve_exact(count)
            .map_err(|_| crate::runtime::arena::resource())?;
        for (key, fingerprint) in proof.namespace_dependencies() {
            dependencies.push(DependencyStamp::Input {
                key: key.clone(),
                fingerprint: *fingerprint,
            });
        }
        for identity in [signature.definition, proof.plan().caller().def_key] {
            for key in [
                InputKey::DefinitionSource(identity),
                InputKey::DefinitionHeader(identity),
                InputKey::DeclarationContext(identity),
                InputKey::DefinitionBody(identity),
            ] {
                let value = db.input(&key).ok_or_else(|| {
                    fault("LRR-E-CALL-CONTEXT", "lexical contract source input absent")
                })?;
                dependencies.push(DependencyStamp::Input {
                    key,
                    fingerprint: value.fingerprint(),
                });
            }
        }
        dependencies.sort_unstable();
        dependencies.dedup();
        let mut parameters = Vec::new();
        parameters
            .try_reserve_exact(proof.plan().primitive_parameters().len())
            .map_err(|_| crate::runtime::arena::resource())?;
        parameters.extend_from_slice(proof.plan().primitive_parameters());
        let certificate = Self {
            definition: signature.definition,
            signature: signature.fingerprint(),
            origin: DeferredReturnOrigin::Lexical {
                plan: proof.plan().identity(),
                parameters: parameters.into_boxed_slice(),
            },
            source_revision: db.source_revision(),
            query_revision: db.revision(),
            source,
            descriptor: primitive_runtime_id(source)?,
            written_hint: printed_primitive_hint(&signature.result[0].text),
            dependencies: dependencies.into_boxed_slice(),
        };
        Ok(certificate)
    }
    pub(crate) fn validate_packet(
        &self,
        signature: &crate::syntax::signatures::FunctionSignature,
        contracts: &PreparedCallContracts,
    ) -> Result<(), Diagnostic> {
        if signature.definition != self.definition
            || signature.fingerprint() != self.signature
            || signature.result_symbolic.is_some()
            || signature.result.len() != 1
            || self.written_hint != printed_primitive_hint(&signature.result[0].text)
            || contracts.definition != self.definition
            || contracts.source_result != Some(TyTerm::Primitive(self.source))
            || contracts.result_descriptor != Some(self.descriptor)
            || primitive_runtime_id(self.source)? != self.descriptor
            || self.dependencies.is_empty()
                && !matches!(self.origin, DeferredReturnOrigin::DeclaredGoal { .. })
            || self.dependencies.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "normalized deferred return certificate differs from its declaration/packet",
            ));
        }
        if matches!(self.origin, DeferredReturnOrigin::DeclaredGoal { .. })
            && (!self.dependencies.is_empty()
                || self.source_revision != 0
                || self.query_revision != 0)
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "declared return proof has a legacy dependency envelope",
            ));
        }
        if let DeferredReturnOrigin::Lexical { parameters, .. } = &self.origin {
            if parameters.len() != signature.parameters.len()
                || parameters.len() != contracts.source_parameters.len()
                || parameters.len() != contracts.parameter_descriptors.len()
                || !contracts.source_substitution.is_empty()
                || !contracts.runtime_substitution.is_empty()
                || !signature.generics.is_empty()
                || !signature.predicates.is_empty()
            {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "lexical primitive source contract arity or substitutions differ",
                ));
            }
            for (index, p) in parameters.iter().enumerate() {
                if contracts.source_parameters[index] != TyTerm::Primitive(*p)
                    || contracts.parameter_descriptors[index] != primitive_runtime_id(*p)?
                {
                    return Err(fault(
                        "LRR-E-CALL-CONTEXT",
                        "lexical parameter source contract differs",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn require_current(
        &self,
        db: &mut Database,
        callee: &SourceCallable,
    ) -> Result<(), Diagnostic> {
        callee.revalidate(db)?;
        if let DeferredReturnOrigin::DeclaredGoal { guard, .. } = &self.origin {
            if callee.identity() != self.definition
                || callee.signature().fingerprint() != self.signature
                || callee.ordinary_local_contract().is_none()
            {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "declared return selected owner differs",
                ));
            }
            return guard.require_current(db);
        }

        if let DeferredReturnOrigin::Lexical { plan, parameters } = &self.origin {
            let proof = callee
                .lexical_contract()
                .ok_or_else(|| fault("LRR-E-CALL-CONTEXT", "lexical return source owner absent"))?;
            proof.revalidate(db)?;
            if proof.plan().identity() != *plan
                || proof.plan().primitive_parameters() != parameters.as_ref()
                || proof.plan().primitive_result() != self.source
            {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "lexical return source owner differs",
                ));
            }
        }
        // Header equality alone cannot establish current alias meaning. Derived
        // publication also advances this epoch; conservatively demand a new mint.
        if db.source_revision() != self.source_revision
            || db.revision() != self.query_revision
            || callee.signature().fingerprint() != self.signature
            || callee.identity() != self.definition
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "normalized deferred return publication is stale",
            ));
        }
        for stamp in self.dependencies.iter() {
            let current = match stamp {
                DependencyStamp::Input { key, .. } => {
                    db.input(key).map(|value| value.fingerprint())
                }
                DependencyStamp::Query { key, .. } => {
                    match db.fetch(&SyntaxProvider, key.clone()) {
                        QueryPoll::Ready(Ok(value)) => Some(value.fingerprint),
                        QueryPoll::Ready(Err(e)) => return Err(e),
                        QueryPoll::NeedInput(_) => None,
                    }
                }
            };
            let expected = match stamp {
                DependencyStamp::Input { fingerprint, .. }
                | DependencyStamp::Query { fingerprint, .. } => *fingerprint,
            };
            if current != Some(expected) {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "normalized deferred return source dependency changed",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn for_written_hint(
        &self,
        hint: &crate::ir::op::HintRecipe,
    ) -> Result<Option<(PrimitiveType, TypeId)>, Diagnostic> {
        let Some(written) = self.written_hint else {
            return Ok(None);
        };
        if hint != &crate::ir::op::HintRecipe::Primitive(written) {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "root hint differs from certified written return syntax",
            ));
        }
        Ok(Some((self.source, self.descriptor)))
    }
    pub(crate) fn resolved_result(&self) -> (PrimitiveType, TypeId) {
        (self.source, self.descriptor)
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        let extra = match &self.origin {
            DeferredReturnOrigin::DeclaredGoal { guard, .. } => guard
                .metadata_bytes()
                .checked_add(2 * std::mem::size_of::<usize>())?,
            DeferredReturnOrigin::PrimitiveNamespace | DeferredReturnOrigin::Alias { .. } => 0,
            DeferredReturnOrigin::Lexical { parameters, .. } => parameters
                .len()
                .checked_mul(std::mem::size_of::<PrimitiveType>())?,
        };
        self.dependencies.iter().try_fold(
            std::mem::size_of::<Self>().checked_add(extra)?,
            |bytes, stamp| {
                bytes
                    .checked_add(std::mem::size_of::<DependencyStamp>())?
                    .checked_add(stamp.heap_bytes())
            },
        )
    }
}
impl StableEncode for NormalizedDeferredReturn {
    const DOMAIN: &'static str = "lrr/v5/normalized-deferred-return-certificate-v5";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.signature);
        match &self.origin {
            DeferredReturnOrigin::DeclaredGoal { observation, guard } => {
                e.u32(3);
                e.digest(*observation);
                e.digest(guard.fingerprint());
            }
            DeferredReturnOrigin::PrimitiveNamespace => e.u32(2),
            DeferredReturnOrigin::Alias { resolution, alias } => {
                e.u32(0);
                e.digest(*resolution);
                e.digest(*alias);
            }
            DeferredReturnOrigin::Lexical { plan, parameters } => {
                e.u32(1);
                e.digest(*plan);
                e.sequence(parameters, |e, p| e.u32(p.tag()));
            }
        }
        e.u64(self.source_revision);
        e.u64(self.query_revision);
        e.u32(self.source.tag());
        e.u32(self.descriptor.0);
        e.option(&self.written_hint, |e, p| e.u32(p.tag()));
        e.sequence(&self.dependencies, |e, s| e.record(s));
    }
}
/// Lexical thunk identity only; this never resolves a name or admits a source type.
fn printed_primitive_hint(text: &str) -> Option<PrimitiveType> {
    use PrimitiveType as P;
    Some(match text {
        "bool" => P::Bool,
        "char" => P::Char,
        "i8" => P::I8,
        "i16" => P::I16,
        "i32" => P::I32,
        "i64" => P::I64,
        "i128" => P::I128,
        "isize" => P::Isize,
        "u8" => P::U8,
        "u16" => P::U16,
        "u32" => P::U32,
        "u64" => P::U64,
        "u128" => P::U128,
        "usize" => P::Usize,
        "f32" => P::F32,
        "f64" => P::F64,
        _ => return None,
    })
}
pub enum ContractPreparation {
    Ready(PreparedCallContracts),
    NeedType {
        definition: Digest,
        syntax: Vec<TokenRecipe>,
    },
    NeedAlias(AliasTarget),
    NeedName(SignatureName),
}
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub(crate) fn primitive_runtime_id(p: PrimitiveType) -> Result<TypeId, Diagnostic> {
    crate::runtime::shape::PrimitiveIdentity::from_recipe(p).map(|identity| identity.id())
}
impl PreparedCallContracts {
    pub(crate) fn require_inference_current(&self, db: &mut Database) -> QueryPoll<()> {
        match &self.inferred_equations {
            Some(proof) => QueryPoll::Ready(proof.require_current(db)),
            None => QueryPoll::Ready(Ok(())),
        }
    }

    /// Validate the reached return source identity and bind an unbound result parameter.
    pub fn bind_return_parameter(&mut self, actual: &ArgumentType) -> Result<(), Diagnostic> {
        self.bind_return_parameter_in(actual, None)
    }
    /// As `bind_return_parameter`; with the store a declared `dyn Trait`
    /// position in the prepared result admits the returned concrete Type.
    pub fn bind_return_parameter_in(
        &mut self,
        actual: &ArgumentType,
        store: Option<&crate::runtime::cell::GuestStore>,
    ) -> Result<(), Diagnostic> {
        if actual.descriptor.0 == 0 || contains_transaction_term(&actual.source) {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "invalid returned source view or descriptor",
            ));
        }
        if actual.source.heap_bytes().is_none() {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "returned source view exceeds term budget",
            ));
        }
        if let TyTerm::Primitive(p) = actual.source
            && primitive_runtime_id(p)? != actual.descriptor
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "return source view and descriptor disagree",
            ));
        }
        if self.result_descriptor.is_some() {
            self.check_return_descriptor_in(actual.descriptor, store)?;
            let expected = match self.source_result.as_ref() {
                Some(TyTerm::Param(key)) => {
                    Some(self.source_substitution.get(key).ok_or_else(|| {
                        fault(
                            "LRR-E-CALL-CONTEXT",
                            "concrete generic return has no retained source substitution",
                        )
                    })?)
                }
                Some(TyTerm::Primitive(_)) => self.source_result.as_ref(),
                _ => None,
            };
            if expected.is_some_and(|expected| {
                expected != &actual.source && !expected.admits_dyn(&actual.source)
            }) {
                return Err(fault(
                    "LRR-E-GENERIC-BINDING",
                    "returned source identity differs from the retained result contract",
                ));
            }
            return Ok(());
        }
        let Some(TyTerm::Param(key)) = self.source_result else {
            return Err(fault(
                "LRR-E-TYPE-CONTEXT",
                "return requires further source type resolution",
            ));
        };
        if key.definition != self.definition {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "return parameter belongs to another definition",
            ));
        }
        bind(
            &mut self.runtime_substitution,
            &mut self.source_substitution,
            key,
            actual,
            None,
        )?;
        self.result_descriptor = Some(actual.descriptor);
        Ok(())
    }
    /// Only already-concrete return contracts; unresolved returns stay deferred.
    pub fn check_return_descriptor(&self, actual: TypeId) -> Result<(), Diagnostic> {
        self.check_return_descriptor_in(actual, None)
    }
    /// As `check_return_descriptor`; with the store, a declared `dyn Trait`
    /// position in the prepared result admits the returned concrete Type.
    pub fn check_return_descriptor_in(
        &self,
        actual: TypeId,
        store: Option<&crate::runtime::cell::GuestStore>,
    ) -> Result<(), Diagnostic> {
        let expected = self.result_descriptor.ok_or_else(|| {
            fault(
                "LRR-E-TYPE-CONTEXT",
                "reached return requires deferred source type resolution",
            )
        })?;
        if expected != actual && !store.is_some_and(|store| store.dyn_admits(expected, actual)) {
            return Err(fault(
                "LRR-E-CALL-RETURN-TYPE",
                "reached return descriptor differs from its prepared contract",
            ));
        }
        Ok(())
    }
}
/// Pre-argument source facts: no actual arguments, substitution, layout or bound solving.
#[derive(Clone)]
pub struct ArgumentHint {
    pub closed_reference: Option<std::sync::Arc<ClosedReferenceParameter>>,
    pub source: TyTerm,
    pub descriptor: Option<TypeId>,
    /// The declared parameter's written Type when it names nominal or
    /// parameterized structure (`Option<Test>`, `Result<T, u32>`, `(T, u8)`):
    /// its exact descriptor, or a partial context, once the parameters it
    /// names are fixed at the call.
    pub written: Option<Arc<crate::ir::written_type::WrittenType>>,
}
pub enum ArgumentHintPreparation {
    Ready(Vec<ArgumentHint>),
    NeedType {
        definition: Digest,
        syntax: Vec<TokenRecipe>,
    },
    NeedAlias(AliasTarget),
    NeedName(SignatureName),
}
fn push_template(
    templates: &mut Vec<TyTerm>,
    bytes: &mut usize,
    term: TyTerm,
) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(term.heap_bytes().ok_or_else(|| {
            fault(
                "LRR-E-METADATA-LIMIT",
                "parameter source term exceeds its budget",
            )
        })?)
        .and_then(|n| n.checked_add(std::mem::size_of::<TyTerm>()))
        .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "parameter template budget exceeded"))?;
    templates.try_reserve(1).map_err(|_| {
        fault(
            "LRR-E-METADATA-LIMIT",
            "parameter template allocation failed",
        )
    })?;
    templates.push(term);
    Ok(())
}
impl ArgumentHintPreparation {
    /// What an unready preparation still needs, for diagnostics.
    pub fn describe(&self) -> String {
        match self {
            Self::Ready(_) => "ready".into(),
            Self::NeedType { syntax, .. } => format!(
                "type `{}`",
                syntax.iter().map(|t| t.text.as_str()).collect::<String>()
            ),
            Self::NeedAlias(alias) => format!("alias {alias:?}"),
            Self::NeedName(name) => format!("name {name:?}"),
        }
    }
}
pub fn prepare_argument_hints(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
) -> QueryPoll<ArgumentHintPreparation> {
    match argument_hints(db, callee, graph, sources, None) {
        Ok(value) => value,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
/// Prepare only the next written argument's source context. Later parameter
/// syntax and aliases remain cold until their argument boundary is reached.
/// An excess argument has no parameter hint; arity still fails after evaluation.
pub fn prepare_argument_hint(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    index: usize,
) -> QueryPoll<ArgumentHintPreparation> {
    match argument_hints(db, callee, graph, sources, Some(index)) {
        Ok(value) => value,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
fn argument_hints(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    index: Option<usize>,
) -> Result<QueryPoll<ArgumentHintPreparation>, Diagnostic> {
    callee.revalidate(db)?;
    let signature = callee.signature();
    let mut templates = Vec::new();
    let mut template_bytes = 0usize;
    let mut reference_hints = BTreeMap::new();
    for (ordinal, parameter) in signature.parameters.iter().enumerate() {
        if index.is_some_and(|selected| selected != ordinal) {
            continue;
        }
        if parameter.ty.first().is_some_and(|token| token.text == "&")
            && let Some(proof) =
                ClosedReferenceParameter::capture(db, callee, graph, sources, ordinal)?
        {
            template_bytes = template_bytes
                .checked_add(proof.allocated_bytes()?)
                .and_then(|n| n.checked_add(128))
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "reference hint metadata budget"))?;
            reference_hints.insert(templates.len(), proof.clone());
            push_template(
                &mut templates,
                &mut template_bytes,
                proof.source_template().clone(),
            )?;
            continue;
        }
        match super::source_parameter_goals::prepare(
            db,
            graph,
            sources,
            signature.definition,
            ordinal,
        ) {
            QueryPoll::Ready(Ok(super::source_parameter_goals::ParameterTypeGoal::Ready(goal))) => {
                push_template(&mut templates, &mut template_bytes, goal.source().clone())?;
            }
            QueryPoll::Ready(Ok(super::source_parameter_goals::ParameterTypeGoal::NeedType {
                definition,
                syntax,
            })) => {
                return Ok(QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedType {
                    definition,
                    syntax,
                })));
            }
            QueryPoll::Ready(Ok(super::source_parameter_goals::ParameterTypeGoal::NeedAlias(
                other,
            ))) => {
                return Ok(QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedAlias(
                    other,
                ))));
            }
            QueryPoll::Ready(Ok(super::source_parameter_goals::ParameterTypeGoal::NeedName(
                other,
            ))) => {
                return Ok(QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedName(
                    other,
                ))));
            }
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        }
    }
    let mut hints = Vec::new();
    hints
        .try_reserve_exact(templates.len())
        .map_err(|_| fault("LRR-E-METADATA-LIMIT", "argument hint allocation failed"))?;
    for (ordinal, source) in templates.into_iter().enumerate() {
        let descriptor = match &source {
            TyTerm::Primitive(p) => Some(primitive_runtime_id(*p)?),
            _ => None,
        };
        let closed_reference = reference_hints.remove(&ordinal);
        // Only Types a primitive/structural descriptor cannot already name.
        // Hint preparation at an argument boundary only; the evaluated
        // contract compares source terms.
        let written = if index.is_some()
            && closed_reference.is_none()
            && !matches!(source, TyTerm::Primitive(_) | TyTerm::Param(_))
            && (super::written_terms::contains_nominal(&source)
                || contains_parameter(&source, signature.definition))
        {
            match super::written_terms::written_template(db, sources, &source, signature.definition)
            {
                QueryPoll::Ready(Ok(written)) => written.map(Arc::new),
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            }
        } else {
            None
        };
        hints.push(ArgumentHint {
            source,
            descriptor,
            closed_reference,
            written,
        });
    }
    Ok(QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))))
}

/// Inputs represent fully evaluated arguments. This function never moves/evaluates them.
pub fn prepare_contracts(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    explicit: &[(u32, ArgumentType)],
    result_hint: Option<&ArgumentType>,
    arguments: &[ArgumentType],
) -> QueryPoll<ContractPreparation> {
    match prepare(
        db,
        callee,
        graph,
        sources,
        explicit,
        result_hint,
        EvaluatedArguments::Ordinary(arguments, None),
    ) {
        Ok(value) => value,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
/// Ordinary arguments whose reached descriptors can bind generic parameters
/// structurally (`&T`, `(T, U)`, `W<T>`).
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_contracts_in_store(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    explicit: &[(u32, ArgumentType)],
    result_hint: Option<&ArgumentType>,
    arguments: &[ArgumentType],
    store: &crate::runtime::cell::GuestStore,
) -> QueryPoll<ContractPreparation> {
    match prepare(
        db,
        callee,
        graph,
        sources,
        explicit,
        result_hint,
        EvaluatedArguments::Ordinary(arguments, Some(store)),
    ) {
        Ok(value) => value,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
/// Bind each generic parameter in `template` to the runtime descriptor at
/// the same position of the reached argument descriptor.
/// Const generic array lengths (`[T; N]` of an impl's `const N: usize`)
/// the reached argument's source type binds; a parameter bound twice must
/// agree.
fn bind_const_lengths(
    template: &TyTerm,
    actual: &TyTerm,
    definition: Digest,
    lengths: &mut BTreeMap<ParamKey, u64>,
) -> Result<(), Diagnostic> {
    use crate::traits::term::{ConstTerm, GenericArg};
    match (template, actual) {
        (TyTerm::Array(element, length), TyTerm::Array(actual_element, actual_length)) => {
            if let (ConstTerm::Param(key), ConstTerm::Value(value)) = (length, actual_length)
                && key.definition == definition
                && lengths
                    .insert(*key, *value)
                    .is_some_and(|prior| prior != *value)
            {
                return Err(fault(
                    "LRR-E-GENERIC-BINDING",
                    "reached const generic lengths disagree",
                ));
            }
            bind_const_lengths(element, actual_element, definition, lengths)
        }
        (TyTerm::Ref(_, _, template), TyTerm::Ref(_, _, actual))
        | (TyTerm::Slice(template), TyTerm::Slice(actual)) => {
            bind_const_lengths(template, actual, definition, lengths)
        }
        (TyTerm::Tuple(templates), TyTerm::Tuple(actuals)) => {
            for (template, actual) in templates.iter().zip(actuals) {
                bind_const_lengths(template, actual, definition, lengths)?;
            }
            Ok(())
        }
        (TyTerm::Nominal(_, templates), TyTerm::Nominal(_, actuals)) => {
            for (template, actual) in templates.iter().zip(actuals) {
                if let (GenericArg::Type(template), GenericArg::Type(actual)) = (template, actual) {
                    bind_const_lengths(template, actual, definition, lengths)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
pub(crate) fn bind_runtime_parameters(
    template: &TyTerm,
    descriptor: TypeId,
    store: &crate::runtime::cell::GuestStore,
    definition: Digest,
    runtime: &mut BTreeMap<ParamKey, TypeId>,
) -> Result<(), Diagnostic> {
    use crate::runtime::shape::TypeKey;
    match template {
        TyTerm::Param(key) if key.definition == definition => {
            // A bare `dyn Trait` is a coercion target (`Box::new(8)` under a
            // `Box<dyn Foo>` result context), never a generic argument: the
            // parameter binds from its value.
            if matches!(
                store.type_descriptor(descriptor)?.key.as_ref(),
                crate::runtime::shape::TypeKey::Dyn { .. }
            ) {
                return Ok(());
            }
            if let Some(prior) = runtime.get(key).copied()
                && prior != descriptor
            {
                // A declared `dyn Trait` position and the concrete Type of a
                // value agree; the first binding stands.
                if store.dyn_admits(prior, descriptor) || store.dyn_admits(descriptor, prior) {
                    return Ok(());
                }
                return Err(fault(
                    "LRR-E-GENERIC-BINDING",
                    "reached generic runtime descriptors disagree",
                ));
            }
            runtime.insert(*key, descriptor);
        }
        TyTerm::Ref(_, _, target) => {
            if let TypeKey::Ref { target: actual, .. } =
                store.type_descriptor(descriptor)?.key.as_ref()
            {
                bind_runtime_parameters(target, *actual, store, definition, runtime)?;
            }
        }
        TyTerm::Tuple(fields) => {
            if let TypeKey::Tuple(actual) = store.type_descriptor(descriptor)?.key.as_ref() {
                for (field, actual) in fields.iter().zip(actual.iter()) {
                    bind_runtime_parameters(field, *actual, store, definition, runtime)?;
                }
            }
        }
        TyTerm::Array(element, _) => {
            if let TypeKey::Array {
                element: actual, ..
            } = store.type_descriptor(descriptor)?.key.as_ref()
            {
                bind_runtime_parameters(element, *actual, store, definition, runtime)?;
            }
        }
        TyTerm::Slice(element) => {
            if let TypeKey::Slice { element: actual } =
                store.type_descriptor(descriptor)?.key.as_ref()
            {
                bind_runtime_parameters(element, *actual, store, definition, runtime)?;
            }
        }
        TyTerm::Nominal(_, args) => {
            match store.type_descriptor(descriptor)?.key.as_ref() {
                TypeKey::Nominal { args: actual, .. } => {
                    let types = args.iter().filter_map(|arg| match arg {
                        crate::traits::term::GenericArg::Type(ty) => Some(ty),
                        _ => None,
                    });
                    for (ty, actual) in types.zip(actual.iter()) {
                        bind_runtime_parameters(ty, *actual, store, definition, runtime)?;
                    }
                }
                // `*const T` / `*mut T` (E-R5-008): the reserved-identity
                // nominal binds its pointee like a type argument.
                TypeKey::RawPtr { target: actual, .. } => {
                    if let Some((_, pointee)) =
                        crate::traits::pointer_terms::as_raw_pointer(template)
                    {
                        bind_runtime_parameters(pointee, *actual, store, definition, runtime)?;
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}
/// A template with a trait object position (`&dyn Trait`, `Box<dyn Trait>`).
pub(crate) fn contains_object(term: &TyTerm) -> bool {
    match term {
        TyTerm::Dyn(_) => true,
        TyTerm::Ref(_, _, target) | TyTerm::Array(target, _) | TyTerm::Slice(target) => {
            contains_object(target)
        }
        TyTerm::Tuple(fields) => fields.iter().any(contains_object),
        TyTerm::Nominal(_, args) => args.iter().any(
            |arg| matches!(arg, crate::traits::term::GenericArg::Type(ty) if contains_object(ty)),
        ),
        _ => false,
    }
}
/// Structural agreement where each `dyn` position accepts any concrete type
/// (the reached value keeps its own type; dispatch uses it).
fn object_compatible(template: &TyTerm, actual: &TyTerm) -> bool {
    match (template, actual) {
        (TyTerm::Dyn(_), _) => true,
        (TyTerm::Ref(_, m, t), TyTerm::Ref(_, n, a)) => m == n && object_compatible(t, a),
        (TyTerm::Tuple(t), TyTerm::Tuple(a)) => {
            t.len() == a.len() && t.iter().zip(a).all(|(t, a)| object_compatible(t, a))
        }
        (TyTerm::Nominal(d, t), TyTerm::Nominal(e, a)) => {
            d == e
                && t.len() == a.len()
                && t.iter().zip(a).all(|(t, a)| match (t, a) {
                    (
                        crate::traits::term::GenericArg::Type(t),
                        crate::traits::term::GenericArg::Type(a),
                    ) => object_compatible(t, a),
                    (t, a) => t == a,
                })
        }
        (t, a) => {
            super::signature_subst::erase_regions(t) == super::signature_subst::erase_regions(a)
        }
    }
}
fn contains_parameter(term: &TyTerm, definition: Digest) -> bool {
    match term {
        TyTerm::Param(key) => key.definition == definition,
        TyTerm::Array(_, crate::traits::term::ConstTerm::Param(key))
            if key.definition == definition =>
        {
            true
        }
        TyTerm::Ref(_, _, target) | TyTerm::Array(target, _) | TyTerm::Slice(target) => {
            contains_parameter(target, definition)
        }
        TyTerm::Tuple(fields) => fields.iter().any(|field| contains_parameter(field, definition)),
        TyTerm::Nominal(_, args) => args.iter().any(|arg| {
            matches!(arg, crate::traits::term::GenericArg::Type(ty) if contains_parameter(ty, definition))
        }),
        _ => false,
    }
}
pub(crate) fn prepare_contracts_with_references(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    explicit: &[(u32, ArgumentType)],
    result_hint: Option<&ArgumentType>,
    arguments: ReferenceCallArguments<'_>,
) -> QueryPoll<ContractPreparation> {
    match prepare(
        db,
        callee,
        graph,
        sources,
        explicit,
        result_hint,
        EvaluatedArguments::References(arguments),
    ) {
        Ok(poll) => poll,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
enum EvaluatedArguments<'a> {
    Ordinary(
        &'a [ArgumentType],
        Option<&'a crate::runtime::cell::GuestStore>,
    ),
    References(ReferenceCallArguments<'a>),
}
/// Common cheap validation of fully evaluated call inputs. This performs no
/// source/body preparation and mints no revision-bound return/GenericEnv proof.
/// The reached local-body coordinator uses it before its derived publication;
/// all public contract adapters retain the same check at their own boundary.
pub(crate) fn preflight_evaluated_call(
    callee: &SourceCallable,
    explicit: &[(u32, ArgumentType)],
    result_hint: Option<&ArgumentType>,
    arguments: &[ArgumentType],
) -> Result<(), Diagnostic> {
    let signature = callee.signature();
    if arguments.len() != signature.parameters.len() {
        return Err(fault(
            "LRR-E-CALL-ARITY",
            "evaluated argument count differs from the selected signature",
        ));
    }
    let mut view_bytes = 0usize;
    for actual in arguments
        .iter()
        .chain(explicit.iter().map(|(_, actual)| actual))
        .chain(result_hint)
    {
        view_bytes = view_bytes
            .checked_add(actual.source.heap_bytes().ok_or_else(|| {
                fault(
                    "LRR-E-METADATA-LIMIT",
                    "argument source view exceeds term budgets",
                )
            })?)
            .and_then(|n| n.checked_add(std::mem::size_of::<TyTerm>()))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "call source-view budget exceeded"))?;
        if contains_transaction_term(&actual.source) {
            return Err(fault(
                "LRR-E-TYPE-CONTEXT",
                "call source views cannot contain unresolved local inference variables",
            ));
        }
        if actual.descriptor.0 == 0 {
            return Err(fault(
                "LRR-E-TYPE-CONTEXT",
                "call argument has no concrete runtime descriptor",
            ));
        }
        if let TyTerm::Primitive(p) = actual.source
            && primitive_runtime_id(p)? != actual.descriptor
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "primitive argument source view and runtime descriptor disagree",
            ));
        }
    }
    Ok(())
}
fn prepare(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    explicit: &[(u32, ArgumentType)],
    result_hint: Option<&ArgumentType>,
    evaluated: EvaluatedArguments<'_>,
) -> Result<QueryPoll<ContractPreparation>, Diagnostic> {
    let arguments = match &evaluated {
        EvaluatedArguments::Ordinary(arguments, _) => *arguments,
        EvaluatedArguments::References(reference) => reference.arguments,
    };
    callee.revalidate(db)?;
    preflight_evaluated_call(callee, explicit, result_hint, arguments)?;
    let signature = callee.signature();
    let definition = signature.definition;
    let mut hints = match prepare_argument_hints(db, callee, graph, sources) {
        QueryPoll::Ready(Ok(ArgumentHintPreparation::Ready(hints))) => hints,
        QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedType { definition, syntax })) => {
            return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedType {
                definition,
                syntax,
            })));
        }
        QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedAlias(alias))) => {
            return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedAlias(alias))));
        }
        QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedName(name))) => {
            return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedName(name))));
        }
        QueryPoll::Ready(Err(error)) => return Err(error),
        QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
    };
    let closed_references = match &evaluated {
        EvaluatedArguments::References(reference) => closed_reference_calls::bind_references(
            db,
            callee,
            (graph, sources),
            &mut hints,
            reference.store,
            arguments,
            reference.references,
        )?,
        EvaluatedArguments::Ordinary(..) => {
            if let Some(ordinal) = hints
                .iter()
                .position(|hint| hint.closed_reference.is_some())
            {
                return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedType {
                    definition,
                    syntax: signature.parameters[ordinal].ty.clone(),
                })));
            }
            None
        }
    };
    let templates: Vec<_> = hints.into_iter().map(|hint| hint.source).collect();
    // Transaction identity is local metadata and never serialized as VM data.
    let transaction = Digest::of_bytes(&[definition.0.as_slice(), b"call-contract"].concat());
    let mut instantiated = signature.clone();
    let mut normalized_deferred_return = None;
    if let Some(source) = closed_references
        .as_ref()
        .and_then(|proof| proof.result_source())
    {
        instantiated.result_symbolic = Some(source.clone());
    }
    for (parameter, term) in instantiated.parameters.iter_mut().zip(&templates) {
        parameter.symbolic = Some(term.clone());
    }
    // Written aliases become source contracts only after immutable owning
    // declaration lookup and bounded normalization. Primitive spelling alone
    // keeps its existing deferred named-result path.
    if instantiated.result_symbolic.is_none() && callee.ordinary_local_contract().is_some() {
        let key = QueryKey::PreparedReturnPlan(definition);
        let selected = match db.fetch(&SyntaxProvider, key.clone()) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
        };
        let QueryValue::PreparedReturnPlan(plan) = selected.value.as_ref() else {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "declared return query variant differs",
            ));
        };
        let observation = plan.observation();
        if !selected.cacheable || !observation.matches(definition) {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "declared return query has no retained source proof",
            ));
        }
        match observation.goal() {
            // A written structural or nominal result (`Opt<u8>`, `(T, P)`):
            // closed now, or once its parameters bind, as for a module
            // declaration's written result.
            super::source_return_goals::ReturnTypeGoal::Ready(proof)
                if !matches!(proof.source(), TyTerm::Primitive(_))
                    && (closed_term(proof.source())
                        || contains_parameter(proof.source(), definition)
                        || contains_object(proof.source())) =>
            {
                plan.guard().require_current(db)?;
                instantiated.result_symbolic = Some(proof.source().clone());
            }
            super::source_return_goals::ReturnTypeGoal::Ready(proof) => {
                let TyTerm::Primitive(source) = proof.source() else {
                    return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedType {
                        definition,
                        syntax: signature.result.clone(),
                    })));
                };
                plan.guard().require_current(db)?;
                let certificate = NormalizedDeferredReturn::mint_declared(
                    callee,
                    observation,
                    plan.observation_fingerprint(),
                    Arc::clone(plan.guard()),
                    *source,
                )?;
                instantiated.result_symbolic = Some(TyTerm::Primitive(*source));
                normalized_deferred_return = Some(Box::new(certificate));
            }
            super::source_return_goals::ReturnTypeGoal::NeedAlias(alias) => {
                return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedAlias(
                    alias.clone(),
                ))));
            }
            super::source_return_goals::ReturnTypeGoal::NeedName(name) => {
                return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedName(
                    name.clone(),
                ))));
            }
        }
    } else if instantiated.result_symbolic.is_none() && callee.lexical_contract().is_some() {
        let proof = NormalizedDeferredReturn::mint_lexical(db, callee)?;
        instantiated.result_symbolic = Some(TyTerm::Primitive(proof.source));
        normalized_deferred_return = Some(Box::new(proof));
    } else if instantiated.result_symbolic.is_none()
        && (signature.result.len() > 1
            || signature
                .result
                .first()
                .is_some_and(|token| token.text == "Self"))
    {
        // A written composite result (tuple/array of resolved names).
        match super::source_types::resolve_written_type(
            db,
            graph,
            sources,
            signature,
            &signature.result,
        ) {
            // Closed now, or closed once its generic parameters bind.
            QueryPoll::Ready(Ok(Some(term)))
                if closed_term(&term)
                    || contains_parameter(&term, definition)
                    || contains_object(&term) =>
            {
                instantiated.result_symbolic = Some(term);
            }
            QueryPoll::Ready(Ok(_)) => {}
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
        }
    } else if instantiated.result_symbolic.is_none() && signature.result.len() == 1 {
        let resolved = match super::source_types::resolve_named(
            db,
            graph,
            sources,
            signature,
            &signature.result[0].text,
        ) {
            QueryPoll::Ready(Ok(selected)) => selected,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(request) => return Ok(QueryPoll::NeedInput(request)),
        };
        if let SignatureName::Primitive(primitive) = resolved {
            normalized_deferred_return = Some(Box::new(NormalizedDeferredReturn::mint_primitive(
                db, callee, graph, sources, primitive,
            )?));
            instantiated.result_symbolic = Some(TyTerm::Primitive(primitive));
        } else if let SignatureName::Source(found) = resolved {
            let normalizer = AliasNormalizer::new(graph, sources)?;
            match normalizer.normalize_with_dependencies(db, found.identity) {
                QueryPoll::Ready(Ok((AliasTarget::Type(TyTerm::Primitive(primitive)), trace))) => {
                    normalized_deferred_return = Some(Box::new(NormalizedDeferredReturn::mint(
                        db, callee, &found, primitive, trace,
                    )?));
                    instantiated.result_symbolic = Some(TyTerm::Primitive(primitive));
                }
                QueryPoll::Ready(Ok((AliasTarget::Nominal(nominal), _))) => {
                    instantiated.result_symbolic = Some(TyTerm::Nominal(nominal, Vec::new()));
                }
                // A compound alias target (`type Result = core::result::
                // Result<(), Error>`), walked in the alias's own context.
                QueryPoll::Ready(Ok((
                    alias @ AliasTarget::Deferred {
                        definition: alias_definition,
                        ..
                    },
                    _,
                ))) => match super::source_types::deferred_alias_type(
                    db,
                    graph,
                    sources,
                    alias_definition,
                    &[],
                ) {
                    QueryPoll::Ready(Ok(Some(term))) => instantiated.result_symbolic = Some(term),
                    QueryPoll::Ready(Ok(None)) => {
                        return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedAlias(alias))));
                    }
                    QueryPoll::Ready(Err(error)) => return Err(error),
                    QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
                },
                QueryPoll::Ready(Ok((alias, _))) => {
                    return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedAlias(alias))));
                }
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            }
        }
    }
    let mut symbolic = SignatureSubstitution::new(&instantiated, transaction)?;
    let mut runtime = BTreeMap::new();
    let mut source_bindings = BTreeMap::new();
    let mut explicit_seen = std::collections::BTreeSet::new();
    for (ordinal, actual) in explicit {
        if !explicit_seen.insert(*ordinal) {
            return Err(fault(
                "LRR-E-GENERIC-BINDING",
                "duplicate explicit type argument ordinal",
            ));
        }
        symbolic.bind_type_parameter(*ordinal, &actual.source)?;
        bind(
            &mut runtime,
            &mut source_bindings,
            ParamKey {
                definition,
                ordinal: *ordinal,
            },
            actual,
            None,
        )?;
    }
    if let (Some(TyTerm::Param(key)), Some(hint)) = (&instantiated.result_symbolic, result_hint) {
        symbolic.bind_type_parameter(key.ordinal, &hint.source)?;
        bind(&mut runtime, &mut source_bindings, *key, hint, None)?;
    }
    let store = match &evaluated {
        EvaluatedArguments::Ordinary(_, store) => *store,
        EvaluatedArguments::References(reference) => Some(reference.store),
    };
    let mut structural_generic = false;
    // Const generic lengths (`[T; N]`) the reached arrays bind.
    let mut const_lengths = BTreeMap::new();
    // Callee views of trait-object parameters: the reached concrete types.
    let mut object_actuals = BTreeMap::new();
    let mut descriptors = Vec::new();
    for (index, (template, actual)) in templates.iter().zip(arguments).enumerate() {
        if actual.source.heap_bytes().is_none() {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "argument source view exceeds term budgets",
            ));
        }
        match template {
            TyTerm::Param(key) if key.definition == definition => {
                symbolic.constrain_argument(index, &actual.source)?;
                bind(&mut runtime, &mut source_bindings, *key, actual, store)?;
                descriptors.push(actual.descriptor);
            }
            TyTerm::Primitive(p) => {
                let expected = primitive_runtime_id(*p)?;
                if expected != actual.descriptor {
                    if std::env::var_os("LRR_DBG_ARG").is_some() {
                        eprintln!(
                            "DBG arg mismatch template={:?} expected={:?} actual={:?} src={:?} def={:?}",
                            template, expected, actual.descriptor, actual.source, definition
                        );
                    }
                    return Err(fault(
                        "LRR-E-CALL-ARG-TYPE",
                        "argument descriptor differs from the reached concrete signature",
                    ));
                }
                // A matching runtime descriptor is not equality of source
                // types: a caller's rigid parameter remains rigid here.
                symbolic.constrain_argument(index, &actual.source)?;
                descriptors.push(expected);
            }
            TyTerm::Ref(_, _, _)
                if closed_references
                    .as_ref()
                    .is_some_and(|proof| proof.admits(index)) =>
            {
                symbolic.constrain_argument(index, &actual.source)?;
                descriptors.push(actual.descriptor);
            }
            // An associated-type position (`T::V`) is the reached argument's
            // own type (normalized at the hint when its generics were bound);
            // the callee views the concrete type, as for trait objects.
            template if super::associated_types::contains_projection(template) => {
                structural_generic = true;
                object_actuals.insert(index, actual.source.clone());
                descriptors.push(actual.descriptor);
            }
            // A closed nominal/tuple parameter accepts exactly the same
            // reached source type; its runtime descriptor is the argument's.
            // Generic structure (`&T`, `(T, U)`, `W<T>`): unify the source
            // shapes and bind each parameter's descriptor from the argument.
            template
                if store.is_some()
                    && contains_parameter(template, definition)
                    && closed_references
                        .as_ref()
                        .is_none_or(|proof| !proof.admits(index)) =>
            {
                let store = store.expect("checked store");
                // `&[T; N]` at a generic `&[U]` coercion site unsizes first:
                // the parameter binds from the coerced `&[T]` view.
                let coerced = unsizing_view(template, &actual.source)
                    .map(|source| {
                        unsized_reference_descriptor(store, actual.descriptor, template)
                            .map(|descriptor| (source, descriptor))
                    })
                    .transpose()?
                    // `&T` at a generic `*const U` parameter (E-R5-008).
                    .or_else(|| pointer_coercion_view(store, template, actual));
                let (source, descriptor) = match &coerced {
                    Some((source, descriptor)) => (source, *descriptor),
                    None => (&actual.source, actual.descriptor),
                };
                symbolic.constrain_argument_erased(index, source)?;
                bind_runtime_parameters(template, descriptor, store, definition, &mut runtime)?;
                bind_const_lengths(template, source, definition, &mut const_lengths)?;
                structural_generic = true;
                descriptors.push(descriptor);
            }
            // A trait-object position accepts the reached concrete value.
            template if contains_object(template) && !contains_parameter(template, definition) => {
                if !object_compatible(template, &actual.source) {
                    return Err(fault(
                        "LRR-E-CALL-ARG-TYPE",
                        "argument source type differs from the trait-object parameter type",
                    ));
                }
                structural_generic = true;
                object_actuals.insert(index, actual.source.clone());
                descriptors.push(actual.descriptor);
            }
            // A closed reference/slice parameter without a location
            // certificate (`&str`, `&[u8]`): the same type up to regions.
            TyTerm::Ref(..) | TyTerm::Slice(..)
                if closed_term(template)
                    && closed_references
                        .as_ref()
                        .is_none_or(|proof| !proof.admits(index)) =>
            {
                if super::signature_subst::erase_regions(template)
                    == super::signature_subst::erase_regions(&actual.source)
                    || crate::traits::pointer_terms::same_function_pointer_reference(
                        template,
                        &actual.source,
                    )
                    || store.is_some_and(|store| function_item_reference(store, template, actual))
                {
                    structural_generic = true;
                    descriptors.push(actual.descriptor);
                } else if unsizes_to(&actual.source, template) {
                    // A call argument is a coercion site: the sealed packet
                    // applies the shared runtime unsizing to this descriptor.
                    let store = store.ok_or_else(|| {
                        fault(
                            "LRR-E-CALL-ARG-TYPE",
                            "unsizing argument coercion requires its reached store",
                        )
                    })?;
                    descriptors.push(unsized_reference_descriptor(
                        store,
                        actual.descriptor,
                        template,
                    )?);
                    structural_generic = true;
                } else {
                    return Err(fault(
                        "LRR-E-CALL-ARG-TYPE",
                        "argument source type differs from the reference parameter type",
                    ));
                }
            }
            // A fn-pointer parameter (E-R5-008): a function item or fn pointer
            // argument. The sealed packet applies the shared pointer
            // coercion to the erased fn-pointer descriptor.
            TyTerm::Function(_) => {
                let Some(store) = store else {
                    return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedType {
                        definition,
                        syntax: signature.parameters[index].ty.clone(),
                    })));
                };
                let descriptor = store
                    .find_type(&crate::runtime::shape::TypeKey::erased_function_pointer())
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-CALL-ARG-TYPE",
                            "fn pointer parameter requires an evaluated function item or pointer",
                        )
                    })?;
                descriptors.push(descriptor);
            }
            TyTerm::Nominal(..) | TyTerm::Tuple(..) | TyTerm::Array(..)
                if closed_term(template) =>
            {
                if &actual.source != template
                    && let Some(store) = store
                    && let Some((view, descriptor)) = pointer_coercion_view(store, template, actual)
                    && super::signature_subst::erase_regions(&view)
                        == super::signature_subst::erase_regions(template)
                {
                    // `&x` at a `*const T` parameter (E-R5-008).
                    symbolic.constrain_argument(index, &view)?;
                    descriptors.push(descriptor);
                    continue;
                }
                if &actual.source != template {
                    return Err(fault(
                        "LRR-E-CALL-ARG-TYPE",
                        "argument source type differs from the closed parameter type",
                    ));
                }
                // A spelled source term grants no by-value admission: the
                // reached descriptor itself must be that closed Type.
                let Some(store) = store else {
                    return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedType {
                        definition,
                        syntax: signature.parameters[index].ty.clone(),
                    })));
                };
                // (A descriptor recording no closed source term, such as a
                // const-generic nominal, is left to the source comparison.)
                if store
                    .descriptor_source_term(actual.descriptor)
                    .is_some_and(|term| {
                        super::signature_subst::erase_regions(&term)
                            != super::signature_subst::erase_regions(template)
                    })
                {
                    return Err(fault(
                        "LRR-E-CALL-ARG-TYPE",
                        "argument descriptor differs from the closed parameter type",
                    ));
                }
                symbolic.constrain_argument(index, &actual.source)?;
                descriptors.push(actual.descriptor);
            }
            _ => {
                return Ok(QueryPoll::Ready(Ok(ContractPreparation::NeedType {
                    definition,
                    syntax: signature.parameters[index].ty.clone(),
                })));
            }
        }
    }
    if let (Some(TyTerm::Primitive(primitive)), Some(hint)) =
        (&instantiated.result_symbolic, result_hint)
        && primitive_runtime_id(*primitive)? != hint.descriptor
    {
        return Err(fault(
            "LRR-E-CALL-RETURN-TYPE",
            "concrete call return descriptor differs from the reached result context",
        ));
    }
    if let (Some(TyTerm::Primitive(_)), Some(hint)) = (&instantiated.result_symbolic, result_hint) {
        symbolic.constrain_result(&hint.source)?;
    }
    // Reached argument Types may determine a still-unbound parameter through
    // an actual associated equality, e.g. K::Value = Option<V>. Consume only
    // those declaration equations; this does not validate unrelated bounds.
    let inferred_equations = if let (Some(store), Some(body)) = (store, callee.body()) {
        let proof = match super::associated_equations::constrain(
            db,
            &super::associated_equations::EquationOwner::DeclaredBody(body.key()),
            signature,
            &mut symbolic,
        ) {
            QueryPoll::Ready(Ok(inference)) => inference.guard,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
        };
        if proof.is_some() {
            for parameter in &signature.generics {
                if parameter.kind != crate::syntax::signatures::GenericKind::Type {
                    continue;
                }
                let key = ParamKey {
                    definition,
                    ordinal: parameter.ordinal,
                };
                if runtime.contains_key(&key) {
                    continue;
                }
                let Some(bound) = symbolic
                    .binding(parameter.ordinal)
                    .filter(|term| closed_term(term))
                else {
                    continue;
                };
                let Some(descriptor) = store.find_structural_term(bound) else {
                    continue;
                };
                source_bindings.insert(key, bound.clone());
                runtime.insert(key, descriptor);
                structural_generic = true;
            }
        }
        proof
    } else {
        None
    };
    // Structurally bound parameters: publish their source identities beside
    // their descriptors, and give the callee closed parameter views.
    let templates = if structural_generic {
        let mut substitution = crate::traits::subst::Substitution::default();
        for (key, descriptor) in &runtime {
            let Some(bound) = symbolic.binding(key.ordinal) else {
                continue;
            };
            if contains_transaction_term(bound) {
                continue;
            }
            source_bindings.entry(*key).or_insert_with(|| bound.clone());
            let _ = descriptor;
            substitution.insert(*key, crate::traits::term::GenericArg::Type(bound.clone()))?;
        }
        for (key, length) in &const_lengths {
            substitution.insert(
                *key,
                crate::traits::term::GenericArg::Const(crate::traits::term::ConstTerm::Value(
                    *length,
                )),
            )?;
        }
        if let Some(result) = &instantiated.result_symbolic
            && !matches!(result, TyTerm::Param(_))
            && contains_parameter(result, definition)
        {
            let applied = substitution.apply(result)?;
            if closed_term(&applied) {
                instantiated.result_symbolic = Some(applied);
            }
        }
        templates
            .iter()
            .enumerate()
            .map(|(index, template)| match object_actuals.remove(&index) {
                Some(actual) => Ok(actual),
                None => substitution.apply(template),
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        templates
    };
    // A declared result naming generics (`-> Self` of `impl<T> W<T>`,
    // `-> Option<T>`): the parameters the arguments left unbound bind from
    // the reached result context (`let v: Vec<i32> = Vec::new()`), and the
    // bindings close the result.
    // A raw-pointer result (`-> *const T`, E-R5-008) is lowered to a term at
    // signature capture and closes the same way.
    if (signature.result_symbolic.is_none()
        || signature
            .result_symbolic
            .as_ref()
            .is_some_and(|result| crate::traits::pointer_terms::as_raw_pointer(result).is_some()))
        && let Some(result) = instantiated.result_symbolic.clone()
        && !matches!(result, TyTerm::Param(_))
        && contains_parameter(&result, definition)
    {
        if let (Some(hint), Some(store)) = (result_hint, store)
            && closed_term(&hint.source)
            && symbolic.constrain_result(&hint.source).is_ok()
        {
            let mut reached = runtime.clone();
            bind_runtime_parameters(&result, hint.descriptor, store, definition, &mut reached)?;
            for (key, descriptor) in reached {
                if runtime.contains_key(&key) {
                    continue;
                }
                let Some(bound) = symbolic.binding(key.ordinal) else {
                    continue;
                };
                if contains_transaction_term(bound) || !closed_term(bound) {
                    continue;
                }
                // A result context's `dyn Trait` is a coercion target, not the
                // generic argument (`Box::new(8)` under `Box<dyn Foo>` binds
                // `T` from its argument, not from the object type).
                if matches!(bound, TyTerm::Dyn(_)) {
                    continue;
                }
                source_bindings.insert(key, bound.clone());
                runtime.insert(key, descriptor);
            }
        }
        let mut substitution = crate::traits::subst::Substitution::default();
        for (key, bound) in &source_bindings {
            if key.definition == definition && !contains_transaction_term(bound) {
                substitution.insert(*key, crate::traits::term::GenericArg::Type(bound.clone()))?;
            }
        }
        let applied = substitution.apply(&result)?;
        if closed_term(&applied) {
            instantiated.result_symbolic = Some(applied);
        }
    }
    let result_descriptor = match instantiated.result_symbolic.as_ref() {
        Some(TyTerm::Param(key)) => runtime.get(key).copied(),
        Some(TyTerm::Primitive(p)) => Some(primitive_runtime_id(*p)?),
        _ => closed_references
            .as_ref()
            .and_then(|proof| proof.result_descriptor()),
    };
    Ok(QueryPoll::Ready(Ok(ContractPreparation::Ready(
        PreparedCallContracts {
            closed_references,
            inferred_equations,
            definition,
            source_parameters: templates,
            parameter_descriptors: descriptors,
            source_substitution: source_bindings,
            runtime_substitution: runtime,
            source_result: instantiated.result_symbolic.clone(),
            result_descriptor,
            normalized_deferred_return,
            structural_references: structural_generic,
        },
    ))))
}
fn bind(
    runtime: &mut BTreeMap<ParamKey, TypeId>,
    source: &mut BTreeMap<ParamKey, TyTerm>,
    key: ParamKey,
    actual: &ArgumentType,
    store: Option<&crate::runtime::cell::GuestStore>,
) -> Result<(), Diagnostic> {
    // A parameter already bound to a Type with a declared `dyn Trait` position
    // (`Vec<Box<dyn T>>`'s `T`) admits the concrete Type of a later value; the
    // first binding stands.
    if let (Some(prior), Some(store)) = (runtime.get(&key).copied(), store)
        && prior != actual.descriptor
        && store.dyn_admits(prior, actual.descriptor)
    {
        return Ok(());
    }
    if runtime
        .get(&key)
        .is_some_and(|prior| *prior != actual.descriptor)
    {
        return Err(fault(
            "LRR-E-GENERIC-BINDING",
            "reached generic runtime descriptors disagree",
        ));
    }
    if source.get(&key).is_some_and(|prior| {
        super::signature_subst::erase_regions(prior)
            != super::signature_subst::erase_regions(&actual.source)
    }) {
        return Err(fault(
            "LRR-E-GENERIC-BINDING",
            "reached generic source identities disagree",
        ));
    }
    runtime.insert(key, actual.descriptor);
    source.entry(key).or_insert_with(|| actual.source.clone());
    Ok(())
}

/// rustc's built-in reference unsizing (`CoerceUnsized` for `&`/`&mut`) at a
/// coercion site: `&[T; N]` / `&mut [T; N]` reach `&[T]`, and `&mut [T; N]`
/// reaches `&mut [T]`. Regions are not compared (borrow checking is
/// `cargo check`'s); the element Types must be identical.
pub(crate) fn unsizes_to(actual: &TyTerm, expected: &TyTerm) -> bool {
    let (TyTerm::Ref(_, from_mutable, from), TyTerm::Ref(_, to_mutable, to)) = (actual, expected)
    else {
        return false;
    };
    let (TyTerm::Array(from_element, _), TyTerm::Slice(to_element)) = (from.as_ref(), to.as_ref())
    else {
        return false;
    };
    (*from_mutable || !*to_mutable)
        && super::signature_subst::erase_regions(from_element)
            == super::signature_subst::erase_regions(to_element)
}
/// A reference to a function item (or to a fn pointer) at a reference to
/// fn-pointer parameter: the operator impls for `fn()` compare code
/// addresses, and a function item has one (E-R5-008).
fn function_item_reference(
    store: &crate::runtime::cell::GuestStore,
    template: &TyTerm,
    actual: &ArgumentType,
) -> bool {
    use crate::runtime::shape::TypeKey;
    let TyTerm::Ref(_, _, pointee) = template else {
        return false;
    };
    if !matches!(pointee.as_ref(), TyTerm::Function(_)) {
        return false;
    }
    let Ok(descriptor) = store.type_descriptor(actual.descriptor) else {
        return false;
    };
    let TypeKey::Ref { target, .. } = descriptor.key.as_ref() else {
        return false;
    };
    store.type_descriptor(*target).is_ok_and(|target| {
        matches!(
            target.key.as_ref(),
            TypeKey::Callable { captures, .. } if captures.is_empty()
        )
    })
}
/// The raw-pointer view and descriptor a reference or a pointer of the other
/// mutability takes at a raw-pointer parameter `template` (E-R5-008); `None`
/// when the argument already is one, or no pointer coercion applies.
fn pointer_coercion_view(
    store: &crate::runtime::cell::GuestStore,
    template: &TyTerm,
    actual: &ArgumentType,
) -> Option<(TyTerm, TypeId)> {
    use crate::{runtime::shape::TypeKey, traits::pointer_terms};
    let (to_mutable, _) = pointer_terms::as_raw_pointer(template)?;
    let (referent, from_mutable) = match store.type_descriptor(actual.descriptor).ok()?.key.as_ref()
    {
        TypeKey::Ref { target, .. } => (*target, None),
        TypeKey::RawPtr { target, mutable } => (*target, Some(*mutable)),
        _ => return None,
    };
    if from_mutable == Some(to_mutable) {
        return None;
    }
    let pointee = match &actual.source {
        TyTerm::Ref(_, _, inner) => inner.as_ref().clone(),
        source => pointer_terms::as_raw_pointer(source)?.1.clone(),
    };
    let descriptor = store.find_type(&TypeKey::RawPtr {
        mutable: to_mutable,
        target: referent,
    })?;
    Some((pointer_terms::raw_pointer(to_mutable, pointee), descriptor))
}
/// The `&[T]`/`&mut [T]` source view an array reference takes at a slice
/// reference coercion site whose element may still be generic (`&[U]`).
pub(crate) fn unsizing_view(template: &TyTerm, actual: &TyTerm) -> Option<TyTerm> {
    let (TyTerm::Ref(_, to_mutable, to), TyTerm::Ref(region, from_mutable, from)) =
        (template, actual)
    else {
        return None;
    };
    let (TyTerm::Slice(_), TyTerm::Array(element, _)) = (to.as_ref(), from.as_ref()) else {
        return None;
    };
    (*from_mutable || !*to_mutable).then(|| {
        TyTerm::Ref(
            region.clone(),
            *to_mutable,
            Box::new(TyTerm::Slice(element.clone())),
        )
    })
}
/// The reached descriptor of the `&[T]`/`&mut [T]` an array reference unsizes
/// to. Every interned array Type carries its slice reference views.
pub(crate) fn unsized_reference_descriptor(
    store: &crate::runtime::cell::GuestStore,
    actual: TypeId,
    expected: &TyTerm,
) -> Result<TypeId, Diagnostic> {
    use crate::runtime::shape::TypeKey;
    let TyTerm::Ref(_, mutable, _) = expected else {
        return Err(fault(
            "LRR-E-CALL-ARG-TYPE",
            "unsizing coercion requires a reference target",
        ));
    };
    let element = match store.type_descriptor(actual)?.key.as_ref() {
        TypeKey::Ref { target, .. } => match store.type_descriptor(*target)?.key.as_ref() {
            TypeKey::Array { element, .. } => Some(*element),
            _ => None,
        },
        _ => None,
    };
    element
        .and_then(|element| store.find_type(&TypeKey::Slice { element }))
        .and_then(|slice| {
            store.find_type(&TypeKey::Ref {
                mutable: *mutable,
                target: slice,
            })
        })
        .ok_or_else(|| {
            fault(
                "LRR-E-CALL-ARG-TYPE",
                "unsizing argument descriptor differs from its array reference",
            )
        })
}
/// A source type with no generic parameters, inference or projection: its
/// identity alone determines the runtime value family.
pub(crate) fn closed_term(term: &TyTerm) -> bool {
    match term {
        TyTerm::Primitive(_) | TyTerm::Str | TyTerm::Never | TyTerm::Dyn(_) => true,
        TyTerm::Ref(region, _, target) => {
            !matches!(region, crate::traits::term::RegionTerm::Placeholder(_))
                && closed_term(target)
        }
        TyTerm::Nominal(_, args) => args.iter().all(|arg| match arg {
            crate::traits::term::GenericArg::Type(ty) => closed_term(ty),
            _ => false,
        }),
        TyTerm::Tuple(fields) => fields.iter().all(closed_term),
        TyTerm::Array(element, crate::traits::term::ConstTerm::Value(_))
        | TyTerm::Slice(element) => closed_term(element),
        // A fn pointer with closed parameter and result types (E-R5-008).
        TyTerm::Function(function) => {
            function.variables.is_empty()
                && function.value.parameters.iter().all(closed_term)
                && closed_term(&function.value.result)
        }
        _ => false,
    }
}
pub(crate) fn contains_transaction_term(term: &TyTerm) -> bool {
    let mut stack = vec![term];
    while let Some(term) = stack.pop() {
        match term {
            TyTerm::Infer(_) | TyTerm::Placeholder(_) => return true,
            TyTerm::Ref(crate::traits::term::RegionTerm::Placeholder(_), _, _)
            | TyTerm::Array(_, crate::traits::term::ConstTerm::Placeholder(_)) => return true,
            TyTerm::Projection(key) => {
                if key.arguments().any(transaction_argument) {
                    return true;
                }
                stack.push(&key.trait_ref.self_ty);
                stack.extend(key.arguments().filter_map(|arg| match arg {
                    crate::traits::term::GenericArg::Type(t) => Some(t),
                    _ => None,
                }));
            }
            TyTerm::Function(function) => {
                stack.extend(&function.value.parameters);
                stack.push(&function.value.result);
            }
            TyTerm::Tuple(fields) => stack.extend(fields),
            TyTerm::Nominal(_, args) | TyTerm::Opaque(_, args) => {
                if args.iter().any(transaction_argument) {
                    return true;
                }
                stack.extend(args.iter().filter_map(|arg| match arg {
                    crate::traits::term::GenericArg::Type(ty) => Some(ty),
                    _ => None,
                }));
            }
            TyTerm::Ref(_, _, term) | TyTerm::Array(term, _) | TyTerm::Slice(term) => {
                stack.push(term)
            }
            _ => {}
        }
    }
    false
}

fn transaction_argument(arg: &crate::traits::term::GenericArg) -> bool {
    matches!(
        arg,
        crate::traits::term::GenericArg::Region(crate::traits::term::RegionTerm::Placeholder(_))
            | crate::traits::term::GenericArg::Const(crate::traits::term::ConstTerm::Placeholder(
                _
            ))
    )
}

#[cfg(test)]
mod primitive_namespace_authority_tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        db::key::OutlineKey,
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::{
            callables::{CallResolver, CallSelection},
            compile_input_catalog::CompileInputCatalog,
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            manifest::Namespace,
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParseKey,
        vm::context::GenericEnv,
    };
    use std::sync::Arc;

    fn world(
        text: &str,
        edition: u16,
        sealed: bool,
    ) -> (
        Database,
        PreparedSourceView,
        NameNamespaces,
        Box<SourceCallable>,
    ) {
        let unit = Digest::of_bytes(b"LRR primitive result source authority");
        let profile = ProfileIdentity {
            edition,
            ..ProfileIdentity::current()
        };
        let cfg = CfgSet::guest_default();
        let blob = SourceBlob::new(text.as_bytes().to_vec()).unwrap();
        let path = SourcePath::new("primitive-result-authority.rs").unwrap();
        let selection = SourceSelection {
            path: path.clone(),
            outline: OutlineKey {
                parse: ParseKey {
                    content: blob.content(),
                    profile: profile.clone(),
                },
                crate_key: unit,
                module_path: vec![],
                cfg: cfg.fingerprint(),
            },
        };
        let inputs = PreparedSourceInputs::refresh(
            None,
            vec![SourceUnitInput {
                path,
                crate_key: unit,
                module_path: vec![],
                profile: profile.clone(),
                cfg,
                blob,
            }],
            Default::default(),
        )
        .unwrap();
        let SourceSelectionOutcome::Selected(view) = inputs.select(&selection).unwrap() else {
            panic!("source absent")
        };
        let graph = NameNamespaces::from_outlines(&[view.outline()]).unwrap();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        let caller = view
            .outline()
            .items
            .iter()
            .find(|i| i.def_key.declared_name == "main")
            .unwrap();
        if sealed {
            db.begin_expansion_epoch(1).unwrap();
            for namespace in [Namespace::Value, Namespace::Type] {
                let mut discovery = SourceDiscovery::new(
                    vec![view.share()],
                    caller.scope.clone(),
                    namespace,
                    Digest::of_bytes(b"LRR primitive result current world"),
                    1,
                )
                .unwrap()
                .with_declarative_compiler(
                    Digest::of_bytes(b"LRR declaration producer"),
                    Digest::of_bytes(b"LRR compiler producer"),
                    Arc::new(CompileInputCatalog::standalone(&profile).unwrap()),
                )
                .unwrap();
                assert!(matches!(
                    discovery.poll().unwrap(),
                    SourceDiscoveryStep::Sealed(_)
                ));
                discovery.publish(&mut db).unwrap();
            }
        }
        let sources = [&view];
        let resolver = CallResolver::new(&graph, &sources).unwrap();
        let QueryPoll::Ready(Ok(CallSelection::Source(callee))) =
            resolver.module_function(&mut db, caller.def_key.fingerprint(), "chosen")
        else {
            panic!("genuine chosen definition absent")
        };
        (db, view, graph, callee)
    }

    #[test]
    fn genuine_plain_and_sealed_primitive_results_keep_source_authority_all_editions() {
        for edition in [2015, 2018, 2021, 2024] {
            for sealed in [false, true] {
                let (mut db, view, graph, callee) = world(
                    "fn main(){chosen();}fn chosen()->u32{42u32}",
                    edition,
                    sealed,
                );
                let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) =
                    prepare_contracts(&mut db, &callee, &graph, &[&view], &[], None, &[])
                else {
                    panic!("actual primitive source preparation refused")
                };
                assert_eq!(
                    packet.source_result,
                    Some(TyTerm::Primitive(PrimitiveType::U32))
                );
                assert!(packet.normalized_deferred_return.is_some());
                assert!(GenericEnv::capture_current(&mut db, &callee, &packet).is_ok());
            }
        }
    }

    #[test]
    fn unrelated_graph_or_empty_source_list_cannot_erase_current_primitive_shadow() {
        for sealed in [false, true] {
            let (mut db, view, _, callee) = world(
                "type u32=bool;fn main(){chosen();}fn chosen()->u32{true}",
                2024,
                sealed,
            );
            let wrong = NameNamespaces::default();
            assert!(matches!(
                prepare_contracts(&mut db, &callee, &wrong, &[], &[], None, &[]),
                QueryPoll::Ready(Err(Diagnostic {
                    code: "LRR-E-CALL-CONTEXT",
                    ..
                }))
            ));
            assert!(
                NormalizedDeferredReturn::mint_primitive(
                    &mut db,
                    &callee,
                    &wrong,
                    &[&view],
                    PrimitiveType::U32
                )
                .is_err()
            );
        }
    }

    #[test]
    fn foreign_retained_sources_cannot_supply_primitive_namespace_authority() {
        let (mut db, view, _, callee) =
            world("fn main(){chosen();}fn chosen()->u32{42u32}", 2024, false);
        let (mut foreign_db, foreign_view, _, _) = world(
            "type u32=bool;fn main(){chosen();}fn chosen()->u32{true}",
            2024,
            false,
        );
        let wrong = NameNamespaces::default();
        assert!(
            NormalizedDeferredReturn::mint_primitive(
                &mut db,
                &callee,
                &wrong,
                &[&foreign_view],
                PrimitiveType::U32
            )
            .is_err()
        );
        assert!(
            NormalizedDeferredReturn::mint_primitive(
                &mut foreign_db,
                &callee,
                &wrong,
                &[&view],
                PrimitiveType::U32
            )
            .is_err()
        );
    }

    #[test]
    fn published_epoch_change_invalidates_the_primitive_namespace_certificate() {
        let (mut db, view, graph, callee) =
            world("fn main(){chosen();}fn chosen()->u32{42u32}", 2024, true);
        let QueryPoll::Ready(Ok(ContractPreparation::Ready(packet))) =
            prepare_contracts(&mut db, &callee, &graph, &[&view], &[], None, &[])
        else {
            panic!("genuine primitive proof absent")
        };
        db.begin_expansion_epoch(2).unwrap();
        assert!(GenericEnv::capture_current(&mut db, &callee, &packet).is_err());
    }
}

#[cfg(test)]
#[path = "test_catalog_primitive_namespace_tests.rs"]
mod test_catalog_primitive_namespace_tests;
