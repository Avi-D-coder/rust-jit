//! Original effective-meta resolution. Pure composition; no helper execution.
use super::{
    builtin_attributes::{AttributeDecision, BuiltinAttributeCatalog},
    item_attributes::classify_custom_candidate,
    macro_names::MacroNamespaces,
    procedural_exports::PreparedProcCatalog,
    source_macros::SourceMacroIndex,
};
use crate::{
    core::{diagnostic::Diagnostic, digest::Digest},
    expand::{derive::ItemHelperScope, identity::ExpansionKey, scopes::MacroName},
    syntax::{imports::ImportSegment, macro_calls::SourceAttributePath, outline::ScopeKey},
};
pub struct OriginalAttributeResolver<'a> {
    pub source: &'a SourceMacroIndex,
    pub namespaces: &'a MacroNamespaces,
    pub procedural: &'a PreparedProcCatalog,
    pub builtin: &'a BuiltinAttributeCatalog,
    pub helpers: &'a ItemHelperScope,
    pub owner: Digest,
    pub cfg: Digest,
    pub active_derives: &'a [ExpansionKey],
}
pub enum OriginalAttributeResolution {
    Inactive,
    Candidate(AttributeDecision),
}
impl OriginalAttributeResolver<'_> {
    pub fn resolve(
        &self,
        attribute: &SourceAttributePath,
        scope: &ScopeKey,
    ) -> Result<OriginalAttributeResolution, Diagnostic> {
        if self.source.cfg() != self.cfg || !self.helpers.context_matches(self.owner, self.cfg) {
            return Err(Diagnostic::new(
                "LRR-E-MACRO-SCOPE",
                "original attribute resolution owner/cfg context differs".into(),
            ));
        }
        if !self.source.attribute_eligible(attribute, scope)? {
            return Ok(OriginalAttributeResolution::Inactive);
        }
        let unqualified = if !attribute.absolute() && attribute.segments().len() == 1 {
            match &attribute.segments()[0] {
                ImportSegment::Name(name) => {
                    Some(MacroName::parse(name, attribute.key().profile.edition)?)
                }
                _ => None,
            }
        } else {
            None
        };
        let namespace =
            self.source
                .lookup_attribute_candidate(attribute, self.namespaces, scope)?;
        let decision = classify_custom_candidate(
            self.helpers,
            self.owner,
            self.cfg,
            self.active_derives,
            unqualified.as_ref().map(MacroName::as_str),
            namespace,
            self.procedural,
        )?;
        Ok(OriginalAttributeResolution::Candidate(
            self.builtin.classify(decision)?,
        ))
    }
}

/// Lexical candidate inspection. A procedural replacement requires a new cursor
/// over reanalysed output; this cursor cannot skip transformations or suspension.
pub struct OriginalItemAttributeCursor {
    parse: crate::syntax::parse::ParseKey,
    cfg: Digest,
    owner: Digest,
    scope: ScopeKey,
    item: crate::syntax::spans::SourceRange,
    ranges: Vec<crate::syntax::spans::SourceRange>,
    next: usize,
}
pub enum OriginalAttributePoll {
    Complete,
    Candidate {
        index: usize,
        decision: AttributeDecision,
    },
}
fn cursor_fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
impl OriginalItemAttributeCursor {
    pub(crate) fn matches_selection(
        &self,
        parse: &crate::syntax::parse::ParseKey,
        owner: Digest,
        cfg: Digest,
        item: crate::syntax::spans::SourceRange,
        meta: crate::syntax::spans::SourceRange,
        index: usize,
    ) -> bool {
        &self.parse == parse
            && self.owner == owner
            && self.cfg == cfg
            && self.item == item
            && self.next == index
            && self.ranges.get(index) == Some(&meta)
    }
    pub fn new(
        resolver: &OriginalAttributeResolver<'_>,
        parsed: &crate::syntax::parse::ParsedFile,
        scope: ScopeKey,
        item: crate::syntax::spans::SourceRange,
    ) -> Result<Option<Self>, Diagnostic> {
        if resolver.source.cfg() != resolver.cfg
            || !resolver
                .helpers
                .context_matches(resolver.owner, resolver.cfg)
        {
            return Err(cursor_fault("original attribute cursor owner/cfg mismatch"));
        }
        let parse = parsed.key();
        let Some(ranges) = resolver
            .source
            .original_item_attributes(&parse, &scope, item)?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            parse,
            cfg: resolver.cfg,
            owner: resolver.owner,
            scope,
            item,
            ranges: ranges.to_vec(),
            next: 0,
        }))
    }
    pub fn poll(
        &self,
        resolver: &OriginalAttributeResolver<'_>,
        parsed: &crate::syntax::parse::ParsedFile,
    ) -> Result<OriginalAttributePoll, Diagnostic> {
        if parsed.key() != self.parse
            || resolver.cfg != self.cfg
            || resolver.owner != self.owner
            || resolver.source.cfg() != self.cfg
            || !resolver.helpers.context_matches(self.owner, self.cfg)
            || resolver
                .source
                .original_item_attributes(&self.parse, &self.scope, self.item)?
                != Some(self.ranges.as_slice())
        {
            return Err(cursor_fault("stale original attribute cursor"));
        }
        let Some(range) = self.ranges.get(self.next) else {
            return Ok(OriginalAttributePoll::Complete);
        };
        let path = parsed.attribute_path_at(*range)?;
        match resolver.resolve(&path, &self.scope)? {
            OriginalAttributeResolution::Candidate(decision) => {
                Ok(OriginalAttributePoll::Candidate {
                    index: self.next,
                    decision,
                })
            }
            OriginalAttributeResolution::Inactive => {
                Err(cursor_fault("indexed attribute became inactive"))
            }
        }
    }
    /// Advances only a non-transforming candidate. This does not seal namespace
    /// membership, prepare a request, or authorize macro execution.
    pub fn advance_nontransforming_candidate(
        &mut self,
        resolver: &OriginalAttributeResolver<'_>,
        parsed: &crate::syntax::parse::ParsedFile,
        expected_index: usize,
    ) -> Result<(), Diagnostic> {
        use super::item_attributes::CustomAttributeDecision;
        use crate::expand::builtin_attributes::BuiltinAttributeAction;
        if expected_index != self.next {
            return Err(cursor_fault("original attribute cursor index differs"));
        }
        let allowed = match self.poll(resolver, parsed)? {
            OriginalAttributePoll::Candidate {
                decision: AttributeDecision::Custom(CustomAttributeDecision::Helpers(_)),
                ..
            } => true,
            OriginalAttributePoll::Candidate {
                decision: AttributeDecision::Builtin(descriptor),
                ..
            } => descriptor.action() == BuiltinAttributeAction::Derive,
            _ => false,
        };
        if !allowed {
            return Err(cursor_fault(
                "attribute requires suspension or transformation",
            ));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| cursor_fault("attribute cursor overflow"))?;
        Ok(())
    }
}
