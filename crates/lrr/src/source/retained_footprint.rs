//! Cold admission bound for original source ownership, without publishing it.
use super::PreparedSourceView;
use crate::{core::diagnostic::Diagnostic, db::dependency::InputValue};

fn limit() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-METADATA-LIMIT",
        "original source retention admission budget".into(),
    )
}
fn unsupported() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-SOURCE-RETENTION",
        "non-original publication requires role-aware retained-root accounting".into(),
    )
}
impl PreparedSourceView {
    /// Conservative preparation bound, not installed-cache accounting. Repeated
    /// shared handles are charged repeatedly; nothing registers or publishes a
    /// source, grants currentness, or makes a MacroExport body executable.
    pub(crate) fn original_retained_admission_bytes(
        &self,
        maximum: usize,
    ) -> Result<usize, Diagnostic> {
        let state = &self.state;
        if state._module_origin.is_some() || state._ordinary_module_origin.is_some() {
            return Err(unsupported());
        }
        let count = state
            .inputs
            .len()
            .checked_add(state.files.len())
            .and_then(|n| n.checked_add(state.parses.len()))
            .and_then(|n| n.checked_add(state.outlines.len()))
            .filter(|n| *n <= 100_000)
            .ok_or_else(limit)?;
        // Bound traversal and conservative BTree storage before walking rows.
        let mut bytes = count.checked_mul(128).ok_or_else(limit)?;
        let mut add = |amount: usize| -> Result<(), Diagnostic> {
            bytes = bytes
                .checked_add(amount)
                .filter(|n| *n <= maximum)
                .ok_or_else(limit)?;
            Ok(())
        };
        add(std::mem::size_of::<Self>())?;
        add(state.file_metadata_bytes()?)?;
        add(2 * std::mem::size_of::<usize>())?;
        add(self.selection.metadata_bytes()?)?;
        // The view also owns a distinct UnitLocation path and module vector.
        add(std::mem::size_of_val(&self.location))?;
        add(self.location.path.as_str().len())?;
        add(self
            .location
            .module_path
            .capacity()
            .checked_mul(std::mem::size_of::<String>())
            .ok_or_else(limit)?)?;
        for name in &self.location.module_path {
            add(name.capacity())?;
        }
        for (key, value) in &state.inputs {
            match value {
                InputValue::GeneratedSyntax(_)
                | InputValue::RetainedSyntax(_)
                | InputValue::DiscoverySeal(_)
                | InputValue::ModuleRecord { .. }
                | InputValue::IncludeRecord { .. }
                | InputValue::ExpansionRecord(_) => {
                    return Err(unsupported());
                }
                _ => {}
            }
            add(std::mem::size_of_val(key))?;
            add(key.heap_bytes())?;
            add(value.estimated_bytes()?)?;
            // Include shared input allocation headers conservatively, even for
            // inline variants. Actual deduplication belongs to the role owner.
            add(2 * std::mem::size_of::<usize>())?;
            for blob in value.source_blobs() {
                add(blob.bytes().len())?;
                add(2 * std::mem::size_of::<usize>())?;
            }
        }
        for file in state.files.values() {
            if file.parsed.generated_origin.is_some() {
                return Err(unsupported());
            }
            add(file.parsed.estimated_bytes())?;
            add(file.outline.estimated_bytes())?;
            add(4 * std::mem::size_of::<usize>())?;
        }
        for parsed in state.parses.values() {
            if parsed.generated_origin.is_some() {
                return Err(unsupported());
            }
            add(parsed.estimated_bytes())?;
            add(2 * std::mem::size_of::<usize>())?;
        }
        for outline in state.outlines.values() {
            add(outline.estimated_bytes())?;
            add(2 * std::mem::size_of::<usize>())?;
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        core::{
            digest::{Digest, StableEncode},
            profile::ProfileIdentity,
        },
        source::{
            changes::{
                PreparedSourceInputs, SourceSelection, SourceSelectionOutcome, SourceUnitInput,
            },
            snapshot::{CfgSet, SourceBlob, SourcePath},
        },
        syntax::parse::ParsedFile,
    };
    #[test]
    fn original_source_admission_counts_actual_roots_without_publication() {
        let text = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/match/match-borrowed-str.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let unit = SourceUnitInput {
                path: SourcePath::new("upstream-str.rs").unwrap(),
                crate_key: Digest([234; 32]),
                module_path: vec![],
                profile: profile.clone(),
                cfg: CfgSet::guest_default(),
                blob: SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
            };
            let parsed = ParsedFile::parse(&unit.blob, profile).unwrap();
            let outline = parsed.outline(unit.crate_key, vec![], &unit.cfg).unwrap();
            let selection = SourceSelection {
                path: unit.path.clone(),
                outline: crate::db::key::OutlineKey {
                    parse: parsed.key(),
                    crate_key: unit.crate_key,
                    module_path: vec![],
                    cfg: unit.cfg.fingerprint(),
                },
            };
            let prepared = PreparedSourceInputs::refresh_with_dependencies(
                None,
                vec![unit],
                Default::default(),
                vec![],
            )
            .unwrap();
            let SourceSelectionOutcome::Selected(view) = prepared.select(&selection).unwrap()
            else {
                panic!("actual original source absent")
            };
            let before = view.retained_state_weak().strong_count();
            let bytes = view.original_retained_admission_bytes(64 << 20).unwrap();
            assert!(bytes >= text.len() + parsed.estimated_bytes() + outline.estimated_bytes());
            assert_eq!(view.retained_state_weak().strong_count(), before);
            let shared = view.share();
            assert_eq!(
                shared.original_retained_admission_bytes(64 << 20).unwrap(),
                bytes
            );
            assert_eq!(
                view.original_retained_admission_bytes(bytes - 1)
                    .unwrap_err()
                    .code,
                "LRR-E-METADATA-LIMIT"
            );
            assert_eq!(
                view.original_retained_admission_bytes(bytes).unwrap(),
                bytes
            );
            // Capture/measurement did not publish the original source anywhere.
            let db = crate::db::memo::Database::default();
            assert!(db.prepared_source(&selection).unwrap().is_none());
            drop(prepared);
            assert_eq!(
                view.original_retained_admission_bytes(64 << 20).unwrap(),
                bytes
            );
        }
    }
}
