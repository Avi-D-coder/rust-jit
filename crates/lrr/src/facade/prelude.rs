//! Compiler-owned async trait identities. Names resolve normally; execution is denied.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    frontend::names::{NameDefinition, NameNamespaces},
    source::manifest::Namespace,
    syntax::{outline::ScopeKey, visibility::VisibilityScope},
};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AsyncTrait {
    Future,
    IntoFuture,
    AsyncFn,
    AsyncFnMut,
    AsyncFnOnce,
}
impl AsyncTrait {
    pub const ALL: [Self; 5] = [
        Self::Future,
        Self::IntoFuture,
        Self::AsyncFn,
        Self::AsyncFnMut,
        Self::AsyncFnOnce,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Future => "Future",
            Self::IntoFuture => "IntoFuture",
            Self::AsyncFn => "AsyncFn",
            Self::AsyncFnMut => "AsyncFnMut",
            Self::AsyncFnOnce => "AsyncFnOnce",
        }
    }
    pub fn module(self) -> &'static str {
        match self {
            Self::Future | Self::IntoFuture => "future",
            _ => "ops",
        }
    }
    pub fn in_prelude(self, edition: u16) -> bool {
        match self {
            Self::Future | Self::IntoFuture => edition == 2024,
            _ => matches!(edition, 2015 | 2018 | 2021 | 2024),
        }
    }
    fn tag(self) -> u32 {
        match self {
            Self::Future => 0,
            Self::IntoFuture => 1,
            Self::AsyncFn => 2,
            Self::AsyncFnMut => 3,
            Self::AsyncFnOnce => 4,
        }
    }
}
/// Fixed catalog: callers cannot register ordinary definitions as async facades.
#[derive(Default)]
pub struct AsyncTraitCatalog;
impl AsyncTraitCatalog {
    pub fn new() -> Self {
        Self
    }
    pub fn core_unit(&self) -> Digest {
        super::unit(&crate::core::profile::ProfileIdentity::current(), "core")
    }
    pub fn std_unit(&self) -> Digest {
        super::unit(&crate::core::profile::ProfileIdentity::current(), "std")
    }
    pub fn definition(&self, kind: AsyncTrait) -> NameDefinition {
        let mut e = StableEncoder::new("lrr/v2/async-facade-trait");
        e.digest(self.core_unit());
        e.u32(kind.tag());
        NameDefinition {
            namespace: Namespace::Type,
            identity: e.finish(),
            defining_unit: self.core_unit(),
            definition_edition: 2024,
        }
    }
    pub fn prelude_scope(&self, edition: u16, standard: bool) -> Result<ScopeKey, Diagnostic> {
        let version = match edition {
            2015 => "rust_2015",
            2018 => "rust_2018",
            2021 => "rust_2021",
            2024 => "rust_2024",
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-TOOLCHAIN-CAPABILITY",
                    "unknown prelude source edition".into(),
                ));
            }
        };
        Ok(ScopeKey {
            crate_key: if standard {
                self.std_unit()
            } else {
                self.core_unit()
            },
            module_path: vec!["prelude".into(), version.into()],
            owner: None,
        })
    }
    /// Add bounded synthetic routes to the existing deterministic namespace index.
    /// No Rust toolchain source, host library or runtime symbol is loaded.
    pub fn install(&self, graph: &mut NameNamespaces) -> Result<(), Diagnostic> {
        for unit in [self.core_unit(), self.std_unit()] {
            for path in [&[][..], &["future"][..], &["ops"][..], &["prelude"][..]] {
                graph.declare_module(unit, path, 2024, VisibilityScope::Public)?;
            }
            for kind in AsyncTrait::ALL {
                graph.define(
                    NameNamespaces::path(unit, &[kind.module(), kind.name()], 2024)?,
                    self.definition(kind),
                    VisibilityScope::Public,
                )?;
            }
            for edition in [2015, 2018, 2021, 2024] {
                let scope = self.prelude_scope(edition, unit == self.std_unit())?;
                let parts = scope
                    .module_path
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>();
                graph.declare_module(unit, &parts, edition, VisibilityScope::Public)?;
                for kind in AsyncTrait::ALL
                    .into_iter()
                    .filter(|kind| kind.in_prelude(edition))
                {
                    let mut route = parts.clone();
                    route.push(kind.name());
                    graph.define(
                        NameNamespaces::path(unit, &route, edition)?,
                        self.definition(kind),
                        VisibilityScope::Public,
                    )?;
                }
            }
        }
        Ok(())
    }
    /// Select after ordinary path/namespace lookup, preserving aliases and shadows.
    pub fn select(&self, definition: &NameDefinition) -> Result<Option<AsyncTrait>, Diagnostic> {
        for kind in AsyncTrait::ALL {
            let expected = self.definition(kind);
            if definition.identity == expected.identity {
                if definition != &expected {
                    return Err(Diagnostic::new(
                        "LRR-E-FACADE-IDENTITY",
                        "async facade definition provenance differs".into(),
                    ));
                }
                return Ok(Some(kind));
            }
        }
        Ok(None)
    }
    /// Call only for a reached executable trait request. Pure name disclosure is valid.
    pub fn require_execution(&self, definition: &NameDefinition) -> Result<(), Diagnostic> {
        if let Some(kind) = self.select(definition)? {
            return Err(Diagnostic::new(
                "LRR-E-FEATURE-ASYNC",
                format!(
                    "{} executable trait semantics are outside the live profile",
                    kind.name()
                ),
            ));
        }
        Ok(())
    }
}
