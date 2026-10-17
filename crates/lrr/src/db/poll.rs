//! Unfinished input demand is outside both semantic query values and diagnostics.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::identity::ExpansionKey,
    source::snapshot::SourcePath,
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ExpansionRequest {
    pub key: ExpansionKey,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GeneratedSourceRequest {
    pub consumer_unit: Digest,
    pub compile_environment: Digest,
    pub path: SourcePath,
    pub expected_content: Option<Digest>,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum InputRequest {
    Discover {
        key: super::seals::SealKey,
    },
    CaptureCompileEnvironment {
        unit: Digest,
    },
    Expand(Box<ExpansionRequest>),
    CaptureGeneratedSource(GeneratedSourceRequest),
    CaptureModule {
        request: Box<crate::source::module_capture::ModuleCaptureRequest>,
        compile_environment: Digest,
    },
    CaptureInclude {
        request: Box<crate::source::include_paths::GeneratedSourceRequest>,
        compile_environment: Digest,
    },
}
impl InputRequest {
    pub fn validate(&self) -> Result<(), Diagnostic> {
        match self {
            Self::Discover { key } => key.validate(),
            Self::CaptureCompileEnvironment { .. } => Ok(()),
            Self::Expand(r) => r.key.validate(),
            Self::CaptureModule { request, .. } => request.metadata_bytes().map(|_| ()),
            Self::CaptureInclude { request, .. } => request
                .metadata_bytes()
                .map(|_| ())
                .ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-QUERY-REQUEST",
                        "include request metadata overflows".into(),
                    )
                }),
            Self::CaptureGeneratedSource(_) => Ok(()),
        }
    }
}
impl StableEncode for InputRequest {
    const DOMAIN: &'static str = "lrr/v2/query-input-request";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Discover { key } => {
                e.u8(4);
                e.record(key);
            }
            Self::CaptureCompileEnvironment { unit } => {
                e.u8(3);
                e.digest(*unit);
            }

            Self::Expand(r) => {
                e.u8(0);
                e.record(&r.key)
            }
            Self::CaptureModule {
                request,
                compile_environment,
            } => {
                e.u8(5);
                e.digest(request.identity());
                e.digest(*compile_environment);
            }
            Self::CaptureInclude {
                request,
                compile_environment,
            } => {
                e.u8(2);
                e.digest(request.identity());
                e.digest(*compile_environment);
            }
            Self::CaptureGeneratedSource(r) => {
                e.u8(1);
                e.digest(r.consumer_unit);
                e.digest(r.compile_environment);
                e.string(r.path.as_str());
                e.option(&r.expected_content, |e, d| e.digest(*d))
            }
        }
    }
}
#[derive(Debug, Clone)]
pub enum QueryPoll<T> {
    Ready(Result<T, Diagnostic>),
    NeedInput(InputRequest),
}
impl<T> QueryPoll<T> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> QueryPoll<U> {
        match self {
            Self::Ready(v) => QueryPoll::Ready(v.map(f)),
            Self::NeedInput(r) => QueryPoll::NeedInput(r),
        }
    }
    /// Explicit completion projection. Missing input stays distinct (None).
    pub fn into_ready(self) -> Option<Result<T, Diagnostic>> {
        match self {
            Self::Ready(v) => Some(v),
            Self::NeedInput(_) => None,
        }
    }
}
