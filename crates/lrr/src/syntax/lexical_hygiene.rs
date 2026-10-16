//! Exact token comparison for the ordinary local/parameter resolver. Normal
//! forms are retained once during parsing, using the pinned hygiene machinery.
use super::{ParsedFile, TokenContext};
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{spans::SourceRange, token_wire::NameContextKey},
};

impl ParsedFile {
    pub(super) fn token_context(
        &self,
        range: SourceRange,
    ) -> Result<Option<&TokenContext>, Diagnostic> {
        let fail = || {
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "generated name crosses or lacks its token context attachment".into(),
            )
        };
        let Some(contexts) = &self.token_contexts else {
            return if self.generated_origin.is_none() {
                Ok(None)
            } else {
                Err(fail())
            };
        };
        let index = contexts.partition_point(|context| context.end <= range.start);
        let context = contexts.get(index).ok_or_else(fail)?;
        if range.start >= range.end || range.end > context.end {
            return Err(fail());
        }
        Ok(Some(context))
    }

    /// Prepared pinned class for an actual identifier occurrence. This does
    /// not adjust definition-site ribs or certify a namespace binding.
    pub(crate) fn local_name_class(
        &self,
        range: SourceRange,
    ) -> Result<Option<NameContextKey>, Diagnostic> {
        let Some(context) = self.token_context(range)? else {
            return Ok(None);
        };
        let facts = self.hygiene_facts().ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "lexical source lacks its prepared hygiene facts".into(),
            )
        })?;
        Ok(Some(facts.name_local(context.context)?))
    }

    /// Compare actual single identifier occurrences; equality is neither a
    /// namespace admission nor a definition-site scope adjustment. Incomplete
    /// source joins must remain unresolved instead of capturing caller names.
    pub(crate) fn same_local_name_context(
        &self,
        reference: SourceRange,
        binding: SourceRange,
    ) -> Result<bool, Diagnostic> {
        match (
            self.local_name_class(reference)?,
            self.local_name_class(binding)?,
        ) {
            (None, None) => Ok(true),
            (Some(reference), Some(binding)) => Ok(reference == binding),
            _ => Err(Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "lexical reference and binding source contexts differ".into(),
            )),
        }
    }
}
