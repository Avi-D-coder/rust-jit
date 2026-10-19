//! Cold storage bound for the existing module-prefix worklist.
use super::*;
use crate::frontend::names::retention::Size;
fn alias(s: &mut Size, row: &Alias) -> Result<(), Diagnostic> {
    s.module(&row.target)?;
    if let Some(canonical) = &row.canonical {
        s.module(canonical)?;
    }
    s.visibility(&row.visibility)?;
    s.scope(&row.origin)
}
impl ModulePaths {
    pub(crate) fn retained_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut s = Size::new::<Self>();
        s.tree::<(ModulePath, Binding)>(self.bindings.len())?;
        for (p, row) in &self.bindings {
            s.module(p)?;
            match row {
                Binding::Declared(v) => s.visibility(v)?,
                Binding::Alias(row) => alias(&mut s, row)?,
                Binding::ImportedAliases(rows) => {
                    s.vector(rows)?;
                    for row in rows {
                        alias(&mut s, row)?;
                    }
                }
            }
        }
        s.tree::<(ModulePath, BTreeMap<ModulePath, BTreeSet<VisibilityScope>>)>(self.weak.len())?;
        for (p, rows) in &self.weak {
            s.module(p)?;
            s.tree::<(ModulePath, BTreeSet<VisibilityScope>)>(rows.len())?;
            for (p, scopes) in rows {
                s.module(p)?;
                s.tree::<VisibilityScope>(scopes.len())?;
                for v in scopes {
                    s.visibility(v)?;
                }
            }
        }
        Ok(s.finish())
    }
}
