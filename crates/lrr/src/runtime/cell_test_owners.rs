//! Read-only test inventory of actual occupied owner records, not cumulative
//! append-only allocation charges. No production authority or mutation API.
use super::*;
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OwnerInventory {
    pub(crate) cells: Vec<(u64, CellState)>,
    pub(crate) objects: Vec<u64>,
    pub(crate) tickets: Vec<u64>,
    pub(crate) guards: Vec<u64>,
    pub(crate) literals: Vec<u64>,
}
fn occupied<T>(arena: &GuestArena<T>, budget: &RunBudget) -> Vec<u64> {
    (1..=budget.handles())
        .map(|i| (u64::from(budget.epoch()) << 32) | u64::from(i))
        .filter(|bits| arena.get(Handle::<T>::from_bits(*bits)).is_ok())
        .collect()
}
impl GuestStore {
    pub(crate) fn owner_inventory_for_test(&self) -> OwnerInventory {
        OwnerInventory {
            cells: occupied(&self.cells, &self.budget)
                .into_iter()
                .map(|bits| {
                    (
                        bits,
                        self.cells
                            .get(Handle::<Cell>::from_bits(bits))
                            .unwrap()
                            .state,
                    )
                })
                .collect(),
            objects: occupied(&self.objects, &self.budget),
            tickets: occupied(&self.tickets, &self.budget),
            guards: occupied(&self.constructions, &self.budget),
            literals: occupied(&self.literals, &self.budget),
        }
    }
}
