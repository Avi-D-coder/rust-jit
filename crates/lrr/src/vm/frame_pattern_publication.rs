//! One resumable publication for every pattern owner. Search already selected
//! these bindings; publication never matches, parses or asks a source query.
use super::pattern_engine_frame::{owning_read, window_read_mode};
use super::*;
use crate::{
    core::digest::Digest,
    runtime::cell::{
        IntrinsicRead,
        aggregate_copy::{AggregateCopy, CopyProgress},
    },
    vm::pattern::{
        DefaultBindingMode,
        matcher::{Staged, copy_place, place_fault},
    },
};

/// A Copy sub-array window being replicated into a fresh `[T; K]`, one
/// element per advance (an aggregate element through its resumable Copy).
struct SubarrayWork {
    build: crate::runtime::cell::subarray::SubarrayBuild,
    len: u32,
    next: u32,
    copy: Option<(CellId, AggregateCopy)>,
}

pub(super) struct Publication {
    owner: NodeId,
    solution: u64,
    tree: Option<Digest>,
    guard_view: bool,
    // The transaction's exact selected bindings move here once; Expose restores
    // them for Commit/retry. No mutable execution state is shared or deep copied.
    staged: Vec<Staged>,
    preparing: bool,
    next: usize,
    copy: Option<(CellId, AggregateCopy)>,
    sub: Option<SubarrayWork>,
    transfers: Vec<Option<TicketId>>,
}

impl Frame {
    pub(super) fn begin_pattern_publication(
        &mut self,
        owner: NodeId,
        guard_view: bool,
        store: &mut GuestStore,
    ) -> Result<Publication, Diagnostic> {
        self.preflight_pattern_publication(owner, guard_view, store)?;
        let open = self.open_patterns.current_mut(owner)?;
        store.reserve_pattern_bindings(open.staged.len())?;
        let transfers = vec![None; open.staged.len()];
        Ok(Publication {
            owner,
            solution: open.solution,
            tree: open.tree.as_ref().map(|tree| tree.identity()),
            guard_view,
            staged: std::mem::take(&mut open.staged),
            preparing: true,
            next: 0,
            copy: None,
            sub: None,
            transfers,
        })
    }
    /// One step of a by-value sub-array binding's preparation. Non-Copy
    /// elements move at publication instead, like any non-Copy value binding.
    fn prepare_subarray(
        &mut self,
        publication: &mut Publication,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let index = publication.next;
        let place = copy_place(&publication.staged[index].place);
        let Some(work) = publication.sub.as_mut() else {
            let mode = window_read_mode(store, &place)?
                .ok_or_else(|| invalid("sub-array preparation lacks its window"))?;
            if mode == IntrinsicRead::Move {
                publication.next = index
                    .checked_add(1)
                    .ok_or_else(|| invalid("pattern preparation cursor overflow"))?;
                return Ok(());
            }
            let len = place
                .subarray_len()
                .ok_or_else(|| invalid("sub-array preparation lacks its window"))?;
            publication.sub = Some(SubarrayWork {
                build: store.begin_subarray_build(&place).map_err(place_fault)?,
                len,
                next: 0,
                copy: None,
            });
            return Ok(());
        };
        if work.next == work.len {
            let work = publication
                .sub
                .take()
                .ok_or_else(|| invalid("sub-array preparation work absent"))?;
            publication.transfers[index] = Some(store.subarray_finish(work.build)?);
            publication.next = index
                .checked_add(1)
                .ok_or_else(|| invalid("pattern preparation cursor overflow"))?;
            return Ok(());
        }
        let element = store
            .project_index(&place, ValueRepr::unsigned(TypeId(13), u64::from(work.next))?)
            .map_err(place_fault)?;
        let (cell, mode) = owning_read(store, &element)?;
        if let Some((expected, copy)) = &mut work.copy {
            if cell != *expected || mode != IntrinsicRead::CopyAggregate {
                return Err(invalid("sub-array aggregate Copy source changed"));
            }
            if let CopyProgress::Complete(ticket) = self.advance_copy_work(copy, store)? {
                store.subarray_push(&work.build, work.next, ticket)?;
                work.copy = None;
                work.next += 1;
            }
            return Ok(());
        }
        match mode {
            IntrinsicRead::CopyScalar => {
                let ticket = store.copy_from(cell)?;
                store.subarray_push(&work.build, work.next, ticket)?;
                work.next += 1;
            }
            IntrinsicRead::CopyAggregate => {
                work.copy = Some((cell, AggregateCopy::begin(store, cell)?));
            }
            IntrinsicRead::Move => {
                return Err(invalid("sub-array elements disagree on their read mode"));
            }
        }
        Ok(())
    }
    pub(super) fn advance_pattern_publication(
        &mut self,
        publication: &mut Publication,
        store: &mut GuestStore,
    ) -> Result<bool, Diagnostic> {
        let open = self.open_patterns.current(publication.owner)?;
        if !open.matched
            || !open.exposed.is_empty()
            || !open.staged.is_empty()
            || open.solution != publication.solution
            || open.tree.as_ref().map(|tree| tree.identity()) != publication.tree
        {
            return Err(invalid(
                "pattern publication solution changed while suspended",
            ));
        }
        if publication.transfers.len() != publication.staged.len() {
            return Err(invalid("pattern transfer preparation count changed"));
        }
        if publication.preparing {
            if publication
                .staged
                .get(publication.next)
                .is_some_and(|staged| {
                    staged.mode == DefaultBindingMode::Move
                        && staged.place.subarray_len().is_some()
                })
            {
                self.prepare_subarray(publication, store)?;
                return Ok(false);
            }
            let Some(staged) = publication.staged.get(publication.next) else {
                if publication.copy.is_some() || publication.sub.is_some() {
                    return Err(invalid("pattern preparation ended with active Copy work"));
                }
                publication.preparing = false;
                publication.next = 0;
                return Ok(false);
            };
            if let Some((expected, copy)) = &mut publication.copy {
                let (cell, mode) = owning_read(store, &staged.place)?;
                if cell != *expected || mode != IntrinsicRead::CopyAggregate {
                    return Err(invalid("pattern aggregate Copy source changed"));
                }
                match self.advance_copy_work(copy, store)? {
                    CopyProgress::Pending => return Ok(false),
                    CopyProgress::Complete(ticket) => {
                        publication.transfers[publication.next] = Some(ticket);
                        publication.copy = None;
                    }
                }
            } else if staged.mode == DefaultBindingMode::Move {
                let (cell, mode) = owning_read(store, &staged.place)?;
                match mode {
                    IntrinsicRead::CopyScalar => {
                        publication.transfers[publication.next] = Some(store.copy_from(cell)?)
                    }
                    IntrinsicRead::CopyAggregate => {
                        publication.copy = Some((cell, AggregateCopy::begin(store, cell)?));
                        return Ok(false);
                    }
                    IntrinsicRead::Move => {}
                }
            }
            publication.next = publication
                .next
                .checked_add(1)
                .ok_or_else(|| invalid("pattern preparation cursor overflow"))?;
            return Ok(false);
        }
        let Some(staged) = publication.staged.get(publication.next) else {
            if publication.copy.is_some() || publication.transfers.iter().any(Option::is_some) {
                return Err(invalid(
                    "pattern publication completed with private Copy work",
                ));
            }
            let open = self.open_patterns.current_mut(publication.owner)?;
            if publication.guard_view {
                open.exposed = publication
                    .staged
                    .iter()
                    .map(|staged| staged.binding)
                    .collect();
                open.staged = std::mem::take(&mut publication.staged);
            } else {
                open.matched = false;
            }
            return Ok(true);
        };
        self.publish_pattern_binding(staged, &mut publication.transfers[publication.next], store)?;
        publication.next = publication
            .next
            .checked_add(1)
            .ok_or_else(|| invalid("pattern publication cursor overflow"))?;
        Ok(false)
    }
}
