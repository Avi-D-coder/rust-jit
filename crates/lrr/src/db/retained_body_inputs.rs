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
            let (body, derived) = rows(&input, &source, parsed, definition)?;
            if self.has_definition_rows(&derived) {
                return Ok(body);
            }
            let mut expected = original_rows(&source, definition)?;
            if !self.has_definition_rows(&expected) {
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
                expected = rows(old, &source, &old.replay()?, definition)?.1;
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
            // Charge the peak of both old and staged rows. External source
            // owners may retain the old immutable inputs after replacement.
            if account_add(self.cache_usage()?.memo_bytes, extra)?
                > self.run_budgets.unwrap_or_default().query_bytes
            {
                return Err(fault(
                    "LRR-E-METADATA-LIMIT",
                    "retained body publication exceeds query metadata cap",
                ));
            }
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
