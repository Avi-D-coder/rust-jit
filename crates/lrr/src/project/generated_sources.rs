//! Pure generated-input lookup. Filesystem authority ends at preparation capture.
use super::tool_manifest::ToolManifest;
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::{
        generated::GeneratedBlob,
        snapshot::{SourceBlob, SourcePath},
    },
};
use std::{collections::BTreeMap, path::Path, sync::Arc};
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-GENERATED-SOURCE", message.into())
}
/// One immutable validated generation, indexed by exact consumer unit.
pub struct GeneratedSources {
    manifest: Arc<ToolManifest>,
    producers: BTreeMap<Digest, (Digest, Digest)>,
}
impl GeneratedSources {
    pub fn new(manifest: Arc<ToolManifest>) -> Result<Self, Diagnostic> {
        manifest.validate()?;
        let mut producers = BTreeMap::new();
        for (id, output) in &manifest.build_outputs {
            let membership = output.tree.membership_fingerprint();
            for consumer in &output.consumers {
                if producers.insert(*consumer, (*id, membership)).is_some() {
                    return Err(fault("generated consumer has multiple producers"));
                }
            }
        }
        Ok(Self {
            manifest,
            producers,
        })
    }
    /// A normalized virtual member name; absence is certified by captured membership.
    pub fn lookup(
        &self,
        unit: Digest,
        path: SourcePath,
    ) -> Result<GeneratedLookup<'_>, Diagnostic> {
        let (producer, membership) = *self
            .producers
            .get(&unit)
            .ok_or_else(|| fault("unit has no captured generated output"))?;
        let output = self
            .manifest
            .build_outputs
            .get(&producer)
            .ok_or_else(|| fault("generated producer absent from immutable generation"))?;
        let origin = Path::new(output.tree.out_dir())
            .join(path.as_str())
            .to_str()
            .ok_or_else(|| fault("generated origin is not UTF-8"))?
            .to_owned();
        Ok(GeneratedLookup {
            unit,
            producer,
            blob: output.tree.files().get(&path),
            path,
            origin,
            membership,
        })
    }
    /// Route a virtual original OUT_DIR path through the unit's captured CompileEnv.
    pub fn lookup_original(
        &self,
        unit: Digest,
        original: &Path,
    ) -> Result<GeneratedLookup<'_>, Diagnostic> {
        let env = self
            .manifest
            .compile_env
            .get(&unit)
            .ok_or_else(|| fault("generated unit compile environment absent"))?;
        let root = env
            .out_dir
            .as_deref()
            .ok_or_else(|| fault("unit has no captured OUT_DIR"))?;
        let relative = original
            .strip_prefix(root)
            .map_err(|_| fault("include path does not belong to the unit's captured OUT_DIR"))?;
        self.lookup(unit, SourcePath::new(relative)?)
    }
}
/// Borrowed bytes keep their validated immutable generation alive through the index.
#[derive(Debug)]
pub struct GeneratedLookup<'a> {
    unit: Digest,
    producer: Digest,
    path: SourcePath,
    origin: String,
    membership: Digest,
    blob: Option<&'a GeneratedBlob>,
}
impl GeneratedLookup<'_> {
    pub fn unit(&self) -> Digest {
        self.unit
    }
    pub fn producer(&self) -> Digest {
        self.producer
    }
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    pub fn original_path(&self) -> &str {
        &self.origin
    }
    pub fn membership(&self) -> Digest {
        self.membership
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        self.blob.map(GeneratedBlob::bytes)
    }
    /// UTF-8 is checked only when the include is demanded as Rust/text source.
    pub fn source(&self) -> Result<Option<SourceBlob>, Diagnostic> {
        self.blob
            .map(|blob| SourceBlob::new(blob.bytes().to_vec()))
            .transpose()
    }
}
impl StableEncode for GeneratedLookup<'_> {
    const DOMAIN: &'static str = "lrr/v2/generated-lookup";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.unit);
        e.digest(self.producer);
        e.string(self.path.as_str());
        e.string(&self.origin);
        e.option(&self.blob, |e, blob| e.record(*blob));
    }
}
