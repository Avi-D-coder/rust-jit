//! Snapshot-only span subrequests. Private wire IDs are validated before use.
use super::*;
use crate::proc_client::CapturedSpanSource;
use proc_macro_api::bidirectional_protocol::msg::{SubRequest, SubResponse};

fn fault(message: &str) -> AdapterError {
    AdapterError {
        code: "LRR-E-MACRO-OUTPUT",
        message: message.into(),
    }
}
impl SpanStore {
    fn proc_record(&self, file: u32, ast: Option<u32>) -> Result<&SpanRecord, AdapterError> {
        let slot = if ast.is_some() {
            file & EditionedFileId::FILE_ID_MASK
        } else {
            file
        };
        if ast.is_some_and(|id| id != ROOT_ERASED_FILE_AST_ID.into_raw()) {
            return Err(fault("unregistered procedural span anchor"));
        }
        for edition in span::Edition::iter() {
            let anchor = SpanAnchor {
                file_id: EditionedFileId::new(FileId::from_raw(slot), edition),
                ast_id: ROOT_ERASED_FILE_AST_ID,
            };
            // File/local-file carry a plain FileId; anchored operations carry
            // the full EditionedFileId. Compare known encodings before lookup.
            if ast.is_some() && anchor.file_id.as_u32() != file {
                continue;
            }
            if let Some(record) = self
                .anchors
                .get(&anchor)
                .and_then(|key| self.records.get(key))
            {
                return Ok(record);
            }
        }
        Err(fault("unregistered procedural span file"))
    }
    fn proc_source<'a>(
        &self,
        record: &SpanRecord,
        sources: &'a [CapturedSpanSource],
    ) -> Result<&'a CapturedSpanSource, AdapterError> {
        let mut matches = sources
            .iter()
            .filter(|source| source.source() == &record.source);
        let source = matches
            .next()
            .ok_or_else(|| fault("span operation requires captured source bytes"))?;
        if matches.next().is_some() {
            return Err(fault("duplicate captured source identity"));
        }
        Ok(source)
    }
    fn proc_range(
        &self,
        record: &SpanRecord,
        start: u32,
        end: u32,
        sources: &[CapturedSpanSource],
    ) -> Result<(), AdapterError> {
        if start > end {
            return Err(fault("reversed procedural span range"));
        }
        let source = self.proc_source(record, sources)?;
        if source.text().get(start as usize..end as usize).is_none() {
            return Err(fault("procedural span range exceeds captured UTF-8 source"));
        }
        Ok(())
    }
    pub(crate) fn proc_callback(
        &self,
        request: SubRequest,
        sources: &[CapturedSpanSource],
    ) -> Result<SubResponse, AdapterError> {
        match request {
            SubRequest::FilePath { file_id } => Ok(SubResponse::FilePathResult {
                name: self.proc_record(file_id, None)?.source.path.clone(),
            }),
            SubRequest::LocalFilePath { file_id } => {
                let record = self.proc_record(file_id, None)?;
                Ok(SubResponse::LocalFilePathResult {
                    name: self
                        .proc_source(record, sources)?
                        .local_path()
                        .map(str::to_owned),
                })
            }
            SubRequest::SourceText {
                file_id,
                ast_id,
                start,
                end,
            } => {
                let record = self.proc_record(file_id, Some(ast_id))?;
                let source = self.proc_source(record, sources)?;
                // Ranges created by the native token parser can be synthetic.
                // They are not allowed to alias arbitrary caller source text.
                let text = if start >= record.start && end <= record.end && start <= end {
                    source
                        .text()
                        .get(start as usize..end as usize)
                        .map(str::to_owned)
                } else {
                    None
                };
                Ok(SubResponse::SourceTextResult { text })
            }
            SubRequest::LineColumn {
                file_id,
                ast_id,
                offset,
            } => {
                let record = self.proc_record(file_id, Some(ast_id))?;
                self.proc_range(record, offset, offset, sources)?;
                let prefix = &self.proc_source(record, sources)?.text()[..offset as usize];
                let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count() + 1)
                    .map_err(|_| fault("span line overflow"))?;
                let column =
                    u32::try_from(prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1)
                        .map_err(|_| fault("span column overflow"))?;
                Ok(SubResponse::LineColumnResult { line, column })
            }
            SubRequest::ByteRange {
                file_id,
                ast_id,
                start,
                end,
            } => {
                let record = self.proc_record(file_id, Some(ast_id))?;
                self.proc_range(record, start, end, sources)?;
                Ok(SubResponse::ByteRangeResult {
                    range: start as usize..end as usize,
                })
            }
            SubRequest::SpanSource {
                file_id,
                ast_id,
                start,
                end,
                ctx,
            } => {
                let record = self.proc_record(file_id, Some(ast_id))?;
                self.proc_range(record, start, end, sources)?;
                let context = self.transport_context(record.context)?;
                if context.into_u32() != ctx {
                    return Err(fault("unregistered procedural span source context"));
                }
                if self
                    .contexts
                    .get(&record.context)
                    .is_none_or(|context| context.parent.is_some())
                {
                    return Err(fault("expanded span source ancestry is not supplied"));
                }
                Ok(SubResponse::SpanSourceResult {
                    file_id,
                    ast_id,
                    start,
                    end,
                    ctx,
                })
            }
            SubRequest::SpanParent {
                file_id,
                ast_id,
                start,
                end,
                ctx,
            } => {
                let record = self.proc_record(file_id, Some(ast_id))?;
                self.proc_range(record, start, end, sources)?;
                if self.transport_context(record.context)?.into_u32() != ctx {
                    return Err(fault("unregistered procedural parent context"));
                }
                if self
                    .contexts
                    .get(&record.context)
                    .is_none_or(|context| context.parent.is_some())
                {
                    return Err(fault("expanded span parent ancestry is not supplied"));
                }
                Ok(SubResponse::SpanParentResult { parent_span: None })
            }
            SubRequest::SpanJoin { .. } => Err(fault(
                "span join requires unambiguous source/anchor identity",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        proc_client::SourceDigestEncoding,
        token_wire::{self, Edition, SourceIdentity, Transparency},
    };
    #[test]
    fn captured_callbacks_check_wire_ids_utf8_and_source_identity() {
        let text = "\n\"hé\"";
        let unit = Key([7; 32]);
        let source = SourceIdentity {
            unit,
            content: Key(*blake3::hash(text.as_bytes()).as_bytes()),
            path: "snapshot.rs".into(),
        };
        let forest = token_wire::parse_source(
            text,
            source.clone(),
            ContextRecord {
                parent: None,
                expansion: None,
                transparency: Transparency::Opaque,
                defining_unit: unit,
                edition: Edition::E2024,
            },
            Key([8; 32]),
        )
        .unwrap();
        let captured =
            CapturedSpanSource::new(source, text.into(), SourceDigestEncoding::RawBlake3, None)
                .unwrap();
        let mut store = SpanStore::default();
        store.import(&forest).unwrap();
        let record = forest
            .spans()
            .iter()
            .find(|s| s.start == 1 && s.end == 6)
            .unwrap();
        let span = store.span(record.key()).unwrap();
        let file = span.anchor.file_id.as_u32();
        let ast = span.anchor.ast_id.into_raw();
        let request = SubRequest::SourceText {
            file_id: file,
            ast_id: ast,
            start: 1,
            end: 6,
        };
        assert!(
            matches!(store.proc_callback(request, std::slice::from_ref(&captured)).unwrap(), SubResponse::SourceTextResult { text: Some(text) } if text == "\"hé\"")
        );
        assert!(matches!(
            store
                .proc_callback(
                    SubRequest::LineColumn {
                        file_id: file,
                        ast_id: ast,
                        offset: 5
                    },
                    std::slice::from_ref(&captured)
                )
                .unwrap(),
            SubResponse::LineColumnResult { line: 2, column: 4 }
        ));
        for (file_id, ast_id, offset) in [
            (file | 0x8000_0000, ast, 1),
            (file, ast ^ 1, 1),
            (file, ast, 4),
            (file, ast, 7),
        ] {
            assert_eq!(
                store
                    .proc_callback(
                        SubRequest::LineColumn {
                            file_id,
                            ast_id,
                            offset
                        },
                        std::slice::from_ref(&captured)
                    )
                    .unwrap_err()
                    .code,
                "LRR-E-MACRO-OUTPUT"
            );
        }
        assert!(
            store
                .proc_callback(
                    SubRequest::LocalFilePath {
                        file_id: span.anchor.file_id.file_id().index()
                    },
                    &[]
                )
                .is_err()
        );
        assert!(
            store
                .proc_callback(
                    SubRequest::SourceText {
                        file_id: file,
                        ast_id: ast,
                        start: 1,
                        end: 6
                    },
                    &[captured.clone(), captured]
                )
                .is_err()
        );
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::token_wire::{self, Edition, SourceIdentity, Transparency};
    #[test]
    fn procedural_labels_roundtrip_without_changing_owned_hygiene_or_equivalence() {
        let mut store = SpanStore::default();
        let mut inputs = Vec::new();
        for id in [21, 22] {
            let unit = Key([id; 32]);
            let input = token_wire::parse_source(
                "item",
                SourceIdentity {
                    unit,
                    content: Key(*blake3::hash(b"item").as_bytes()),
                    path: format!("{id}.rs"),
                },
                ContextRecord {
                    parent: None,
                    expansion: None,
                    transparency: Transparency::Opaque,
                    defining_unit: unit,
                    edition: Edition::E2024,
                },
                Key([23; 32]),
            )
            .unwrap();
            inputs.push(input);
        }
        let first = store.import_proc(&inputs[0]).unwrap();
        let second = store.import_proc(&inputs[1]).unwrap();
        let contexts: Vec<_> = inputs.iter().map(|f| f.contexts()[0].key()).collect();
        assert_eq!(
            store.hygiene.registered_context(contexts[0]).unwrap(),
            store.hygiene.registered_context(contexts[1]).unwrap()
        );
        assert_ne!(
            store.transport_context(contexts[0]).unwrap(),
            store.transport_context(contexts[1]).unwrap()
        );
        assert_eq!(
            store.hygiene.records().len(),
            2,
            "private labels must not become owned semantic marks"
        );
        assert!(
            store
                .hygiene
                .equivalent(
                    contexts[0],
                    contexts[1],
                    crate::hygiene::ResolutionMode::Local
                )
                .unwrap()
        );
        assert_eq!(store.export_proc(&first, Key([24; 32])).unwrap(), inputs[0]);
        assert_eq!(
            store.export_proc(&second, Key([24; 32])).unwrap(),
            inputs[1]
        );
        assert_eq!(store.hygiene.records().len(), 2);
    }
}
