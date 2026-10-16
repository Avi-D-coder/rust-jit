//! One immutable ordinary-source plan per actual match or condition owner.
//! Query dependencies establish currentness; installed operations never call here.
use super::{
    condition_ranges, error, literal_match,
    pattern_product::{PatternProductPlan, ProductRoot},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
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
    staged_products: Result<Vec<ProductRoot>, Diagnostic>,
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

    /// Actual owned handles, once per retained root. No allocation or clones;
    /// the memo owner counts each handle and charges distinct Product payloads.
    /// These source graphs do not select the production lowering route yet.
    pub(crate) fn products(&self) -> impl Iterator<Item = &Arc<PatternProductPlan>> {
        self.staged_products
            .iter()
            .flat_map(|roots| roots.iter())
            .filter_map(|root| root.product.as_ref().ok())
    }

    /// Source preparation may refuse one root without requiring all other arm
    /// patterns or any cold result body to have an executable shape.
    pub(crate) fn staged_product(
        &self,
        root: NodeId,
    ) -> Option<Result<&Arc<PatternProductPlan>, &Diagnostic>> {
        let key = self.index.pc(root, Phase::Entry).ok()?;
        if Self::staged_owner_for(&self.index, &key).ok()?.as_ref() != Some(&self.owner) {
            return None;
        }
        let roots = match &self.staged_products {
            Ok(roots) => roots,
            Err(error) => return Some(Err(error)),
        };
        let position = roots.binary_search_by_key(&root, |row| row.root).ok()?;
        Some(roots[position].product.as_ref())
    }

    /// Deliberately separate from live owner_for until the complete decision /
    /// cached binding authority / memo ownership join switches atomically.
    /// Only pattern descendants can acquire a condition owner through ancestry;
    /// scrutinee, guard and result expressions keep ordinary expression routing.
    pub(crate) fn staged_owner_for(
        index: &BodyIndex,
        key: &PcKey,
    ) -> Result<Option<PcKey>, Diagnostic> {
        if let Some(owner) = Self::owner_for(index, key)? {
            return Ok(Some(owner));
        }
        if &key.body != index.body() {
            return Err(error("staged pattern body differs"));
        }
        let mut node = index
            .nodes()
            .get(key.node as usize)
            .ok_or_else(|| error("staged pattern node absent"))?;
        if node.role != NodeRole::Pattern {
            return Ok(None);
        }
        for _ in 0..4096 {
            let Some(parent) = node.parent else {
                return Ok(None);
            };
            node = index
                .nodes()
                .get(parent.0 as usize)
                .ok_or_else(|| error("staged pattern parent absent"))?;
            match node.role {
                NodeRole::Expression(ExpressionKind::Other) if node.clause_scope.is_some() => {
                    return index.pc(node.id, Phase::Entry).map(Some);
                }
                NodeRole::Pattern | NodeRole::Syntax => {}
                _ => return Ok(None),
            }
        }
        Err(error(
            "staged condition pattern nesting exceeds preparation budget",
        ))
    }

    pub(crate) fn requires_binding_admission(&self, key: &PcKey) -> bool {
        match &self.prepared {
            Prepared::Match(Some(plan)) => plan.requires_binding_admission(key),
            Prepared::Condition(Ok(plan)) => plan.requires_binding_admission(key),
            _ => false,
        }
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
            Prepared::Condition(plan) => condition_ranges::prepare_inner(
                &self.parsed,
                &self.body,
                &self.index,
                key,
                plan.as_ref().map_err(Clone::clone),
                admission,
            ),
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
        } else if let Some(expression) = ast::LetExpr::cast(syntax.clone()) {
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
        let base_bytes = std::mem::size_of::<PatternSourcePlan>()
            .checked_add(payload)
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(|| error("retained pattern plan metadata budget"))?;
        // This is staged source metadata, not a production dispatcher. Retain
        // deferred source refusals instead of making them executable admission.
        let staged_products = if enabled && !attributes {
            PatternProductPlan::prepare_owner(
                self,
                body,
                &index,
                &syntax,
                32 * 1024 * 1024 - base_bytes,
            )
        } else {
            Ok(Vec::new())
        };
        let staged_bytes = match &staged_products {
            Ok(roots) => {
                let mut bytes = roots
                    .capacity()
                    .checked_mul(std::mem::size_of::<ProductRoot>())
                    .ok_or_else(|| error("retained product root metadata overflow"))?;
                for root in roots {
                    bytes = bytes
                        .checked_add(
                            root.heap_bytes()
                                .ok_or_else(|| error("retained product error size overflow"))?,
                        )
                        .ok_or_else(|| error("retained product root metadata overflow"))?;
                }
                bytes
            }
            Err(error) => error
                .heap_bytes()
                .ok_or_else(|| super::error("retained staged product error size overflow"))?,
        };
        // Exclusive allocation accounting. Product allocations/control blocks
        // and their shared index roots are charged separately by the memo owner.
        let bytes = base_bytes
            .checked_add(staged_bytes)
            .filter(|bytes| *bytes <= 32 * 1024 * 1024)
            .ok_or_else(|| error("retained pattern plan metadata budget"))?;
        let mut encoder = StableEncoder::new("lrr/v2/ordinary-pattern-source-plan");
        encoder.record(owner);
        encoder.digest(self.content);
        encoder.record(&self.profile);
        encoder.digest(index.semantic_identity());
        match &staged_products {
            Ok(roots) => {
                encoder.u8(0);
                encoder.sequence(roots, |e, row| row.encode(e));
            }
            Err(error) => {
                encoder.u8(1);
                encoder.string(error.code);
                encoder.string(&error.message);
            }
        }
        Ok(PatternSourcePlan {
            owner: owner.clone(),
            parsed: Arc::clone(self),
            index,
            body: body.clone(),
            prepared,
            staged_products,
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
