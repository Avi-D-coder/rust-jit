use super::key::{BucketKey, QueryKey};
use crate::{
    core::digest::{Digest, StableEncode, StableEncoder},
    source::snapshot::{CfgSet, SourceBlob},
    syntax::outline::{BodyHandle, HeaderRecipe, ScopeBlocker, ScopeKey, ScopeMembers},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum InputKey {
    MacroExportSource(Digest),
    SourceDependencies,
    ScopeCertificateSelection(Digest),
    GeneratedSyntax(Digest),
    RetainedSyntax(Digest),
    DiscoverySeal(super::seals::SealKey),
    CapturedSource(Digest),
    DefinitionSource(Digest),
    DeclarationContext(Digest),
    ExpansionEpoch,
    UnitCompileEnv(Digest),
    ModuleRecord(Digest),
    IncludeRecord(Digest),
    ExpansionRecord(Digest),
    SourceBlob(Digest),
    DefinitionHeader(Digest),
    DefinitionBody(Digest),
    ScopeMembers(ScopeKey),
    ScopeBlockers(ScopeKey),
    NameCandidates(BucketKey),
    CrateCfg(Digest),
    CompileEnv(String),
}
#[derive(Debug, Clone)]
pub enum InputValue {
    MacroExportSource(Box<super::MacroExportSourceReceipt>),
    ScopeCertificateSelection {
        selection: Digest,
        candidate_set: Digest,
    },
    SourceDependencies(Arc<crate::source::dependencies::SourceDependencies>),
    GeneratedSyntax(Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>),
    RetainedSyntax(Arc<super::retained_syntax::RetainedSyntaxInput>),
    DiscoverySeal(Box<super::seals::DiscoveryInput>),
    CapturedSource(crate::source::changes::SourceSelection),
    DefinitionSource(crate::syntax::parse::ParseKey),
    DeclarationContext(crate::syntax::outline::DeclarationContext),
    ExpansionEpoch(u64),
    UnitCompileEnv(Arc<crate::expand::builtins::FrozenCompileEnvironment>),
    ModuleRecord {
        input: Digest,
        captured: Arc<crate::source::module_capture::CapturedModule>,
    },
    IncludeRecord {
        input: Digest,
        captured: crate::source::included::CapturedInclude,
    },
    ExpansionRecord(crate::expand::record::ExpansionRecord),
    SourceBlob(SourceBlob),
    DefinitionHeader(HeaderRecipe),
    DefinitionBody(BodyHandle),
    ScopeMembers(ScopeMembers),
    ScopeBlockers(Vec<ScopeBlocker>),
    NameCandidates(Vec<Digest>),
    CrateCfg(CfgSet),
    CompileEnv(String),
}
impl InputValue {
    pub(crate) fn source_blob(&self) -> Option<&SourceBlob> {
        match self {
            Self::SourceBlob(blob) => Some(blob),
            Self::IncludeRecord { captured, .. } => Some(captured.blob()),
            _ => None,
        }
    }
    pub(crate) fn source_blobs(&self) -> impl Iterator<Item = &SourceBlob> {
        self.source_blob().into_iter().chain(
            match self {
                Self::ModuleRecord { captured, .. } => Some(captured),
                _ => None,
            }
            .into_iter()
            .flat_map(|captured| captured.observations().iter())
            .filter_map(|observation| match observation {
                crate::source::module_capture::ModuleObservation::Present(record) => {
                    Some(record.blob())
                }
                _ => None,
            }),
        )
    }
    /// Owned payload capacities; source byte storage is accounted separately.
    pub(crate) fn estimated_bytes(&self) -> Result<usize, crate::core::diagnostic::Diagnostic> {
        let mut bytes = std::mem::size_of::<Self>();
        let mut add = |n: usize| {
            bytes = bytes
                .checked_add(n)
                .ok_or(crate::core::diagnostic::Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "input metadata accounting overflow".into(),
                ))?;
            Ok::<_, crate::core::diagnostic::Diagnostic>(())
        };
        match self {
            Self::MacroExportSource(receipt) => add(receipt.metadata_bytes())?,
            Self::SourceDependencies(graph) => add(graph.metadata_bytes()?)?,
            Self::GeneratedSyntax(input) => add(input.metadata_bytes()?)?,
            Self::RetainedSyntax(input) => add(input.metadata_bytes()?)?,
            Self::DiscoverySeal(input) => add(input.metadata_bytes()?)?,
            Self::CapturedSource(source) => add(source.metadata_bytes()?)?,
            Self::ExpansionEpoch(_) | Self::ScopeCertificateSelection { .. } => {}
            Self::DefinitionSource(k) => add(k.profile.semantics.capacity())?,
            Self::UnitCompileEnv(env) => add(env.allocated_bytes()?)?,
            Self::ModuleRecord { captured, .. } => add(captured.metadata_bytes()?)?,
            Self::IncludeRecord { captured, .. } => {
                add(captured
                    .metadata_bytes()
                    .ok_or(crate::core::diagnostic::Diagnostic::new(
                        "LRR-E-METADATA-LIMIT",
                        "include record size overflow".into(),
                    ))?)?
            }
            Self::ExpansionRecord(record) => add(record.estimated_bytes()?)?,
            Self::SourceBlob(_) | Self::DefinitionBody(_) => {}
            Self::DefinitionHeader(v) => add(v.estimated_bytes())?,
            Self::DeclarationContext(v) => add(v.scope.heap_bytes())?,
            Self::CompileEnv(v) => add(v.capacity())?,
            Self::NameCandidates(v) => add(std::mem::size_of::<Digest>() * v.capacity())?,
            Self::ScopeBlockers(v) => add(std::mem::size_of::<ScopeBlocker>() * v.capacity())?,
            Self::ScopeMembers(v) => {
                add(std::mem::size_of::<ScopeBlocker>() * v.blockers.capacity())?;
                for ((_, name), values) in &v.candidates {
                    add(64
                        + std::mem::size_of::<(crate::source::manifest::Namespace, String)>()
                        + std::mem::size_of::<Vec<Digest>>())?;
                    add(name.capacity())?;
                    add(values.capacity() * std::mem::size_of::<Digest>())?;
                }
            }
            Self::CrateCfg(v) => {
                add(v.metadata_bytes()?)?;
            }
        }
        Ok(bytes)
    }
    pub fn matches(&self, k: &InputKey) -> bool {
        if let (Self::MacroExportSource(receipt), InputKey::MacroExportSource(location)) = (self, k)
        {
            return receipt.location_identity() == *location;
        }
        if let (
            Self::ScopeCertificateSelection { selection, .. },
            InputKey::ScopeCertificateSelection(key),
        ) = (self, k)
        {
            return selection == key;
        }
        if let (Self::RetainedSyntax(input), InputKey::RetainedSyntax(key)) = (self, k) {
            return input.key().content == *key;
        }
        if let (Self::GeneratedSyntax(input), InputKey::GeneratedSyntax(key)) = (self, k) {
            return input.key.content == *key;
        }
        if let (Self::DiscoverySeal(input), InputKey::DiscoverySeal(key)) = (self, k) {
            return input.seal.key() == *key;
        }
        if let (Self::CapturedSource(source), InputKey::CapturedSource(key)) = (self, k) {
            return source.location_identity() == *key
                && source.canonical().is_ok_and(|s| s == *source);
        }
        if let (Self::UnitCompileEnv(env), InputKey::UnitCompileEnv(unit)) = (self, k) {
            return env.unit() == *unit;
        }

        if let (Self::ModuleRecord { input, .. }, InputKey::ModuleRecord(key)) = (self, k) {
            return input == key;
        }
        if let (Self::IncludeRecord { input, .. }, InputKey::IncludeRecord(key)) = (self, k) {
            return input == key;
        }
        if let (Self::ExpansionRecord(record), InputKey::ExpansionRecord(key)) = (self, k) {
            return record.key().fingerprint() == *key;
        }
        matches!(
            (self, k),
            (Self::SourceDependencies(_), InputKey::SourceDependencies)
                | (Self::DeclarationContext(_), InputKey::DeclarationContext(_))
                | (Self::DefinitionSource(_), InputKey::DefinitionSource(_))
                | (Self::ExpansionEpoch(_), InputKey::ExpansionEpoch)
                | (Self::SourceBlob(_), InputKey::SourceBlob(_))
                | (Self::DefinitionHeader(_), InputKey::DefinitionHeader(_))
                | (Self::DefinitionBody(_), InputKey::DefinitionBody(_))
                | (Self::ScopeMembers(_), InputKey::ScopeMembers(_))
                | (Self::ScopeBlockers(_), InputKey::ScopeBlockers(_))
                | (Self::NameCandidates(_), InputKey::NameCandidates(_))
                | (Self::CrateCfg(_), InputKey::CrateCfg(_))
                | (Self::CompileEnv(_), InputKey::CompileEnv(_))
        )
    }
    pub fn fingerprint(&self) -> Digest {
        // A body handle has position-independent semantic identity. This input
        // also supplies its actual locator to source-dependent queries, so
        // changing either endpoint must invalidate those queries.
        let mut e = StableEncoder::new(match self {
            Self::DefinitionBody(_) => "lrr/v3/body-location-input-value",
            _ => "lrr/v2/input-value",
        });
        match self {
            Self::MacroExportSource(receipt) => {
                e.u32(31);
                e.record(receipt.as_ref());
            }
            Self::ScopeCertificateSelection {
                selection,
                candidate_set,
            } => {
                e.u32(30);
                e.digest(*selection);
                e.digest(*candidate_set);
            }
            Self::RetainedSyntax(input) => {
                e.u32(29);
                e.record(input.as_ref());
            }
            Self::SourceDependencies(graph) => {
                e.u32(27);
                e.record(graph.as_ref());
            }
            Self::GeneratedSyntax(input) => {
                e.u32(26);
                e.record(input.as_ref());
            }
            Self::DiscoverySeal(input) => {
                e.u32(24);
                e.record(input.as_ref());
            }
            Self::CapturedSource(source) => {
                e.u32(25);
                e.string(source.path.as_str());
                e.record(&QueryKey::Outline(source.outline.clone()));
            }
            Self::DeclarationContext(v) => {
                e.u32(9);
                e.record(v);
            }
            Self::DefinitionSource(k) => {
                e.u32(8);
                e.record(k);
            }
            Self::ExpansionEpoch(epoch) => {
                e.u32(23);
                e.u64(*epoch);
            }
            Self::UnitCompileEnv(env) => {
                e.u32(22);
                e.digest(env.fingerprint());
            }

            Self::ModuleRecord { input, captured } => {
                e.u32(28);
                e.digest(*input);
                e.digest(captured.identity());
            }
            Self::IncludeRecord { input, captured } => {
                e.u32(21);
                e.digest(*input);
                e.digest(captured.identity());
            }
            Self::ExpansionRecord(record) => {
                e.u32(20);
                e.record(&record.descriptor());
            }
            Self::SourceBlob(v) => {
                e.u32(1);
                e.digest(v.content());
            }
            Self::DefinitionHeader(v) => {
                e.u32(2);
                e.record(v);
            }
            Self::DefinitionBody(v) => {
                e.u32(3);
                e.record(v);
                e.u32(v.range.start);
                e.u32(v.range.end);
            }
            Self::ScopeMembers(v) => {
                e.u32(4);
                e.record(v);
            }
            Self::CrateCfg(v) => {
                e.u32(7);
                e.record(v);
            }
            Self::ScopeBlockers(v) => {
                e.u32(5);
                e.sequence(v, |e, b| e.record(b));
            }
            Self::NameCandidates(v) => {
                e.u32(6);
                e.sequence(v, |e, d| e.digest(*d));
            }
            Self::CompileEnv(v) => {
                e.u32(10);
                e.string(v);
            }
        };
        e.finish()
    }
}
impl InputKey {
    pub(crate) fn heap_bytes(&self) -> usize {
        match self {
            Self::ScopeMembers(k) | Self::ScopeBlockers(k) => k.heap_bytes(),
            Self::NameCandidates(k) => k.scope.heap_bytes() + k.name.capacity(),
            Self::CompileEnv(k) => k.capacity(),
            _ => 0,
        }
    }
}
#[derive(Debug, Clone)]
pub struct InputRecord {
    pub value: Arc<InputValue>,
    pub fingerprint: Digest,
}
pub fn missing_fingerprint() -> Digest {
    StableEncoder::new("lrr/v2/missing-input").finish()
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum DependencyStamp {
    Input { key: InputKey, fingerprint: Digest },
    Query { key: QueryKey, fingerprint: Digest },
}
impl DependencyStamp {
    pub fn heap_bytes(&self) -> usize {
        match self {
            Self::Query { key, .. } => key.heap_bytes(),
            Self::Input { key, .. } => match key {
                InputKey::ScopeMembers(k) | InputKey::ScopeBlockers(k) => k.heap_bytes(),
                InputKey::NameCandidates(k) => k.scope.heap_bytes() + k.name.capacity(),
                InputKey::CompileEnv(k) => k.capacity(),
                _ => 0,
            },
        }
    }
    pub fn same_key(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Input { key: a, .. }, Self::Input { key: b, .. }) => a == b,
            (Self::Query { key: a, .. }, Self::Query { key: b, .. }) => a == b,
            _ => false,
        }
    }
}

impl StableEncode for InputKey {
    const DOMAIN: &'static str = "lrr/v2/input-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::MacroExportSource(location) => {
                e.u32(31);
                e.digest(*location);
            }
            Self::ScopeCertificateSelection(id) => {
                e.u32(30);
                e.digest(*id);
            }
            Self::RetainedSyntax(id) => {
                e.u32(29);
                e.digest(*id);
            }
            Self::SourceDependencies => e.u32(27),
            Self::GeneratedSyntax(id) => {
                e.u32(26);
                e.digest(*id);
            }
            Self::DiscoverySeal(key) => {
                e.u32(24);
                e.record(key);
            }
            Self::CapturedSource(id) => {
                e.u32(25);
                e.digest(*id);
            }
            Self::DeclarationContext(d) => {
                e.u32(9);
                e.digest(*d);
            }
            Self::DefinitionSource(d) => {
                e.u32(8);
                e.digest(*d);
            }
            Self::SourceBlob(d) => {
                e.u32(1);
                e.digest(*d);
            }
            Self::DefinitionHeader(d) => {
                e.u32(2);
                e.digest(*d);
            }
            Self::DefinitionBody(d) => {
                e.u32(3);
                e.digest(*d);
            }
            Self::ScopeMembers(s) => {
                e.u32(4);
                e.record(s);
            }
            Self::ScopeBlockers(s) => {
                e.u32(5);
                e.record(s);
            }
            Self::NameCandidates(k) => {
                e.u32(6);
                e.record(k);
            }
            Self::CrateCfg(d) => {
                e.u32(7);
                e.digest(*d);
            }
            Self::CompileEnv(s) => {
                e.u32(10);
                e.string(s);
            }
            Self::ExpansionRecord(d) => {
                e.u32(20);
                e.digest(*d);
            }
            Self::ModuleRecord(d) => {
                e.u32(28);
                e.digest(*d);
            }
            Self::IncludeRecord(d) => {
                e.u32(21);
                e.digest(*d);
            }
            Self::UnitCompileEnv(d) => {
                e.u32(22);
                e.digest(*d);
            }
            Self::ExpansionEpoch => e.u32(23),
        }
    }
}
impl StableEncode for DependencyStamp {
    const DOMAIN: &'static str = "lrr/v2/fact-dependency";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Input { key, fingerprint } => {
                e.u32(0);
                e.record(key);
                e.digest(*fingerprint);
            }
            Self::Query { key, fingerprint } => {
                e.u32(1);
                e.record(key);
                e.digest(*fingerprint);
            }
        }
    }
}
