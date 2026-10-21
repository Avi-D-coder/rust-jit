//! Actual dispatcher observations for the closed interpreter operation family.
//! This module neither redirects host stdout nor infers output from source text.
//! No sink exists in this family. Exhaustive enums prevent a future external
//! operation/outcome from silently inheriting a no-external-effects witness.
use super::{RunPoll, TerminalExecution, TerminalFacts};
use crate::{
    core::diagnostic::Diagnostic,
    ir::{
        op::Op,
        recipe::{ControlRecipe, RecipeExit, VerifiedRecipe},
    },
    vm::step::StepOutcome,
};
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ObservedTerminal {
    Completed { unit: bool },
    GuestPanic { message: String },
}
#[derive(Debug, Clone, Default, serde::Serialize)]
pub(crate) struct CaptureFacts {
    vm_dispatches: u64,
    vm_outcomes: u64,
    classified_operation_candidates: u64,
    source_call_boundaries: u64,
    initializer_frames_entered: u64,
    effect_recipe_dispatch_attempts: u64,
    unsupported_operation_candidates: u64,
    typed_fault_exit_candidates: u64,
    observed_fault: bool,
    observed_suspension: bool,
    host_preparation_suspensions: u64,
    terminal: Option<ObservedTerminal>,
    cleanup_observed: bool,
    closed_no_external_effects: bool,
}
impl CaptureFacts {
    pub(crate) fn complete(&self) -> bool {
        self.closed_no_external_effects
    }
}
#[derive(Default)]
pub(super) struct RunCapture {
    facts: CaptureFacts,
}
fn add(value: &mut u64, amount: u64) -> Result<(), Diagnostic> {
    *value = value.checked_add(amount).ok_or_else(|| {
        Diagnostic::new(
            "LRR-E-METADATA-LIMIT",
            "runtime capture counter overflow".into(),
        )
    })?;
    Ok(())
}
impl RunCapture {
    pub(super) fn facts(&self) -> &CaptureFacts {
        &self.facts
    }
    /// Observation only: reached initializer entry never depends on this count.
    pub(super) fn initializer_frame_entered(&mut self) {
        self.facts.initializer_frames_entered =
            self.facts.initializer_frames_entered.saturating_add(1);
    }
    pub(super) fn before_vm_dispatch(
        &mut self,
        verified: &VerifiedRecipe,
    ) -> Result<(), Diagnostic> {
        self.observe_recipe(&verified.recipe().operations, &verified.recipe().exit)
    }
    fn observe_recipe(&mut self, operations: &[Op], exit: &RecipeExit) -> Result<(), Diagnostic> {
        // Fixed-size state; update a draft so overflow cannot publish a partial
        // observation. No extra guest charge or refusal substitutes VM behavior.
        let mut facts = self.facts.clone();
        add(&mut facts.vm_dispatches, 1)?;
        add(
            &mut facts.classified_operation_candidates,
            u64::try_from(operations.len()).map_err(|_| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "capture operation count does not fit u64".into(),
                )
            })?,
        )?;
        for op in operations {
            match op {
                Op::SourceConstant { .. }
                | Op::PrimitiveFloatingConstant { .. }
                | Op::ProjectField { .. }
                | Op::ProjectTuple { .. }
                | Op::ProjectIndex { .. }
                | Op::DerefPlace { .. }
                | Op::ReadPlace { .. }
                | Op::AssignPlace { .. }
                | Op::MakePlace { .. }
                | Op::ParameterPlace { .. }
                | Op::ValuePlace { .. }
                | Op::PromotedPlace { .. }
                | Op::PlaceTransfer { .. }
                | Op::Borrow { .. }
                | Op::RawBorrow { .. }
                | Op::ConstructStart { .. }
                | Op::ConstructField { .. }
                | Op::ConstructRest { .. }
                | Op::ConstructFinish { .. }
                | Op::ValuePath { .. }
                | Op::MakeClosure { .. }
                | Op::Assert { .. }
                | Op::FormatArguments { .. }
                | Op::MethodReceiver { .. }
                | Op::CalleeDeref { .. }
                | Op::ComparisonOperand { .. }
                | Op::IteratorCall { .. }
                | Op::CompoundAssignPlace { .. }
                | Op::ArrayRepeat { .. }
                | Op::ArrayRepeatElement { .. }
                | Op::Panic { .. }
                | Op::Declaration
                | Op::CallPrepare { .. }
                | Op::CallAdvance { .. }
                | Op::CheckParameters { .. }
                | Op::ReadParameter { .. }
                | Op::CallSeal { .. }
                | Op::CheckUnit { .. }
                | Op::BranchHint { .. }
                | Op::DeclareLocal { .. }
                | Op::CompoundHint { .. }
                | Op::LocalHint { .. }
                | Op::AssignLocal { .. }
                | Op::NumericHint { .. }
                | Op::Literal { .. }
                | Op::Unit { .. }
                | Op::SetHint { .. }
                | Op::ResultTransfer { .. }
                | Op::ScopeEnter { .. }
                | Op::ScopeLeave { .. }
                | Op::BindLocal { .. }
                | Op::ReadLocal { .. }
                | Op::Discard { .. }
                | Op::NumericBinary { .. }
                | Op::NumericUnary { .. }
                | Op::PrimitiveCast { .. }
                | Op::PointerCast { .. } => {}
                Op::Pattern { .. } => {}
                Op::Unsupported(_) => add(&mut facts.unsupported_operation_candidates, 1)?,
            }
        }
        match exit {
            RecipeExit::Effect { .. } => add(&mut facts.effect_recipe_dispatch_attempts, 1)?,
            RecipeExit::Fault(_) | RecipeExit::MatchNoArm => {
                add(&mut facts.typed_fault_exit_candidates, 1)?
            }
            RecipeExit::Backedge(_)
            | RecipeExit::Call { .. }
            | RecipeExit::Next(_)
            | RecipeExit::Branch { .. } => {}
            RecipeExit::Control(control) => match control {
                ControlRecipe::Return { .. }
                | ControlRecipe::Break { .. }
                | ControlRecipe::Continue { .. } => {}
            },
        }
        self.facts = facts;
        Ok(())
    }
    pub(super) fn after_vm_dispatch(&mut self, outcome: &StepOutcome) -> Result<(), Diagnostic> {
        add(&mut self.facts.vm_outcomes, 1)?;
        match outcome {
            StepOutcome::Call { .. } => add(&mut self.facts.source_call_boundaries, 1)?,
            StepOutcome::Dispatch
            | StepOutcome::PatternPrepare { .. }
            | StepOutcome::Backedge(_)
            | StepOutcome::Next(_)
            | StepOutcome::Return(_)
            | StepOutcome::GuestPanic(_) => {}
        }
        Ok(())
    }
    pub(super) fn observe_poll(
        &mut self,
        result: &RunPoll,
        retained: Option<&TerminalFacts>,
    ) -> Result<(), Diagnostic> {
        // Permanent incomplete latch: resuming a NeedInput result cannot erase
        // an earlier observation gap. Repeated terminal polling also refuses.
        if self.facts.terminal.is_some() {
            self.facts.observed_fault = true;
        }
        self.facts.closed_no_external_effects = false;
        match result {
            // A reached solver root demands the actual impl catalog; the host
            // services it through the ordinary compiler/module preparation and
            // publishes immutable metadata (host preparation streams stay outside
            // guest capture), then resumes the same interpreter. It performs no
            // guest effect, so it is counted rather than latched. Every other
            // suspension kind stays an unclassified observation gap.
            RunPoll::NeedInput(crate::db::poll::InputRequest::Discover { .. }) => {
                add(&mut self.facts.host_preparation_suspensions, 1)?
            }
            RunPoll::NeedInput(_) => self.facts.observed_suspension = true,
            RunPoll::Fault(_) => self.facts.observed_fault = true,
            RunPoll::Completed(report) => {
                let Some(TerminalFacts {
                    outcome: TerminalExecution::Completed { unit },
                    cleanup_ran,
                }) = retained
                else {
                    self.facts.observed_fault = true;
                    return Ok(());
                };
                if *unit != matches!(report.value, super::ValueSummary::Unit) {
                    self.facts.observed_fault = true;
                    return Ok(());
                }
                self.facts.terminal = Some(ObservedTerminal::Completed { unit: *unit });
                self.facts.cleanup_observed = *cleanup_ran == Some(true);
            }
            RunPoll::Panicked(report) => {
                let Some(TerminalFacts {
                    outcome: TerminalExecution::GuestPanic { message },
                    cleanup_ran,
                }) = retained
                else {
                    self.facts.observed_fault = true;
                    return Ok(());
                };
                if message != report.panic.description() {
                    self.facts.observed_fault = true;
                    return Ok(());
                }
                let mut owned = String::new();
                owned.try_reserve_exact(message.len()).map_err(|_| {
                    Diagnostic::new(
                        "LRR-E-METADATA-LIMIT",
                        "runtime capture panic allocation".into(),
                    )
                })?;
                owned.push_str(message);
                self.facts.terminal = Some(ObservedTerminal::GuestPanic { message: owned });
                self.facts.cleanup_observed = *cleanup_ran == Some(true);
            }
        }
        self.facts.closed_no_external_effects = self.facts.terminal.is_some()
            && self.facts.cleanup_observed
            && self.facts.vm_dispatches != 0
            && self.facts.vm_dispatches == self.facts.vm_outcomes
            && self.facts.effect_recipe_dispatch_attempts == 0
            && self.facts.unsupported_operation_candidates == 0
            && self.facts.typed_fault_exit_candidates == 0
            && !self.facts.observed_fault
            && !self.facts.observed_suspension;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::digest::Digest,
        ir::{op::DiagnosticTemplate, pc::SlotId},
        source::manifest::{BodyKey, PcKey},
    };
    fn pc() -> PcKey {
        PcKey {
            body: BodyKey {
                definition: Digest([1; 32]),
                exact_body_bytes: Digest([2; 32]),
            },
            node: 0,
            phase: 0,
        }
    }
    #[test]
    fn initializer_frame_counter_observes_entry_without_limiting_execution() {
        let mut capture = RunCapture::default();
        assert_eq!(capture.facts.initializer_frames_entered, 0);
        capture.initializer_frame_entered();
        assert_eq!(capture.facts.initializer_frames_entered, 1);
        capture.facts.initializer_frames_entered = u64::MAX;
        capture.initializer_frame_entered();
        assert_eq!(capture.facts.initializer_frames_entered, u64::MAX);
    }
    #[test]
    fn selected_effect_candidate_is_observed_without_bypassing_vm() {
        let mut capture = RunCapture::default();
        capture
            .observe_recipe(
                &[Op::Unit {
                    destination: SlotId(0),
                }],
                &RecipeExit::Effect {
                    site: crate::ir::recipe::EffectSiteId(0),
                    resume: pc(),
                    result: SlotId(0),
                },
            )
            .unwrap();
        assert_eq!(capture.facts.effect_recipe_dispatch_attempts, 1);
        assert!(!capture.facts.complete());
    }
    #[test]
    fn overflow_refuses_without_partial_observation() {
        let mut capture = RunCapture::default();
        capture.facts.vm_dispatches = u64::MAX;
        assert!(
            capture
                .observe_recipe(&[], &RecipeExit::Next(pc()))
                .is_err()
        );
        assert_eq!(capture.facts.classified_operation_candidates, 0);
    }
    #[test]
    fn typed_fault_and_unsupported_candidates_are_not_supported_effects() {
        let mut capture = RunCapture::default();
        capture
            .observe_recipe(
                &[Op::Unsupported(DiagnosticTemplate {
                    message: "refused".into(),
                })],
                &RecipeExit::Fault(DiagnosticTemplate {
                    message: "refused".into(),
                }),
            )
            .unwrap();
        assert_eq!(capture.facts.typed_fault_exit_candidates, 1);
        assert_eq!(capture.facts.unsupported_operation_candidates, 1);
        assert!(!capture.facts.complete());
    }
    fn completed() -> RunPoll {
        RunPoll::Completed(super::super::RunReport {
            live_edition: 2015,
            semantics_profile: crate::core::profile::SEMANTICS_PROFILE,
            schema: "lrr/v2/development-run",
            status: "completed",
            value: super::super::ValueSummary::Unit,
            opcode_counters: Default::default(),
            installed_recipes: 1,
            installed_lrir_bytes: 1,
            output: Default::default(),
        })
    }
    #[test]
    fn catalog_discovery_is_counted_host_preparation_not_an_observation_gap() {
        let mut capture = RunCapture::default();
        capture
            .observe_poll(
                &RunPoll::NeedInput(crate::db::poll::InputRequest::Discover {
                    key: crate::db::seals::SealKey {
                        domain: crate::db::seals::SealDomain::ImplCatalog { id: Digest([3; 32]) },
                        unit_graph: Digest([4; 32]),
                        epoch: 1,
                    },
                }),
                None,
            )
            .unwrap();
        assert_eq!(capture.facts.host_preparation_suspensions, 1);
        assert!(!capture.facts.observed_suspension);
        capture
            .observe_recipe(
                &[Op::Unit {
                    destination: SlotId(0),
                }],
                &RecipeExit::Next(pc()),
            )
            .unwrap();
        capture.after_vm_dispatch(&StepOutcome::Next(pc())).unwrap();
        let retained = TerminalFacts {
            outcome: TerminalExecution::Completed { unit: true },
            cleanup_ran: Some(true),
        };
        capture.observe_poll(&completed(), Some(&retained)).unwrap();
        assert!(capture.facts.complete());
    }
    #[test]
    fn detached_public_report_and_suspension_cannot_certify_closed_capture() {
        let mut capture = RunCapture::default();
        capture
            .observe_recipe(
                &[Op::Unit {
                    destination: SlotId(0),
                }],
                &RecipeExit::Next(pc()),
            )
            .unwrap();
        capture.after_vm_dispatch(&StepOutcome::Next(pc())).unwrap();
        capture.observe_poll(&completed(), None).unwrap();
        assert!(!capture.facts.complete());
        let mut suspended = RunCapture::default();
        suspended
            .observe_poll(
                &RunPoll::NeedInput(crate::db::poll::InputRequest::CaptureCompileEnvironment {
                    unit: Digest([0; 32]),
                }),
                None,
            )
            .unwrap();
        suspended
            .observe_recipe(
                &[Op::Unit {
                    destination: SlotId(0),
                }],
                &RecipeExit::Next(pc()),
            )
            .unwrap();
        suspended
            .after_vm_dispatch(&StepOutcome::Next(pc()))
            .unwrap();
        let retained = TerminalFacts {
            outcome: TerminalExecution::Completed { unit: true },
            cleanup_ran: Some(true),
        };
        suspended
            .observe_poll(&completed(), Some(&retained))
            .unwrap();
        assert!(!suspended.facts.complete());
    }
}
