//! The run's impl member index, built once when the source graph is known.
//! Nominal values whose declaration has no `Drop` impl are cleaned up by drop
//! glue alone (fields in declaration order); a type with such an impl still
//! demands its guest `drop` witness.
use super::*;

impl RunMachine {
    pub(super) fn ensure_drop_inventory(&mut self, db: &Database) -> Result<(), Diagnostic> {
        if self.members.is_some() {
            return Ok(());
        }
        let Some(graph) = &self.graph else {
            return Ok(());
        };
        // One member index per completed environment, including impls and
        // traits emitted by macro expansion (and derives).
        let members = match graph {
            RunNames::Completed(completed) => completed.member_index()?,
            _ => {
                let sources: Vec<_> = self.sources.iter().collect();
                let mut members =
                    crate::frontend::member_index::MemberIndex::build(graph, &sources)?;
                for unit in db.generated_syntax_units() {
                    members.add_generated(graph, &unit)?;
                }
                Arc::new(members)
            }
        };
        if let Some(types) = members.drop_types() {
            self.store.install_drop_inventory(types);
        }
        if let Some(types) = members.copy_types() {
            self.store.install_copy_inventory(types);
        }
        self.store.install_owned_box(members.owned_box());
        self.members = Some(members);
        Ok(())
    }
}
