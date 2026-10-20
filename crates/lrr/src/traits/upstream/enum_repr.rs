//! Actual source enum representation, prepared before shared solver entry.
use super::{
    context::Context, facts::SourceFacts, identity::DefinitionId, model::Ty,
    prepared::DefinitionReceipt,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    ir::op::PrimitiveType,
};
use rustc_type_ir::{self as ty};
use std::sync::Arc;
#[derive(Debug)]
pub(crate) struct PreparedEnumDiscriminant {
    definition: Digest,
    receipt: DefinitionReceipt,
    schema: Arc<QueryValue>,
    integer: PrimitiveType,
    guard: PreparedInputGuard,
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
impl PreparedEnumDiscriminant {
    pub(crate) fn prepare(db: &mut Database, definition: Digest) -> QueryPoll<Option<Self>> {
        let Some(receipt) = ready!(DefinitionReceipt::observe(db, definition)) else {
            return QueryPoll::Ready(Ok(None));
        };
        match receipt.is_enum() {
            Ok(true) => {}
            Ok(false) => return QueryPoll::Ready(Ok(None)),
            Err(e) => return QueryPoll::Ready(Err(e)),
        }
        let key = QueryKey::TypeSchema(definition);
        let captured = ready!(db.fetch(&SyntaxProvider, key.clone()));
        let QueryValue::TypeSchema(schema) = captured.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("enum repr schema variant differs")));
        };
        if !captured.cacheable || !receipt.matches_enum_schema(schema) {
            return QueryPoll::Ready(Err(fault("enum repr source schema differs")));
        }
        let integer = match crate::frontend::declaration_repr::declared_repr(schema, "enum") {
            Ok(repr) => repr.integer.unwrap_or(PrimitiveType::Isize),
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &key, captured.fingerprint));
        QueryPoll::Ready(Ok(Some(Self {
            definition,
            receipt,
            schema: captured.value,
            integer,
            guard,
        })))
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
    #[cfg(test)]
    pub(crate) fn integer(&self) -> PrimitiveType {
        self.integer
    }
    pub(super) fn matches(&self, receipt: &DefinitionReceipt) -> bool {
        matches!(self.schema.as_ref(),QueryValue::TypeSchema(schema) if self.definition==schema.definition && self.receipt.same_source_metadata(receipt) && receipt.matches_enum_schema(schema))
    }
    pub(super) fn ty<'cx, F: SourceFacts<'cx>>(
        &self,
        cx: Context<'cx, F>,
        id: DefinitionId,
    ) -> Ty<'cx, F> {
        assert_eq!(
            id.source(),
            self.definition,
            "enum repr belongs to another source definition"
        );
        use PrimitiveType as P;
        match self.integer {
            P::I8 => cx.ty(ty::TyKind::Int(ty::IntTy::I8)),
            P::I16 => cx.ty(ty::TyKind::Int(ty::IntTy::I16)),
            P::I32 => cx.ty(ty::TyKind::Int(ty::IntTy::I32)),
            P::I64 => cx.ty(ty::TyKind::Int(ty::IntTy::I64)),
            P::I128 => cx.ty(ty::TyKind::Int(ty::IntTy::I128)),
            P::Isize => cx.ty(ty::TyKind::Int(ty::IntTy::Isize)),
            P::U8 => cx.ty(ty::TyKind::Uint(ty::UintTy::U8)),
            P::U16 => cx.ty(ty::TyKind::Uint(ty::UintTy::U16)),
            P::U32 => cx.ty(ty::TyKind::Uint(ty::UintTy::U32)),
            P::U64 => cx.ty(ty::TyKind::Uint(ty::UintTy::U64)),
            P::U128 => cx.ty(ty::TyKind::Uint(ty::UintTy::U128)),
            P::Usize => cx.ty(ty::TyKind::Uint(ty::UintTy::Usize)),
            _ => unreachable!("repr integer came from captured declaration hints"),
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}

/// Actual source records become shared integer Types without inventing full ADT
/// facts, language items, root readiness or a trait answer.
#[cfg(test)]
pub(crate) fn check_source_context(
    db: &mut Database,
    seal: &crate::db::seals::SealKey,
    plans: &[PreparedEnumDiscriminant],
) -> Result<(), Diagnostic> {
    use crate::db::{
        dependency::{InputKey, InputValue},
        // Configuration is the actual captured root's solver policy.
    };
    use crate::{
        core::digest::StableEncode, traits::upstream::configuration::SolverConfigurationKey,
    };
    use rustc_type_ir::inherent::IntoKind;
    let input = db
        .input(&InputKey::DiscoverySeal(seal.clone()))
        .ok_or_else(|| fault("actual enum catalog absent"))?;
    let InputValue::DiscoverySeal(catalog) = input.as_ref() else {
        return Err(fault("actual enum catalog differs"));
    };
    let root = catalog
        .impl_catalog
        .as_ref()
        .ok_or_else(|| fault("actual enum root absent"))?
        .root();
    let key = QueryKey::SolverConfiguration(Box::new(SolverConfigurationKey::from_source(root)));
    let captured = match db.fetch(&SyntaxProvider, key) {
        QueryPoll::Ready(Ok(v)) => v.value,
        QueryPoll::Ready(Err(e)) => return Err(e),
        QueryPoll::NeedInput(_) => return Err(fault("enum configuration needs input")),
    };
    let QueryValue::SolverConfiguration(configuration) = captured.as_ref() else {
        return Err(fault("enum configuration differs"));
    };
    for plan in plans {
        plan.require_current(db)?;
    }
    let local = root.outline.crate_key;
    configuration.with_source_context(db, Arc::clone(&input), None, |cx, _| {
        for plan in plans {
            let id = plan
                .receipt
                .definition_id(catalog.seal.fingerprint(), local);
            let encoded = plan.ty(cx, id);
            let agrees = match (plan.integer, encoded.kind()) {
                (PrimitiveType::I8, ty::TyKind::Int(ty::IntTy::I8))
                | (PrimitiveType::I128, ty::TyKind::Int(ty::IntTy::I128))
                | (PrimitiveType::Isize, ty::TyKind::Int(ty::IntTy::Isize))
                | (PrimitiveType::U128, ty::TyKind::Uint(ty::UintTy::U128))
                | (PrimitiveType::Usize, ty::TyKind::Uint(ty::UintTy::Usize))
                | (PrimitiveType::U64, ty::TyKind::Uint(ty::UintTy::U64)) => true,
                _ => false,
            };
            assert!(
                agrees,
                "actual enum repr source integer differs from shared Type"
            );
            assert!(plan.matches(&plan.receipt));
        }
        assert!(
            cx.services().ready(cx).is_err(),
            "enum source records grant no full ADT facts or solver root readiness"
        );
    })
}
