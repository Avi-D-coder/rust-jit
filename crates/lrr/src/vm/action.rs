//! Worker-local exact-once publication of selected call boundaries.
use crate::{
    core::diagnostic::Diagnostic,
    ir::{
        pc::{PcKey, SlotId},
        recipe::{CallSiteId, Origin},
    },
};
use std::{cell::Cell, rc::Rc};

#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize)]
pub struct ActionId(u64);
impl ActionId {
    pub fn get(self) -> u64 {
        self.0
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, serde::Serialize)]
pub struct ActionSnapshot {
    pub published: u64,
    pub consumed: u64,
    pub pending: Option<ActionId>,
}
#[derive(Debug)]
struct ActionState {
    published: Cell<u64>,
    consumed: Cell<u64>,
    pending: Cell<Option<ActionId>>,
}
#[derive(Debug)]
pub(crate) struct RunActions(Rc<ActionState>);
impl RunActions {
    pub(crate) fn new() -> Self {
        Self(Rc::new(ActionState {
            published: Cell::new(0),
            consumed: Cell::new(0),
            pending: Cell::new(None),
        }))
    }
    pub(crate) fn share(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
    pub(crate) fn snapshot(&self) -> ActionSnapshot {
        ActionSnapshot {
            published: self.0.published.get(),
            consumed: self.0.consumed.get(),
            pending: self.0.pending.get(),
        }
    }
    pub(crate) fn preflight_publish(&self) -> Result<(), Diagnostic> {
        if self.0.pending.get().is_some() {
            return Err(invalid("pending action cannot be overwritten"));
        }
        self.0.published.get().checked_add(1).ok_or_else(|| {
            Diagnostic::new("LRR-E-RUNTIME-RESOURCE", "run action ID exhausted".into())
        })?;
        Ok(())
    }
    pub(crate) fn publish_call(
        &self,
        caller: PcKey,
        site: CallSiteId,
        resume: PcKey,
        result: SlotId,
        origin: Origin,
    ) -> Result<CallAction, Diagnostic> {
        self.preflight_publish()?;
        if caller.body != resume.body
            || caller.node != site.0
            || caller.body != origin.span.body_key
        {
            return Err(invalid("call action source/continuation identity mismatch"));
        }
        let id = ActionId(
            self.0
                .published
                .get()
                .checked_add(1)
                .ok_or_else(|| invalid("action admission changed"))?,
        );
        self.0.published.set(id.0);
        self.0.pending.set(Some(id));
        Ok(CallAction {
            run: Rc::clone(&self.0),
            id,
            caller,
            site,
            resume,
            result,
            origin,
        })
    }
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-INTERNAL-IR", message.into())
}
/// An opaque publication handle. It is deliberately not Clone.
#[derive(Debug)]
pub struct CallAction {
    run: Rc<ActionState>,
    id: ActionId,
    caller: PcKey,
    site: CallSiteId,
    resume: PcKey,
    result: SlotId,
    origin: Origin,
}
#[derive(Debug)]
pub(crate) struct ConsumedCall {
    pub(crate) id: ActionId,
    pub(crate) resume: PcKey,
    pub(crate) result: SlotId,
    pub(crate) origin: Origin,
}
impl CallAction {
    pub(crate) fn consume(
        self,
        actions: &RunActions,
        caller: &PcKey,
        site: CallSiteId,
        resume: &PcKey,
        result: SlotId,
    ) -> Result<ConsumedCall, Diagnostic> {
        if !Rc::ptr_eq(&self.run, &actions.0)
            || actions.0.pending.get() != Some(self.id)
            || &self.caller != caller
            || self.site != site
            || &self.resume != resume
            || self.result != result
        {
            return Err(invalid(
                "call action is stale or belongs to another continuation/run",
            ));
        }
        let consumed = actions
            .0
            .consumed
            .get()
            .checked_add(1)
            .ok_or_else(|| invalid("action consume counter overflow"))?;
        if consumed != self.id.0 {
            return Err(invalid("call action consumption order mismatch"));
        }
        actions.0.consumed.set(consumed);
        actions.0.pending.set(None);
        Ok(ConsumedCall {
            id: self.id,
            resume: self.resume,
            result: self.result,
            origin: self.origin,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn boundary() -> (PcKey, CallSiteId, PcKey, SlotId, Origin) {
        use crate::{
            core::{digest::Digest, profile::ProfileIdentity},
            ir::pc::{NodeId, Phase},
            source::snapshot::{CfgSet, SourceBlob},
            syntax::{parse::ParsedFile, spans::BodySpan},
        };
        let blob = SourceBlob::new(b"fn caller(){callee()}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(Digest([76; 32]), vec![], &CfgSet::guest_default())
            .unwrap();
        let body = parsed
            .index_body(outline.items[0].body.as_ref().unwrap(), Default::default())
            .unwrap();
        let caller = body.pc(NodeId(0), Phase::Entry).unwrap();
        let resume = body.pc(NodeId(0), Phase::Exit).unwrap();
        let result = body.nodes()[0].value.unwrap();
        let origin = Origin {
            span: BodySpan {
                body_key: body.body().clone(),
                relative_start: 0,
                relative_end: body.nodes()[0].range.end,
            },
            expansion_ancestry: vec![],
        };
        (caller, CallSiteId(0), resume, result, origin)
    }
    // Forge a replay only in the regression model; production handles cannot clone.
    fn replay(action: &CallAction) -> CallAction {
        CallAction {
            run: Rc::clone(&action.run),
            id: action.id,
            caller: action.caller.clone(),
            site: action.site,
            resume: action.resume.clone(),
            result: action.result,
            origin: action.origin.clone(),
        }
    }
    #[test]
    fn call_actions_publish_once_share_monotonic_ids_and_preserve_origins() {
        let actions = RunActions::new();
        let shared = actions.share();
        let (caller, site, resume, result, origin) = boundary();
        for id in 1..=3 {
            let publisher = if id == 2 { &shared } else { &actions };
            let packet = publisher
                .publish_call(caller.clone(), site, resume.clone(), result, origin.clone())
                .unwrap();
            assert_eq!(packet.id.get(), id);
            assert_eq!(
                publisher.preflight_publish().unwrap_err().code,
                "LRR-E-INTERNAL-IR"
            );
            let stale = replay(&packet);
            let consumed = packet
                .consume(&shared, &caller, site, &resume, result)
                .unwrap();
            assert_eq!(consumed.origin, origin);
            assert_eq!(consumed.resume, resume);
            assert_eq!(consumed.result, result);
            assert_eq!(
                stale
                    .consume(&actions, &caller, site, &resume, result)
                    .unwrap_err()
                    .code,
                "LRR-E-INTERNAL-IR"
            );
            assert_eq!(
                actions.snapshot(),
                ActionSnapshot {
                    published: id,
                    consumed: id,
                    pending: None
                }
            );
        }
    }
    #[test]
    fn wrong_run_and_continuation_cannot_consume_or_replace_pending_call_actions() {
        let a = RunActions::new();
        let b = RunActions::new();
        let (caller, site, resume, result, origin) = boundary();
        let packet = a
            .publish_call(caller.clone(), site, resume.clone(), result, origin.clone())
            .unwrap();
        let other = b
            .publish_call(caller.clone(), site, resume.clone(), result, origin)
            .unwrap();
        let before_a = a.snapshot();
        let before_b = b.snapshot();
        assert_eq!(
            replay(&packet)
                .consume(&b, &caller, site, &resume, result)
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(
            replay(&packet)
                .consume(&a, &caller, site, &resume, SlotId(result.0 + 1))
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(a.snapshot(), before_a);
        assert_eq!(b.snapshot(), before_b);
        packet.consume(&a, &caller, site, &resume, result).unwrap();
        other.consume(&b, &caller, site, &resume, result).unwrap();
    }
    #[test]
    fn call_action_identity_and_id_exhaustion_fault_before_publication() {
        let actions = RunActions::new();
        let (caller, site, resume, result, origin) = boundary();
        let before = actions.snapshot();
        assert_eq!(
            actions
                .publish_call(
                    caller.clone(),
                    CallSiteId(site.0 + 1),
                    resume.clone(),
                    result,
                    origin.clone()
                )
                .unwrap_err()
                .code,
            "LRR-E-INTERNAL-IR"
        );
        assert_eq!(actions.snapshot(), before);
        actions.0.published.set(u64::MAX);
        actions.0.consumed.set(u64::MAX);
        let exhausted = actions.snapshot();
        assert_eq!(
            actions
                .publish_call(caller, site, resume, result, origin)
                .unwrap_err()
                .code,
            "LRR-E-RUNTIME-RESOURCE"
        );
        assert_eq!(actions.snapshot(), exhausted);
    }
}
