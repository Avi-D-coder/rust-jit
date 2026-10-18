//! Exact token comparison for the ordinary local/parameter resolver. Normal
//! forms are retained once during parsing, using the pinned hygiene machinery.
use super::{ParsedFile, TokenContext};
use crate::{
    core::diagnostic::Diagnostic,
    syntax::{
        spans::SourceRange,
        token_wire::{Key, NameContextKey},
    },
};

impl ParsedFile {
    /// Raw context attached to this actual token occurrence. Callers must use
    /// the pinned hygiene service for namespace/rib adjustment; this is data.
    pub(crate) fn identifier_context(&self, range: SourceRange) -> Result<Option<Key>, Diagnostic> {
        Ok(self.token_context(range)?.map(|context| context.context))
    }
    /// Pinned item-namespace normalization for the actual selected identifier.
    /// This is context data, not member/source authority or rib adjustment.
    pub(crate) fn item_name_context(&self, range: SourceRange) -> Result<Option<Key>, Diagnostic> {
        let Some(context) = self.token_context(range)? else {
            return Ok(None);
        };
        let facts = self.hygiene_facts().ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "item source lacks its prepared hygiene facts".into(),
            )
        })?;
        Ok(Some(facts.item(context.context)?))
    }

    /// For a generated `$crate` token: the defining crate root it names and
    /// whether that is the crate whose original source contains this
    /// expansion (so it is exactly `crate`). A non-local root names another
    /// crate (e.g. a standard library macro's).
    pub(crate) fn dollar_crate_root(
        &self,
        range: SourceRange,
    ) -> Result<Option<(crate::core::digest::Digest, bool)>, Diagnostic> {
        let Some(context) = self.token_context(range)? else {
            return Ok(None);
        };
        let facts = self.hygiene_facts().ok_or_else(|| {
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "dollar-crate source lacks its prepared hygiene facts".into(),
            )
        })?;
        let defining = facts.dollar_crate(context.context)?;
        let mut current = context.context;
        loop {
            let record = facts.context(current)?;
            match record.parent {
                Some(parent) if parent != current => current = parent,
                _ => {
                    return Ok(Some((
                        crate::core::digest::Digest(defining.0),
                        record.defining_unit == defining,
                    )));
                }
            }
        }
    }
    pub(super) fn token_context(
        &self,
        range: SourceRange,
    ) -> Result<Option<&TokenContext>, Diagnostic> {
        let fail = || {
            // Failure facts only. Preserve exact containment; another token's
            // context cannot stand in for this actual source occurrence.
            let interval = self.token_contexts.as_ref().and_then(|contexts| {
                let index = contexts.partition_point(|context| context.end <= range.start);
                contexts.get(index).map(|context| {
                    let start = index.checked_sub(1).map_or(0, |i| contexts[i].end);
                    (index, start, context.end, context.context)
                })
            });
            let text = self.parsed.syntax_node().text().to_string();
            let fragment = text.get(range.start as usize..range.end as usize);
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                format!(
                    "generated name crosses or lacks its token context attachment \
                     (source {:?}, generated {:?}, range {}..{}, interval {:?}, text {:?})",
                    self.content, self.generated_origin, range.start, range.end, interval, fragment
                ),
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
        // attach_generated_token_editions admits contiguous intervals from0.
        // TokenContext stores only endpoints; the preceding endpoint is this
        // interval's start, rather than a separately stored source location.
        let start = index
            .checked_sub(1)
            .map_or(0, |previous| contexts[previous].end);
        if range.start >= range.end || range.start < start || range.end > context.end {
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
