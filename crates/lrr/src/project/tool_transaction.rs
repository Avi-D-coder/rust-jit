//! Explicit preparation publication boundary; no build tool invocation or query-provider access.
use super::{
    cargo_units::CargoUnitKey,
    tool_manifest::{
        InputCapture, InputInventory, InputPlan, PreparedFiles, ToolManifest, ToolStore,
    },
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::generated::{GeneratedRoot, GeneratedTree},
};
use std::path::Path;
fn stale() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-TOOL-PREPARE-STALE",
        "preparation changed before publication; repeat explicit preparation".into(),
    )
}
/// Open this before authorized tool work and consume it only for one final publication.
pub struct ToolTransaction<'a> {
    plan: &'a InputPlan,
    before: InputCapture,
}
impl<'a> ToolTransaction<'a> {
    pub fn begin(plan: &'a InputPlan) -> Result<Self, Diagnostic> {
        Ok(Self {
            plan,
            before: plan.capture()?,
        })
    }
    pub fn inputs(&self) -> &InputInventory {
        self.before.inventory()
    }
    pub fn commit(self, store: &ToolStore, candidate: &ToolManifest) -> Result<Digest, Diagnostic> {
        self.commit_with(store, candidate, |unit, path| {
            GeneratedRoot::open(unit, path)?.capture()
        })
    }
    fn commit_with(
        self,
        store: &ToolStore,
        candidate: &ToolManifest,
        mut capture: impl FnMut(&CargoUnitKey, &Path) -> Result<GeneratedTree, Diagnostic>,
    ) -> Result<Digest, Diagnostic> {
        candidate.validate()?;
        if candidate.inputs != *self.before.inventory() {
            return Err(stale());
        }
        self.before.verify_unchanged(&self.plan.capture()?)?;
        for (id, output) in &candidate.build_outputs {
            let unit = candidate.graph.units.get(id).ok_or_else(stale)?;
            let current = capture(&unit.key, Path::new(output.tree.out_dir()))?;
            if current.fingerprint() != output.tree.fingerprint() {
                return Err(stale());
            }
        }
        let files_before = PreparedFiles::capture(candidate)?;
        let after = self.plan.capture()?;
        let files_after = PreparedFiles::capture(candidate)?;
        store.publish_verified(candidate, &self.before, &after, &files_before, &files_after)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        project::tool_manifest::{InputKind, InputRoot},
        source::snapshot::SourcePath,
    };
    use std::{collections::BTreeSet, fs, sync::Barrier};
    mod fixture {
        use crate as lrr;
        include!("../../tests/support/tool_manifest_fixture.rs");
    }
    use fixture::Fixture;
    fn plan(fixture: &Fixture) -> InputPlan {
        InputPlan::new(
            vec![
                InputRoot::open(
                    InputKind::Source,
                    &fixture.root,
                    BTreeSet::from([
                        SourcePath::new("lib.rs").unwrap(),
                        SourcePath::new("build.rs").unwrap(),
                    ]),
                    BTreeSet::new(),
                    false,
                    BTreeSet::new(),
                )
                .unwrap(),
            ],
            BTreeSet::new(),
        )
        .unwrap()
    }
    fn bind(fixture: &mut Fixture, transaction: &ToolTransaction<'_>) {
        fixture.manifest.inputs = transaction.inputs().clone();
        for env in fixture.manifest.compile_env.values_mut() {
            env.declared.clear();
        }
    }
    #[test]
    fn racing_out_dir_rejects_transaction_publication_then_stable_commit_succeeds() {
        let mut fixture = Fixture::new();
        let input_plan = plan(&fixture);
        let transaction = ToolTransaction::begin(&input_plan).unwrap();
        bind(&mut fixture, &transaction);
        let store = ToolStore::open(&fixture.root.join("store")).unwrap();
        let previous = transaction.commit(&store, &fixture.manifest).unwrap();
        let transaction = ToolTransaction::begin(&input_plan).unwrap();
        let root = fixture
            .manifest
            .build_outputs
            .values()
            .next()
            .unwrap()
            .tree
            .out_dir()
            .to_owned();
        let path = Path::new(&root).join("generated.rs");
        let barrier = Barrier::new(2);
        std::thread::scope(|scope| {
            let writer = scope.spawn(|| {
                barrier.wait();
                fs::write(&path, b"47_i64\n").unwrap();
                barrier.wait();
            });
            let result = transaction.commit_with(&store, &fixture.manifest, |unit, path| {
                GeneratedRoot::open(unit, path)?.capture_with(|| {
                    barrier.wait();
                    barrier.wait();
                })
            });
            assert_eq!(result.unwrap_err().code, "LRR-E-SNAPSHOT-RACE");
            writer.join().unwrap();
        });
        assert_eq!(store.load(previous).unwrap().fingerprint(), previous);
        assert_eq!(fs::read_dir(fixture.root.join("store")).unwrap().count(), 1);
        let producer = *fixture.manifest.build_outputs.keys().next().unwrap();
        fixture
            .manifest
            .build_outputs
            .get_mut(&producer)
            .unwrap()
            .tree = GeneratedRoot::open(
            &fixture.manifest.graph.units[&producer].key,
            Path::new(&root),
        )
        .unwrap()
        .capture()
        .unwrap();
        let transaction = ToolTransaction::begin(&input_plan).unwrap();
        bind(&mut fixture, &transaction);
        let next = transaction.commit(&store, &fixture.manifest).unwrap();
        assert_ne!(next, previous);
        assert_eq!(
            store.load(next).unwrap().build_outputs[&producer]
                .tree
                .files()[&SourcePath::new("generated.rs").unwrap()]
                .bytes(),
            b"47_i64\n"
        );
        assert_eq!(fs::read_dir(fixture.root.join("store")).unwrap().count(), 2);
    }
    #[test]
    fn stale_source_or_generated_membership_refuses_transaction_before_staging() {
        let mut fixture = Fixture::new();
        let input_plan = plan(&fixture);
        let transaction = ToolTransaction::begin(&input_plan).unwrap();
        bind(&mut fixture, &transaction);
        let store = ToolStore::open(&fixture.root.join("store")).unwrap();
        fs::write(fixture.root.join("lib.rs"), b"changed").unwrap();
        assert_eq!(
            transaction
                .commit(&store, &fixture.manifest)
                .unwrap_err()
                .code,
            "LRR-E-TOOL-PREPARE-STALE"
        );
        assert_eq!(fs::read_dir(fixture.root.join("store")).unwrap().count(), 0);
        let transaction = ToolTransaction::begin(&input_plan).unwrap();
        bind(&mut fixture, &transaction);
        let root = fixture
            .manifest
            .build_outputs
            .values()
            .next()
            .unwrap()
            .tree
            .out_dir()
            .to_owned();
        fs::write(Path::new(&root).join("new.rs"), b"new").unwrap();
        assert_eq!(
            transaction
                .commit(&store, &fixture.manifest)
                .unwrap_err()
                .code,
            "LRR-E-TOOL-PREPARE-STALE"
        );
        assert_eq!(fs::read_dir(fixture.root.join("store")).unwrap().count(), 0);
    }
}
