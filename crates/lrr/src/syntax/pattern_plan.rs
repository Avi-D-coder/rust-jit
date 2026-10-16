//! One immutable ordinary-source plan per actual match or condition owner.
//! Query dependencies establish currentness; installed operations never call here.
use super::{condition_ranges, error, literal_match};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    frontend::scalar_pattern_binding::ScalarPatternBindingAdmission,
    ir::{
        pc::{NodeId, PcKey, Phase},
        recipe::VerifiedRecipe,
    },
    syntax::{
        body_index::{BodyIndex, ExpressionKind, NodeRole},
        outline::BodyHandle,
        parse::ParsedFile,
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

#[derive(Debug)]
enum Prepared {
    Match(Option<literal_match::Plan>),
    Condition(Result<condition_ranges::Plan, Diagnostic>),
    Refused(Diagnostic),
    Disabled(Box<VerifiedRecipe>),
}

#[derive(Debug)]
pub struct PatternSourcePlan {
    owner: PcKey,
    parsed: Arc<ParsedFile>,
    index: Arc<BodyIndex>,
    body: BodyHandle,
    prepared: Prepared,
    fingerprint: Digest,
    bytes: usize,
}
impl PatternSourcePlan {
    pub(crate) fn owner(&self) -> &PcKey {
        &self.owner
    }
    pub(crate) fn parsed(&self) -> &Arc<ParsedFile> {
        &self.parsed
    }
    pub(crate) fn index(&self) -> &Arc<BodyIndex> {
        &self.index
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn requires_binding_admission(&self, key: &PcKey) -> bool {
        matches!(&self.prepared, Prepared::Match(Some(plan)) if plan.requires_binding_admission(key))
    }

    /// Indexed ownership only. An expression inside a match result does not
    /// become a pattern phase merely because it has a match ancestor.
    pub(crate) fn owner_for(index: &BodyIndex, key: &PcKey) -> Result<Option<PcKey>, Diagnostic> {
        if &key.body != index.body() {
            return Err(error("pattern phase body differs"));
        }
        let mut node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("pattern phase node absent"))?;
        match node.role {
            NodeRole::Expression(ExpressionKind::Match) => {
                return index.pc(node.id, Phase::Entry).map(Some);
            }
            NodeRole::Expression(ExpressionKind::Other) if node.clause_scope.is_some() => {
                return index.pc(node.id, Phase::Entry).map(Some);
            }
            NodeRole::Pattern | NodeRole::MatchArm => {}
            _ => return Ok(None),
        }
        for _ in 0..4096 {
            let Some(parent) = node.parent else {
                return Ok(None);
            };
            node = index
                .nodes()
                .get(parent.0 as usize)
                .ok_or_else(|| error("pattern parent absent"))?;
            match node.role {
                NodeRole::Expression(ExpressionKind::Match) => {
                    return index.pc(node.id, Phase::Entry).map(Some);
                }
                // MatchArmList and other real syntax wrappers are indexed too.
                // The initial role check above keeps result/source expressions
                // from acquiring pattern ownership through this ancestry walk.
                NodeRole::Pattern | NodeRole::MatchArm | NodeRole::Syntax => {}
                _ => return Ok(None),
            }
        }
        Err(error(
            "pattern ownership nesting exceeds preparation budget",
        ))
    }

    pub(crate) fn lower(
        &self,
        key: &PcKey,
        admission: Option<&ScalarPatternBindingAdmission>,
    ) -> Result<VerifiedRecipe, Diagnostic> {
        if Self::owner_for(&self.index, key)?.as_ref() != Some(&self.owner) {
            return Err(error("retained pattern phase belongs to another owner"));
        }
        self.index
            .pc(NodeId(key.node), Phase::from_code(key.phase))?;
        match &self.prepared {
            Prepared::Match(plan) => literal_match::prepare_inner(
                &self.parsed,
                &self.body,
                &self.index,
                key,
                plan.as_ref(),
                admission,
            ),
            Prepared::Condition(plan) => {
                if admission.is_some() {
                    return Err(error("condition cannot consume match namespace admission"));
                }
                condition_ranges::prepare_inner(
                    &self.parsed,
                    &self.body,
                    &self.index,
                    key,
                    plan.as_ref().map_err(Clone::clone),
                )
            }
            Prepared::Disabled(recipe) if key == &self.owner => Ok(recipe.as_ref().clone()),
            Prepared::Disabled(_) => {
                self.refuse(key, "disabled pattern owner has no evaluated continuation")
            }
            Prepared::Refused(diagnostic) => self.refuse(key, &diagnostic.message),
        }
    }
    fn refuse(&self, key: &PcKey, message: &str) -> Result<VerifiedRecipe, Diagnostic> {
        crate::frontend::thunks::unsupported(
            &self.index,
            key.clone(),
            message.into(),
            self.index.pc(NodeId(key.node), Phase::Exit)?,
        )
    }
}

impl ParsedFile {
    pub(crate) fn prepare_pattern_plan(
        self: &Arc<Self>,
        body: &BodyHandle,
        index: Arc<BodyIndex>,
        owner: &PcKey,
    ) -> Result<PatternSourcePlan, Diagnostic> {
        if owner.phase != 0 || Self::check_pattern_identity(self, body, &index, owner).is_err() {
            return Err(error("pattern plan owning source/body/profile differs"));
        }
        if PatternSourcePlan::owner_for(&index, owner)?.as_ref() != Some(owner) {
            return Err(error("pattern plan key is not its exact owning entry"));
        }
        // Preserve the ordinary entry admission when this match expression is
        // itself an enclosing match's boolean guard. The cached provider joins
        // before generic thunk dispatch, so preparation owns this same check.
        if index.nodes()[owner.node as usize]
            .boolean_scope
            .is_some_and(|scope| {
                index.scopes()[scope.0 as usize].kind
                    == crate::syntax::body_index::ScopeKind::MatchGuard
            })
        {
            self.require_boolean_match_guard(body, &index, NodeId(owner.node))?;
        }
        self.entered_body_root(body)?;
        let syntax = self
            .entered_node_syntax(body, &index, NodeId(owner.node))?
            .ok_or_else(|| error("pattern owner syntax attachment absent"))?;
        let (enabled, attributes) = super::statement_cfg(&syntax, index.cfg())?;
        let prepared = if !enabled {
            // The existing generic disabled-source route exits before pattern
            // lowering. Retain its recipe once rather than duplicating semantics.
            Prepared::Disabled(Box::new(self.initial_thunk(body, &index, owner)?))
        } else if attributes {
            Prepared::Refused(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "entered pattern owner attributes require macro integration".into(),
            ))
        } else if let Some(expression) = ast::MatchExpr::cast(syntax.clone()) {
            Prepared::Match(literal_match::plan(
                self,
                &index,
                body,
                expression.syntax(),
            )?)
        } else if let Some(expression) = ast::LetExpr::cast(syntax) {
            Prepared::Condition(condition_ranges::plan(
                self,
                body,
                &index,
                owner,
                &expression,
            ))
        } else {
            return Err(error("pattern owner syntax role differs"));
        };
        let payload = match &prepared {
            Prepared::Match(Some(plan)) => plan.metadata_bytes(),
            Prepared::Condition(Err(diagnostic)) | Prepared::Refused(diagnostic) => diagnostic
                .heap_bytes()
                .ok_or_else(|| error("pattern diagnostic size overflow"))?,
            Prepared::Disabled(recipe) => recipe.metadata_bytes(),
            // A successful condition plan contains only fixed typed rows and
            // scalar endpoint ranges; its complete inline payload is already
            // charged by size_of::<PatternSourcePlan>() below. There is no Vec,
            // String, AST handle or uncharged external allocation in this arm.
            Prepared::Condition(Ok(_)) | Prepared::Match(None) => 0,
        };
        let bytes = std::mem::size_of::<PatternSourcePlan>()
            .checked_add(payload)
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(|| error("retained pattern plan metadata budget"))?;
        let mut encoder = StableEncoder::new("lrr/v1/ordinary-pattern-source-plan");
        encoder.record(owner);
        encoder.digest(self.content);
        encoder.record(&self.profile);
        encoder.digest(index.fingerprint());
        Ok(PatternSourcePlan {
            owner: owner.clone(),
            parsed: Arc::clone(self),
            index,
            body: body.clone(),
            prepared,
            fingerprint: encoder.finish(),
            bytes,
        })
    }
    fn check_pattern_identity(
        &self,
        body: &BodyHandle,
        index: &BodyIndex,
        owner: &PcKey,
    ) -> Result<(), Diagnostic> {
        if body.key() != owner.body
            || index.body() != &owner.body
            || index.profile() != &self.profile
        {
            return Err(error("pattern source identity differs"));
        }
        Ok(())
    }
}
