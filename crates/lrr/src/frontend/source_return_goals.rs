//! Immutable declared result facts from the common source/header Type service.
//! No runtime descriptor or entered callee body answers a declared Type goal.
use super::{
    signature_names::SignatureName, source_types::NamedTypePreparation, type_aliases::AliasTarget,
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
    traits::term::TyTerm,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct DeclaredReturnType {
    definition: Digest,
    signature: Digest,
    source: TyTerm,
}
impl DeclaredReturnType {
    pub(crate) fn definition(&self) -> Digest {
        self.definition
    }
    pub(crate) fn signature_identity(&self) -> Digest {
        self.signature
    }
    pub(crate) fn source(&self) -> &TyTerm {
        &self.source
    }
    pub(crate) fn require(&self, definition: Digest, signature: Digest) -> Result<(), Diagnostic> {
        if self.definition != definition || self.signature != signature {
            return Err(fault("declared return goal owner differs"));
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
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(crate::runtime::arena::resource)
    }
}
impl StableEncode for DeclaredReturnType {
    const DOMAIN: &'static str = "lrr/v1/declared-return-type-goal";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.signature);
        e.record(&self.source);
    }
}
#[derive(Debug, Clone)]
pub(crate) enum ReturnTypeGoal {
    Ready(DeclaredReturnType),
    NeedAlias(AliasTarget),
    NeedName(SignatureName),
}
/// Facts only. The future query's ordinary input guard owns currentness; this
/// payload cannot select a callable, substitute a runtime Type, or enter code.
#[derive(Debug, Clone)]
pub struct ReturnGoalObservation {
    definition: Digest,
    signature: Digest,
    goal: ReturnTypeGoal,
}
impl ReturnGoalObservation {
    pub(crate) fn matches(&self, definition: Digest) -> bool {
        self.definition == definition
            && match &self.goal {
                ReturnTypeGoal::Ready(goal) => goal.require(definition, self.signature).is_ok(),
                _ => true,
            }
    }
    pub(crate) fn signature_identity(&self) -> Digest {
        self.signature
    }
    pub(crate) fn goal(&self) -> &ReturnTypeGoal {
        &self.goal
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .checked_add(goal_heap(&self.goal).unwrap_or(usize::MAX))
            .unwrap_or(usize::MAX)
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
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
fn goal_heap(goal: &ReturnTypeGoal) -> Option<usize> {
    match goal {
        ReturnTypeGoal::Ready(goal) => goal.source.heap_bytes(),
        ReturnTypeGoal::NeedName(name) => name_heap(name),
        ReturnTypeGoal::NeedAlias(alias) => match alias {
            AliasTarget::Type(source) => source.heap_bytes(),
            AliasTarget::NeedOwnerContext(scope) => Some(scope.heap_bytes()),
            AliasTarget::Unresolved(lookup) => lookup
                .metadata_bytes()
                .ok()?
                .checked_sub(std::mem::size_of_val(lookup)),
            AliasTarget::Deferred { target, .. } => target.iter().try_fold(
                target
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::syntax::ast_adapter::TokenRecipe>())?,
                |bytes, token| bytes.checked_add(token.text.capacity()),
            ),
            _ => Some(0),
        },
    }
}
fn encode_name(name: &SignatureName, e: &mut StableEncoder) {
    match name {
        SignatureName::Parameter(key) => {
            e.u8(0);
            e.record(key);
        }
        SignatureName::Primitive(primitive) => {
            e.u8(1);
            e.u32(primitive.tag());
        }
        SignatureName::Str => e.u8(2),
        SignatureName::Source(definition) => {
            e.u8(3);
            e.record(definition);
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
impl StableEncode for ReturnGoalObservation {
    const DOMAIN: &'static str = "lrr/v1/declared-return-goal-observation";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.digest(self.signature);
        match &self.goal {
            ReturnTypeGoal::Ready(goal) => {
                e.u8(0);
                e.record(goal);
            }
            ReturnTypeGoal::NeedName(name) => {
                e.u8(1);
                encode_name(name, e);
            }
            ReturnTypeGoal::NeedAlias(alias) => {
                e.u8(2);
                match alias {
                    AliasTarget::Type(source) => {
                        e.u8(0);
                        e.record(source);
                    }
                    AliasTarget::Nominal(definition) => {
                        e.u8(1);
                        e.digest(*definition);
                    }
                    AliasTarget::Str => e.u8(2),
                    AliasTarget::NeedProducer(producer) => {
                        e.u8(3);
                        e.digest(*producer);
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
                        e.sequence(target, |e, token| e.record(token));
                    }
                }
            }
        }
    }
}
fn classify(definition: Digest, signature: Digest, named: NamedTypePreparation) -> ReturnTypeGoal {
    let source = match named {
        NamedTypePreparation::Ready(proof) => proof.source().clone(),
        NamedTypePreparation::Parameter(key) => TyTerm::Param(key),
        NamedTypePreparation::Alias(AliasTarget::Type(source)) => source,
        NamedTypePreparation::Alias(other) => return ReturnTypeGoal::NeedAlias(other),
        NamedTypePreparation::Name(other) => return ReturnTypeGoal::NeedName(other),
    };
    ReturnTypeGoal::Ready(DeclaredReturnType {
        definition,
        signature,
        source,
    })
}
/// Pure future query provider. Actual parent/header input dependencies belong
/// to the existing memo framework. This leaf adds no DB key/provider hooks.
pub(crate) fn prepare_local_query(
    db: &mut Database,
    definition: Digest,
) -> QueryPoll<ReturnGoalObservation> {
    macro_rules! ready {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let selected = ready!(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(definition)));
    let QueryValue::FunctionSignature(signature) = selected.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("declared return signature query differs")));
    };
    if signature.definition != definition {
        return QueryPoll::Ready(Err(fault("declared return signature owner differs")));
    }
    let owner = match db.input(&InputKey::LocalDeclarationOwner(definition)) {
        Some(owner) => owner,
        None => {
            // FunctionSignature already checked the actual immutable Fn and
            // absence of ret_type. Explicit module Type occurrences still
            // require their declaration-environment/header occurrence join.
            let goal = if signature.result.is_empty() {
                ReturnTypeGoal::Ready(DeclaredReturnType {
                    definition,
                    signature: selected.fingerprint,
                    source: TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit),
                })
            } else {
                ReturnTypeGoal::NeedName(SignatureName::NeedOwnerContext(signature.scope.clone()))
            };
            let result = ReturnGoalObservation {
                definition,
                signature: selected.fingerprint,
                goal,
            };
            if result.estimated_bytes() > 32 * 1024 * 1024 {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            }
            return QueryPoll::Ready(Ok(result));
        }
    };
    let InputValue::LocalDeclarationOwner(owner) = owner.as_ref() else {
        return QueryPoll::Ready(Err(fault("declared return owner input differs")));
    };
    let parent = ready!(db.fetch(&SyntaxProvider, QueryKey::SourceLocalPlan(owner.clone())));
    let QueryValue::SourceLocalPlan(plan) = parent.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("declared return owning plan differs")));
    };
    let named = ready!(plan.prepare_return_type(db, signature));
    let result = ReturnGoalObservation {
        definition,
        signature: selected.fingerprint,
        goal: classify(definition, selected.fingerprint, named),
    };
    if result.estimated_bytes() > 32 * 1024 * 1024 {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    QueryPoll::Ready(Ok(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expand::source_discovery::{SourceDiscovery, SourceDiscoveryStep},
        frontend::{
            callables::CallSelection, compile_input_catalog::CompileInputCatalog,
            source_locals::PreparedSourceLocalPlan,
        },
        ir::op::PrimitiveType as P,
        source::manifest::{ItemKind, Namespace},
        traits::term::ParamKey,
    };
    use std::sync::Arc;
    const SUFFIX: &str = include_str!(
        "../../../../fixtures/upstream/rustc-checkout/tests/ui/numbers-arithmetic/integer-literal-suffix-inference.rs"
    );
    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("declared return source: {error:?}"),
            QueryPoll::NeedInput(input) => panic!("declared return demand: {input:?}"),
        }
    }
    fn world(
        text: &str,
        edition: u16,
        retained: bool,
        entry: &str,
    ) -> (Database, Arc<PreparedSourceLocalPlan>, Digest) {
        let (provider, _, inputs) =
            crate::expand::source_declarative::assembly::tests::world(text, edition);
        let source = provider.module_source().share();
        let mut db = Database::default();
        inputs.publish(&mut db).unwrap();
        db.begin_expansion_epoch(1).unwrap();
        db.trace.detailed = true;
        let mut discovery = SourceDiscovery::new(
            vec![source.share()],
            source.outline().root_scope.clone(),
            Namespace::Value,
            Digest::of_bytes(b"declared return source control"),
            1,
        )
        .unwrap()
        .with_declarative_compiler(
            Digest::of_bytes(b"declared return compiler control"),
            crate::expand::source_declarative::engine_version(),
            Arc::new(CompileInputCatalog::standalone(&source.outline().profile).unwrap()),
        )
        .unwrap();
        loop {
            match discovery.poll().unwrap() {
                SourceDiscoveryStep::NeedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::NeedGeneratedExpansion { request } => discovery
                    .admit_expansion(Arc::new(request.expand().unwrap()))
                    .unwrap(),
                SourceDiscoveryStep::Sealed(_) => {
                    discovery.publish(&mut db).unwrap();
                    break;
                }
                _ => panic!("actual declared-return discovery did not complete"),
            }
        }
        let CallSelection::Source(selected) = ready(discovery.entry_function(&mut db, entry))
        else {
            panic!("actual source entry is not a function")
        };
        let mut body = selected.body().unwrap().key();
        if retained {
            let prepared = discovery
                .prepare_body_syntax(source.selection(), selected.identity())
                .unwrap();
            let input = ready(crate::db::retained_syntax::RetainedSyntaxInput::prepare(
                &mut db,
                prepared,
                discovery.key().clone(),
            ));
            ready(db.publish_retained_syntax(Arc::clone(&input)));
            body = ready(db.publish_retained_body(input, selected.identity()));
        }
        let plan = ready(db.prepare_source_local_plan(&discovery, &[&source], &body));
        (db, plan, selected.identity())
    }
    fn definition(plan: &PreparedSourceLocalPlan, name: &str) -> Digest {
        plan.context()
            .facts()
            .items
            .iter()
            .find(|row| row.item.def_key.declared_name == name)
            .unwrap()
            .item
            .def_key
            .fingerprint()
    }
    fn no_child_code(db: &Database, start: usize) {
        assert!(
            !db.trace.events[start..].iter().any(|event| matches!(
                event.key,
                QueryKey::BodyStructure(_)
                    | QueryKey::Thunk(_)
                    | QueryKey::PatternPlan(_)
                    | QueryKey::Constant(_)
            )),
            "declared result entered executable preparation"
        );
    }
    #[test]
    fn complete_suffix_result_headers_share_source_type_service_all_editions_and_origins() {
        let expected = [
            ("id_i8", P::I8),
            ("id_i16", P::I16),
            ("id_i32", P::I32),
            ("id_i64", P::I64),
            ("id_uint", P::Usize),
            ("id_u8", P::U8),
            ("id_u16", P::U16),
            ("id_u32", P::U32),
            ("id_u64", P::U64),
        ];
        for edition in [2015, 2018, 2021, 2024] {
            for mode in 0..4 {
                let text = match mode {
                    0 | 1 => SUFFIX.to_owned(),
                    2 => format!("macro_rules! emit{{()=>{{{SUFFIX}}}}}emit!();"),
                    _ => format!(
                        "macro_rules! emit{{()=>{{{SUFFIX}}}}}macro_rules! wrap{{()=>{{emit!();}}}}wrap!();"
                    ),
                };
                let (mut db, plan, main) = world(&text, edition, mode == 1, "main");
                assert_eq!(
                    plan.context()
                        .facts()
                        .items
                        .iter()
                        .filter(|row| row.item.def_key.item_kind == ItemKind::Function)
                        .count(),
                    expected.len()
                );
                let start = db.trace.events.len();
                for (name, primitive) in expected {
                    let id = definition(&plan, name);
                    let goal = ready(prepare_local_query(&mut db, id));
                    let ReturnTypeGoal::Ready(result) = goal.goal() else {
                        panic!("actual unchanged result pending: {:?}", goal.goal())
                    };
                    result.require(id, goal.signature_identity()).unwrap();
                    assert_eq!(result.source(), &TyTerm::Primitive(primitive));
                    assert!(goal.matches(id));
                    assert!(!goal.matches(Digest([82; 32])));
                }
                let main_goal = ready(prepare_local_query(&mut db, main));
                assert!(matches!(main_goal.goal(), ReturnTypeGoal::Ready(result)
                    if result.source() == &TyTerm::Primitive(P::Unit)));
                no_child_code(&db, start);
                if mode >= 2 {
                    let id = definition(&plan, "id_i8");
                    db.begin_expansion_epoch(2).unwrap();
                    assert!(matches!(
                        prepare_local_query(&mut db, id),
                        QueryPoll::Ready(Err(_))
                    ));
                }
            }
        }
    }
    #[test]
    fn result_goal_uses_actual_generic_and_keeps_nested_type_hygiene_pending() {
        // Append an actual named source owner outside the complete unchanged
        // upstream main. It is selected by SourceDiscovery, never synthesized.
        let text = format!(
            "{SUFFIX}\nfn return_type_control(){{fn own<T>(x:T)->T{{x}} fn nested<T>(x:T)->(T,){{(x,)}} type Named=u16; fn alias()->Named{{0}}}}\n"
        );
        let (mut db, plan, _) = world(&text, 2024, false, "return_type_control");
        let own = definition(&plan, "own");
        let nested = definition(&plan, "nested");
        let alias = definition(&plan, "alias");
        let start = db.trace.events.len();
        let own_goal = ready(prepare_local_query(&mut db, own));
        assert!(matches!(own_goal.goal(), ReturnTypeGoal::Ready(result)
            if result.source() == &TyTerm::Param(ParamKey { definition: own, ordinal: 0 })));
        let nested_goal = ready(prepare_local_query(&mut db, nested));
        assert!(matches!(
            nested_goal.goal(),
            ReturnTypeGoal::NeedName(SignatureName::NeedOwnerContext(_))
        ));
        match prepare_local_query(&mut db, alias) {
            QueryPoll::Ready(Err(error)) => {
                assert_eq!(error.code, "LRR-E-TYPE-CONTEXT");
                assert!(error.message.contains("AliasContext"), "{error:?}");
            }
            _ => panic!("unjoined local alias return authority was certified"),
        }
        no_child_code(&db, start);
    }
    #[test]
    fn memo_retains_one_return_plan_and_guard_until_relevant_epoch_changes() {
        let (mut db, plan, _) = world(SUFFIX, 2024, false, "main");
        let id = definition(&plan, "id_i8");
        let key = QueryKey::PreparedReturnPlan(id);
        let first = ready(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::PreparedReturnPlan(first_plan) = first.value.as_ref() else {
            panic!()
        };
        let second = ready(db.fetch(&SyntaxProvider, key));
        let QueryValue::PreparedReturnPlan(second_plan) = second.value.as_ref() else {
            panic!()
        };
        assert!(Arc::ptr_eq(&first.value, &second.value));
        assert!(Arc::ptr_eq(first_plan.guard(), second_plan.guard()));
        first_plan.guard().require_current(&db).unwrap();
        assert!(
            matches!(first_plan.observation().goal(), ReturnTypeGoal::Ready(proof)
            if proof.source() == &TyTerm::Primitive(P::I8))
        );
        db.begin_expansion_epoch(2).unwrap();
        assert!(first_plan.guard().require_current(&db).is_err());
    }
    #[test]
    fn result_goal_refuses_substituted_signature_and_revoked_source() {
        let (mut db, plan, _) = world(SUFFIX, 2024, false, "main");
        let id = definition(&plan, "id_i8");
        let selected = ready(db.fetch(&SyntaxProvider, QueryKey::FunctionSignature(id)));
        let QueryValue::FunctionSignature(signature) = selected.value.as_ref() else {
            panic!()
        };
        let mut wrong = signature.as_ref().clone();
        wrong.result[0].text.push_str("_wrong");
        let start = db.trace.events.len();
        assert!(matches!(
            plan.prepare_return_type(&mut db, &wrong),
            QueryPoll::Ready(Err(_))
        ));
        ready(prepare_local_query(&mut db, id));
        no_child_code(&db, start);
        db.publish_inputs(Default::default()).unwrap();
        assert!(matches!(
            prepare_local_query(&mut db, id),
            QueryPoll::Ready(Err(_))
        ));
    }
}

/// Immutable declared result and its relevant input closure, prepared once and
/// shared by reached call contracts. Facts alone never grant execution.
#[derive(Debug, Clone)]
pub struct PreparedReturnPlan {
    observation: ReturnGoalObservation,
    observation_fingerprint: Digest,
    guard: std::sync::Arc<crate::db::memo::PreparedInputGuard>,
}
impl PreparedReturnPlan {
    pub(crate) fn definition(&self) -> Digest {
        self.observation.definition
    }
    pub(crate) fn observation(&self) -> &ReturnGoalObservation {
        &self.observation
    }
    pub(crate) fn observation_fingerprint(&self) -> Digest {
        self.observation_fingerprint
    }
    pub(crate) fn guard(&self) -> &std::sync::Arc<crate::db::memo::PreparedInputGuard> {
        &self.guard
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.observation
            .estimated_bytes()
            .checked_add(self.guard.metadata_bytes())
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<Self>() + 2 * std::mem::size_of::<usize>())
            })
            .unwrap_or(usize::MAX)
    }
}
impl StableEncode for PreparedReturnPlan {
    const DOMAIN: &'static str = "lrr/v1/prepared-declared-return-plan";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.observation_fingerprint);
        e.record(&self.observation);
        e.digest(self.guard.fingerprint());
    }
}
pub(crate) fn prepare_plan(db: &mut Database, definition: Digest) -> QueryPoll<PreparedReturnPlan> {
    macro_rules! ready {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let key = QueryKey::DeclaredReturnGoal(definition);
    let value = ready!(db.fetch(&SyntaxProvider, key.clone()));
    let QueryValue::DeclaredReturnGoal(observation) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("prepared return goal variant differs")));
    };
    if !value.cacheable || !observation.matches(definition) {
        return QueryPoll::Ready(Err(fault(
            "prepared return goal has no retained source proof",
        )));
    }
    let guard = std::sync::Arc::new(ready!(db.prepare_input_guard(
        &SyntaxProvider,
        &key,
        value.fingerprint
    )));
    let plan = PreparedReturnPlan {
        observation: observation.as_ref().clone(),
        observation_fingerprint: value.fingerprint,
        guard,
    };
    if plan.metadata_bytes() > 32 * 1024 * 1024 {
        return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
    }
    QueryPoll::Ready(Ok(plan))
}
