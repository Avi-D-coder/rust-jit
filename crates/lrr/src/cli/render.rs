//! Render only against immutable captured source; no filesystem reads or queries.
use crate::core::digest::Digest;
use crate::{
    core::diagnostic::Diagnostic,
    source::snapshot::Snapshot,
    syntax::{
        origin::{OriginLocation, RenderedMacroOrigin, SourceExcerpt, SourceOrigin},
        spans::{AttachedSpan, TokenSpanLease},
    },
};
use std::{collections::BTreeMap, sync::Arc};
const MAX_EXCERPT_BYTES: usize = 1024;
const MAX_RENDERED_FRAMES: usize = 64;

fn excerpt(text: &str, start: usize) -> Option<SourceExcerpt> {
    text.get(..start)?;
    let line_start = text[..start].rfind('\n').map_or(0, |n| n + 1);
    let line_end = text[start..]
        .find(['\n', '\r'])
        .map_or(text.len(), |n| start + n);
    // Keep the selected offset visible even on a very long source line. Never
    // split UTF-8, and bound the escaped representation as well as its input.
    let mut from = start.saturating_sub(128).max(line_start);
    while !text.is_char_boundary(from) {
        from += 1;
    }
    let mut rendered = String::new();
    rendered.try_reserve_exact(MAX_EXCERPT_BYTES).ok()?;
    let mut end = from;
    for ch in text[from..line_end].chars() {
        let escaped = ch.is_control()
            || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}');
        let size = if escaped {
            ch.escape_default().count()
        } else {
            ch.len_utf8()
        };
        if rendered.len().checked_add(size)? > MAX_EXCERPT_BYTES {
            break;
        }
        if escaped {
            rendered.extend(ch.escape_default());
        } else {
            rendered.push(ch);
        }
        end = end.checked_add(ch.len_utf8())?;
    }
    Some(SourceExcerpt {
        text: rendered,
        omitted_before: from > line_start,
        omitted_after: end < line_end,
    })
}

fn location(snapshot: &Snapshot, attached: AttachedSpan) -> OriginLocation {
    let excerpt = snapshot
        .files()
        .get(&attached.file)
        .and_then(|blob| blob.text().ok())
        .and_then(|text| excerpt(text, usize::try_from(attached.range.start).ok()?));
    OriginLocation {
        unit: snapshot.project().selected_crate,
        snapshot: snapshot.id(),
        file: attached.file,
        range: attached.range,
        start_line: attached.start.line,
        start_column: attached.start.column,
        end_line: attached.end.line,
        end_column: attached.end.column,
        excerpt,
    }
}
/// Renderer-only retained snapshots. A producer's relative path is interpreted
/// only in its own captured unit; reading this index never opens a file.
struct CapturedWorlds {
    primary: Arc<Snapshot>,
    by_unit: BTreeMap<Digest, Arc<Snapshot>>,
}
impl CapturedWorlds {
    fn new(primary: Arc<Snapshot>, secondary: &[Arc<Snapshot>]) -> Result<Self, Diagnostic> {
        let fault = || {
            Diagnostic::new(
                "LRR-E-INTERNAL-SOURCE-MAP",
                "diagnostic source worlds conflict or exceed capture limits".into(),
            )
        };
        if secondary.len() >= MAX_RENDERED_FRAMES {
            return Err(fault());
        }
        let mut by_unit: BTreeMap<Digest, Arc<Snapshot>> = BTreeMap::new();
        let mut bytes = 0usize;
        let mut files = 0usize;
        for snapshot in std::iter::once(&primary).chain(secondary) {
            let unit = snapshot.project().selected_crate;
            if let Some(old) = by_unit.get(&unit) {
                if old.id() != snapshot.id() {
                    return Err(fault());
                }
                continue;
            }
            files = files
                .checked_add(snapshot.files().len())
                .filter(|n| *n <= 100_000)
                .ok_or_else(fault)?;
            for blob in snapshot.files().values() {
                bytes = bytes
                    .checked_add(blob.bytes().len())
                    .filter(|n| *n <= 128 * 1024 * 1024)
                    .ok_or_else(fault)?;
            }
            by_unit.insert(unit, snapshot.clone());
        }
        Ok(Self { primary, by_unit })
    }
    fn snapshot(&self, unit: crate::syntax::token_wire::Key) -> Option<&Arc<Snapshot>> {
        self.by_unit.get(&Digest(unit.0))
    }
}
fn source_location(worlds: &CapturedWorlds, origin: &SourceOrigin) -> Option<OriginLocation> {
    let snapshot = worlds.snapshot(origin.source.unit)?;
    TokenSpanLease::new(snapshot.clone(), snapshot.project().selected_crate)
        .attach_source_range(&origin.source, origin.range)
        .ok()
        .map(|at| location(snapshot, at))
}
pub(super) fn attach(diagnostic: Diagnostic, snapshot: Arc<Snapshot>) -> Diagnostic {
    attach_captured_worlds(diagnostic, snapshot, &[])
}
/// Additional worlds must already be captured and admitted by the caller.
/// Attachment grants no native execution, macro or source-capture authority.
pub(super) fn attach_captured_worlds(
    mut diagnostic: Diagnostic,
    snapshot: Arc<Snapshot>,
    secondary: &[Arc<Snapshot>],
) -> Diagnostic {
    let worlds = match CapturedWorlds::new(snapshot.clone(), secondary) {
        Ok(worlds) => worlds,
        Err(fault) => {
            if diagnostic.origin_attachment_failure.is_none() {
                diagnostic.origin_attachment_failure = Some(fault.code);
            }
            // Preserve primary-source labels and the original semantic failure.
            CapturedWorlds {
                primary: snapshot.clone(),
                by_unit: BTreeMap::from([(snapshot.project().selected_crate, snapshot)]),
            }
        }
    };
    attach_in_worlds(diagnostic, worlds)
}
fn attach_in_worlds(mut diagnostic: Diagnostic, worlds: CapturedWorlds) -> Diagnostic {
    let snapshot = &worlds.primary;
    if let Some(origin) = &diagnostic.macro_origin {
        // Missing sources stay explicitly unattached. A stale/missing source cannot
        // supply a plausible line from a different snapshot or trigger a disk read.
        let invocation = origin
            .invocation
            .as_ref()
            .and_then(|o| source_location(&worlds, o));
        let definition = origin
            .definition
            .as_ref()
            .and_then(|d| source_location(&worlds, &d.location));
        let failure = origin.failure_span.as_ref().and_then(|span| {
            let captured = worlds.snapshot(span.source.unit)?;
            TokenSpanLease::new(captured.clone(), captured.project().selected_crate)
                .attach(span)
                .ok()
                .map(|at| location(captured, at))
        });
        let direct_frames = [&invocation, &definition, &failure]
            .into_iter()
            .filter(|at| at.is_some())
            .count();
        let parent_limit = MAX_RENDERED_FRAMES - direct_frames;
        diagnostic.rendered_macro_origin = Some(Box::new(RenderedMacroOrigin {
            snapshot: snapshot.id(),
            invocation,
            definition,
            failure,
            parents: origin
                .parents
                .iter()
                .take(parent_limit)
                .map(|parent| {
                    parent
                        .invocation
                        .as_ref()
                        .and_then(|at| source_location(&worlds, at))
                })
                .collect(),
            omitted_parent_count: origin.parents.len().saturating_sub(parent_limit),
        }));
    }
    if let Some(origin) = &diagnostic.generated_origin {
        let used = diagnostic
            .rendered_macro_origin
            .as_ref()
            .map_or(0, |macro_origin| {
                macro_origin.parents.len()
                    + [
                        &macro_origin.invocation,
                        &macro_origin.definition,
                        &macro_origin.failure,
                    ]
                    .into_iter()
                    .filter(|at| at.is_some())
                    .count()
            });
        let remaining = MAX_RENDERED_FRAMES.saturating_sub(used);
        let invocation = (remaining != 0)
            .then(|| {
                origin
                    .invocation
                    .as_ref()
                    .and_then(|at| source_location(&worlds, at))
            })
            .flatten();
        let remaining = remaining - usize::from(invocation.is_some());
        let definition = (remaining != 0)
            .then(|| {
                origin
                    .definition
                    .as_ref()
                    .and_then(|definition| source_location(&worlds, &definition.location))
            })
            .flatten();
        let remaining = remaining - usize::from(definition.is_some());
        let parent_limit = remaining.min(origin.parents.len());
        let limit = remaining - parent_limit;
        diagnostic.rendered_generated_origin =
            Some(Box::new(crate::syntax::origin::RenderedGeneratedOrigin {
                snapshot: snapshot.id(),
                invocation,
                definition,
                tokens: origin
                    .tokens
                    .iter()
                    .take(limit)
                    .map(|span| {
                        let captured = worlds.snapshot(span.source.unit)?;
                        TokenSpanLease::new(captured.clone(), captured.project().selected_crate)
                            .attach(span)
                            .ok()
                            .map(|at| location(captured, at))
                    })
                    .collect(),
                omitted_tokens: origin.tokens.len().saturating_sub(limit),
                parents: origin
                    .parents
                    .iter()
                    .take(parent_limit)
                    .map(|parent| {
                        parent
                            .invocation
                            .as_ref()
                            .and_then(|at| source_location(&worlds, at))
                    })
                    .collect(),
                omitted_parents: origin.parents.len().saturating_sub(parent_limit),
            }));
    }
    if let Some(stack) = &diagnostic.guest_stack {
        let macro_used = diagnostic
            .rendered_macro_origin
            .as_ref()
            .map_or(0, |origin| {
                origin.parents.len()
                    + [&origin.invocation, &origin.definition, &origin.failure]
                        .into_iter()
                        .filter(|at| at.is_some())
                        .count()
            });
        let generated_used = diagnostic
            .rendered_generated_origin
            .as_ref()
            .map_or(0, |origin| {
                origin.tokens.len()
                    + origin.parents.len()
                    + [&origin.invocation, &origin.definition]
                        .into_iter()
                        .filter(|at| at.is_some())
                        .count()
            });
        let mut remaining = MAX_RENDERED_FRAMES.saturating_sub(macro_used + generated_used);
        let mut omitted_locations = 0usize;
        let frames = stack
            .frames
            .iter()
            .take(MAX_RENDERED_FRAMES)
            .map(|frame| {
                let count = frame.locations.len().min(remaining);
                remaining -= count;
                omitted_locations = omitted_locations.saturating_add(frame.locations.len() - count);
                frame
                    .locations
                    .iter()
                    .take(count)
                    .map(|at| source_location(&worlds, at))
                    .collect()
            })
            .collect();
        diagnostic.rendered_guest_stack =
            Some(Box::new(crate::syntax::origin::RenderedGuestStack {
                snapshot: snapshot.id(),
                frames,
                omitted_frames: stack
                    .omitted_frames
                    .saturating_add(stack.frames.len().saturating_sub(MAX_RENDERED_FRAMES)),
                omitted_locations,
            }));
    }
    diagnostic
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        source::{loader::SourceLoader, manifest::ItemKind},
        syntax::{
            parse::ParsedFile,
            token_wire::{Key, SourceIdentity},
        },
    };
    #[test]
    fn rustc_stock_native_definition_worlds_refuse_path_aliases_and_stale_captures() {
        if let Some(expected) = std::env::var_os("LRR_VALIDATION_SOURCE_ROOT") {
            let root = std::fs::canonicalize(expected).unwrap();
            let compiled = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            assert_eq!(std::fs::canonicalize(compiled).unwrap(), root);
            for (relative, embedded) in [
                (
                    "crates/lrr/src/cli/render.rs",
                    include_bytes!("render.rs").as_slice(),
                ),
                (
                    "crates/lrr/src/syntax/origin.rs",
                    include_bytes!("../syntax/origin.rs").as_slice(),
                ),
                (
                    "crates/lrr/src/syntax/attributes.rs",
                    include_bytes!("../syntax/attributes.rs").as_slice(),
                ),
            ] {
                assert_eq!(
                    embedded,
                    std::fs::read(root.join(relative)).unwrap(),
                    "validation embedded source differs: {relative}"
                );
            }
        }
        let original = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
        ));
        let primary = Arc::new(
            SourceLoader::standalone(original)
                .unwrap()
                .capture()
                .unwrap(),
        );
        let selected = primary.project().selected_crate;
        let path = primary.project().root_source.clone();
        let blob = &primary.files()[&path];
        let parsed = ParsedFile::parse(blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(selected, vec![], &primary.cfg_by_crate()[&selected])
            .unwrap();
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.item_kind == ItemKind::Macro)
            .unwrap();
        let (range, _) = parsed
            .macro_definition_header_at(item.declaration_span)
            .unwrap();
        let invocation = SourceOrigin {
            source: SourceIdentity {
                unit: Key(selected.0),
                content: Key(blob.content().0),
                path: path.as_str().into(),
            },
            range,
        };
        let helper = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/proc-macro/auxiliary/test-macros.rs"
        );
        let native_blob =
            crate::source::snapshot::SourceBlob::new(helper.as_bytes().to_vec()).unwrap();
        let native_parsed = ParsedFile::parse_macro_facet(&native_blob, 2015).unwrap();
        let native_cfg = crate::source::snapshot::CfgSet::guest_default();
        let native_wire = lrr_expand_adapter::token_wire::parse_source(
            helper,
            lrr_expand_adapter::token_wire::SourceIdentity {
                unit: lrr_expand_adapter::token_wire::Key([219; 32]),
                content: lrr_expand_adapter::token_wire::Key(native_blob.content().0),
                path: path.as_str().into(),
            },
            lrr_expand_adapter::token_wire::ContextRecord {
                parent: None,
                expansion: None,
                transparency: lrr_expand_adapter::token_wire::Transparency::Opaque,
                defining_unit: lrr_expand_adapter::token_wire::Key([219; 32]),
                edition: lrr_expand_adapter::token_wire::Edition::E2015,
            },
            lrr_expand_adapter::token_wire::Key([221; 32]),
        )
        .unwrap();
        let native_wire =
            crate::syntax::token_wire::TokenForest::decode(&native_wire.encode().unwrap()).unwrap();
        // Distinct native-world identity and deliberately identical relative path.
        // This is an external LRR renderer adaptation, not a native execution.
        let native_unit = Digest([219; 32]);
        let mut project = primary.project().clone();
        let mut key = project.crates[&selected].clone();
        key.guest_edition = 2015;
        key.target_name = "test_macros".into();
        project.selected_crate = native_unit;
        project.crates = BTreeMap::from([(native_unit, key)]);
        project.identity = "captured-stock-macro-host".into();
        let native = Arc::new(
            Snapshot::new(
                project.clone(),
                BTreeMap::from([(path.clone(), native_blob.clone())]),
                BTreeMap::from([(native_unit, native_cfg.clone())]),
                BTreeMap::new(),
                None,
            )
            .unwrap(),
        );
        let changed = Arc::new(
            Snapshot::new(
                project,
                BTreeMap::from([(
                    path.clone(),
                    crate::source::snapshot::SourceBlob::new(
                        format!("// edited\n{helper}").into_bytes(),
                    )
                    .unwrap(),
                )]),
                BTreeMap::from([(native_unit, native_cfg.clone())]),
                BTreeMap::new(),
                Some(native.id()),
            )
            .unwrap(),
        );
        for (form, export, function) in [
            (
                lrr_expand_adapter::proc_client::ProcForm::FunctionLike,
                "panic_bang",
                "panic_bang",
            ),
            (
                lrr_expand_adapter::proc_client::ProcForm::Attribute,
                "panic_attr",
                "panic_attr",
            ),
            (
                lrr_expand_adapter::proc_client::ProcForm::Derive,
                "Panic",
                "panic_derive",
            ),
        ] {
            let range = native_parsed
                .source_procedural_definition_origin(export, form, &native_cfg)
                .unwrap()
                .unwrap();
            let definition = crate::syntax::origin::DefinitionOrigin {
                identity: Digest::of_bytes(export.as_bytes()),
                name: export.into(),
                location: SourceOrigin {
                    source: SourceIdentity {
                        unit: Key(native_unit.0),
                        content: Key(native_blob.content().0),
                        path: path.as_str().into(),
                    },
                    range,
                },
            };
            let failure = native_wire
                .spans()
                .iter()
                .find(|span| {
                    span.start == range.start && span.end == range.start.checked_add(2).unwrap()
                })
                .unwrap()
                .clone();
            let mut diagnostic =
                Diagnostic::new("LRR-E-MACRO-PANIC", "retained native fault".into());
            diagnostic.macro_origin = Some(Box::new(crate::syntax::origin::MacroOrigin {
                invocation_identity: Digest([220; 32]),
                invocation: Some(invocation.clone()),
                definition: Some(definition),
                failure_kind: None,
                failure_span_key: Some(failure.key()),
                failure_span: Some(failure.clone()),
                parents: vec![],
            }));
            let missing = attach(diagnostic.clone(), primary.clone());
            assert!(
                missing
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .definition
                    .is_none()
            );
            let rendered = attach_captured_worlds(
                diagnostic.clone(),
                primary.clone(),
                std::slice::from_ref(&native),
            );
            assert!(rendered.origin_attachment_failure.is_none());
            let attached = rendered.rendered_macro_origin.as_ref().unwrap();
            let live = attached.invocation.as_ref().unwrap();
            let host = attached.definition.as_ref().unwrap();
            assert_eq!(live.unit, selected);
            assert_eq!(live.snapshot, primary.id());
            assert_eq!(host.unit, native_unit);
            assert_eq!(host.snapshot, native.id());
            let failure_at = attached.failure.as_ref().unwrap();
            assert_eq!(failure_at.unit, native_unit);
            assert_eq!(failure_at.snapshot, native.id());
            assert_eq!(failure_at.range.start, failure.start);
            assert_eq!(failure_at.range.end, failure.end);
            assert_eq!(live.file, host.file);
            assert_ne!(live.unit, host.unit);
            assert!(
                host.excerpt
                    .as_ref()
                    .unwrap()
                    .text
                    .contains(&format!("fn {function}"))
            );
            assert_eq!(
                host.start_line as usize,
                helper[..range.start as usize]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                    + 1
            );
            let stale = attach_captured_worlds(
                diagnostic.clone(),
                primary.clone(),
                std::slice::from_ref(&changed),
            );
            assert!(
                stale
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .definition
                    .is_none()
            );
            assert!(
                stale
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .invocation
                    .is_some()
            );
            let conflict = attach_captured_worlds(
                diagnostic.clone(),
                primary.clone(),
                &[native.clone(), changed.clone()],
            );
            assert_eq!(conflict.code, diagnostic.code);
            assert_eq!(conflict.message, diagnostic.message);
            assert_eq!(
                conflict.origin_attachment_failure,
                Some("LRR-E-INTERNAL-SOURCE-MAP")
            );
            assert!(
                conflict
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .definition
                    .is_none()
            );
            assert!(
                conflict
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .invocation
                    .is_some()
            );
            assert!(
                conflict
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .failure
                    .is_none()
            );
            assert!(
                stale
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .failure
                    .is_none()
            );
            let duplicate = attach_captured_worlds(
                diagnostic.clone(),
                primary.clone(),
                &[native.clone(), native.clone()],
            );
            assert!(duplicate.origin_attachment_failure.is_none());
            assert!(
                duplicate
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .definition
                    .is_some()
            );
            let mut prior = diagnostic.clone();
            prior.origin_attachment_failure = Some("LRR-E-METADATA-LIMIT");
            let prior =
                attach_captured_worlds(prior, primary.clone(), &[native.clone(), changed.clone()]);
            assert_eq!(
                prior.origin_attachment_failure,
                Some("LRR-E-METADATA-LIMIT")
            );
            let excess = vec![native.clone(); MAX_RENDERED_FRAMES];
            let excess = attach_captured_worlds(diagnostic, primary.clone(), &excess);
            assert_eq!(
                excess.origin_attachment_failure,
                Some("LRR-E-INTERNAL-SOURCE-MAP")
            );
            assert!(
                excess
                    .rendered_macro_origin
                    .as_ref()
                    .unwrap()
                    .definition
                    .is_none()
            );
        }
    }
    #[test]
    fn rustc_stock_definition_location_requires_actual_capture_unit_content_and_range() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
        ));
        let snapshot = Arc::new(SourceLoader::standalone(path).unwrap().capture().unwrap());
        let project = snapshot.project();
        let blob = &snapshot.files()[&project.root_source];
        let parsed = ParsedFile::parse(blob, ProfileIdentity::current()).unwrap();
        let outline = parsed
            .outline(
                project.selected_crate,
                vec![],
                &snapshot.cfg_by_crate()[&project.selected_crate],
            )
            .unwrap();
        let item = outline
            .items
            .iter()
            .find(|item| item.def_key.item_kind == ItemKind::Macro)
            .unwrap();
        let (range, name) = parsed
            .macro_definition_header_at(item.declaration_span)
            .unwrap();
        assert_eq!(name, item.def_key.declared_name);
        assert_ne!(
            item.def_key.fingerprint(),
            crate::core::digest::Digest::default()
        );
        let origin = SourceOrigin {
            source: SourceIdentity {
                unit: Key(project.selected_crate.0),
                content: Key(blob.content().0),
                path: project.root_source.as_str().into(),
            },
            range,
        };
        let worlds = CapturedWorlds::new(snapshot.clone(), &[]).unwrap();
        let at = source_location(&worlds, &origin).unwrap();
        assert_eq!((at.start_line, at.start_column), (2, 1));
        assert_eq!(
            at.excerpt.as_ref().unwrap().text,
            "macro_rules! one_arg_macro {"
        );
        let mut wrong = origin.clone();
        wrong.source.unit = Key([71; 32]);
        assert!(source_location(&worlds, &wrong).is_none());
        wrong = origin.clone();
        wrong.source.content = Key([72; 32]);
        assert!(source_location(&worlds, &wrong).is_none());
        wrong = origin.clone();
        wrong.range.end = u32::try_from(blob.bytes().len())
            .unwrap()
            .checked_add(1)
            .unwrap();
        assert!(source_location(&worlds, &wrong).is_none());

        // LRR renderer budget regression uses the captured upstream source,
        // without claiming these synthetic parent frames are real expansions.
        use crate::syntax::origin::{DefinitionOrigin, ExpansionOrigin, MacroOrigin};
        let mut diagnostic = Diagnostic::new("LRR-E-MACRO-EXPANSION", "render budget".into());
        diagnostic.macro_origin = Some(Box::new(MacroOrigin {
            invocation_identity: crate::core::digest::Digest::default(),
            invocation: None,
            definition: Some(DefinitionOrigin {
                identity: item.def_key.fingerprint(),
                name,
                location: origin.clone(),
            }),
            failure_kind: None,
            failure_span_key: None,
            failure_span: None,
            parents: (0..70)
                .map(|_| ExpansionOrigin {
                    expansion: crate::core::digest::Digest::default(),
                    invocation_identity: None,
                    invocation: Some(origin.clone()),
                })
                .collect(),
        }));
        let attached = attach(diagnostic, snapshot.clone());
        let rendered = attached.rendered_macro_origin.as_ref().unwrap();
        assert_eq!(rendered.parents.len(), 63);
        assert_eq!(rendered.omitted_parent_count, 7);
        assert_eq!(attached.macro_origin.as_ref().unwrap().parents.len(), 70);
        let display = attached.to_string();
        assert_eq!(display.matches("\n  -->").count(), MAX_RENDERED_FRAMES);
        assert!(display.contains("7 outer macro expansion frames omitted"));
        assert!(attached.heap_bytes().unwrap() >= MAX_RENDERED_FRAMES * MAX_EXCERPT_BYTES);
    }

    #[test]
    fn captured_excerpt_window_is_bounded_utf8_safe_and_escapes_terminal_controls() {
        let stock = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/macros/macro-invocation-span-error-7970.rs"
        );
        let line = stock
            .lines()
            .find(|line| line.contains("one_arg_macro!();"))
            .unwrap();
        // LRR rendering resource/terminal controls surround an unchanged stock
        // line; this is not a language fixture or a rustc compiletest pass.
        let prefix = "é\u{7f}".repeat(2000);
        let text = format!("{prefix}{line}\t\u{1b}[31m\u{202e}{}", line.repeat(2000));
        let at = excerpt(&text, prefix.len()).unwrap();
        assert!(at.text.len() <= MAX_EXCERPT_BYTES);
        assert!(at.omitted_before && at.omitted_after);
        assert!(at.text.contains(line));
        assert!(at.text.contains("\\t\\u{1b}[31m\\u{202e}"));
        assert!(!at.text.chars().any(char::is_control));
        assert!(!at.text.contains('\u{202e}'));
        assert!(
            excerpt(&text, 1).is_none(),
            "split UTF-8 attached a plausible excerpt"
        );
        assert!(excerpt(&text, text.len() + 1).is_none());
    }
}
