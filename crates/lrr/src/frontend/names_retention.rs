//! Cold, checked retained allocation upper bounds. No semantic identity or
//! lookup is derived here; the completed owner caches the result once.
use super::*;
pub(in crate::frontend) struct Size {
    bytes: usize,
    rows: usize,
}
impl Size {
    pub(in crate::frontend) fn new<T>() -> Self {
        Self {
            bytes: std::mem::size_of::<T>(),
            rows: 0,
        }
    }
    pub(in crate::frontend) fn add(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "completed namespace allocation limit".into(),
                )
            })?;
        Ok(())
    }
    pub(in crate::frontend) fn tree<T>(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.rows = self
            .rows
            .checked_add(count)
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "completed namespace row limit".into(),
                )
            })?;
        self.add(
            count
                .checked_mul(
                    std::mem::size_of::<T>()
                        .checked_mul(16)
                        .and_then(|n| n.checked_add(256))
                        .ok_or_else(crate::runtime::arena::resource)?,
                )
                .ok_or_else(crate::runtime::arena::resource)?,
        )
    }
    pub(in crate::frontend) fn vector<T>(&mut self, values: &Vec<T>) -> Result<(), Diagnostic> {
        self.add(
            values
                .capacity()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(crate::runtime::arena::resource)?,
        )
    }
    pub(in crate::frontend) fn scope(&mut self, scope: &ScopeKey) -> Result<(), Diagnostic> {
        self.strings(&scope.module_path)
    }
    pub(in crate::frontend) fn strings(&mut self, names: &Vec<String>) -> Result<(), Diagnostic> {
        self.vector(names)?;
        for name in names {
            self.add(name.capacity())?;
        }
        Ok(())
    }
    pub(in crate::frontend) fn names(
        &mut self,
        names: &Vec<crate::expand::scopes::MacroName>,
    ) -> Result<(), Diagnostic> {
        self.vector(names)?;
        for name in names {
            self.add(name.retained_heap_bytes())?;
        }
        Ok(())
    }
    pub(in crate::frontend) fn path(&mut self, path: &NamePath) -> Result<(), Diagnostic> {
        self.add(path.retained_heap_bytes()?)
    }
    pub(in crate::frontend) fn module(
        &mut self,
        path: &super::super::module_paths::ModulePath,
    ) -> Result<(), Diagnostic> {
        self.names(&path.names)
    }
    pub(in crate::frontend) fn visibility(
        &mut self,
        visibility: &VisibilityScope,
    ) -> Result<(), Diagnostic> {
        if let VisibilityScope::Within { module_path, .. } = visibility {
            self.strings(module_path)?;
        }
        Ok(())
    }
    pub(in crate::frontend) fn finish(self) -> usize {
        self.bytes
    }
}
impl NameNamespaces {
    /// Cold graph storage, including distinct retained declaration payloads.
    /// BTree node slack is conservatively bounded per actual row.
    pub(crate) fn exclusive_retained_bytes(&self) -> Result<usize, Diagnostic> {
        let mut s = Size::new::<Self>();
        s.tree::<((Namespace, Digest), Arc<DeclaredItemName>)>(self.declared_names.len())?;
        s.tree::<(Digest, ScopeKey)>(self.module_frontier_scopes.len())?;
        for scope in self.module_frontier_scopes.values() {
            s.scope(scope)?;
        }
        s.tree::<Digest>(self.ordinary_definitions.len())?;
        s.tree::<(Digest, AnonymousImport)>(self.anonymous.len())?;
        for row in self.anonymous.values() {
            s.scope(&row.scope)?;
            s.vector(&row.definitions)?;
        }
        s.tree::<GlobItemEdge>(self.source_glob_items.len())?;
        s.tree::<GlobItemEdge>(self.manual_globs.len())?;
        for (_, a, b, v) in self.source_glob_items.iter().chain(&self.manual_globs) {
            s.path(a)?;
            s.path(b)?;
            s.visibility(v)?;
        }
        s.tree::<GlobModuleEdge>(self.source_glob_modules.len())?;
        for (a, b, v) in &self.source_glob_modules {
            s.module(a)?;
            s.module(b)?;
            s.visibility(v)?;
        }
        s.tree::<(crate::db::key::OutlineKey, Digest)>(self.source_proofs.len())?;
        for key in self.source_proofs.keys() {
            s.add(key.parse.profile.semantics.capacity())?;
            s.strings(&key.module_path)?;
        }
        for table in [&self.values, &self.types, &self.macros] {
            s.add(table.retained_metadata_bytes()?)?;
        }
        s.tree::<(ScopeKey, BTreeSet<Digest>)>(self.frontiers.len())?;
        for (scope, rows) in &self.frontiers {
            s.scope(scope)?;
            s.tree::<Digest>(rows.len())?;
        }
        s.tree::<(Digest, (crate::syntax::imports::SourceImport, u16))>(self.imports.len())?;
        for (row, _) in self.imports.values() {
            s.add(crate::syntax::imports::retained_bytes(row)?)?;
        }
        s.tree::<((Namespace, NamePath), BTreeSet<NamePath>)>(self.links.len())?;
        for ((_, path), rows) in &self.links {
            s.path(path)?;
            s.tree::<NamePath>(rows.len())?;
            for p in rows {
                s.path(p)?;
            }
        }
        s.tree::<((Namespace, Digest), VisibilityScope)>(self.visibility.len())?;
        for v in self.visibility.values() {
            s.visibility(v)?;
        }
        s.tree::<Digest>(self.modules.len())?;
        s.tree::<(Digest, EnumVariantDefinition)>(self.enum_variants.len())?;
        for member in self.enum_variants.values() {
            s.add(member.name.capacity())?;
        }
        s.tree::<(Digest, Vec<String>)>(self.enum_scopes.len())?;
        for (_, path) in &self.enum_scopes {
            s.strings(path)?;
        }
        s.tree::<Digest>(self.deferred_imports.len())?;
        s.tree::<Digest>(self.candidate_globs.len())?;
        s.tree::<(NamePath, super::super::module_paths::ModulePath)>(self.module_aliases.len())?;
        for (a, b) in &self.module_aliases {
            s.path(a)?;
            s.module(b)?;
        }
        s.tree::<(NamePath, BTreeSet<super::super::module_paths::ModulePath>)>(
            self.named_module_links.len(),
        )?;
        for (p, rows) in &self.named_module_links {
            s.path(p)?;
            s.tree::<super::super::module_paths::ModulePath>(rows.len())?;
            for p in rows {
                s.module(p)?;
            }
        }
        s.tree::<(Digest, NameDefinition)>(self.macro_import_facets.len())?;
        s.tree::<(NamePath, BTreeSet<super::super::module_paths::ModulePath>)>(
            self.glob_module_links.len(),
        )?;
        for (p, rows) in &self.glob_module_links {
            s.path(p)?;
            s.tree::<super::super::module_paths::ModulePath>(rows.len())?;
            for p in rows {
                s.module(p)?;
            }
        }
        // The graph owns these opaque declaration receipts. They have no
        // parser/record Arcs; deduplicate genuine allocation handles once.
        let mut seen = std::collections::BTreeSet::new();
        for root in self.declared_names.values() {
            if seen.insert(Arc::as_ptr(root)) {
                s.add(root.metadata_bytes()?)?;
                s.add(2 * std::mem::size_of::<usize>())?;
            }
        }
        Ok(s.finish())
    }
}
use std::collections::BTreeSet;
