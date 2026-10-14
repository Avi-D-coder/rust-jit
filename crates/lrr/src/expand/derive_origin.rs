//! Exact helper-declaration parent provenance, separate from macro outputs.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    source::changes::SourceSelection,
};
#[derive(Debug, Clone)]
pub(crate) enum DeriveDeclarationOrigin {
    Generated(super::record::ExpansionDescriptor),
    Source {
        selection: SourceSelection,
        producer: Digest,
        item: Digest,
        header: Digest,
    },
}
impl DeriveDeclarationOrigin {
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        match self {
            Self::Generated(record) => Ok(record.estimated_bytes()),
            Self::Source { selection, .. } => selection
                .metadata_bytes()?
                .checked_add(128)
                .ok_or_else(|| fault("source derive origin metadata overflow")),
        }
    }
    pub(crate) fn validate_database(
        &self,
        db: &mut crate::db::memo::Database,
    ) -> Result<(), Diagnostic> {
        use crate::db::{
            dependency::{InputKey, InputValue},
            key::QueryKey,
            poll::QueryPoll,
            provider::SyntaxProvider,
            value::QueryValue,
        };
        match self {
            Self::Generated(record) => {
                if !matches!(db.input(&InputKey::ExpansionRecord(record.key.fingerprint())).as_deref(),
                    Some(InputValue::ExpansionRecord(actual)) if actual.descriptor().fingerprint()==record.fingerprint())
                {
                    return Err(fault("derive declaration generated parent is not adopted"));
                }
            }
            Self::Source {
                selection,
                producer,
                item,
                header,
            } => {
                if db
                    .input(&InputKey::GeneratedSyntax(selection.outline.parse.content))
                    .is_some()
                {
                    return Err(fault(
                        "generated syntax requires an expansion parent origin",
                    ));
                }
                if !matches!(db.input(&InputKey::CapturedSource(selection.location_identity())).as_deref(),
                    Some(InputValue::CapturedSource(actual)) if actual==selection)
                {
                    return Err(fault("derive declaration captured source differs"));
                }
                let QueryPoll::Ready(Ok(outcome)) = db.fetch(
                    &SyntaxProvider,
                    QueryKey::Outline(selection.outline.clone()),
                ) else {
                    return Err(fault("derive declaration source outline unavailable"));
                };
                let QueryValue::Outline(outline) = outcome.value.as_ref() else {
                    return Err(fault("derive declaration source outline variant differs"));
                };
                if !outline.items.iter().any(|row| {
                    row.def_key.fingerprint() == *item
                        && row.header.fingerprint() == *header
                        && crate::syntax::outline::attribute_producer(row)
                            .is_some_and(|call| call.item_input == *producer)
                }) {
                    return Err(fault(
                        "derive declaration original item or producer differs",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn published_in(
        &self,
        sources: &[SourceSelection],
        records: &[super::record::ExpansionDescriptor],
    ) -> bool {
        match self {
            Self::Generated(record) => records
                .iter()
                .any(|row| row.fingerprint() == record.fingerprint()),
            Self::Source { selection, .. } => sources.contains(selection),
        }
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CATALOG-INCOMPLETE", message.into())
}
impl StableEncode for DeriveDeclarationOrigin {
    const DOMAIN: &'static str = "lrr/v2/derive-declaration-origin";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Generated(record) => {
                e.u8(0);
                e.record(record);
            }
            Self::Source {
                selection,
                producer,
                item,
                header,
            } => {
                e.u8(1);
                e.string(selection.path.as_str());
                e.digest(
                    crate::db::key::QueryKey::Outline(selection.outline.clone()).fingerprint(),
                );
                e.digest(*producer);
                e.digest(*item);
                e.digest(*header);
            }
        }
    }
}
