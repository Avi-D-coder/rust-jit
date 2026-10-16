//! Exact configured-source module requests and adopted candidate leases.
//! This transport publishes no ordinary source/parser/body inputs and does not
//! yet activate a child in SourceDiscovery or certify export completeness.
pub use crate::db::memo::macro_export_inputs::MacroExportSourceObservation;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{DependencyStamp, InputKey, InputValue},
        memo::Database,
        poll::{InputRequest, QueryPoll},
        provider::SyntaxProvider,
    },
    expand::source_declarative::macro_exports::SourceMacroExportFacet,
    frontend::source_module_declarations::SourceModuleDeclaration,
    source::{
        include_paths::{IncludeAuthorities, IncludeRootKind},
        module_capture::AdoptedModuleCapture,
    },
};
use std::{path::Path, sync::Arc};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-EXPORT-PUBLICATION", message.into())
}
/// A cold request bound to actual configured source owners and declarations.
/// Private construction prevents a caller module_path from becoming ancestry.
pub struct MacroExportModuleRequest {
    parent: Arc<SourceMacroExportFacet>,
    child: Arc<SourceMacroExportFacet>,
    parent_chain: Option<Arc<MacroExportModuleAdoption>>,
    parent_role: Arc<MacroExportSourceObservation>,
    child_role: Arc<MacroExportSourceObservation>,
    declaration: SourceModuleDeclaration,
    input: InputRequest,
    identity: Digest,
    depth: usize,
    bytes: usize,
    admission_bytes: usize,
}
impl std::fmt::Debug for MacroExportModuleRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacroExportModuleRequest")
            .field("identity", &self.identity)
            .field("depth", &self.depth)
            .finish_non_exhaustive()
    }
}
impl MacroExportModuleRequest {
    /// Pure preparation from genuine current source receipts. Capture remains
    /// the existing explicit coordinator effect for the returned InputRequest.
    pub fn prepare(
        db: &mut Database,
        parent: Arc<SourceMacroExportFacet>,
        parent_chain: Option<Arc<MacroExportModuleAdoption>>,
        child: Arc<SourceMacroExportFacet>,
        declaration: SourceModuleDeclaration,
        authorities: &IncludeAuthorities,
    ) -> Result<Arc<Self>, Diagnostic> {
        let parent_role = Arc::new(
            db.macro_export_source_observation(parent.source().selection(), parent.unit_graph())?
                .ok_or_else(|| fault("macro-export module parent role is unpublished"))?,
        );
        let child_role = Arc::new(
            db.macro_export_source_observation(child.source().selection(), child.unit_graph())?
                .ok_or_else(|| fault("macro-export module child role is unpublished"))?,
        );
        parent_role.require_facet(&parent)?;
        child_role.require_facet(&child)?;
        if parent.unit() != child.unit()
            || parent.unit_graph() != child.unit_graph()
            || parent.preparation() != child.preparation()
            || parent.environment() != child.environment()
            || !parent.source().same_publication(child.source())
            || parent.source().selection().outline.parse.profile
                != child.source().selection().outline.parse.profile
            || parent.source().selection().outline.cfg != child.source().selection().outline.cfg
            || declaration.source().selection() != parent.source().selection()
            || !declaration.source().same_publication(parent.source())
            || declaration.scope() != &child.source().outline().root_scope
            || declaration.scope().owner.is_some()
            || declaration.scope().module_path.len()
                <= parent.source().outline().root_scope.module_path.len()
            || !declaration
                .scope()
                .module_path
                .starts_with(&parent.source().outline().root_scope.module_path)
        {
            return Err(fault(
                "macro-export module declaration/owners/profile/configuration differ",
            ));
        }
        let (depth, directory) = if let Some(chain) = &parent_chain {
            chain.revalidate(db)?;
            if chain.request.child.identity() != parent.identity() {
                return Err(fault(
                    "macro-export module parent adoption belongs to another source",
                ));
            }
            (
                chain
                    .request
                    .depth
                    .checked_add(1)
                    .filter(|depth| *depth <= 256)
                    .ok_or_else(|| fault("macro-export module ancestry depth limit"))?,
                chain.capture.record().directory(),
            )
        } else {
            if !parent.source().selection().outline.module_path.is_empty() {
                return Err(fault(
                    "macro-export module parent requires its own adopted ancestry",
                ));
            }
            (
                1,
                Path::new(parent.source().selection().path.as_str())
                    .parent()
                    .unwrap_or(Path::new("")),
            )
        };
        let root = authorities
            .root(authorities.unique_source_root()?)
            .ok_or_else(|| fault("macro-export module source authority is absent"))?;
        if authorities.unit() != parent.unit()
            || root.path != parent.package_root()
            || root.kind != IncludeRootKind::Source
            || parent.package_root() != child.package_root()
        {
            return Err(fault(
                "macro-export module authority differs from configured source root",
            ));
        }
        // The existing shared layout/candidate machinery derives every path
        // from the actual declaration and actual adopted/root directory.
        let input = declaration.capture_request(authorities, directory, parent.environment())?;
        input.validate()?;
        let InputRequest::CaptureModule { request, .. } = &input else {
            return Err(fault("macro-export module request kind differs"));
        };
        if request.candidates().iter().any(|candidate| {
            candidate.generation() != parent.preparation()
                || candidate.from().root != root.identity
                || candidate.root() != root.identity
                || candidate.root_kind() != IncludeRootKind::Source
                || candidate.from().path != parent.source().selection().path
        }) {
            return Err(fault(
                "macro-export module request preparation/source authority differs",
            ));
        }
        if !request.candidates().iter().any(|candidate| {
            candidate.path() == &child.source().selection().path && candidate.unit() == child.unit()
        }) {
            return Err(fault(
                "configured macro-export child is not a declared source candidate",
            ));
        }
        let bytes = std::mem::size_of::<Self>()
            .checked_add(2 * std::mem::size_of::<usize>())
            .and_then(|n| n.checked_add(declaration.retained_metadata_bytes().ok()?))
            .and_then(|n| n.checked_add(request.metadata_bytes().ok()?))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| fault("macro-export module request metadata limit"))?;
        let admission_bytes = bytes
            .checked_add(parent.metadata_bytes())
            .and_then(|n| n.checked_add(child.metadata_bytes()))
            .and_then(|n| n.checked_add(parent_role.metadata_bytes()))
            .and_then(|n| n.checked_add(child_role.metadata_bytes()))
            .and_then(|n| n.checked_add(4 * std::mem::size_of::<usize>()))
            .and_then(|n| {
                n.checked_add(
                    parent_chain
                        .as_ref()
                        .map_or(0, |chain| chain.admission_bytes),
                )
            })
            .filter(|n| *n <= 320 * 1024 * 1024)
            .ok_or_else(|| fault("macro-export module ancestry retained admission limit"))?;
        let mut e = StableEncoder::new("lrr/v1/macro-export-module-source-request");
        e.digest(parent.identity());
        e.digest(child.identity());
        e.option(&parent_chain, |e, chain| e.digest(chain.identity));
        e.record(&declaration);
        e.digest(input.fingerprint());
        for dependency in parent_role
            .dependencies()
            .iter()
            .chain(child_role.dependencies())
        {
            let DependencyStamp::Input { key, fingerprint } = dependency else {
                return Err(fault("macro-export role dependency kind differs"));
            };
            e.record(key);
            e.digest(*fingerprint);
        }
        Ok(Arc::new(Self {
            parent,
            child,
            parent_chain,
            parent_role,
            child_role,
            declaration,
            input,
            identity: e.finish(),
            depth,
            bytes,
            admission_bytes,
        }))
    }
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn input(&self) -> &InputRequest {
        &self.input
    }
    pub fn parent(&self) -> &Arc<SourceMacroExportFacet> {
        &self.parent
    }
    pub fn child(&self) -> &Arc<SourceMacroExportFacet> {
        &self.child
    }
    pub fn parent_adoption(&self) -> Option<&Arc<MacroExportModuleAdoption>> {
        self.parent_chain.as_ref()
    }
    pub fn declaration(&self) -> &SourceModuleDeclaration {
        &self.declaration
    }
    /// Exclusive request/declaration storage; actual facet, ancestry and role
    /// Arc allocations are separate retained roots, never omitted totals.
    pub fn exclusive_metadata_bytes(&self) -> usize {
        self.bytes
    }
    /// Conservative original-source graph bound, deliberately charging shared
    /// ancestors/facets repeatedly. It is not a deduplicated cache-pool total.
    pub fn retained_admission_bytes(&self) -> usize {
        self.admission_bytes
    }
    pub fn role_roots(&self) -> [&Arc<MacroExportSourceObservation>; 2] {
        [&self.parent_role, &self.child_role]
    }
    pub fn role_dependencies(&self) -> impl Iterator<Item = &DependencyStamp> {
        self.parent_role
            .dependencies()
            .iter()
            .chain(self.child_role.dependencies())
    }
    pub fn revalidate(&self, db: &mut Database) -> Result<(), Diagnostic> {
        if let Some(chain) = &self.parent_chain {
            chain.revalidate(db)?;
        }
        for (facet, expected) in [
            (&self.parent, &self.parent_role),
            (&self.child, &self.child_role),
        ] {
            let actual = db
                .macro_export_source_observation(facet.source().selection(), facet.unit_graph())?
                .ok_or_else(|| fault("macro-export module source role is no longer published"))?;
            actual.require_facet(facet)?;
            if actual.dependencies() != expected.dependencies() {
                return Err(fault("macro-export module selected preparation changed"));
            }
        }
        Ok(())
    }
    /// Demand only the existing genuinely adopted ModuleRecord. This never
    /// captures files or registers an ordinary prepared source/query provider.
    pub fn adopted(
        self: &Arc<Self>,
        db: &mut Database,
    ) -> QueryPoll<Arc<MacroExportModuleAdoption>> {
        if let Err(error) = self.revalidate(db) {
            return QueryPoll::Ready(Err(error));
        }
        let capture = match SyntaxProvider::module_capture(db, &self.input) {
            QueryPoll::Ready(Ok(capture)) => capture,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready((|| {
            let InputRequest::CaptureModule { request, .. } = &self.input else {
                return Err(fault("macro-export module request kind differs"));
            };
            capture.record().validate(request)?;
            if capture.input_identity() != self.input.fingerprint() {
                return Err(fault("macro-export module adopted request differs"));
            }
            let selected = capture.record().selected()?;
            let child = self.child.source().selection();
            if selected.location().unit != self.child.unit()
                || selected.location().path != child.path
                || selected.blob().content() != child.outline.parse.content
                || selected.blob().bytes() != self.child.source().captured_macro_blob()?.bytes()
            {
                return Err(fault(
                    "macro-export module selected candidate differs from configured child",
                ));
            }
            let key = InputKey::ModuleRecord(self.input.fingerprint());
            let current = db
                .input(&key)
                .ok_or_else(|| fault("macro-export module adopted input disappeared"))?;
            if !matches!(current.as_ref(), InputValue::ModuleRecord { input, captured }
                if *input == self.input.fingerprint() && captured.identity() == capture.record().identity())
            {
                return Err(fault("macro-export module adopted input changed"));
            }
            let dependency = DependencyStamp::Input {
                key,
                fingerprint: current.fingerprint(),
            };
            let mut e = StableEncoder::new("lrr/v1/macro-export-adopted-module-source");
            e.digest(self.identity);
            e.digest(capture.record().identity());
            let bytes = std::mem::size_of::<MacroExportModuleAdoption>()
                .checked_add(2 * std::mem::size_of::<usize>())
                .ok_or_else(|| fault("macro-export module adoption metadata overflow"))?;
            let mut admission_bytes = self
                .admission_bytes
                .checked_add(bytes)
                .and_then(|n| {
                    n.checked_add(
                        std::mem::size_of::<AdoptedModuleCapture>()
                            + 6 * std::mem::size_of::<usize>(),
                    )
                })
                .and_then(|n| n.checked_add(capture.input_root().estimated_bytes().ok()?))
                .filter(|n| *n <= 320 * 1024 * 1024)
                .ok_or_else(|| fault("macro-export adopted module retained admission limit"))?;
            for blob in capture.input_root().source_blobs() {
                admission_bytes = admission_bytes
                    .checked_add(blob.bytes().len())
                    .and_then(|n| n.checked_add(2 * std::mem::size_of::<usize>()))
                    .filter(|n| *n <= 320 * 1024 * 1024)
                    .ok_or_else(|| fault("macro-export adopted source buffer admission limit"))?;
            }
            Ok(Arc::new(MacroExportModuleAdoption {
                request: Arc::clone(self),
                capture,
                dependency,
                identity: e.finish(),
                bytes,
                admission_bytes,
            }))
        })())
    }
}
/// An actual uniquely selected adopted child. This lease exposes no ordinary
/// source input map and is not yet an export seal or activation permission.
pub struct MacroExportModuleAdoption {
    request: Arc<MacroExportModuleRequest>,
    capture: Arc<AdoptedModuleCapture>,
    dependency: DependencyStamp,
    identity: Digest,
    bytes: usize,
    admission_bytes: usize,
}
impl std::fmt::Debug for MacroExportModuleAdoption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MacroExportModuleAdoption")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}
impl MacroExportModuleAdoption {
    pub fn identity(&self) -> Digest {
        self.identity
    }
    pub fn request(&self) -> &Arc<MacroExportModuleRequest> {
        &self.request
    }
    pub fn capture(&self) -> &Arc<AdoptedModuleCapture> {
        &self.capture
    }
    pub fn module_dependency(&self) -> &DependencyStamp {
        &self.dependency
    }
    pub fn exclusive_metadata_bytes(&self) -> usize {
        self.bytes
    }
    pub fn retained_admission_bytes(&self) -> usize {
        self.admission_bytes
    }
    pub(crate) fn require_child(
        &self,
        source: &crate::source::changes::PreparedSourceView,
        role: &MacroExportSourceObservation,
    ) -> Result<(), Diagnostic> {
        role.require_facet(&self.request.child)?;
        if source.selection() != self.request.child.source().selection()
            || !source.same_publication(self.request.child.source())
        {
            return Err(fault("macro-export adopted child prepared owner differs"));
        }
        Ok(())
    }
    /// The real complete seal must bind both actual source contributions and
    /// every selected role/module dependency. A capture lease is not a seal.
    pub fn validate_discovery(
        &self,
        db: &mut Database,
        input: &crate::db::seals::DiscoveryInput,
    ) -> Result<(), Diagnostic> {
        self.revalidate(db)?;
        let mut current = Some(self);
        let mut depth = 0usize;
        while let Some(module) = current {
            depth = depth
                .checked_add(1)
                .filter(|depth| *depth <= 256)
                .ok_or_else(|| fault("macro-export discovery ancestry depth limit"))?;
            for (facet, role) in [
                (&module.request.parent, &module.request.parent_role),
                (&module.request.child, &module.request.child_role),
            ] {
                if !input
                    .sources
                    .iter()
                    .any(|source| source == facet.source().selection())
                {
                    return Err(fault("macro-export module source is absent from discovery"));
                }
                role.validate_discovery(input)?;
            }
            let DependencyStamp::Input { key, fingerprint } = &module.dependency else {
                return Err(fault("macro-export module dependency kind differs"));
            };
            if input.compiler_inputs.get(key) != Some(fingerprint)
                || input
                    .seal
                    .input_dependencies()
                    .binary_search(&(key.fingerprint(), *fingerprint))
                    .is_err()
            {
                return Err(fault(
                    "macro-export adopted module is absent from complete discovery",
                ));
            }
            current = module.request.parent_adoption().map(Arc::as_ref);
        }
        Ok(())
    }
    pub fn revalidate(&self, db: &mut Database) -> Result<(), Diagnostic> {
        self.request.revalidate(db)?;
        let DependencyStamp::Input { key, fingerprint } = &self.dependency else {
            return Err(fault("macro-export module dependency kind differs"));
        };
        let current = db
            .input(key)
            .ok_or_else(|| fault("macro-export module adoption is no longer published"))?;
        if current.fingerprint() != *fingerprint {
            return Err(fault("macro-export module adoption changed"));
        }
        Ok(())
    }
}
