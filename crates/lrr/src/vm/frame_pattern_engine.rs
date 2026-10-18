//! Frame execution of the single pattern transaction shared by every owner.
//! Recipes select the owner, scrutinee slot, retained tree and continuations;
//! this module never parses or resolves guest syntax.
use super::*;
use crate::{
    runtime::{cell::IntrinsicRead, reference::Access},
    vm::pattern::{
        DefaultBindingMode,
        matcher::{OpenPattern, Search, Staged, copy_place, place_fault},
    },
};

fn transaction(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-TRANSACTION", message.into())
}
/// Owning-read admission of a staged by-value binding. An owning scrutinee
/// cell that still holds an unresolved numeric literal is an intrinsic Copy
/// scalar whose type a later use fixes.
fn owning_read(
    store: &GuestStore,
    place: &crate::runtime::place::Place,
) -> Result<(CellId, IntrinsicRead), Diagnostic> {
    if let crate::runtime::place::PlaceRoot::Cell(cell) = place.root
        && place.projections.is_empty()
        && matches!(store.state(cell)?, CellState::Live(value) if matches!(value.kind, 11 | 12))
    {
        return Ok((cell, IntrinsicRead::CopyScalar));
    }
    let (cell, _, mode) = store.admit_owning_place_read(place).map_err(place_fault)?;
    Ok((cell, mode))
}

impl Frame {
    /// Ordinary Value-namespace classification of a reached tree's plain
    /// identifiers, supplied by the dispatcher for exactly this PC.
    pub(crate) fn set_pattern_names(
        &mut self,
        pc: PcKey,
        names: Arc<[crate::vm::pattern::matcher::PatternTarget]>,
    ) {
        self.pattern_names = Some((pc, names));
    }
    pub(crate) fn preflight_pattern_open(
        &self,
        owner: NodeId,
        source: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.body.nodes().get(owner.0 as usize).is_none() {
            return Err(transaction("pattern owner is absent"));
        }
        match self.body.slot_kind(source)? {
            SlotKind::Value => {
                let slot = self.slot(source)?;
                if !self.active_scopes.contains(&slot.scope) {
                    return Err(transaction("pattern scrutinee scope is inactive"));
                }
                if !matches!(store.state(self.cell(source)?)?, CellState::Live(_)) {
                    return Err(transaction("pattern scrutinee temporary is not live"));
                }
            }
            SlotKind::Place => {
                store
                    .place_type_id(self.executed_place(source)?)
                    .map_err(place_fault)?;
            }
            _ => return Err(transaction("pattern scrutinee slot kind differs")),
        }
        Ok(())
    }
    pub(crate) fn open_pattern(
        &mut self,
        owner: NodeId,
        source: SlotId,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_pattern_open(owner, source, store)?;
        let (root, root_source, owned_temporary, scope) = match self.body.slot_kind(source)? {
            SlotKind::Value => (
                crate::runtime::place::Place::cell(self.cell(source)?),
                self.source_view(source)?,
                true,
                self.slot(source)?.scope,
            ),
            _ => {
                let place = copy_place(self.executed_place(source)?);
                let root_source = self.place_slot(source)?.source.clone();
                let scope = self.place_slot(source)?.scope;
                self.clear_place(source)?;
                (place, root_source, false, scope)
            }
        };
        self.open_patterns.open(OpenPattern {
            owner,
            scope,
            root,
            root_source,
            owned_temporary,
            tree: None,
            solution: 0,
            matched: false,
            staged: Vec::new(),
            exposed: Vec::new(),
        });
        Ok(())
    }
    fn pattern_name_classes(
        &self,
        tree: &crate::syntax::pattern_tree::PatternTree,
    ) -> Result<Option<Arc<[crate::vm::pattern::matcher::PatternTarget]>>, Diagnostic> {
        if tree.names().is_empty() {
            return Ok(None);
        }
        match &self.pattern_names {
            Some((pc, names)) if pc == &self.pc && names.len() == tree.names().len() => {
                Ok(Some(Arc::clone(names)))
            }
            _ => Err(Diagnostic::new(
                "LRR-E-PATTERN-BINDING",
                "plain pattern identifier requires ordinary Value-namespace classification".into(),
            )),
        }
    }
    pub(crate) fn preflight_pattern_test(
        &self,
        owner: NodeId,
        tree: &Arc<crate::syntax::pattern_tree::PatternTree>,
        result: SlotId,
        retry: bool,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let open = self.open_patterns.current(owner)?;
        if retry
            && (!open.matched
                || !open
                    .tree
                    .as_ref()
                    .is_some_and(|previous| Arc::ptr_eq(previous, tree)))
        {
            return Err(transaction(
                "pattern retry requires the same arm's previously selected solution",
            ));
        }
        if !open.exposed.is_empty() {
            return Err(transaction("pattern test while a guard view is published"));
        }
        self.preflight_write_type(result, TypeId(2), store)
    }
    /// Search once, without publishing a result or changing ownership. The
    /// VM can suspend this exact operation for an actually reached constant.
    pub(crate) fn search_pattern(
        &self,
        owner: NodeId,
        tree: &Arc<crate::syntax::pattern_tree::PatternTree>,
        result: SlotId,
        retry: bool,
        store: &GuestStore,
    ) -> Result<crate::vm::pattern::matcher::SearchOutcome, Diagnostic> {
        self.preflight_pattern_test(owner, tree, result, retry, store)?;
        let names = self.pattern_name_classes(tree)?;
        let open = self.open_patterns.current(owner)?;
        let skip = self.pattern_solution(owner, retry)?;
        Search::run(
            store,
            names.as_deref(),
            skip,
            &open.root,
            open.root_source.as_ref(),
            tree,
            self.body.profile().edition,
        )
    }
    fn pattern_solution(&self, owner: NodeId, retry: bool) -> Result<u64, Diagnostic> {
        let open = self.open_patterns.current(owner)?;
        if retry {
            open.solution
                .checked_add(1)
                .ok_or_else(|| transaction("pattern solution counter overflow"))
        } else {
            Ok(0)
        }
    }
    /// Commit only the completed non-consuming search, using the same
    /// transaction for every pattern owner and source origin.
    pub(crate) fn complete_pattern_test(
        &mut self,
        owner: NodeId,
        tree: &Arc<crate::syntax::pattern_tree::PatternTree>,
        result: SlotId,
        retry: bool,
        found: Option<Vec<Staged>>,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_pattern_test(owner, tree, result, retry, store)?;
        let skip = self.pattern_solution(owner, retry)?;
        let matched = found.is_some();
        store.reserve_owning_read_result(self.slot(result)?.cell.is_none(), false)?;
        self.prepare_write(result, store)?;
        let ticket = store.ticket(ValueRepr::boolean(matched))?;
        self.write(result, ticket, store)?;
        let open = self.open_patterns.current_mut(owner)?;
        open.tree = Some(Arc::clone(tree));
        open.solution = skip;
        open.matched = matched;
        open.staged = found.unwrap_or_default();
        Ok(())
    }
    /// Retain exactly this operation when the matcher needs an initializer;
    /// all earlier recipe operations and the scrutinee remain completed.
    pub(crate) fn suspend_pattern_test(&mut self, index: usize) -> Result<(), Diagnostic> {
        self.rerun_index = Some(
            u32::try_from(index).map_err(|_| transaction("pattern operation index overflow"))?,
        );
        Ok(())
    }
    pub(crate) fn test_pattern(
        &mut self,
        owner: NodeId,
        tree: &Arc<crate::syntax::pattern_tree::PatternTree>,
        result: SlotId,
        retry: bool,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        match self.search_pattern(owner, tree, result, retry, store)? {
            crate::vm::pattern::matcher::SearchOutcome::Complete(found) => {
                self.complete_pattern_test(owner, tree, result, retry, found, store)
            }
            crate::vm::pattern::matcher::SearchOutcome::Initializer(_) => Err(transaction(
                "constant pattern requires the resumable operation dispatcher",
            )),
        }
    }
    pub(crate) fn preflight_pattern_publication(
        &self,
        owner: NodeId,
        guard_view: bool,
        store: &GuestStore,
    ) -> Result<(), Diagnostic> {
        let open = self.open_patterns.current(owner)?;
        if !open.matched || !open.exposed.is_empty() {
            return Err(transaction(
                "pattern bindings require one matched, unpublished solution",
            ));
        }
        for staged in &open.staged {
            self.preflight_declaration(staged.binding)?;
            match staged.mode {
                DefaultBindingMode::Move => {
                    let (_, mode) = owning_read(store, &staged.place)?;
                    match mode {
                        IntrinsicRead::CopyScalar => {}
                        IntrinsicRead::Move if guard_view => {
                            return Err(Diagnostic::new(
                                "LRR-E-GUARD-MOVE",
                                "guard view of a non-Copy by-value binding requires guard reference integration"
                                    .into(),
                            ));
                        }
                        IntrinsicRead::Move => {}
                        IntrinsicRead::CopyAggregate if open.owned_temporary && !guard_view => {}
                        IntrinsicRead::CopyAggregate => {
                            return Err(Diagnostic::new(
                                "LRR-E-UNSUPPORTED",
                                "by-value binding of a Copy aggregate place requires aggregate copy activation"
                                    .into(),
                            ));
                        }
                    }
                }
                DefaultBindingMode::Ref | DefaultBindingMode::RefMut => {
                    store
                        .admit_place_borrow(
                            &staged.place,
                            if staged.mode == DefaultBindingMode::RefMut {
                                Access::Mutable
                            } else {
                                Access::Shared
                            },
                        )
                        .map_err(place_fault)?;
                }
            }
        }
        Ok(())
    }
    fn publish_pattern_binding(
        &mut self,
        staged: &Staged,
        owned_temporary: bool,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let (ticket, source) = match staged.mode {
            DefaultBindingMode::Move => {
                let (cell, mode) = owning_read(store, &staged.place)?;
                let ticket = match mode {
                    IntrinsicRead::CopyScalar => store.copy_from(cell)?,
                    IntrinsicRead::Move => store.move_from(cell)?,
                    IntrinsicRead::CopyAggregate if owned_temporary => store.move_from(cell)?,
                    IntrinsicRead::CopyAggregate => {
                        return Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "by-value binding of a Copy aggregate place requires aggregate copy activation"
                                .into(),
                        ));
                    }
                };
                (ticket, staged.source.clone())
            }
            DefaultBindingMode::Ref | DefaultBindingMode::RefMut => {
                let mutable = staged.mode == DefaultBindingMode::RefMut;
                let ticket = store
                    .borrow_ticket(
                        &staged.place,
                        if mutable {
                            Access::Mutable
                        } else {
                            Access::Shared
                        },
                    )
                    .map_err(place_fault)?;
                let range = self.body.nodes()[staged.binding.0 as usize].range;
                let source = staged.source.clone().map(|pointee| {
                    crate::traits::term::TyTerm::Ref(
                        crate::traits::term::RegionTerm::Local {
                            definition: self.body.body().definition,
                            start: range.start,
                            end: range.end,
                        },
                        mutable,
                        Box::new(pointee),
                    )
                });
                (ticket, source)
            }
        };
        let value = store.peek_ticket(ticket)?;
        let physical = self
            .body
            .slot_count()
            .checked_add(staged.binding.0)
            .ok_or_else(|| invalid("binding physical slot overflow"))?;
        let cell = store.new_cell(
            StorageContract::Unbound,
            staged.mutable,
            CellOwner::FrameSlot {
                frame: self.anchor,
                slot: physical,
            },
        )?;
        store.validate_initialization_value(cell, value)?;
        store.initialize(cell, ticket)?;
        self.bindings[staged.binding.0 as usize] = Some(cell);
        self.set_binding_source_view(staged.binding, source)?;
        Ok(())
    }
    fn retire_pattern_binding(
        &mut self,
        binding: NodeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if let Some(cell) = self
            .bindings
            .get_mut(binding.0 as usize)
            .and_then(Option::take)
        {
            store.admit_discard(cell)?;
            if matches!(store.state(cell)?, CellState::Live(_)) {
                let ticket = store.begin_drop(cell)?;
                store.discard(ticket)?;
            }
            store.retire_cell(cell)?;
        }
        self.set_binding_source_view(binding, None)
    }
    /// Publish the staged bindings. A guard view never moves its source; a
    /// commit transfers each binding once, in binding order, after admitting
    /// every transfer so a refusal changes no ownership.
    pub(crate) fn publish_pattern(
        &mut self,
        owner: NodeId,
        guard_view: bool,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.preflight_pattern_publication(owner, guard_view, store)?;
        let open = self.open_patterns.current_mut(owner)?;
        let owned_temporary = open.owned_temporary;
        let staged = if guard_view {
            open.staged
                .iter()
                .map(|staged| Staged {
                    binding: staged.binding,
                    place: copy_place(&staged.place),
                    source: staged.source.clone(),
                    mode: staged.mode,
                    mutable: staged.mutable,
                })
                .collect::<Vec<_>>()
        } else {
            open.matched = false;
            std::mem::take(&mut open.staged)
        };
        store.reserve_pattern_bindings(staged.len())?;
        for binding in &staged {
            self.publish_pattern_binding(binding, owned_temporary && !guard_view, store)?;
        }
        if guard_view {
            self.open_patterns.current_mut(owner)?.exposed =
                staged.iter().map(|staged| staged.binding).collect();
        }
        Ok(())
    }
    pub(crate) fn conceal_pattern(
        &mut self,
        owner: NodeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        let exposed = std::mem::take(&mut self.open_patterns.current_mut(owner)?.exposed);
        for binding in exposed.into_iter().rev() {
            self.retire_pattern_binding(binding, store)?;
        }
        Ok(())
    }
    pub(crate) fn close_pattern(
        &mut self,
        owner: NodeId,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        self.conceal_pattern(owner, store)?;
        self.open_patterns.close(owner)?;
        Ok(())
    }
}
