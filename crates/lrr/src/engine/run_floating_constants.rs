//! Bounded run-owned source proofs. No guest value is produced here.
use super::super::runtime_macros::floating_constants::FloatingFrameSite;
use super::*;
use crate::{
    db::dependency::{InputKey, InputValue},
    frontend::{
        call_contracts::primitive_runtime_id, primitive_constants::FloatingConstantRecipe,
        runtime_macro_plan::RuntimeNoItemProof,
    },
    traits::term::TyTerm,
};
impl RunMachine {
    pub(super) fn retain_no_item_proof(
        &mut self,
        proof: RuntimeNoItemProof,
    ) -> Result<(), Diagnostic> {
        let range = proof.invocation();
        let key = (proof.body().clone(), range.start, range.end);
        if let Some(existing) = self.runtime_no_item_proofs.get(&key) {
            if existing != &proof {
                return Err(call_fault("conflicting no-item certificate"));
            }
            return Ok(());
        }
        let bytes = proof
            .allocated_bytes()
            .and_then(|n| n.checked_add(96))
            .and_then(|n| self.runtime_no_item_bytes.checked_add(n))
            .filter(|n| *n <= 4 * 1024 * 1024 && self.runtime_no_item_proofs.len() < 4096)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "runtime no-item certificate budget".into(),
                )
            })?;
        self.runtime_no_item_proofs.insert(key, proof);
        self.runtime_no_item_bytes = bytes;
        Ok(())
    }
    pub(super) fn bootstrap_no_item_proofs(
        &mut self,
        db: &mut Database,
        key: &crate::db::seals::SealKey,
        seal_fingerprint: crate::core::digest::Digest,
    ) -> Result<(), Diagnostic> {
        let input = db
            .input(&InputKey::DiscoverySeal(key.clone()))
            .ok_or_else(|| call_fault("matched discovery input absent"))?;
        let InputValue::DiscoverySeal(snapshot) = input.as_ref() else {
            return Err(call_fault("matched discovery input variant differs"));
        };
        if snapshot.seal.key() != *key || snapshot.seal.fingerprint() != seal_fingerprint {
            return Err(call_fault("matched discovery input certificate differs"));
        }
        if snapshot.runtime_no_item_proofs.len() > 4096 {
            return Err(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "sealed no-item certificate count".into(),
            ));
        }
        let total = snapshot
            .runtime_no_item_proofs
            .iter()
            .try_fold(self.runtime_no_item_bytes, |bytes, (_, proof)| {
                proof
                    .allocated_bytes()
                    .and_then(|n| n.checked_add(96))
                    .and_then(|n| bytes.checked_add(n))
            })
            .filter(|bytes| *bytes <= 4 * 1024 * 1024);
        if total.is_none() {
            return Err(Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "sealed no-item certificate copy budget".into(),
            ));
        }
        for (_, proof) in &snapshot.runtime_no_item_proofs {
            self.retain_no_item_proof(proof.clone())?
        }
        Ok(())
    }
    fn floating_expected(&self) -> Result<Option<ArgumentType>, Diagnostic> {
        let node = self
            .frame
            .body
            .nodes()
            .get(self.frame.pc().node as usize)
            .ok_or_else(|| call_fault("floating frame node absent"))?;
        let Some(slot) = node.value else {
            return Ok(None);
        };
        let Some(source) = self.frame.source_view(slot)? else {
            return Ok(None);
        };
        let descriptor = match self.frame.hint(slot)? {
            Some(id) => id,
            None => match &source {
                TyTerm::Primitive(p) => primitive_runtime_id(*p)?,
                _ => {
                    return Err(Diagnostic::new(
                        "LRR-E-TYPE-CONTEXT",
                        "floating expected source lacks its runtime descriptor".into(),
                    ));
                }
            },
        };
        Ok(Some(ArgumentType { source, descriptor }))
    }
    pub(super) fn prepare_floating_recipe(
        &mut self,
        db: &mut Database,
        contexts: &[super::super::runtime_macros::RuntimeMacroContext<'_>],
    ) -> QueryPoll<Option<crate::ir::recipe::VerifiedRecipe>> {
        let pc = self.frame.pc().clone();
        if pc.phase != 0
            || !self
                .frame
                .body
                .nodes()
                .get(pc.node as usize)
                .is_some_and(|node| {
                    node.role
                        == crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Path,
                        )
                })
        {
            return QueryPoll::Ready(Ok(None));
        }
        if let Some(retained) = self.floating_plans.get(&pc) {
            return QueryPoll::Ready(Ok(Some(retained.recipe().clone())));
        }
        let Some(graph) = self.graph.as_ref() else {
            return QueryPoll::Ready(Ok(None));
        };
        let expected = || self.floating_expected();
        for context in contexts {
            match context.prepare_floating_constant(
                db,
                graph,
                FloatingFrameSite {
                    body: &self.frame.body,
                    pc: &pc,
                    expected: &expected,
                },
                self.runtime_no_item_proofs.values(),
            ) {
                QueryPoll::Ready(Ok(Some(plan))) => {
                    let bytes = plan
                        .recipe()
                        .metadata_bytes()
                        .checked_add(
                            std::mem::size_of::<FloatingConstantRecipe>()
                                + std::mem::size_of::<PcKey>()
                                + 128,
                        )
                        .and_then(|n| self.floating_plan_bytes.checked_add(n))
                        .filter(|n| *n <= 4 * 1024 * 1024 && self.floating_plans.len() < 4096);
                    let Some(bytes) = bytes else {
                        return QueryPoll::Ready(Err(Diagnostic::new(
                            "LRR-E-METADATA-LIMIT",
                            "floating source proof budget".into(),
                        )));
                    };
                    let recipe = plan.recipe().clone();
                    self.floating_plans.insert(pc, plan);
                    self.floating_plan_bytes = bytes;
                    return QueryPoll::Ready(Ok(Some(recipe)));
                }
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
            }
        }
        QueryPoll::Ready(Ok(None))
    }
    pub(super) fn revalidate_floating_recipe(
        &self,
        db: &mut Database,
        contexts: &[super::super::runtime_macros::RuntimeMacroContext<'_>],
        id: super::super::install::RecipeId,
    ) -> QueryPoll<()> {
        let recipe = match self.installed.get(id) {
            Ok(r) => r,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        if !recipe
            .recipe()
            .operations
            .iter()
            .skip(self.frame.resume_operation_index())
            .any(|op| matches!(op, Op::PrimitiveFloatingConstant { .. }))
        {
            return QueryPoll::Ready(Ok(()));
        }
        let pc = self.frame.pc();
        let Some(retained) = self.floating_plans.get(pc) else {
            return QueryPoll::Ready(Err(call_fault(
                "installed floating opcode lacks its run-owned source proof",
            )));
        };
        if retained.recipe().recipe().fingerprint() != recipe.recipe().fingerprint() {
            return QueryPoll::Ready(Err(call_fault(
                "installed floating recipe differs from its retained proof",
            )));
        }
        let Some(graph) = self.graph.as_ref() else {
            return QueryPoll::Ready(Err(call_fault("floating source namespace absent")));
        };
        let expected = || self.floating_expected();
        for context in contexts {
            match context.revalidate_floating_constant(
                db,
                graph,
                FloatingFrameSite {
                    body: &self.frame.body,
                    pc,
                    expected: &expected,
                },
                self.runtime_no_item_proofs.values(),
                retained,
            ) {
                QueryPoll::Ready(Ok(Some(()))) => return QueryPoll::Ready(Ok(())),
                QueryPoll::Ready(Ok(None)) => {}
                QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
            }
        }
        QueryPoll::Ready(Err(Diagnostic::new(
            "LRR-E-TYPE-CONTEXT",
            "floating recipe lacks its exact current source context".into(),
        )))
    }
}
