//! Linear ownership of a saved phase. No query re-entry executes its payload.
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    service::expansion::{DemandContext, DemandToken, ExpansionCoordinator},
};
pub struct SavedPhase<S> {
    token: DemandToken,
    state: S,
}
pub enum ResumePoll<S> {
    Waiting(SavedPhase<S>),
    Ready {
        state: S,
        output: Digest,
    },
    Rejected {
        saved: SavedPhase<S>,
        error: Diagnostic,
    },
}
impl<S> SavedPhase<S> {
    pub fn new(token: DemandToken, state: S) -> Self {
        Self { token, state }
    }
    pub fn state(&self) -> &S {
        &self.state
    }
    pub fn token(&self) -> &DemandToken {
        &self.token
    }
    pub fn resume(
        self,
        coordinator: &mut ExpansionCoordinator,
        current: &DemandContext,
    ) -> ResumePoll<S> {
        match coordinator.take_ready(current, &self.token) {
            Ok(None) => ResumePoll::Waiting(self),
            Ok(Some(Ok(output))) => ResumePoll::Ready {
                state: self.state,
                output,
            },
            Ok(Some(Err(error))) | Err(error) => ResumePoll::Rejected { saved: self, error },
        }
    }
}
