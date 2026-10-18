//! Declared parameter Type goals. Selection/execution authority stays with the
//! ordinary callable/source member service; no child code is prepared here.
use super::{
    names::NameNamespaces,
    signature_names::SignatureName,
    source_types::{self, NamedTypePreparation},
    type_aliases::AliasTarget,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::changes::PreparedSourceView,
    syntax::ast_adapter::TokenRecipe,
    traits::term::TyTerm,
};

/// Immutable source fact, not a callable, runtime descriptor or current-use
/// authority. Only a genuine current declaration goal constructs this value.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct DeclaredParameterType {
    definition: Digest,
    signature: Digest,
    ordinal: u32,
    source: TyTerm,
}
impl DeclaredParameterType {
    pub(crate) fn definition(&self) -> Digest {
        self.definition
    }
    pub(crate) fn signature_identity(&self) -> Digest {
        self.signature
    }
    pub(crate) fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub(crate) fn source(&self) -> &TyTerm {
        &self.source
    }
    pub(crate) fn require(
        &self,
        definition: Digest,
        signature_identity: Digest,
        ordinal: usize,
    ) -> Result<(), Diagnostic> {
        if self.definition() != definition
            || self.signature_identity() != signature_identity
            || self.ordinal() as usize != ordinal
        {
            return Err(fault("declared parameter goal owner differs"));
        }
        Ok(())
    }
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        std::mem::size_of::<Self>()
            .checked_add(
                self.source
                    .heap_bytes()
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(crate::runtime::arena::resource)
    }
}
impl StableEncode for DeclaredParameterType {
    const DOMAIN: &'static str = "lrr/v1/declared-parameter-type-goal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.signature);
        e.u32(self.ordinal);
        e.record(&self.source);
    }
}
#[derive(Debug, Clone)]
pub(crate) enum ParameterTypeGoal {
    Ready(DeclaredParameterType),
    NeedType {
        definition: Digest,
        syntax: Vec<TokenRecipe>,
    },
    NeedAlias(AliasTarget),
    NeedName(SignatureName),
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
fn ready(
    definition: Digest,
    signature: Digest,
    ordinal: usize,
    source: TyTerm,
) -> QueryPoll<ParameterTypeGoal> {
    let goal = DeclaredParameterType {
        definition,
        signature,
        ordinal: ordinal as u32,
        source,
    };
    QueryPoll::Ready(
        goal.allocated_bytes()
            .map(|_| ParameterTypeGoal::Ready(goal)),
    )
}
/// A local goal's complete memo payload owns facts only, not parser/source roots.
#[derive(Debug, Clone)]
pub struct ParameterGoalObservation {
    definition: Digest,
    ordinal: u32,
    signature: Digest,
    goal: ParameterTypeGoal,
}
impl ParameterGoalObservation {
    pub(crate) fn matches(&self, definition: Digest, ordinal: u32) -> bool {
        self.definition == definition
            && self.ordinal == ordinal
            && match &self.goal {
                ParameterTypeGoal::Ready(goal) => goal
                    .require(definition, self.signature, ordinal as usize)
                    .is_ok(),
                _ => true,
            }
    }
    pub(crate) fn goal(&self) -> &ParameterTypeGoal {
        &self.goal
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .checked_add(goal_heap(&self.goal).unwrap_or(usize::MAX))
            .unwrap_or(usize::MAX)
    }
}
fn tokens_heap(tokens: &Vec<TokenRecipe>) -> Option<usize> {
    tokens.iter().try_fold(
        tokens
            .capacity()
            .checked_mul(std::mem::size_of::<TokenRecipe>())?,
        |n, t| n.checked_add(t.text.capacity()),
    )
}
fn name_heap(name: &SignatureName) -> Option<usize> {
    match name {
        SignatureName::NeedOwnerContext(scope) => Some(scope.heap_bytes()),
        SignatureName::Unresolved(lookup) => lookup
            .metadata_bytes()
            .ok()?
            .checked_sub(std::mem::size_of_val(lookup)),
        _ => Some(0),
    }
}
fn goal_heap(goal: &ParameterTypeGoal) -> Option<usize> {
    match goal {
        ParameterTypeGoal::Ready(goal) => goal.source.heap_bytes(),
        ParameterTypeGoal::NeedType { syntax, .. } => tokens_heap(syntax),
        ParameterTypeGoal::NeedName(name) => name_heap(name),
        ParameterTypeGoal::NeedAlias(alias) => match alias {
            AliasTarget::Type(source) => source.heap_bytes(),
            AliasTarget::NeedOwnerContext(scope) => Some(scope.heap_bytes()),
            AliasTarget::Unresolved(lookup) => lookup
                .metadata_bytes()
                .ok()?
                .checked_sub(std::mem::size_of_val(lookup)),
            AliasTarget::Deferred { target, .. } => tokens_heap(target),
            _ => Some(0),
        },
    }
}
impl StableEncode for ParameterGoalObservation {
    const DOMAIN: &'static str = "lrr/v1/local-declared-parameter-goal-observation";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.u32(self.ordinal);
        e.digest(self.signature);
        match &self.goal {
            ParameterTypeGoal::Ready(goal) => {
                e.u8(0);
                e.record(goal);
            }
            ParameterTypeGoal::NeedType { definition, syntax } => {
                e.u8(1);
                e.digest(*definition);
                e.sequence(syntax, |e, t| e.record(t));
            }
            ParameterTypeGoal::NeedName(name) => {
                e.u8(2);
                match name {
                    SignatureName::Parameter(key) => {
                        e.u8(0);
                        e.record(key);
                    }
                    SignatureName::Primitive(p) => {
                        e.u8(1);
                        e.u32(p.tag());
                    }
                    SignatureName::Str => e.u8(2),
                    SignatureName::Source(d) => {
                        e.u8(3);
                        e.record(d);
                    }
                    SignatureName::Unresolved(lookup) => {
                        e.u8(4);
                        e.record(lookup);
                    }
                    SignatureName::NeedOwnerContext(scope) => {
                        e.u8(5);
                        e.record(scope);
                    }
                }
            }
            ParameterTypeGoal::NeedAlias(alias) => {
                e.u8(3);
                match alias {
                    AliasTarget::Type(t) => {
                        e.u8(0);
                        e.record(t);
                    }
                    AliasTarget::Nominal(d) => {
                        e.u8(1);
                        e.digest(*d);
                    }
                    AliasTarget::Str => e.u8(2),
                    AliasTarget::NeedProducer(d) => {
                        e.u8(3);
                        e.digest(*d);
                    }
                    AliasTarget::NeedOwnerContext(scope) => {
                        e.u8(4);
                        e.record(scope);
                    }
                    AliasTarget::Unresolved(lookup) => {
                        e.u8(5);
                        e.record(lookup);
                    }
                    AliasTarget::Deferred { definition, target } => {
                        e.u8(6);
                        e.digest(*definition);
                        e.sequence(target, |e, t| e.record(t));
                    }
                }
            }
        }
    }
}
fn classify(
    definition: Digest,
    signature: Digest,
    ordinal: usize,
    named: QueryPoll<NamedTypePreparation>,
) -> QueryPoll<ParameterTypeGoal> {
    match named {
        QueryPoll::Ready(Ok(NamedTypePreparation::Ready(proof))) => {
            ready(definition, signature, ordinal, proof.source().clone())
        }
        QueryPoll::Ready(Ok(NamedTypePreparation::Parameter(key))) => {
            ready(definition, signature, ordinal, TyTerm::Param(key))
        }
        QueryPoll::Ready(Ok(NamedTypePreparation::Alias(AliasTarget::Type(source)))) => {
            ready(definition, signature, ordinal, source)
        }
        // A single written name selecting a struct/enum declaration names that
        // nominal type without generic arguments.
        QueryPoll::Ready(Ok(NamedTypePreparation::Alias(AliasTarget::Nominal(nominal)))) => {
            ready(definition, signature, ordinal, TyTerm::Nominal(nominal, Vec::new()))
        }
        QueryPoll::Ready(Ok(NamedTypePreparation::Alias(other))) => {
            QueryPoll::Ready(Ok(ParameterTypeGoal::NeedAlias(other)))
        }
        QueryPoll::Ready(Ok(NamedTypePreparation::Name(other))) => {
            QueryPoll::Ready(Ok(ParameterTypeGoal::NeedName(other)))
        }
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}
/// Pure local query provider. Its genuine owning parent supplies all namespace
/// and source roots; no caller graph or child executable body is needed.
pub(crate) fn prepare_local_query(
    db: &mut Database,
    definition: Digest,
    ordinal: u32,
) -> QueryPoll<ParameterGoalObservation> {
    macro_rules! ready {
        ($v:expr) => {
            match $v {
                QueryPoll::Ready(Ok(v)) => v,
                QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
            }
        };
    }
    let selected = ready!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
    let QueryValue::FunctionSignature(signature) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("declared parameter signature query differs")));
    };
    let parameter = match signature.parameters.get(ordinal as usize) {
        Some(p) => p,
        None => return QueryPoll::Ready(Err(fault("declared parameter ordinal absent"))),
    };
    let owner = match db.input(&InputKey::LocalDeclarationOwner(definition)) {
        Some(owner) => owner,
        None => return QueryPoll::Ready(Err(fault("local declared parameter owner absent"))),
    };
    let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
        return QueryPoll::Ready(Err(fault("declared parameter owner input differs")));
    };
    let parent = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(owner.clone())));
    let QueryValue::SourceLocalPlan(plan) = parent.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("declared parameter owning plan differs")));
    };
    ready!(plan.require_current(db));
    let local_receiver = if parameter.receiver {
        ready!(plan.local_receiver_term(db, definition, &parameter.ty))
    } else {
        None
    };
    let goal = if let Some(term) = local_receiver {
        ready!(classify(
            definition,
            selected.fingerprint,
            ordinal as usize,
            QueryPoll::Ready(Ok(NamedTypePreparation::Alias(AliasTarget::Type(term))))
        ))
    } else if parameter.receiver || parameter.variadic {
        ParameterTypeGoal::NeedType {
            definition,
            syntax: parameter.ty.clone(),
        }
    } else {
        let named = plan.prepare_parameter_type(db, signature, ordinal);
        ready!(classify(
            definition,
            selected.fingerprint,
            ordinal as usize,
            named
        ))
    };
    let result = ParameterGoalObservation {
        definition,
        ordinal,
        signature: selected.fingerprint,
        goal,
    };
    QueryPoll::Ready(Ok(result))
}
/// Common consumer. Local declarations use their retained pure memo. Ordinary
/// module declarations retain the existing SourceType adapter until genuine
/// declaration-environment receipt transport supports the same query key.
pub(crate) fn prepare(
    db: &mut Database,
    graph: &NameNamespaces,
    sources: &[&PreparedSourceView],
    definition: Digest,
    ordinal: usize,
) -> QueryPoll<ParameterTypeGoal> {
    if db
        .input(&InputKey::LocalDeclarationOwner(definition))
        .is_some()
    {
        return match db.fetch(
            &SyntaxProvider,
            QueryKey::DeclaredParameterGoal {
                definition,
                ordinal: ordinal as u32,
            },
        ) {
            QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
                QueryValue::DeclaredParameterGoal(goal)
                    if goal.matches(definition, ordinal as u32) =>
                {
                    QueryPoll::Ready(Ok(goal.goal().clone()))
                }
                _ => QueryPoll::Ready(Err(fault("declared parameter goal query differs"))),
            },
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        };
    }
    let selected = match db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)) {
        QueryPoll::Ready(Ok(value)) => value,
        QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
        QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
    };
    let QueryValue::FunctionSignature(signature) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("declared parameter signature query differs")));
    };
    let Some(parameter) = signature.parameters.get(ordinal) else {
        return QueryPoll::Ready(Err(fault("declared parameter ordinal absent")));
    };
    if parameter.ty.len() != 1 {
        return match &parameter.symbolic {
            Some(source) => ready(definition, selected.fingerprint, ordinal, source.clone()),
            None => match source_types::resolve_written_type(
                db,
                graph,
                sources,
                signature,
                &parameter.ty,
            ) {
                QueryPoll::Ready(Ok(Some(source))) => {
                    ready(definition, selected.fingerprint, ordinal, source)
                }
                QueryPoll::Ready(Ok(None)) => QueryPoll::Ready(Ok(ParameterTypeGoal::NeedType {
                    definition,
                    syntax: parameter.ty.clone(),
                })),
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            },
        };
    }
    // `Self` (a receiver's or a written parameter's) is the impl's written
    // self type: a parameterized nominal of a generic impl, or the bare
    // nominal (`impl<'a> S<'a>`, whose region arguments are not data).
    if parameter.ty[0].text == "Self" {
        if let QueryPoll::Ready(Ok(Some(source))) =
            source_types::resolve_written_type(db, graph, sources, signature, &parameter.ty)
        {
            return ready(definition, selected.fingerprint, ordinal, source);
        }
    }
    let named = source_types::prepare_named(db, graph, sources, signature, &parameter.ty[0].text);
    classify(definition, selected.fingerprint, ordinal, named)
}
