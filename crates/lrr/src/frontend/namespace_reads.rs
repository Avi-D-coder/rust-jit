//! Bounded per-lookup observations emitted by the ordinary resolver worklists.
//! These are demands for current certificates, never namespace authority.
use crate::frontend::{macro_names::MacroPath, module_paths::ModulePath};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::scopes::MacroName,
    source::manifest::Namespace,
    syntax::outline::ScopeKey,
};
use std::collections::BTreeSet;
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum NamespaceRead {
    Scope {
        scope: ScopeKey,
        namespace: Namespace,
    },
    Name {
        scope: ScopeKey,
        namespace: Namespace,
        name: String,
    },
    CompilerEnvironment(Digest),
}
pub(crate) struct NamespaceReads {
    enabled: bool,
    rows: BTreeSet<NamespaceRead>,
    bytes: usize,
}
impl NamespaceReads {
    pub(crate) fn disabled() -> Self {
        Self {
            enabled: false,
            rows: BTreeSet::new(),
            bytes: 0,
        }
    }
    pub(crate) fn enabled() -> Self {
        Self {
            enabled: true,
            rows: BTreeSet::new(),
            bytes: 0,
        }
    }
    pub(crate) fn rows(&self) -> impl Iterator<Item = &NamespaceRead> {
        self.rows.iter()
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    fn insert(&mut self, row: NamespaceRead, heap: usize) -> Result<(), Diagnostic> {
        if self.rows.contains(&row) {
            return Ok(());
        }
        let bytes = heap
            .checked_add(256 + 16 * std::mem::size_of::<NamespaceRead>())
            .and_then(|n| n.checked_add(self.bytes))
            .ok_or_else(crate::runtime::arena::resource)?;
        self.rows.insert(row);
        self.bytes = bytes;
        Ok(())
    }
    pub(crate) fn scope(
        &mut self,
        namespace: Namespace,
        scope: &ScopeKey,
    ) -> Result<(), Diagnostic> {
        if !self.enabled {
            return Ok(());
        }
        let heap = scope.heap_bytes();
        self.insert(
            NamespaceRead::Scope {
                namespace,
                scope: scope.clone(),
            },
            heap,
        )
    }
    pub(crate) fn name(
        &mut self,
        namespace: Namespace,
        path: &MacroPath,
    ) -> Result<(), Diagnostic> {
        if !self.enabled {
            return Ok(());
        }
        self.components(namespace, path.unit, path.components())
    }
    pub(crate) fn module(&mut self, path: &ModulePath) -> Result<(), Diagnostic> {
        if !self.enabled {
            return Ok(());
        }
        self.components(Namespace::Type, path.unit, &path.names)
    }
    fn components(
        &mut self,
        namespace: Namespace,
        unit: Digest,
        parts: &[MacroName],
    ) -> Result<(), Diagnostic> {
        let (name, scope_parts) = match parts.split_last() {
            Some((name, scope)) => (Some(name), scope),
            None => (None, parts),
        };
        let text = parts
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.as_str().len()))
            .ok_or_else(crate::runtime::arena::resource)?;
        let scope = ScopeKey {
            crate_key: unit,
            module_path: scope_parts.iter().map(|p| p.as_str().to_owned()).collect(),
            owner: None,
        };
        self.scope(namespace, &scope)?;
        if let Some(name) = name {
            let heap = scope
                .heap_bytes()
                .checked_add(text)
                .ok_or_else(crate::runtime::arena::resource)?;
            self.insert(
                NamespaceRead::Name {
                    namespace,
                    scope,
                    name: name.as_str().to_owned(),
                },
                heap,
            )?;
        }
        Ok(())
    }
    pub(crate) fn dependency(
        &mut self,
        scope: &ScopeKey,
        alias: &MacroName,
    ) -> Result<(), Diagnostic> {
        if !self.enabled {
            return Ok(());
        }
        self.scope(Namespace::Type, scope)?;
        self.insert(
            NamespaceRead::Name {
                scope: scope.clone(),
                namespace: Namespace::Type,
                name: alias.as_str().to_owned(),
            },
            scope
                .heap_bytes()
                .checked_add(alias.as_str().len())
                .ok_or_else(crate::runtime::arena::resource)?,
        )?;
        self.insert(NamespaceRead::CompilerEnvironment(scope.crate_key), 0)
    }
}

/// Explicit per-lookup adapter. The local/import resolver remains one kernel;
/// observation is the only behavior supplied by this interface.
pub(crate) trait QualifiedNames {
    fn require_source(
        &self,
        source: &crate::source::changes::SourceSelection,
        outline: &crate::syntax::outline::Outline,
    ) -> Result<(), Diagnostic>;
    fn lookup_import_target(
        &self,
        import: &crate::syntax::imports::SourceImport,
        namespace: Namespace,
        edition: u16,
    ) -> Result<super::NameLookup, Diagnostic>;
    fn lookup_import_module(
        &self,
        import: &crate::syntax::imports::SourceImport,
        edition: u16,
    ) -> Result<crate::frontend::macro_names::ModuleLookup, Diagnostic>;
    fn lookup_glob_member(
        &self,
        import: &crate::syntax::imports::SourceImport,
        name: &str,
        namespace: Namespace,
        edition: u16,
    ) -> Result<super::NameLookup, Diagnostic>;
    fn lookup_unqualified(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        prelude: Option<&ScopeKey>,
    ) -> Result<super::NameLookup, Diagnostic>;
}
impl QualifiedNames for super::NameNamespaces {
    fn require_source(
        &self,
        source: &crate::source::changes::SourceSelection,
        outline: &crate::syntax::outline::Outline,
    ) -> Result<(), Diagnostic> {
        super::NameNamespaces::require_source(self, source, outline)
    }
    fn lookup_import_target(
        &self,
        import: &crate::syntax::imports::SourceImport,
        namespace: Namespace,
        edition: u16,
    ) -> Result<super::NameLookup, Diagnostic> {
        super::NameNamespaces::lookup_import_target(self, import, namespace, edition)
    }
    fn lookup_import_module(
        &self,
        import: &crate::syntax::imports::SourceImport,
        edition: u16,
    ) -> Result<crate::frontend::macro_names::ModuleLookup, Diagnostic> {
        super::NameNamespaces::lookup_import_module(self, import, edition)
    }
    fn lookup_glob_member(
        &self,
        import: &crate::syntax::imports::SourceImport,
        name: &str,
        namespace: Namespace,
        edition: u16,
    ) -> Result<super::NameLookup, Diagnostic> {
        super::NameNamespaces::lookup_glob_member(self, import, name, namespace, edition)
    }
    fn lookup_unqualified(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        prelude: Option<&ScopeKey>,
    ) -> Result<super::NameLookup, Diagnostic> {
        super::NameNamespaces::lookup_unqualified(self, namespace, name, caller, edition, prelude)
    }
}
pub(crate) struct ObservedNames<'a> {
    names: &'a super::NameNamespaces,
    reads: std::cell::RefCell<NamespaceReads>,
}
impl<'a> ObservedNames<'a> {
    pub(crate) fn new(names: &'a super::NameNamespaces) -> Self {
        Self {
            names,
            reads: std::cell::RefCell::new(NamespaceReads::enabled()),
        }
    }
    pub(crate) fn finish(self) -> NamespaceReads {
        self.reads.into_inner()
    }
}
impl QualifiedNames for ObservedNames<'_> {
    fn require_source(
        &self,
        source: &crate::source::changes::SourceSelection,
        outline: &crate::syntax::outline::Outline,
    ) -> Result<(), Diagnostic> {
        self.names.require_source(source, outline)
    }
    fn lookup_import_target(
        &self,
        import: &crate::syntax::imports::SourceImport,
        namespace: Namespace,
        edition: u16,
    ) -> Result<super::NameLookup, Diagnostic> {
        self.names.lookup_import_target_observed(
            import,
            namespace,
            edition,
            &mut self.reads.borrow_mut(),
        )
    }
    fn lookup_import_module(
        &self,
        import: &crate::syntax::imports::SourceImport,
        edition: u16,
    ) -> Result<crate::frontend::macro_names::ModuleLookup, Diagnostic> {
        self.names
            .lookup_import_module_observed(import, edition, &mut self.reads.borrow_mut())
    }
    fn lookup_glob_member(
        &self,
        import: &crate::syntax::imports::SourceImport,
        name: &str,
        namespace: Namespace,
        edition: u16,
    ) -> Result<super::NameLookup, Diagnostic> {
        self.names.lookup_glob_member_observed(
            import,
            name,
            namespace,
            edition,
            &mut self.reads.borrow_mut(),
        )
    }
    fn lookup_unqualified(
        &self,
        namespace: Namespace,
        name: &str,
        caller: &ScopeKey,
        edition: u16,
        prelude: Option<&ScopeKey>,
    ) -> Result<super::NameLookup, Diagnostic> {
        self.names.lookup_unqualified_observed(
            namespace,
            name,
            caller,
            edition,
            prelude,
            &mut self.reads.borrow_mut(),
        )
    }
}

/// Completion of the actual resolver read set, shared by Type and Value goals.
/// Neither a locator nor a callback can mint a scoped answer.
pub(crate) enum NamespaceReadCertification {
    Complete(Digest),
    PendingOwner(Digest),
    PendingScope(Digest),
    PendingName(Digest),
}
impl NamespaceReads {
    pub(crate) fn certify(
        &self,
        db: &mut crate::db::memo::Database,
        environment: &crate::expand::source_discovery::CompletedSourceEnvironment,
        seed: Digest,
        mut observe: impl FnMut(
            &crate::db::key::CurrentModuleNameKey,
            &crate::frontend::current_module_names::CurrentModuleNameObservation,
        ) -> Result<(), Diagnostic>,
    ) -> crate::db::poll::QueryPoll<NamespaceReadCertification> {
        use crate::{
            core::digest::{StableEncode, StableEncoder},
            db::{
                dependency::{InputKey, InputValue},
                key::{BucketKey, CurrentModuleNameKey, QueryKey},
                poll::QueryPoll,
                provider::SyntaxProvider,
                value::QueryValue,
            },
            expand::source_discovery::SourceScopeOwner,
            frontend::current_module_names::ModuleNameCompletion,
        };
        macro_rules! polled {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        if !self.enabled {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        let mut semantic = StableEncoder::new("lrr/v1/current-resolver-read-answers");
        semantic.digest(seed);
        semantic.u64(self.rows.len() as u64); // collector bounded100k
        for read in &self.rows {
            match read {
                NamespaceRead::CompilerEnvironment(unit) => {
                    semantic.u8(0);
                    semantic.digest(*unit);
                    // Includes absence. The reached resolved edge loci/name
                    // projections determine semantics, not an unrelated alias.
                    let _ = db.input(&InputKey::UnitCompileEnv(*unit));
                }
                NamespaceRead::Scope { scope, namespace }
                | NamespaceRead::Name {
                    scope, namespace, ..
                } => {
                    let source = match environment.scope_owner(scope) {
                        SourceScopeOwner::Selected(source) => source,
                        SourceScopeOwner::Missing | SourceScopeOwner::Ambiguous(_) => {
                            return QueryPoll::Ready(Ok(NamespaceReadCertification::PendingOwner(
                                scope.fingerprint(),
                            )));
                        }
                    };
                    semantic.record(scope);
                    semantic.u32(namespace.tag());
                    semantic.record(&source.selection().outline);
                    match read {
                        NamespaceRead::Scope { .. } => {
                            semantic.u8(1);
                            let Some(certificate) = checked!(db.current_scope_certificate(
                                source.selection(),
                                scope,
                                *namespace
                            )) else {
                                return QueryPoll::Ready(Ok(
                                    NamespaceReadCertification::PendingScope(scope.fingerprint()),
                                ));
                            };
                            let InputValue::DiscoverySeal(input) = certificate.as_ref() else {
                                return QueryPoll::Ready(Err(replay_error(
                                    "resolver scope certificate variant differs",
                                )));
                            };
                            let actual =
                                polled!(db.fetch(
                                    &SyntaxProvider,
                                    QueryKey::DiscoverySeal(input.seal.key())
                                ));
                            if !matches!(actual.value.as_ref(), QueryValue::DiscoverySeal(seal) if seal.key() == input.seal.key())
                            {
                                return QueryPoll::Ready(Err(replay_error(
                                    "resolver scope current seal differs",
                                )));
                            }
                        }
                        NamespaceRead::Name { name, .. } => {
                            semantic.u8(2);
                            semantic.string(name);
                            let request = CurrentModuleNameKey {
                                source: source.selection().outline.clone(),
                                path: source.selection().path.as_str().to_owned(),
                                bucket: BucketKey {
                                    scope: scope.clone(),
                                    namespace: *namespace,
                                    name: name.clone(),
                                },
                            };
                            let actual = polled!(db.fetch(
                                &SyntaxProvider,
                                QueryKey::CurrentModuleName(Box::new(request.clone()))
                            ));
                            let QueryValue::CurrentModuleName(answer) = actual.value.as_ref()
                            else {
                                return QueryPoll::Ready(Err(replay_error(
                                    "resolver name observation variant differs",
                                )));
                            };
                            if !answer.matches_request(&request) {
                                return QueryPoll::Ready(Err(replay_error(
                                    "resolver name request differs",
                                )));
                            }
                            semantic.digest(answer.semantic_identity());
                            if answer.completion() != ModuleNameCompletion::Complete {
                                return QueryPoll::Ready(Ok(
                                    NamespaceReadCertification::PendingName(
                                        answer.request_identity(),
                                    ),
                                ));
                            }
                            checked!(observe(&request, answer));
                        }
                        NamespaceRead::CompilerEnvironment(_) => unreachable!(),
                    }
                }
            }
        }
        QueryPoll::Ready(Ok(NamespaceReadCertification::Complete(semantic.finish())))
    }
}

fn replay_error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
