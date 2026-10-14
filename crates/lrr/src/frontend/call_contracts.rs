//! Reached call boundaries keep source templates separate from concrete descriptors.
use super::{
    callables::SourceCallable,
    names::NameNamespaces,
    signature_names::{SignatureName, SignatureNames},
    signature_subst::SignatureSubstitution,
    type_aliases::{AliasNormalizer, AliasTarget},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{DependencyStamp, InputKey},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
    },
    ir::op::PrimitiveType,
    runtime::shape::TypeId,
    source::changes::PreparedSourceView,
    syntax::ast_adapter::TokenRecipe,
    traits::term::{ParamKey, TyTerm},
};
use std::collections::BTreeMap;
#[derive(Debug, Clone)]
pub struct ArgumentType {
    pub source: TyTerm,
    pub descriptor: TypeId,
}
pub struct PreparedCallContracts {
    pub definition: Digest,
    pub source_parameters: Vec<TyTerm>,
    pub parameter_descriptors: Vec<TypeId>,
    pub source_substitution: BTreeMap<ParamKey, TyTerm>,
    pub runtime_substitution: BTreeMap<ParamKey, TypeId>,
    pub source_result: Option<TyTerm>,
    pub result_descriptor: Option<TypeId>,
    /// Only the frontend can mint this; public packet fields are checked against it.
    pub normalized_deferred_return: Option<Box<NormalizedDeferredReturn>>,
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
        let rows = trace
            .len()
            .checked_add(9)
            .filter(|n| *n <= 4096)
            .ok_or_else(|| {
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
        let bytes = dependencies
            .iter()
            .try_fold(std::mem::size_of::<Self>(), |bytes, stamp| {
                bytes
                    .checked_add(std::mem::size_of::<DependencyStamp>())?
                    .checked_add(stamp.heap_bytes())
            });
        if bytes.is_none_or(|n| n > 32 * 1024 * 1024) {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "normalized return dependency byte budget exceeded",
            ));
        }
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
            .filter(|n| *n <= 4096)
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
        if certificate
            .metadata_bytes()
            .is_none_or(|n| n > 32 * 1024 * 1024)
        {
            return Err(fault(
                "LRR-E-METADATA-LIMIT",
                "lexical return contract size exceeds budget",
            ));
        }
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
            || self.dependencies.len() > 4096
            || self.dependencies.windows(2).any(|p| p[0] >= p[1])
        {
            return Err(fault(
                "LRR-E-CALL-CONTEXT",
                "normalized deferred return certificate differs from its declaration/packet",
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
            DeferredReturnOrigin::Alias { .. } => 0,
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
    const DOMAIN: &'static str = "lrr/v5/normalized-deferred-return-certificate-v3";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.signature);
        match &self.origin {
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
    use PrimitiveType::*;
    Ok(TypeId(match p {
        Unit => 1,
        Bool => 2,
        Char => 3,
        I8 => 4,
        I16 => 5,
        I32 => 6,
        I64 => 7,
        Isize => 8,
        U8 => 9,
        U16 => 10,
        U32 => 11,
        U64 => 12,
        Usize => 13,
        F32 => 14,
        F64 => 15,
        I128 | U128 => {
            return Err(fault(
                "LRR-E-UNSUPPORTED",
                "128-bit guest integers are outside the live profile",
            ));
        }
    }))
}
impl PreparedCallContracts {
    /// Validate the reached return source identity and bind an unbound result parameter.
    pub fn bind_return_parameter(&mut self, actual: &ArgumentType) -> Result<(), Diagnostic> {
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
            self.check_return_descriptor(actual.descriptor)?;
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
            if expected.is_some_and(|expected| expected != &actual.source) {
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
        )?;
        self.result_descriptor = Some(actual.descriptor);
        Ok(())
    }
    /// Only already-concrete return contracts; unresolved returns stay deferred.
    pub fn check_return_descriptor(&self, actual: TypeId) -> Result<(), Diagnostic> {
        let expected = self.result_descriptor.ok_or_else(|| {
            fault(
                "LRR-E-TYPE-CONTEXT",
                "reached return requires deferred source type resolution",
            )
        })?;
        if expected != actual {
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
    pub source: TyTerm,
    pub descriptor: Option<TypeId>,
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
        .filter(|n| *n <= 32 * 1024 * 1024)
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
    if index >= 100_000 {
        return QueryPoll::Ready(Err(fault(
            "LRR-E-METADATA-LIMIT",
            "argument hint ordinal exceeds its budget",
        )));
    }
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
    let definition = signature.definition;
    let names = SignatureNames::from_database(db, graph, signature)?;
    let normalizer = AliasNormalizer::new(graph, sources)?;
    let mut templates = Vec::new();
    let mut template_bytes = 0usize;
    for (ordinal, parameter) in signature.parameters.iter().enumerate() {
        if index.is_some_and(|selected| selected != ordinal) {
            continue;
        }
        if let Some(proof) = callee.lexical_contract() {
            proof.revalidate(db)?;
            let primitive = proof
                .plan()
                .primitive_parameters()
                .get(ordinal)
                .copied()
                .ok_or_else(|| {
                    fault("LRR-E-CALL-CONTEXT", "lexical parameter certificate absent")
                })?;
            push_template(
                &mut templates,
                &mut template_bytes,
                TyTerm::Primitive(primitive),
            )?;
            continue;
        }
        if let Some(term) = &parameter.symbolic {
            push_template(&mut templates, &mut template_bytes, term.clone())?;
            continue;
        }
        if parameter.ty.len() != 1 {
            return Ok(QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedType {
                definition,
                syntax: parameter.ty.clone(),
            })));
        }
        match names.unqualified(&parameter.ty[0].text, None)? {
            SignatureName::Primitive(p) => {
                push_template(&mut templates, &mut template_bytes, TyTerm::Primitive(p))?
            }
            SignatureName::Parameter(p) => {
                push_template(&mut templates, &mut template_bytes, TyTerm::Param(p))?
            }
            SignatureName::Source(found) => match normalizer.normalize(db, found.identity) {
                QueryPoll::Ready(Ok(AliasTarget::Type(term))) => {
                    push_template(&mut templates, &mut template_bytes, term)?
                }
                QueryPoll::Ready(Ok(other)) => {
                    return Ok(QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedAlias(
                        other,
                    ))));
                }
                QueryPoll::Ready(Err(error)) => return Err(error),
                QueryPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
            },
            other => {
                return Ok(QueryPoll::Ready(Ok(ArgumentHintPreparation::NeedName(
                    other,
                ))));
            }
        }
    }
    let mut hints = Vec::new();
    hints
        .try_reserve_exact(templates.len())
        .map_err(|_| fault("LRR-E-METADATA-LIMIT", "argument hint allocation failed"))?;
    for source in templates {
        let descriptor = match &source {
            TyTerm::Primitive(p) => Some(primitive_runtime_id(*p)?),
            _ => None,
        };
        hints.push(ArgumentHint { source, descriptor });
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
    match prepare(db, callee, graph, sources, explicit, result_hint, arguments) {
        Ok(value) => value,
        Err(error) => QueryPoll::Ready(Err(error)),
    }
}
fn prepare(
    db: &mut Database,
    callee: &SourceCallable,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    explicit: &[(u32, ArgumentType)],
    result_hint: Option<&ArgumentType>,
    arguments: &[ArgumentType],
) -> Result<QueryPoll<ContractPreparation>, Diagnostic> {
    callee.revalidate(db)?;
    let signature = callee.signature();
    let definition = signature.definition;
    if arguments.len() != signature.parameters.len() {
        return Err(fault(
            "LRR-E-CALL-ARITY",
            "evaluated argument count differs from the selected signature",
        ));
    }
    if arguments.len() > 100_000 || explicit.len() > 100_000 {
        return Err(fault(
            "LRR-E-METADATA-LIMIT",
            "call contract argument budget exceeded",
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
            .filter(|n| *n <= 32 * 1024 * 1024)
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
    let hints = match prepare_argument_hints(db, callee, graph, sources) {
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
    let templates: Vec<_> = hints.into_iter().map(|hint| hint.source).collect();
    // Transaction identity is local metadata and never serialized as VM data.
    let transaction = Digest::of_bytes(&[definition.0.as_slice(), b"call-contract"].concat());
    let mut instantiated = signature.clone();
    let mut normalized_deferred_return = None;
    for (parameter, term) in instantiated.parameters.iter_mut().zip(&templates) {
        parameter.symbolic = Some(term.clone());
    }
    // Written aliases become source contracts only after immutable owning
    // declaration lookup and bounded normalization. Primitive spelling alone
    // keeps its existing deferred named-result path.
    if instantiated.result_symbolic.is_none() && callee.lexical_contract().is_some() {
        let proof = NormalizedDeferredReturn::mint_lexical(db, callee)?;
        instantiated.result_symbolic = Some(TyTerm::Primitive(proof.source));
        normalized_deferred_return = Some(Box::new(proof));
    } else if instantiated.result_symbolic.is_none() && signature.result.len() == 1 {
        let names = SignatureNames::from_database(db, graph, signature)?;
        if let SignatureName::Source(found) = names.unqualified(&signature.result[0].text, None)? {
            let normalizer = AliasNormalizer::new(graph, sources)?;
            match normalizer.normalize_with_dependencies(db, found.identity) {
                QueryPoll::Ready(Ok((AliasTarget::Type(TyTerm::Primitive(primitive)), trace))) => {
                    normalized_deferred_return = Some(Box::new(NormalizedDeferredReturn::mint(
                        db, callee, &found, primitive, trace,
                    )?));
                    instantiated.result_symbolic = Some(TyTerm::Primitive(primitive));
                }
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
        )?;
    }
    if let (Some(TyTerm::Param(key)), Some(hint)) = (&instantiated.result_symbolic, result_hint) {
        symbolic.bind_type_parameter(key.ordinal, &hint.source)?;
        bind(&mut runtime, &mut source_bindings, *key, hint)?;
    }
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
                bind(&mut runtime, &mut source_bindings, *key, actual)?;
                descriptors.push(actual.descriptor);
            }
            TyTerm::Primitive(p) => {
                let expected = primitive_runtime_id(*p)?;
                if expected != actual.descriptor {
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
    let result_descriptor = match instantiated.result_symbolic.as_ref() {
        Some(TyTerm::Param(key)) => runtime.get(key).copied(),
        Some(TyTerm::Primitive(p)) => Some(primitive_runtime_id(*p)?),
        _ => None,
    };
    Ok(QueryPoll::Ready(Ok(ContractPreparation::Ready(
        PreparedCallContracts {
            definition,
            source_parameters: templates,
            parameter_descriptors: descriptors,
            source_substitution: source_bindings,
            runtime_substitution: runtime,
            source_result: instantiated.result_symbolic.clone(),
            result_descriptor,
            normalized_deferred_return,
        },
    ))))
}
fn bind(
    runtime: &mut BTreeMap<ParamKey, TypeId>,
    source: &mut BTreeMap<ParamKey, TyTerm>,
    key: ParamKey,
    actual: &ArgumentType,
) -> Result<(), Diagnostic> {
    if runtime
        .get(&key)
        .is_some_and(|prior| *prior != actual.descriptor)
    {
        return Err(fault(
            "LRR-E-GENERIC-BINDING",
            "reached generic runtime descriptors disagree",
        ));
    }
    if source
        .get(&key)
        .is_some_and(|prior| prior != &actual.source)
    {
        return Err(fault(
            "LRR-E-GENERIC-BINDING",
            "reached generic source identities disagree",
        ));
    }
    runtime.insert(key, actual.descriptor);
    source.entry(key).or_insert_with(|| actual.source.clone());
    Ok(())
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
