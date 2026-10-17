use super::{ast_adapter::range, spans::SourceRange};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
        profile::ProfileIdentity,
    },
    source::snapshot::SourceBlob,
};
use ra_ap_syntax::{AstNode, Edition, Parse, ast};
use serde::{Deserialize, Serialize};

#[path = "parse_crlf.rs"]
mod crlf;
#[path = "parse_retained.rs"]
mod retained;

/// Keep pinned-parser newline repairs identical for original files and bounded
/// operand wrappers. This never rewrites the retained tree or source offsets.
pub(super) fn diagnostics(
    parsed: &Parse<ast::SourceFile>,
    edition: Edition,
) -> Vec<ParseDiagnostic> {
    let tree = parsed.tree();
    let mut crlf_errors = crlf::TokenErrors::default();
    parsed
        .errors()
        .into_iter()
        .filter(|error| {
            !crlf_errors.normalized_literal_error(
                tree.syntax(),
                error.range(),
                &error.to_string(),
                edition,
            ) && !final_method_modifier_error(tree.syntax(), error.range(), &error.to_string())
        })
        .map(|error| ParseDiagnostic {
            range: range(error.range()),
            message: error.to_string(),
        })
        .collect()
}

/// rustc's `final` associated-function modifier (`final fn f(&self) {}` in a
/// trait, used by `core`) postdates the pinned parser, which recovers it as an
/// `ERROR` holding only the `final` keyword immediately before an ordinary
/// `FN`. The function itself parses normally; `final` only forbids overriding
/// in impls and has no runtime meaning, so this exact shape is not a
/// declaration parse failure or boundary.
pub(crate) fn is_final_method_modifier(node: &ra_ap_syntax::SyntaxNode) -> bool {
    use ra_ap_syntax::{NodeOrToken, SyntaxKind};
    if node.kind() != SyntaxKind::ERROR
        || node.parent().map(|parent| parent.kind()) != Some(SyntaxKind::ASSOC_ITEM_LIST)
    {
        return false;
    }
    let mut tokens = node
        .children_with_tokens()
        .filter(|element| !element.kind().is_trivia());
    if !matches!(tokens.next(), Some(NodeOrToken::Token(token)) if token.kind() == SyntaxKind::FINAL_KW)
        || tokens.next().is_some()
    {
        return false;
    }
    let next = std::iter::successors(node.next_sibling_or_token(), |element| {
        element.next_sibling_or_token()
    })
    .find(|element| !element.kind().is_trivia());
    matches!(next, Some(NodeOrToken::Node(next)) if next.kind() == SyntaxKind::FN)
}
fn final_method_modifier_error(
    root: &ra_ap_syntax::SyntaxNode,
    range: ra_ap_syntax::TextRange,
    message: &str,
) -> bool {
    if message != "expected an item" || !range.is_empty() {
        return false;
    }
    let Some(at) = root.token_at_offset(range.start()).right_biased() else {
        return false;
    };
    at.kind() == ra_ap_syntax::SyntaxKind::FINAL_KW
        && at.text_range().start() == range.start()
        && at.parent().is_some_and(|parent| is_final_method_modifier(&parent))
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ParseKey {
    pub content: Digest,
    pub profile: ProfileIdentity,
}
impl StableEncode for ParseKey {
    const DOMAIN: &'static str = "lrr/v2/parse-file";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.content);
        e.record(&self.profile);
    }
}
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ParseDiagnostic {
    pub range: SourceRange,
    pub message: String,
}
#[derive(Debug)]
pub struct ParsedFile {
    pub content: Digest,
    pub edition: u16,
    pub profile: ProfileIdentity,
    pub errors: Vec<ParseDiagnostic>,
    pub(crate) parsed: Parse<ast::SourceFile>,
    pub(crate) generated_origin: Option<Digest>,
    pub(crate) declaration_grammar: Option<crate::expand::identity::GrammarSlot>,
    estimated_bytes: usize,
    token_contexts: Option<Vec<TokenContext>>,
    pinned_hygiene: Option<std::sync::Arc<crate::frontend::hygiene::HygieneFacts>>,
    retained_syntax: Option<retained::RetainedSyntax>,
}
// Actual token context keys into the retained pinned facts; no source authority.
#[derive(Debug)]
struct TokenContext {
    end: u32,
    edition: u16,
    context: crate::syntax::token_wire::Key,
}
#[path = "lexical_hygiene.rs"]
mod lexical_hygiene;
impl ParsedFile {
    pub(crate) fn has_runtime_macro_syntax(&self) -> bool {
        self.parsed
            .tree()
            .syntax()
            .descendants()
            .filter(|node| ast::MacroCall::can_cast(node.kind()))
            .any(|node| {
                node.ancestors().any(|parent| {
                    ast::Fn::can_cast(parent.kind())
                        || ast::Const::can_cast(parent.kind())
                        || ast::Static::can_cast(parent.kind())
                })
            })
    }
    /// Pure structural parsing. Does not resolve names, types, or expressions.
    pub fn parse(blob: &SourceBlob, profile: ProfileIdentity) -> Result<Self, Diagnostic> {
        profile.validate_live()?;
        Self::parse_structural(blob, profile)
    }
    /// Declaration-only source facets retain their recorded edition. This grants
    /// no live-entry, body-preparation or macro-execution authority.
    pub fn parse_declarations(
        blob: &SourceBlob,
        profile: ProfileIdentity,
    ) -> Result<Self, Diagnostic> {
        profile.validate_source()?;
        Self::parse_structural(blob, profile)
    }
    /// Explicit preparation admits recorded dependency syntax without changing the live entry profile.
    pub(crate) fn parse_dependency(
        blob: &SourceBlob,
        profile: ProfileIdentity,
    ) -> Result<Self, Diagnostic> {
        profile.validate_source()?;
        Self::parse_structural(blob, profile)
    }
    /// Macro-host source facets retain their declared edition without body authority.
    pub(crate) fn parse_macro_facet(blob: &SourceBlob, edition: u16) -> Result<Self, Diagnostic> {
        let mut profile = ProfileIdentity::current();
        profile.edition = edition;
        Self::parse_structural(blob, profile)
    }
    fn parse_structural(blob: &SourceBlob, profile: ProfileIdentity) -> Result<Self, Diagnostic> {
        let edition = match profile.edition {
            2015 => Edition::Edition2015,
            2018 => Edition::Edition2018,
            2021 => Edition::Edition2021,
            2024 => Edition::Edition2024,
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-TOOLCHAIN-CAPABILITY",
                    "unknown macro source edition".into(),
                ));
            }
        };
        let text = std::str::from_utf8(blob.bytes())
            .map_err(|e| Diagnostic::new("LRR-E-SOURCE-UTF8", e.to_string()))?;
        let parsed = ast::SourceFile::parse(text, edition);
        let errors = diagnostics(&parsed, edition);
        // Rowans's internal allocator overhead is an estimate, documented in
        // docs/query-keys.md. Owned capacities are accounted separately.
        let elements = parsed.tree().syntax().descendants_with_tokens().count();
        let estimated_bytes = elements
            .checked_mul(32)
            .and_then(|n| n.checked_add(blob.bytes().len()))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-SOURCE-LIMIT",
                    "parse metadata accounting overflow".into(),
                )
            })?;
        Ok(Self {
            content: blob.content(),
            edition: profile.edition,
            profile,
            errors,
            parsed,
            estimated_bytes,
            token_contexts: None,
            pinned_hygiene: None,
            generated_origin: None,
            declaration_grammar: None,
            retained_syntax: None,
        })
    }
    pub(crate) fn from_generated_items(
        items: &crate::expand::generated_items::GeneratedItems,
    ) -> Result<Self, Diagnostic> {
        use lrr_expand_adapter::item_output::SyntaxEvent;
        let fail = || {
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "invalid pinned owned item syntax".into(),
            )
        };
        let mut builder = ra_ap_syntax::SyntaxTreeBuilder::default();
        let mut depth = 0usize;
        for (position, event) in items.syntax_events().iter().enumerate() {
            match event {
                SyntaxEvent::Start(raw) => {
                    if *raw >= ra_ap_syntax::SyntaxKind::__LAST as u16 {
                        return Err(fail());
                    }
                    let kind = ra_ap_syntax::SyntaxKind::from(*raw);
                    if position == 0 {
                        if kind != ra_ap_syntax::SyntaxKind::MACRO_ITEMS {
                            return Err(fail());
                        }
                        builder.start_node(ra_ap_syntax::SyntaxKind::SOURCE_FILE);
                    } else {
                        if depth == 0 {
                            return Err(fail());
                        }
                        builder.start_node(kind);
                    }
                    depth = depth.checked_add(1).ok_or_else(fail)?;
                }
                SyntaxEvent::Token { kind, text } => {
                    if depth == 0 || *kind >= ra_ap_syntax::SyntaxKind::__LAST as u16 {
                        return Err(fail());
                    }
                    builder.token(ra_ap_syntax::SyntaxKind::from(*kind), text);
                }
                SyntaxEvent::Finish => {
                    depth = depth.checked_sub(1).ok_or_else(fail)?;
                    builder.finish_node();
                }
            }
        }
        if depth != 0 || items.syntax_events().is_empty() {
            return Err(fail());
        }
        let parsed = builder
            .finish()
            .cast::<ast::SourceFile>()
            .ok_or_else(fail)?;
        let estimated_bytes = items
            .syntax_events()
            .len()
            .checked_mul(32)
            .and_then(|n| n.checked_add(items.text().len()))
            .ok_or_else(fail)?;
        let tokens = lrr_expand_adapter::token_wire::TokenForest::decode(items.record().wire())
            .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let parsed = Self {
            content: {
                let mut e = StableEncoder::new("lrr/v2/generated-syntax-input");
                e.record(&items.record().descriptor());
                e.finish()
            },
            edition: items.record().key().call_edition,
            profile: ProfileIdentity {
                edition: items.record().key().call_edition,
                semantics: ProfileIdentity::current().semantics,
            },
            errors: Vec::new(),
            parsed,
            estimated_bytes,
            token_contexts: None,
            pinned_hygiene: None,
            generated_origin: Some(items.record().descriptor().fingerprint()),
            declaration_grammar: Some(crate::expand::identity::GrammarSlot::Items),
            retained_syntax: None,
        };
        parsed.attach_generated_token_editions(
            &tokens,
            items.origins().iter().map(|origin| {
                (
                    origin.start,
                    origin.end,
                    lrr_expand_adapter::token_wire::Key(origin.span.0),
                )
            }),
            Some(items.hygiene_facts()),
        )
    }
    pub(crate) fn from_generated_declarations(
        syntax: &[lrr_expand_adapter::item_output::SyntaxEvent],
        text_len: usize,
        identity: Digest,
        edition: u16,
        grammar: crate::expand::identity::GrammarSlot,
    ) -> Result<Self, Diagnostic> {
        use lrr_expand_adapter::item_output::SyntaxEvent;
        let fail = || {
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "invalid pinned owned declaration syntax".into(),
            )
        };
        if syntax.is_empty() {
            return Err(fail());
        }
        let mut builder = ra_ap_syntax::SyntaxTreeBuilder::default();
        // An arena container only: no source text or grammar wrapper is parsed.
        // Every expression/statement/type node stays exactly as the pinned
        // bridge emitted it, at its original output offset.
        builder.start_node(ra_ap_syntax::SyntaxKind::SOURCE_FILE);
        let mut depth = 0usize;
        for (position, event) in syntax.iter().enumerate() {
            match event {
                SyntaxEvent::Start(raw) => {
                    if *raw >= ra_ap_syntax::SyntaxKind::__LAST as u16
                        || (depth == 0 && position != 0)
                    {
                        return Err(fail());
                    }
                    depth = depth
                        .checked_add(1)
                        .ok_or_else(fail)?;
                    builder.start_node(ra_ap_syntax::SyntaxKind::from(*raw));
                }
                SyntaxEvent::Token { kind, text } => {
                    if depth == 0 || *kind >= ra_ap_syntax::SyntaxKind::__LAST as u16 {
                        return Err(fail());
                    }
                    builder.token(ra_ap_syntax::SyntaxKind::from(*kind), text);
                }
                SyntaxEvent::Finish => {
                    depth = depth.checked_sub(1).ok_or_else(fail)?;
                    builder.finish_node();
                }
            }
        }
        if depth != 0 {
            return Err(fail());
        }
        builder.finish_node();
        let parsed = builder
            .finish()
            .cast::<ast::SourceFile>()
            .ok_or_else(fail)?;
        if u32::from(parsed.syntax_node().text_range().len()) as usize != text_len {
            return Err(fail());
        }
        let profile = ProfileIdentity {
            edition,
            semantics: ProfileIdentity::current().semantics,
        };
        profile.validate_source()?;
        let estimated_bytes = syntax
            .len()
            .checked_mul(32)
            .and_then(|n| n.checked_add(text_len))
            .ok_or_else(fail)?;
        Ok(Self {
            content: identity,
            edition,
            profile,
            errors: Vec::new(),
            parsed,
            estimated_bytes,
            token_contexts: None,
            pinned_hygiene: None,
            generated_origin: Some(identity),
            declaration_grammar: Some(grammar),
            retained_syntax: None,
        })
    }
    /// Retain the same token-context editions used by the pinned syntax bridge.
    /// Origins must cover this exact generated arena; no caller-edition fallback.
    pub(crate) fn with_generated_token_editions(
        self,
        tokens: &lrr_expand_adapter::token_wire::TokenForest,
        origins: &[lrr_expand_adapter::item_output::Origin],
    ) -> Result<Self, Diagnostic> {
        self.attach_generated_token_editions(
            tokens,
            origins
                .iter()
                .map(|origin| (origin.start, origin.end, origin.span)),
            None,
        )
    }
    fn attach_generated_token_editions(
        mut self,
        tokens: &lrr_expand_adapter::token_wire::TokenForest,
        origins: impl ExactSizeIterator<Item = (u32, u32, lrr_expand_adapter::token_wire::Key)>,
        prepared: Option<std::sync::Arc<crate::frontend::hygiene::HygieneFacts>>,
    ) -> Result<Self, Diagnostic> {
        let fail = || {
            Diagnostic::new(
                "LRR-E-MACRO-OUTPUT",
                "generated token edition attachment differs or exceeds limit".into(),
            )
        };
        if self.generated_origin.is_none() {
            return Err(fail());
        }
        tokens
            .contexts()
            .len()
            .checked_add(tokens.spans().len())
            .and_then(|n| n.checked_mul(256))
            .ok_or_else(fail)?;
        // Normalization can add item/local context records. Bound that retained
        // expansion and interval storage before allocating the fact tables.
        tokens
            .contexts()
            .len()
            .checked_mul(1024)
            .and_then(|bytes| {
                origins
                    .len()
                    .checked_mul(std::mem::size_of::<TokenContext>())
                    .and_then(|intervals| bytes.checked_add(intervals))
            })
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-METADATA-LIMIT",
                    "prepared source hygiene byte budget".into(),
                )
            })?;
        let facts = match prepared {
            Some(facts) => facts,
            None => {
                std::sync::Arc::new(crate::expand::source_hygiene::adapter_output_facts(tokens)?)
            }
        };
        origins
            .len()
            .checked_mul(std::mem::size_of::<TokenContext>())
            .ok_or_else(fail)?;
        let mut editions = Vec::new();
        editions
            .try_reserve_exact(origins.len())
            .map_err(|_| fail())?;
        let mut end = 0;
        for (start, next_end, span) in origins {
            if start != end || next_end <= end {
                return Err(fail());
            }
            // The validated immutable forest already has a strict span-key
            // index; do not rebuild it for every representation conversion.
            let position = tokens
                .spans()
                .binary_search_by_key(&span, |record| record.key())
                .map_err(|_| fail())?;
            let context = crate::syntax::token_wire::Key(tokens.spans()[position].context.0);
            let edition = match facts.context(context)?.edition {
                crate::syntax::token_wire::Edition::E2015 => 2015,
                crate::syntax::token_wire::Edition::E2018 => 2018,
                crate::syntax::token_wire::Edition::E2021 => 2021,
                crate::syntax::token_wire::Edition::E2024 => 2024,
            };
            facts.name_local(context)?;
            editions.push(TokenContext {
                end: next_end,
                edition,
                context,
            });
            end = next_end;
        }
        if end != u32::from(self.parsed.syntax_node().text_range().len()) {
            return Err(fail());
        }
        let bytes = editions
            .capacity()
            .checked_mul(std::mem::size_of::<TokenContext>())
            .ok_or_else(fail)?;
        let fact_bytes = facts.metadata_bytes()?;
        self.estimated_bytes = self
            .estimated_bytes
            .checked_add(bytes)
            .and_then(|n| n.checked_add(fact_bytes))
            .ok_or_else(fail)?;
        self.token_contexts = Some(editions);
        self.pinned_hygiene = Some(facts);
        Ok(self)
    }
    pub(crate) fn hygiene_facts(
        &self,
    ) -> Option<&std::sync::Arc<crate::frontend::hygiene::HygieneFacts>> {
        self.pinned_hygiene.as_ref()
    }
    pub(crate) fn token_edition(&self, range: SourceRange) -> Result<u16, Diagnostic> {
        // Captured module/argument syntax may carry generated provenance while
        // retaining one declared owning edition and no mixed-token map. That
        // is enough for edition selection, never for local-name comparison.
        // same_local_name_context still requires the actual hygiene attachment.
        if self.token_contexts.is_none() {
            return Ok(self.profile.edition);
        }
        Ok(self
            .token_context(range)?
            .map_or(self.profile.edition, |context| context.edition))
    }
    pub fn key(&self) -> ParseKey {
        ParseKey {
            content: self.content,
            profile: self.profile.clone(),
        }
    }
    pub fn estimated_bytes(&self) -> usize {
        self.estimated_bytes
            + self.errors.capacity() * std::mem::size_of::<ParseDiagnostic>()
            + self
                .errors
                .iter()
                .map(|e| e.message.capacity())
                .sum::<usize>()
    }
}

pub fn parser_smoke() -> Result<(), String> {
    let blob = SourceBlob::new(b"fn live() -> i64 { 4_i64 } fn cold() { absent(); }".to_vec())
        .map_err(|e| e.to_string())?;
    let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).map_err(|e| e.to_string())?;
    if !parsed.errors.is_empty() {
        return Err(format!("parse errors: {:?}", parsed.errors));
    }
    let count = parsed
        .parsed
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .count();
    if count != 2 {
        return Err(format!("expected 2 functions, got {count}"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_counts_cold_function_without_resolving() {
        super::parser_smoke().unwrap();
    }
    #[test]
    fn edition_profile_changes_parse_identity_and_live_units_preserve_edition() {
        let blob = SourceBlob::new(b"fn r#gen() {}".to_vec()).unwrap();
        let parsed = ParsedFile::parse(&blob, ProfileIdentity::current()).unwrap();
        assert!(parsed.errors.is_empty());
        let mut old = parsed.key();
        old.profile.edition = 2021;
        assert_ne!(parsed.key().fingerprint(), old.fingerprint());
        assert_eq!(ParsedFile::parse(&blob, old.profile).unwrap().edition, 2021);
        for edition in [2015, 2018, 2021] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let bare =
                ParsedFile::parse(&SourceBlob::new(b"fn gen() {}".to_vec()).unwrap(), profile)
                    .unwrap();
            assert!(bare.errors.is_empty());
            assert_eq!(bare.edition, edition);
        }
        let bare = ParsedFile::parse(
            &SourceBlob::new(b"fn gen() {}".to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        assert!(!bare.errors.is_empty());
    }
    #[test]
    fn final_associated_function_modifier_is_not_a_declaration_error() {
        // Unchanged upstream core source using `final fn` in a trait.
        let core = include_bytes!(
            "../../../../fixtures/upstream/rustc-checkout/library/core/src/io/size_hint.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let profile = ProfileIdentity {
                edition,
                ..ProfileIdentity::current()
            };
            let parsed =
                ParsedFile::parse_dependency(&SourceBlob::new(core.to_vec()).unwrap(), profile)
                    .unwrap();
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let outline = parsed
                .outline(crate::core::digest::Digest([7; 32]), vec![], &crate::source::snapshot::CfgSet::guest_default())
                .unwrap();
            assert!(outline.items.iter().any(|item| item.def_key.declared_name == "size_hint"));
            assert!(outline.scopes.values().all(|scope| {
                scope
                    .blockers
                    .iter()
                    .all(|blocker| blocker.kind != crate::syntax::outline::BlockerKind::ParseBoundary)
            }));
        }
        // Only the exact modifier position is admitted.
        for text in [
            "final fn f() {}",
            "trait T { final }",
            "trait T { final const C: u8 = 0; }",
            "impl S { fn f() {} final }",
        ] {
            let parsed = ParsedFile::parse(
                &SourceBlob::new(text.as_bytes().to_vec()).unwrap(),
                ProfileIdentity::current(),
            )
            .unwrap();
            assert!(!parsed.errors.is_empty(), "{text}");
        }
    }
}

#[cfg(test)]
#[path = "generated_item_editions.rs"]
mod generated_item_edition_tests;

#[cfg(test)]
mod macro_facet_tests {
    use super::*;
    use crate::source::snapshot::CfgSet;
    #[test]
    fn macro_host_and_live_source_editions_remain_distinct() {
        let blob = SourceBlob::new(
            b"#[proc_macro_derive(gen,attributes(gen))] pub fn implementation(){missing_cold();}"
                .to_vec(),
        )
        .unwrap();
        let mut fingerprints = std::collections::BTreeSet::new();
        for edition in [2015, 2018, 2021] {
            let parsed = ParsedFile::parse_macro_facet(&blob, edition).unwrap();
            let catalog = parsed.source_derive_catalog(&CfgSet::default()).unwrap();
            assert_eq!(catalog.declarations().len(), 1);
            assert_eq!(catalog.declarations()[0].1.names().export(), "gen");
            assert!(
                catalog.declarations()[0]
                    .1
                    .names()
                    .helpers()
                    .contains("gen")
            );
            assert_eq!(parsed.edition, edition);
            assert!(fingerprints.insert(parsed.key().fingerprint()));
            let mut old = ProfileIdentity::current();
            old.edition = edition;
            assert_eq!(ParsedFile::parse(&blob, old).unwrap().edition, edition);
        }
        let parsed = ParsedFile::parse_macro_facet(&blob, 2024).unwrap();
        assert!(parsed.source_derive_catalog(&CfgSet::default()).is_err());
        assert!(ParsedFile::parse_macro_facet(&blob, 2099).is_err());
    }
    #[test]
    fn edition_profile_separates_original_body_pattern_temporary_and_recipe_identities() {
        use crate::{
            db::key::{OutlineKey, QueryKey},
            ir::pc::{NodeId, Phase},
            source::snapshot::CfgSet,
            syntax::body_index::BodyIndexLimits,
        };
        let blob = SourceBlob::new(b"fn case(){let flag=true;if flag{}}".to_vec()).unwrap();
        let current = ProfileIdentity::current();
        let mut older = current.clone();
        older.edition = 2021;
        let mut legacy = current.clone();
        legacy.semantics = "lrr-2024-v1".into();
        let mut snapshots = Vec::new();
        for profile in [current.clone(), older, legacy] {
            // Pure parser/cache identity witness; these recipes are not executed.
            let parsed = ParsedFile::parse_structural(&blob, profile.clone()).unwrap();
            assert!(parsed.errors.is_empty());
            let cfg = CfgSet::guest_default();
            let unit = crate::source::manifest::CrateKey {
                package: crate::source::manifest::PackageKey {
                    source_identity: "h001-fixture".into(),
                    package_name: "edition_fixture".into(),
                    version: "0.0.0".into(),
                },
                target_kind: crate::source::manifest::TargetKind::Binary,
                target_name: "case".into(),
                guest_edition: profile.edition,
                semantics_profile: profile.fingerprint(),
                cfg_fingerprint: cfg.fingerprint(),
                dependency_aliases: Digest([201; 32]),
            }
            .fingerprint();
            let outline = parsed.outline(unit, vec![], &cfg).unwrap();
            let body = outline.items[0].body.as_ref().unwrap();
            let index = parsed.index_body(body, BodyIndexLimits::default()).unwrap();
            let pc = index.pc(NodeId(0), Phase::Entry).unwrap();
            let recipe = parsed.initial_thunk(body, &index, &pc).unwrap();
            snapshots.push([
                QueryKey::ParseFile(parsed.key()).fingerprint(),
                QueryKey::Outline(OutlineKey {
                    parse: parsed.key(),
                    crate_key: unit,
                    module_path: vec![],
                    cfg: cfg.fingerprint(),
                })
                .fingerprint(),
                outline.items[0].def_key.fingerprint(),
                QueryKey::BodyStructure(body.key()).fingerprint(),
                index.fingerprint(),
                QueryKey::Thunk(pc).fingerprint(),
                recipe.recipe().fingerprint(),
            ]);
        }
        for (a, left) in snapshots.iter().enumerate() {
            for right in snapshots.iter().skip(a + 1) {
                for (family, (left, right)) in left.iter().zip(right).enumerate() {
                    assert_ne!(left, right, "identity family {family}");
                }
            }
        }
        assert_eq!(current.edition, 2024);
    }
    #[test]
    fn macro_facet_and_live_parser_preserve_each_recorded_edition() {
        let bytes = SourceBlob::new(b"fn gen(){}".to_vec()).unwrap();
        let mut identities = std::collections::BTreeSet::new();
        for edition in [2015, 2018, 2021, 2024] {
            let parsed = ParsedFile::parse_macro_facet(&bytes, edition).unwrap();
            assert_eq!(parsed.edition, edition);
            assert_eq!(parsed.errors.is_empty(), edition != 2024);
            assert!(identities.insert(parsed.key().fingerprint()));
            let live = ParsedFile::parse(&bytes, parsed.profile).unwrap();
            assert_eq!(live.edition, edition);
            assert_eq!(live.errors.is_empty(), edition != 2024);
        }
    }
}

impl ParsedFile {
    /// Structural nested function headers only; no body semantics or query.
    pub(crate) fn has_lexical_callable_syntax(
        &self,
    ) -> Result<bool, crate::core::diagnostic::Diagnostic> {
        use ra_ap_syntax::AstNode;
        let limit = || {
            crate::core::diagnostic::Diagnostic::new(
                "LRR-E-METADATA-LIMIT",
                "lexical source header presence budget".into(),
            )
        };
        let mut work = 0usize;
        for node in self
            .parsed
            .tree()
            .syntax()
            .descendants()
            .filter(|n| ra_ap_syntax::ast::Fn::can_cast(n.kind()))
        {
            let mut depth = 0usize;
            for ancestor in node.ancestors().skip(1) {
                depth = depth
                    .checked_add(1)                    .ok_or_else(limit)?;
                work = work
                    .checked_add(1)
                    .ok_or_else(limit)?;
                if ra_ap_syntax::ast::Fn::can_cast(ancestor.kind()) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}
