//! One resumable publication for every pattern owner. Search already selected
//! these bindings; publication never matches, parses or asks a source query.
use super::pattern_engine_frame::owning_read;
use super::*;
use crate::{
    core::digest::Digest,
    runtime::cell::{
        IntrinsicRead,
        aggregate_copy::{AggregateCopy, CopyProgress},
    },
    vm::pattern::{DefaultBindingMode, matcher::Staged},
};

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
            transfers,
        })
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
            let Some(staged) = publication.staged.get(publication.next) else {
                if publication.copy.is_some() {
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
