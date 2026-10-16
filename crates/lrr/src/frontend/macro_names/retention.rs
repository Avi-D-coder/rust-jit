//! Allocation-only cold traversal, shared by the ordinary namespace owner.
use super::*;
use crate::frontend::names::retention::Size;
impl MacroPath {
    pub(crate) fn retained_heap_bytes(&self) -> Result<usize, Diagnostic> {
        self.components.iter().try_fold(
            self.components
                .capacity()
                .checked_mul(std::mem::size_of::<MacroName>())
                .ok_or_else(crate::runtime::arena::resource)?,
            |n, name| {
                n.checked_add(name.retained_heap_bytes())
                    .ok_or_else(crate::runtime::arena::resource)
            },
        )
    }
}
fn bindings(s: &mut Size, row: &BindingSet) -> Result<(), Diagnostic> {
    s.tree::<MacroDefinition>(row.definitions.len())?;
    s.tree::<MacroPath>(row.reexports.len())?;
    for path in &row.reexports {
        s.path(path)?;
    }
    s.tree::<Digest>(row.pending.len())?;
    s.tree::<(MacroDefinition, BTreeSet<VisibilityScope>)>(row.definition_access.len())?;
    for scopes in row.definition_access.values() {
        s.tree::<VisibilityScope>(scopes.len())?;
        for v in scopes {
            s.visibility(v)?;
        }
    }
    s.tree::<(MacroPath, BTreeSet<VisibilityScope>)>(row.reexport_access.len())?;
    for (p, scopes) in &row.reexport_access {
        s.path(p)?;
        s.tree::<VisibilityScope>(scopes.len())?;
        for v in scopes {
            s.visibility(v)?;
        }
    }
    Ok(())
}
impl MacroNamespaces {
    pub(crate) fn retained_metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut s = Size::new::<Self>();
        s.tree::<(
            (crate::expand::scopes::ScopeKey, MacroName),
            BTreeMap<Digest, DeclarationModuleBinding>,
        )>(self.module_binding_rows.len())?;
        for ((_, name), rows) in &self.module_binding_rows {
            s.add(name.retained_heap_bytes())?;
            s.tree::<(Digest, DeclarationModuleBinding)>(rows.len())?;
            for row in rows.values() {
                s.add(row.name.retained_heap_bytes())?;
                s.scope(&row.scope)?;
                s.names(&row.route_components)?;
                s.visibility(&row.visibility)?;
            }
        }
        s.tree::<(Digest, (crate::expand::scopes::ScopeKey, MacroName))>(
            self.module_binding_identities.len(),
        )?;
        for (_, name) in self.module_binding_identities.values() {
            s.add(name.retained_heap_bytes())?;
        }
        s.tree::<(MacroPath, BTreeSet<BindingOrigin>)>(self.binding_origins.len())?;
        for (p, rows) in &self.binding_origins {
            s.path(p)?;
            s.tree::<BindingOrigin>(rows.len())?;
            for row in rows {
                s.visibility(&row.visibility)?;
                match &row.binding {
                    OriginBinding::Named(p) | OriginBinding::Glob(p) => s.path(p)?,
                    OriginBinding::Definition(_) => {}
                }
            }
        }
        s.tree::<(MacroPath, Bindings)>(self.bindings.len())?;
        for (p, row) in &self.bindings {
            s.path(p)?;
            bindings(&mut s, &row.root)?;
            s.tree::<(crate::syntax::token_wire::NameContextKey, BindingSet)>(row.item.len())?;
            for row in row.item.values() {
                bindings(&mut s, row)?;
            }
        }
        s.tree::<(MacroPath, BTreeMap<MacroPath, BTreeSet<VisibilityScope>>)>(
            self.glob_reexports.len(),
        )?;
        for (p, rows) in &self.glob_reexports {
            s.path(p)?;
            s.tree::<(MacroPath, BTreeSet<VisibilityScope>)>(rows.len())?;
            for (p, scopes) in rows {
                s.path(p)?;
                s.tree::<VisibilityScope>(scopes.len())?;
                for v in scopes {
                    s.visibility(v)?;
                }
            }
        }
        s.add(self.module_paths.retained_metadata_bytes()?)?;
        s.tree::<((Digest, MacroName), Digest)>(self.dependencies.len())?;
        s.tree::<((Digest, MacroName), Digest)>(self.compiler_imports.len())?;
        for (_, name) in self.dependencies.keys().chain(self.compiler_imports.keys()) {
            s.add(name.retained_heap_bytes())?;
        }
        s.tree::<(ScopeKey, Digest)>(self.preludes.len())?;
        for scope in self.preludes.keys() {
            s.scope(scope)?;
        }
        s.tree::<((Digest, MacroName), MacroDefinition)>(self.compiler_globals.len())?;
        for (_, name) in self.compiler_globals.keys() {
            s.add(name.retained_heap_bytes())?;
        }
        s.tree::<(Digest, BTreeMap<(Vec<u32>, Digest), MacroUseBinding>)>(self.macro_uses.len())?;
        for rows in self.macro_uses.values() {
            s.tree::<((Vec<u32>, Digest), MacroUseBinding)>(rows.len())?;
            for ((order, _), row) in rows {
                s.vector(order)?;
                if let Some(names) = &row.selectors {
                    s.tree::<MacroName>(names.len())?;
                    for name in names {
                        s.add(name.retained_heap_bytes())?;
                    }
                }
            }
        }
        s.tree::<(Digest, BTreeSet<Digest>)>(self.export_pending.len())?;
        for rows in self.export_pending.values() {
            s.tree::<Digest>(rows.len())?;
        }
        s.tree::<(Digest, Digest)>(self.body_owner_pending.len())?;
        s.tree::<ScopeKey>(self.implicit_disabled.len())?;
        for scope in &self.implicit_disabled {
            s.scope(scope)?;
        }
        Ok(s.finish())
    }
}
