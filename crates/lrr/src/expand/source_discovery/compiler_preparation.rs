//! The SAME compiler/source route preparation can demand a real physical
//! module before installation. An unresolved graph never becomes runnable.
use super::*;
pub(crate) enum SourceCompilerPreparation {
    Ready(SourceDiscovery),
    NeedModule {
        discovery: SourceDiscovery,
        demand: crate::frontend::source_macro_routes::ModulePreparationDemand,
    },
}
impl SourceCompilerPreparation {
    pub(super) fn require_ready(self) -> Result<SourceDiscovery, Diagnostic> {
        match self {
            Self::Ready(discovery) => Ok(discovery),
            Self::NeedModule { demand, .. } => Err(demand.diagnostic()),
        }
    }
}
