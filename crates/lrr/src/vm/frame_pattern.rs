//! Common transaction dispatch for every ordinary pattern owner. Source
//! planning/verifier select identities; this module never parses guest syntax.
use super::*;
use crate::ir::op::PatternOperation;

impl Frame {
    pub(crate) fn preflight_pattern(
        &self,
        operation: &PatternOperation,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.require_store(store)?;
        match operation {
            PatternOperation::Open { owner, source } => {
                self.preflight_pattern_open(*owner, *source, store)
            }
            PatternOperation::Test {
                owner,
                tree,
                result,
                retry,
            } => self.preflight_pattern_test(*owner, tree, *result, *retry, store),
            PatternOperation::Expose { owner } => {
                self.preflight_pattern_publication(*owner, true, store)
            }
            PatternOperation::Commit { owner } => {
                self.preflight_pattern_publication(*owner, false, store)
            }
            PatternOperation::Conceal { owner } | PatternOperation::Close { owner } => {
                self.open_patterns.current(*owner).map(|_| ())
            }
        }
    }
    pub(crate) fn execute_pattern(
        &mut self,
        operation: &PatternOperation,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_pattern(operation, store)?;
        match operation {
            PatternOperation::Open { owner, source } => self.open_pattern(*owner, *source, store),
            PatternOperation::Test {
                owner,
                tree,
                result,
                retry,
            } => self.test_pattern(*owner, tree, *result, *retry, store),
            PatternOperation::Expose { owner } => self.publish_pattern(*owner, true, store),
            PatternOperation::Commit { owner } => self.publish_pattern(*owner, false, store),
            PatternOperation::Conceal { owner } => self.conceal_pattern(*owner, store),
            PatternOperation::Close { owner } => self.close_pattern(*owner, store),
        }
    }
}
