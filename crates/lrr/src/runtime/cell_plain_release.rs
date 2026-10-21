//! Releasing a Copy aggregate local (a tuple, array or `Copy` struct) that
//! owns no drop glue: its fields are plain data, so releasing the location
//! walks the object graph with the ordinary declaration-order cleanup
//! transitions and never schedules a destructor.
use super::*;

impl GuestStore {
    /// Whether `cell` live-holds a Copy aggregate, the only aggregate whose
    /// release needs no destructor schedule.
    pub(crate) fn holds_plain_aggregate(&self, cell: CellId) -> Result<bool, Diagnostic> {
        let CellState::Live(value) = self.state(cell)? else {
            return Ok(false);
        };
        let ty = TypeId(value.type_id);
        if value.kind != 8 || self.types.is_string(ty) || self.is_union_type(ty) {
            return Ok(false);
        }
        Ok(self.intrinsic_read_mode(cell)? == IntrinsicRead::CopyAggregate)
    }
    /// Release a plain aggregate's owned graph iteratively. The root location
    /// stays `Dropping` for the caller's `retire_cell`.
    pub(crate) fn release_plain_aggregate(&mut self, root: CellId) -> Result<(), Diagnostic> {
        let mut stack = vec![self.begin_drop(root)?];
        while let Some(&parent) = stack.last() {
            match self.aggregate_cleanup_step(parent)? {
                AggregateCleanupStep::Field(child) => {
                    let value = self.peek_ticket(child)?;
                    if value.kind == 8 && !self.types.is_string(TypeId(value.type_id)) {
                        stack.push(child);
                    } else {
                        self.discard(child)?;
                    }
                }
                AggregateCleanupStep::Skipped => {}
                AggregateCleanupStep::Complete => {
                    stack.pop();
                    self.discard(parent)?;
                }
            }
        }
        Ok(())
    }
}
