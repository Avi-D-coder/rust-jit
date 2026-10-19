//! Owned compile-input descriptor boundary; no upstream type reaches the frontend.
use crate::core::{diagnostic::Diagnostic, digest::Digest};
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CompileInputKind {
    Env,
    OptionEnv,
    Concat,
    Include,
    IncludeStr,
    IncludeBytes,
    ConcatBytes,
}
#[derive(Debug, Clone)]
pub struct CompileInputDescriptor {
    name: &'static str,
    kind: CompileInputKind,
    identity: Digest,
}
impl CompileInputDescriptor {
    pub fn name(&self) -> &'static str {
        self.name
    }
    pub fn kind(&self) -> CompileInputKind {
        self.kind
    }
    pub fn definition_identity(&self) -> Digest {
        self.identity
    }
}
pub fn descriptor(name: &str) -> Result<Option<CompileInputDescriptor>, Diagnostic> {
    use lrr_expand_adapter::compile_inputs::CompileInputKind as K;
    lrr_expand_adapter::compile_inputs::descriptor(name)
        .map_err(|e| Diagnostic::new(e.code, e.message))
        .map(|d| {
            d.map(|d| CompileInputDescriptor {
                name: d.name(),
                identity: Digest(d.definition_identity().0),
                kind: match d.kind() {
                    K::Env => CompileInputKind::Env,
                    K::OptionEnv => CompileInputKind::OptionEnv,
                    K::Concat => CompileInputKind::Concat,
                    K::Include => CompileInputKind::Include,
                    K::IncludeStr => CompileInputKind::IncludeStr,
                    K::IncludeBytes => CompileInputKind::IncludeBytes,
                    K::ConcatBytes => CompileInputKind::ConcatBytes,
                },
            })
        })
}
