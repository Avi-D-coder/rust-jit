//! Immutable, demanded associated-equality facts for an evaluated source call.
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
    source::manifest::{BodyKey, PcKey},
    traits::term::{ParamKey, TyTerm},
};
use ra_ap_syntax::{AstNode, ast};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub enum EquationOwner {
    DeclaredBody(BodyKey),
    SelectedCall(PcKey),
}
impl StableEncode for EquationOwner {
    const DOMAIN: &'static str = "lrr/v1/associated-equation-owner";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::DeclaredBody(body) => {
                e.u32(0);
                e.record(body);
            }
            Self::SelectedCall(pc) => {
                e.u32(1);
                e.record(pc);
            }
        }
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct AssociatedEquationsKey {
    pub(crate) owner: EquationOwner,
    pub(crate) definition: Digest,
    pub(crate) bindings: Vec<(ParamKey, TyTerm)>,
    pub(crate) needed: Vec<u32>,
}
impl AssociatedEquationsKey {
    pub(crate) fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .needed
            .capacity()
            .checked_mul(std::mem::size_of::<u32>())?;
        bytes = bytes.checked_add(
            self.bindings
                .capacity()
                .checked_mul(std::mem::size_of::<(ParamKey, TyTerm)>())?,
        )?;
        for (_, term) in &self.bindings {
            bytes = bytes.checked_add(term.heap_bytes()?)?;
        }
        Some(bytes)
    }
}
impl StableEncode for AssociatedEquationsKey {
    const DOMAIN: &'static str = "lrr/v2/associated-equality-goal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.owner);
        e.digest(self.definition);
        e.sequence(&self.bindings, |e, (parameter, term)| {
            e.record(parameter);
            e.record(term);
        });
        e.sequence(&self.needed, |e, ordinal| e.u32(*ordinal));
    }
}
#[derive(Debug, Clone)]
pub struct PreparedAssociatedEquations {
    key: AssociatedEquationsKey,
    signature: Digest,
    equations: Vec<(TyTerm, TyTerm)>,
    pending: bool,
}
impl PreparedAssociatedEquations {
    pub(crate) fn key(&self) -> &AssociatedEquationsKey {
        &self.key
    }
    pub(crate) fn equations(&self) -> &[(TyTerm, TyTerm)] {
        &self.equations
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.key.heap_bytes()?)?
            .checked_add(
                self.equations
                    .capacity()
                    .checked_mul(std::mem::size_of::<(TyTerm, TyTerm)>())?,
            )?;
        for (left, right) in &self.equations {
            bytes = bytes
                .checked_add(left.heap_bytes()?)?
                .checked_add(right.heap_bytes()?)?;
        }
        Some(bytes)
    }
}
impl StableEncode for PreparedAssociatedEquations {
    const DOMAIN: &'static str = "lrr/v2/prepared-associated-equalities";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.signature);
        e.bool(self.pending);
        e.sequence(&self.equations, |e, (left, right)| {
            e.record(left);
            e.record(right);
        });
    }
}
fn pending(key: &AssociatedEquationsKey, signature: Digest) -> PreparedAssociatedEquations {
    PreparedAssociatedEquations {
        key: key.clone(),
        signature,
        equations: Vec::new(),
        pending: true,
    }
}
pub(crate) struct EquationInference {
    pub(crate) guard: Option<std::sync::Arc<crate::db::memo::PreparedInputGuard>>,
    pub(crate) pending: bool,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
macro_rules! ready {
    ($v:expr) => {
        match $v {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
macro_rules! checked {
    ($v:expr) => {
        match $v {
            Ok(v) => v,
            Err(e) => return QueryPoll::Ready(Err(e)),
        }
    };
}
pub(crate) fn prepare(
    db: &mut Database,
    key: &AssociatedEquationsKey,
) -> QueryPoll<PreparedAssociatedEquations> {
    checked!(key.heap_bytes().ok_or_else(crate::runtime::arena::resource));
    let definition = key.definition;
    if key.bindings.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        || key.needed.windows(2).any(|pair| pair[0] >= pair[1])
        || key.bindings.iter().any(|(parameter, term)| {
            parameter.definition != definition || !super::call_contracts::closed_term(term)
        })
    {
        return QueryPoll::Ready(Err(fault("bound equation key is not canonical")));
    }

    let selected = ready!(db.fetch(
        &SyntaxProvider,
        match &key.owner {
            EquationOwner::DeclaredBody(_) => QueryKey::FunctionSignature(definition),
            EquationOwner::SelectedCall(pc) => QueryKey::EnteredCall(pc.clone()),
        }
    ));
    let (signature, source, context, completed, local_scope) =
        match (&key.owner, selected.value.as_ref()) {
            (EquationOwner::DeclaredBody(body), QueryValue::FunctionSignature(signature)) => {
                let environment = db.input(&InputKey::BodyEnvironment(body.clone()));
                let Some(InputValue::BodyEnvironment(environment)) = environment.as_deref() else {
                    return QueryPoll::Ready(Ok(pending(key, selected.fingerprint)));
                };
                ready!(environment.require_current(db));
                if environment.body() != body || body.definition != definition {
                    return QueryPoll::Ready(Err(fault(
                        "bound equation declaration owner differs",
                    )));
                }
                let context = db.input(&InputKey::DeclarationContext(definition));
                let source = db.input(&InputKey::DefinitionSource(definition));
                let (
                    Some(InputValue::DeclarationContext(context)),
                    Some(InputValue::DefinitionSource(source)),
                ) = (context.as_deref(), source.as_deref())
                else {
                    return QueryPoll::Ready(Err(fault(
                        "bound equation declaration inputs absent",
                    )));
                };
                (
                    signature.as_ref(),
                    source.clone(),
                    context.clone(),
                    std::sync::Arc::clone(environment.macros().source_environment()),
                    None,
                )
            }
            (EquationOwner::SelectedCall(pc), QueryValue::EnteredCall(goal)) => {
                checked!(goal.require_current(db));
                if goal.key() != pc {
                    return QueryPoll::Ready(Err(fault("bound equation call occurrence differs")));
                }
                let Some(signature) = goal.signature() else {
                    return QueryPoll::Ready(Ok(pending(key, selected.fingerprint)));
                };
                use super::entered_call_goals::EnteredCallSelection;
                let (source, context, parent, local_scope) = match goal.selection() {
                    EnteredCallSelection::Module(declaration) => {
                        if declaration.identity() != definition
                            || declaration.header() != signature.header.fingerprint()
                        {
                            return QueryPoll::Ready(Err(fault(
                                "bound equation selected module declaration differs",
                            )));
                        }
                        (
                            declaration.source().clone(),
                            declaration.context().clone(),
                            goal.retained_plan(),
                            None,
                        )
                    }
                    EnteredCallSelection::Local(proof) => {
                        let item = proof.item();
                        if item.def_key.fingerprint() != definition
                            || item.header != signature.header
                            || item.scope != signature.scope
                        {
                            return QueryPoll::Ready(Err(fault(
                                "bound equation selected local declaration differs",
                            )));
                        }
                        (
                            proof.source().clone(),
                            crate::syntax::outline::DeclarationContext {
                                scope: item.scope.clone(),
                                range: item.declaration_span,
                            },
                            proof.parent(),
                            (!proof.is_member()
                                && db.input(&InputKey::DefinitionHeader(definition)).is_none())
                            .then_some(proof),
                        )
                    }
                    _ => return QueryPoll::Ready(Ok(pending(key, selected.fingerprint))),
                };
                ready!(parent.require_current(db));
                let Some(environment) = parent.environment() else {
                    return QueryPoll::Ready(Ok(pending(key, selected.fingerprint)));
                };
                // Published module declarations retain their existing input join.
                // An actual local admission instead resolves header occurrences in
                // its genuine declaring parent, without publishing child inputs.
                if local_scope.is_none()
                    && (!matches!(db.input(&InputKey::DefinitionHeader(definition)).as_deref(),
                Some(InputValue::DefinitionHeader(header)) if header == &signature.header)
                        || !matches!(db.input(&InputKey::DefinitionSource(definition)).as_deref(),
                    Some(InputValue::DefinitionSource(actual)) if actual == &source)
                        || !matches!(db.input(&InputKey::DeclarationContext(definition)).as_deref(),
                    Some(InputValue::DeclarationContext(actual)) if actual == &context))
                {
                    return QueryPoll::Ready(Ok(pending(key, selected.fingerprint)));
                }
                (
                    signature,
                    source,
                    context,
                    std::sync::Arc::clone(environment.macros().source_environment()),
                    local_scope,
                )
            }
            _ => {
                return QueryPoll::Ready(Err(fault(
                    "bound equation selected query outcome differs",
                )));
            }
        };
    if signature.definition != definition
        || context.scope != signature.scope
        || source.profile != signature.header.profile
    {
        return QueryPoll::Ready(Err(fault("bound equation source scope/profile differs")));
    }
    let parsed = ready!(db.fetch(&SyntaxProvider, QueryKey::ParseFile(source)));
    let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("bound equation parse differs")));
    };
    let Some(owner) =
        checked!(parsed.syntax_at(context.range, |node| ast::Fn::can_cast(node.kind())))
    else {
        return QueryPoll::Ready(Err(fault("bound equation actual function absent")));
    };
    let sources: Vec<_> = completed.sources().iter().collect();
    let declaration =
        super::source_types::WrittenTypeDeclaration::function_syntax(signature, &owner);
    let declaration = match local_scope {
        Some(proof) => declaration.with_local_scope(proof),
        None => declaration,
    };
    let equations = ready!(super::source_types::actual_bound_equations(
        db,
        completed.names(),
        &sources,
        &declaration,
        &owner,
        &key.needed
    ));
    let bindings: BTreeMap<_, _> = key.bindings.iter().cloned().collect();
    let Some(equations) = equations else {
        return QueryPoll::Ready(Ok(pending(key, selected.fingerprint)));
    };
    let mut normalized = Vec::new();
    let mut unanswered = false;
    if !equations.is_empty() {
        let members = checked!(completed.member_index());
        for (projection, template) in equations {
            let selected = super::associated_types::substitute(&projection, &bindings);
            match ready!(super::associated_types::normalize(
                db,
                completed.names(),
                &sources,
                &members,
                &selected
            )) {
                Some(actual) if super::call_contracts::closed_term(&actual) => {
                    normalized.push((template, actual))
                }
                _ => unanswered = true,
            }
        }
    }
    QueryPoll::Ready(Ok(PreparedAssociatedEquations {
        key: key.clone(),
        signature: selected.fingerprint,
        equations: normalized,
        pending: unanswered,
    }))
}

/// One declaration-equation inference route for source Type goals and reached
/// call contracts. Source consumers record the fetched query in their outer
/// memo closure; runtime consumers retain the returned immutable input guard.
pub(crate) fn constrain(
    db: &mut Database,
    owner: &EquationOwner,
    signature: &crate::syntax::signatures::FunctionSignature,
    symbolic: &mut super::signature_subst::SignatureSubstitution,
) -> QueryPoll<EquationInference> {
    use crate::{db::memo::PreparedInputGuard, syntax::signatures::GenericKind};
    use std::sync::Arc;
    if let EquationOwner::DeclaredBody(body) = owner
        && body.definition != signature.definition
    {
        return QueryPoll::Ready(Err(fault(
            "equation substitution declaration owner differs",
        )));
    }
    let mut pending = false;
    let mut guards = Vec::new();
    loop {
        let needed: Vec<_> = signature
            .generics
            .iter()
            .filter(|parameter| {
                parameter.kind == GenericKind::Type
                    && symbolic
                        .binding(parameter.ordinal)
                        .is_none_or(|term| !super::call_contracts::closed_term(term))
            })
            .map(|parameter| parameter.ordinal)
            .collect();
        if needed.is_empty() {
            break;
        }
        let bindings: Vec<_> = signature
            .generics
            .iter()
            .filter_map(|parameter| {
                let term = symbolic.binding(parameter.ordinal)?;
                super::call_contracts::closed_term(term).then(|| {
                    (
                        ParamKey {
                            definition: signature.definition,
                            ordinal: parameter.ordinal,
                        },
                        term.clone(),
                    )
                })
            })
            .collect();
        let before = bindings.len();
        let key = QueryKey::AssociatedEquations(Box::new(AssociatedEquationsKey {
            owner: owner.clone(),
            definition: signature.definition,
            bindings,
            needed,
        }));
        let selected = ready!(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::AssociatedEquations(equations) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("associated equality goal outcome differs")));
        };
        pending |= equations.pending;
        if equations.equations().is_empty() {
            break;
        }
        for (template, actual) in equations.equations() {
            checked!(symbolic.constrain_equation(template, actual));
        }
        // Guard every consumed fact, even if no runtime descriptor exists.
        guards.push(ready!(db.prepare_input_guard(
            &SyntaxProvider,
            &key,
            selected.fingerprint
        )));
        let after = signature
            .generics
            .iter()
            .filter(|parameter| {
                symbolic
                    .binding(parameter.ordinal)
                    .is_some_and(super::call_contracts::closed_term)
            })
            .count();
        if after <= before {
            break;
        }
    }
    QueryPoll::Ready(Ok(EquationInference {
        guard: if guards.is_empty() {
            None
        } else {
            Some(Arc::new(checked!(PreparedInputGuard::join(
                &guards.iter().collect::<Vec<_>>()
            ))))
        },
        pending,
    }))
}
