//! Pre-entry publication of an actual original declaration in retained syntax.
//! Generated member catalogs and their namespace authority remain independent.
use super::*;
use crate::{
    core::digest::StableEncode,
    db::retained_syntax::RetainedSyntaxInput,
    source::{changes::PreparedSourceView, manifest::BodyKey},
    syntax::{outline::DeclarationContext, parse::ParsedFile},
};

#[path = "retained_callables.rs"]
mod callables;
pub(crate) use callables::{RetainedOriginalDeclaration, RetainedSourceLease};

fn rows(
    input: &RetainedSyntaxInput,
    source: &PreparedSourceView,
    parsed: &ParsedFile,
    definition: Digest,
) -> Result<(BodyKey, BTreeMap<InputKey, InputValue>), Diagnostic> {
    let invalid = |message| fault("LRR-E-EXPANSION-INPUT", message);
    if input.prepared().source() != source.selection() || parsed.key() != *input.key() {
        return Err(invalid("retained definition selected source differs"));
    }
    if !input.prepared().permits_body(definition) {
        return Err(invalid(
            "retained syntax readiness belongs to another original body",
        ));
    }
    let original = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.fingerprint() == definition)
        .ok_or_else(|| invalid("retained definition is not an actual original declaration"))?;
    let original_body = original
        .body
        .as_ref()
        .ok_or_else(|| invalid("retained original declaration has no body"))?;
    if original_body.def_key != definition {
        return Err(invalid("retained original body identity differs"));
    }
    let anchor = input
        .prepared()
        .original_declarations()
        .iter()
        .find(|anchor| anchor.original_definition() == definition)
        .ok_or_else(|| invalid("retained original declaration anchor absent"))?;
    if anchor.original_range() != original.declaration_span {
        return Err(invalid("retained original declaration attachment differs"));
    }
    let outline = parsed.outline(
        source.selection().outline.crate_key,
        source.selection().outline.module_path.clone(),
        source.captured_macro_cfg()?,
    )?;
    let mut candidates = outline
        .items
        .iter()
        .filter(|item| item.declaration_span == anchor.declaration_range());
    let actual = candidates
        .next()
        .ok_or_else(|| invalid("retained original declaration node absent"))?;
    if candidates.next().is_some()
        || actual.header_span != anchor.header_range()
        || actual.body.as_ref().map(|body| body.range) != anchor.body_range()
        || actual.def_key.item_kind != original.def_key.item_kind
        || actual.def_key.declared_name != original.def_key.declared_name
    {
        return Err(invalid("retained original declaration node differs"));
    }
    if actual.scope != original.scope {
        return Err(invalid(
            "retained original owner requires completed scope reconciliation",
        ));
    }
    let mut body = actual
        .body
        .clone()
        .ok_or_else(|| invalid("retained original body disappeared"))?;
    body.require_entry()?;
    // The occurrence anchor, not the post-expansion duplicate ordinal, proves
    // this is the original declaration. Generated declarations get no alias.
    body.def_key = definition;
    parsed.entered_body_root(&body)?;
    let key = body.key();
    Ok((
        key,
        BTreeMap::from([
            (
                InputKey::DefinitionSource(definition),
                InputValue::DefinitionSource(input.key().clone()),
            ),
            (
                InputKey::DefinitionHeader(definition),
                InputValue::DefinitionHeader(actual.header.clone()),
            ),
            (
                InputKey::DefinitionBody(definition),
                InputValue::DefinitionBody(body),
            ),
            (
                InputKey::DeclarationContext(definition),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: actual.scope.clone(),
                    range: actual.declaration_span,
                }),
            ),
        ]),
    ))
}
/// Rows of a selected macro-generated function in its retained tree. The
/// anchor's exact structural location, the unit's own declaration and its
/// owning scope must agree; no generated name or scope is invented here.
fn generated_rows(
    input: &RetainedSyntaxInput,
    source: &PreparedSourceView,
    parsed: &ParsedFile,
    definition: Digest,
) -> Result<(BodyKey, BTreeMap<InputKey, InputValue>), Diagnostic> {
    if input.generated_associated().is_some() {
        return associated_rows(input, source, parsed, definition);
    }
    let invalid = |message| fault("LRR-E-EXPANSION-INPUT", message);
    let (Some(anchor), Some(unit)) = (
        input.prepared().selected_generated(),
        input.generated_unit(),
    ) else {
        return Err(invalid("retained generated body anchor absent"));
    };
    if input.prepared().source() != source.selection()
        || parsed.key() != *input.key()
        || anchor.definition() != definition
        || !input.prepared().permits_body(definition)
    {
        return Err(invalid("retained generated body selection differs"));
    }
    let expected = generated_unit_rows(unit, definition)?;
    let (
        Some(InputValue::DefinitionHeader(unit_header)),
        Some(InputValue::DeclarationContext(unit_context)),
    ) = (
        expected.get(&InputKey::DefinitionHeader(definition)),
        expected.get(&InputKey::DeclarationContext(definition)),
    )
    else {
        return Err(invalid("retained generated unit rows differ"));
    };
    let selected = unit
        .definitions
        .get(&definition)
        .ok_or_else(|| invalid("retained generated declaration absent from its unit"))?;
    let outline = parsed.outline(
        source.selection().outline.crate_key,
        source.selection().outline.module_path.clone(),
        source.captured_macro_cfg()?,
    )?;
    let mut candidates = outline
        .items
        .iter()
        .filter(|item| item.declaration_span == anchor.declaration_range());
    let actual = candidates
        .next()
        .ok_or_else(|| invalid("retained generated declaration node absent"))?;
    // The unit carries its macro's definition edition (a library macro is
    // edition 2024); the retained tree is parsed in the owning source's. Token
    // editions live in the spans; the header rows join modulo that profile.
    let unit_header_in_owner_profile = {
        let mut header = unit_header.clone();
        header.profile = actual.header.profile.clone();
        header
    };
    if candidates.next().is_some()
        || actual.body.as_ref().map(|body| body.range) != Some(anchor.body_range())
        || actual.def_key.item_kind != selected.item_kind
        || actual.def_key.declared_name != selected.declared_name
        || !actual.header.same_semantics(&unit_header_in_owner_profile)
    {
        return Err(invalid("retained generated declaration node differs"));
    }
    if actual.scope != unit_context.scope {
        return Err(invalid(
            "retained generated owner requires completed scope reconciliation",
        ));
    }
    let mut body = actual
        .body
        .clone()
        .ok_or_else(|| invalid("retained generated body disappeared"))?;
    body.require_entry()?;
    body.def_key = definition;
    parsed.entered_body_root(&body)?;
    let key = body.key();
    Ok((
        key,
        BTreeMap::from([
            (
                InputKey::DefinitionSource(definition),
                InputValue::DefinitionSource(input.key().clone()),
            ),
            (
                InputKey::DefinitionHeader(definition),
                InputValue::DefinitionHeader(actual.header.clone()),
            ),
            (
                InputKey::DefinitionBody(definition),
                InputValue::DefinitionBody(body),
            ),
            (
                InputKey::DeclarationContext(definition),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: actual.scope.clone(),
                    range: actual.declaration_span,
                }),
            ),
        ]),
    ))
}
/// Rows of a macro-generated associated function written into an original
/// impl/trait: its actual retained declaration, owner scope and body. It has
/// no other publication; its identity is the generated header key.
fn associated_rows(
    input: &RetainedSyntaxInput,
    source: &PreparedSourceView,
    parsed: &ParsedFile,
    definition: Digest,
) -> Result<(BodyKey, BTreeMap<InputKey, InputValue>), Diagnostic> {
    let invalid = |message| fault("LRR-E-EXPANSION-INPUT", message);
    let (Some(anchor), Some(associated)) = (
        input.prepared().selected_generated(),
        input.generated_associated(),
    ) else {
        return Err(invalid("retained generated associated anchor absent"));
    };
    if input.prepared().source() != source.selection()
        || parsed.key() != *input.key()
        || anchor.definition() != definition
        || !input.prepared().permits_body(definition)
    {
        return Err(invalid("retained generated associated selection differs"));
    }
    let outline = parsed.outline(
        source.selection().outline.crate_key,
        source.selection().outline.module_path.clone(),
        source.captured_macro_cfg()?,
    )?;
    let mut candidates = outline
        .items
        .iter()
        .filter(|item| item.declaration_span == anchor.declaration_range());
    let actual = candidates
        .next()
        .ok_or_else(|| invalid("retained generated associated node absent"))?;
    if candidates.next().is_some()
        || actual.body.as_ref().map(|body| body.range) != Some(anchor.body_range())
        || actual.def_key != associated.def_key
        || actual.scope != associated.scope
    {
        return Err(invalid("retained generated associated node differs"));
    }
    let mut body = actual
        .body
        .clone()
        .ok_or_else(|| invalid("retained generated associated body disappeared"))?;
    body.require_entry()?;
    body.def_key = definition;
    parsed.entered_body_root(&body)?;
    let key = body.key();
    Ok((
        key,
        BTreeMap::from([
            (
                InputKey::DefinitionSource(definition),
                InputValue::DefinitionSource(input.key().clone()),
            ),
            (
                InputKey::DefinitionHeader(definition),
                InputValue::DefinitionHeader(actual.header.clone()),
            ),
            (
                InputKey::DefinitionBody(definition),
                InputValue::DefinitionBody(body),
            ),
            (
                InputKey::DeclarationContext(definition),
                InputValue::DeclarationContext(DeclarationContext {
                    scope: actual.scope.clone(),
                    range: actual.declaration_span,
                }),
            ),
        ]),
    ))
}
/// The rows the unit itself published for one of its declarations
/// (GeneratedSyntaxInput::prepare), recomputed from its retained outline.
fn generated_unit_rows(
    unit: &crate::expand::generated_syntax::GeneratedSyntaxInput,
    definition: Digest,
) -> Result<BTreeMap<InputKey, InputValue>, Diagnostic> {
    let invalid = |message| fault("LRR-E-EXPANSION-INPUT", message);
    let header = unit
        .items
        .items()
        .iter()
        .find(|header| header.key.fingerprint() == definition)
        .ok_or_else(|| invalid("generated unit declaration header absent"))?;
    let outline = unit.outline()?;
    let item = outline
        .items
        .iter()
        .find(|item| {
            (item.declaration_span.start, item.declaration_span.end) == (header.start, header.end)
        })
        .ok_or_else(|| invalid("generated unit declaration absent"))?;
    let mut body = item
        .body
        .clone()
        .ok_or_else(|| invalid("generated unit declaration has no body"))?;
    body.def_key = definition;
    Ok(BTreeMap::from([
        (
            InputKey::DefinitionSource(definition),
            InputValue::DefinitionSource(unit.key.clone()),
        ),
        (
            InputKey::DefinitionHeader(definition),
            InputValue::DefinitionHeader(item.header.clone()),
        ),
        (
            InputKey::DefinitionBody(definition),
            InputValue::DefinitionBody(body),
        ),
        (
            InputKey::DeclarationContext(definition),
            InputValue::DeclarationContext(DeclarationContext {
                scope: item.scope.clone(),
                range: item.declaration_span,
            }),
        ),
    ]))
}
/// A generated declaration whose locator is a retained admission of its own
/// unit: a body-only edit (its body's macro calls attached). Declaration-level
/// Type/namespace facts keep the unit's own coordinates and token origins,
/// exactly as a retained original keeps its original declaration's namespace.
pub(crate) struct GeneratedDeclarationBridge {
    pub(crate) unit: Arc<crate::expand::generated_syntax::GeneratedSyntaxInput>,
    pub(crate) retained: crate::syntax::parse::ParseKey,
    /// The rows the unit itself published (unit coordinates).
    pub(crate) header: crate::syntax::outline::HeaderRecipe,
    pub(crate) context: DeclarationContext,
    pub(crate) body: crate::syntax::outline::BodyHandle,
    retained_start: u32,
    header_len: u32,
}
impl GeneratedDeclarationBridge {
    /// Map a range of the retained declaration's (unedited) header to the
    /// same tokens in the unit. The header text is identical in both trees.
    pub(crate) fn unit_range(
        &self,
        range: crate::syntax::spans::SourceRange,
    ) -> Option<crate::syntax::spans::SourceRange> {
        let start = range.start.checked_sub(self.retained_start)?;
        let end = range.end.checked_sub(self.retained_start)?;
        if end > self.header_len || start > end {
            return None;
        }
        crate::syntax::spans::SourceRange::checked(
            self.context.range.start.checked_add(start)?,
            self.context.range.start.checked_add(end)?,
        )
        .ok()
    }
}
impl Database {
    /// None unless the current locator of `definition` is a retained admission
    /// of its generated unit. The published retained rows must be exactly the
    /// anchored projection of the unit's declaration; no row is republished.
    pub(crate) fn generated_declaration_bridge(
        &self,
        definition: Digest,
    ) -> Result<Option<GeneratedDeclarationBridge>, Diagnostic> {
        let invalid = |message| fault("LRR-E-CALL-CONTEXT", message);
        let Some((InputValue::DefinitionSource(parse), _)) =
            self.published_input(&InputKey::DefinitionSource(definition))
        else {
            return Ok(None);
        };
        let Some((InputValue::RetainedSyntax(retained), _)) =
            self.published_input(&InputKey::RetainedSyntax(parse.content))
        else {
            return Ok(None);
        };
        let Some(unit) = retained.generated_unit() else {
            return Ok(None);
        };
        let anchor = retained
            .prepared()
            .selected_generated()
            .filter(|anchor| anchor.definition() == definition && retained.key() == parse)
            .ok_or_else(|| invalid("generated declaration retained anchor differs"))?;
        if !matches!(self.published_input(&InputKey::GeneratedSyntax(unit.key.content)),
            Some((InputValue::GeneratedSyntax(current), _)) if current.same_unit(unit))
        {
            return Err(invalid("generated declaration unit publication changed"));
        }
        let rows = generated_unit_rows(unit, definition)?;
        let (
            Some(InputValue::DefinitionHeader(header)),
            Some(InputValue::DeclarationContext(context)),
            Some(InputValue::DefinitionBody(body)),
        ) = (
            rows.get(&InputKey::DefinitionHeader(definition)),
            rows.get(&InputKey::DeclarationContext(definition)),
            rows.get(&InputKey::DefinitionBody(definition)),
        )
        else {
            return Err(invalid("generated declaration unit rows differ"));
        };
        let (
            Some((InputValue::DefinitionHeader(current_header), _)),
            Some((InputValue::DeclarationContext(current_context), _)),
            Some((InputValue::DefinitionBody(current_body), _)),
        ) = (
            self.published_input(&InputKey::DefinitionHeader(definition)),
            self.published_input(&InputKey::DeclarationContext(definition)),
            self.published_input(&InputKey::DefinitionBody(definition)),
        )
        else {
            return Err(invalid("generated declaration retained rows absent"));
        };
        let header_len = body
            .range
            .start
            .checked_sub(context.range.start)
            .ok_or_else(|| invalid("generated declaration unit header differs"))?;
        // The unit carries the call edition of the expansion that wrote it (a
        // nested library macro call is edition 2024); the retained tree is in
        // the owning source's profile. Rows join modulo that profile.
        let header = &{
            let mut header = header.clone();
            header.profile = current_header.profile.clone();
            header
        };
        if !current_header.same_semantics(header)
            || current_context.scope != context.scope
            || current_context.range != anchor.declaration_range()
            || context.range != anchor.output_range()
            || current_body.range != anchor.body_range()
            || current_body.def_key != definition
            || current_body
                .range
                .start
                .checked_sub(current_context.range.start)
                != Some(header_len)
        {
            return Err(invalid("generated declaration retained rows differ"));
        }
        Ok(Some(GeneratedDeclarationBridge {
            unit: Arc::clone(unit),
            retained: parse.clone(),
            header: header.clone(),
            context: context.clone(),
            body: body.clone(),
            retained_start: current_context.range.start,
            header_len,
        }))
    }
}
fn original_rows(
    source: &PreparedSourceView,
    definition: Digest,
) -> Result<BTreeMap<InputKey, InputValue>, Diagnostic> {
    let original = source
        .outline()
        .items
        .iter()
        .find(|item| item.def_key.fingerprint() == definition)
        .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "original declaration absent"))?;
    Ok(BTreeMap::from([
        (
            InputKey::DefinitionSource(definition),
            InputValue::DefinitionSource(source.selection().outline.parse.clone()),
        ),
        (
            InputKey::DefinitionHeader(definition),
            InputValue::DefinitionHeader(original.header.clone()),
        ),
        (
            InputKey::DefinitionBody(definition),
            InputValue::DefinitionBody(
                original
                    .body
                    .clone()
                    .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "original body absent"))?,
            ),
        ),
        (
            InputKey::DeclarationContext(definition),
            InputValue::DeclarationContext(DeclarationContext {
                scope: original.scope.clone(),
                range: original.declaration_span,
            }),
        ),
    ]))
}
impl Database {
    fn has_definition_rows(&self, rows: &BTreeMap<InputKey, InputValue>) -> bool {
        rows.iter().all(|(key, value)| {
            self.inputs.get(key).is_some_and(|current| {
                current.fingerprint == value.fingerprint()
                    && match (current.value.as_ref(), value) {
                        (InputValue::DefinitionSource(a), InputValue::DefinitionSource(b)) => {
                            a == b
                        }
                        (InputValue::DefinitionHeader(a), InputValue::DefinitionHeader(b)) => {
                            a == b
                        }
                        (InputValue::DefinitionBody(a), InputValue::DefinitionBody(b)) => a == b,
                        (InputValue::DeclarationContext(a), InputValue::DeclarationContext(b)) => {
                            a == b
                        }
                        _ => false,
                    }
            })
        })
    }
    /// Replace this original member's source/header/body/context as one derived
    /// revision before frame entry. This creates no generated name or scope and
    /// does not compile, expand, execute, or alter an existing guest frame.
    pub fn publish_retained_body(
        &mut self,
        input: Arc<RetainedSyntaxInput>,
        definition: Digest,
    ) -> QueryPoll<BodyKey> {
        if let Err(error) = self.require_body_unowned(definition) {
            return QueryPoll::Ready(Err(error));
        }
        match input.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
        let Some(published) = self.input(&InputKey::RetainedSyntax(input.key().content)) else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "retained syntax must be published before its body",
            )));
        };
        if !matches!(published.as_ref(),InputValue::RetainedSyntax(actual) if actual.key()==input.key() && actual.fingerprint()==input.fingerprint())
        {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "retained body syntax publication differs",
            )));
        }
        let outcome = match self.fetch(
            &super::super::provider::SyntaxProvider,
            QueryKey::ParseFile(input.key().clone()),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let QueryValue::ParseFile(parsed) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "retained body parse result differs",
            )));
        };
        let result = (|| {
            let source = self
                .prepared_source(input.prepared().source())?
                .ok_or_else(|| {
                    fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained body actual source lease absent",
                    )
                })?;
            let generated = input.prepared().selected_generated().is_some();
            let (body, derived) = if generated {
                generated_rows(&input, &source, parsed, definition)?
            } else {
                rows(&input, &source, parsed, definition)?
            };
            if self.has_definition_rows(&derived) {
                return Ok(body);
            }
            // A generated associated function has no publication before its
            // first retained admission: none of its rows may exist yet. Any
            // later admission supersedes an earlier retained one (below).
            let associated = input.generated_associated().is_some();
            let first_associated =
                associated && derived.keys().all(|key| self.inputs.get(key).is_none());
            let unit_or_original = if associated {
                None
            } else if generated {
                Some(generated_unit_rows(
                    input.generated_unit().ok_or_else(|| {
                        fault("LRR-E-EXPANSION-INPUT", "retained generated unit absent")
                    })?,
                    definition,
                )?)
            } else {
                Some(original_rows(&source, definition)?)
            };
            let superseding = !first_associated
                && !unit_or_original
                    .as_ref()
                    .is_some_and(|expected| self.has_definition_rows(expected));
            if superseding {
                let Some(current) = self.input(&InputKey::DefinitionSource(definition)) else {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained previous definition source absent",
                    ));
                };
                let InputValue::DefinitionSource(previous) = current.as_ref() else {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained previous definition source differs",
                    ));
                };
                let Some(old) = self.input(&InputKey::RetainedSyntax(previous.content)) else {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained previous source preparation absent",
                    ));
                };
                let InputValue::RetainedSyntax(old) = old.as_ref() else {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained previous source variant differs",
                    ));
                };
                if old.key() != previous {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained previous source identity differs",
                    ));
                }
                // An immutable previous epoch may be superseded while unowned.
                // Replay proves the old rows' provenance; it is never executed
                // or accepted as current interpreter evidence.
                let expected = if generated {
                    generated_rows(old, &source, &old.replay()?, definition)?.1
                } else {
                    rows(old, &source, &old.replay()?, definition)?.1
                };
                if !self.has_definition_rows(&expected) {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained previous definition rows are inconsistent",
                    ));
                }
            }
            let mut staged = BTreeMap::new();
            let mut extra = 0;
            for (key, value) in derived {
                let fingerprint = value.fingerprint();
                // Semantic body/header fingerprints exclude source offsets.
                // Refresh their actual attachments with the new parse even
                // when the semantic key survives an unrelated expansion edit.
                extra = account_add(extra, input_metadata_bytes(&key, &value)?)?;
                staged.insert(
                    key,
                    InputRecord {
                        value: Arc::new(value),
                        fingerprint,
                    },
                );
            }
            let _ = extra; // E-R5-006: no byte accounting.
            let revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| fault("LRR-E-ID-EXHAUSTED", "retained body revision exhausted"))?;
            self.require_body_unowned(definition)?;
            self.inputs.extend(staged);
            self.revision = revision;
            Ok(body)
        })();
        QueryPoll::Ready(result)
    }
}
