//! Syntax-only pinned matcher/transcriber. No body inference or live compilation.
use crate::{
    AdapterError,
    span_store::SpanStore,
    token_wire::{Edition, Key, TokenForest},
};
use serde::Serialize;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
pub enum FailureKind {
    DefinitionSyntax,
    NoMatchingRule,
    LeftoverTokens,
    Binding,
    UnresolvedBinding,
    UnexpectedToken,
    Limit,
}
#[derive(Debug, Clone, Serialize)]
pub struct MacroFailure {
    pub kind: FailureKind,
    pub span: Option<Key>,
    pub message: String,
}
impl std::fmt::Display for MacroFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for MacroFailure {}
/// Source-index-owned definition authority and stable invocation identity.
/// A generated declaration's owner is not inferred from its token locations.
#[derive(Debug, Clone, Copy)]
pub struct DeclarativeSite {
    pub expansion: Key,
    pub defining_unit: Key,
    pub definition_edition: Edition,
}
/// Retains an immutable owned definition; cached parse errors belong to this definition demand.
pub struct DeclarativeMacro {
    definition: TokenForest,
    parsed: mbe::DeclarativeMacro,
    parse_error: Option<MacroFailure>,
    spans: SpanStore,
    transparency: crate::token_wire::Transparency,
}
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct TranscriptionStats {
    pub preflight_calls: usize,
    pub materializations: usize,
    pub admitted_tokens: usize,
    pub max_preflight_tokens: usize,
    pub admitted_bytes: usize,
    pub max_preflight_bytes: usize,
    pub materialized_leaves: usize,
    pub materialized_raw_peak: usize,
}
pub struct Expansion {
    pub tokens: TokenForest,
    pub matched_arm: u32,
}
thread_local! {
    // Parsed definitions are immutable functions of their definition forests.
    // Equal forests share one parse; the forests themselves are the keys.
    static PARSED: std::cell::RefCell<
        std::collections::HashMap<u64, Vec<(Option<Option<TokenForest>>, TokenForest, std::rc::Rc<DeclarativeMacro>)>>,
    > = Default::default();
}
impl DeclarativeMacro {
    /// Shared parse of a `macro_rules` (`macro2: None`) or macro 2.0
    /// (`macro2: Some(arguments)`) definition.
    pub fn shared(
        macro2: Option<&Option<TokenForest>>,
        definition: &TokenForest,
    ) -> Result<std::rc::Rc<Self>, AdapterError> {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        macro2.hash(&mut hasher);
        definition.hash(&mut hasher);
        let hash = hasher.finish();
        let found = PARSED.with(|cache| {
            cache.borrow().get(&hash).and_then(|rows| {
                rows.iter()
                    .find(|(arguments, body, _)| arguments.as_ref() == macro2 && body == definition)
                    .map(|(_, _, parsed)| parsed.clone())
            })
        });
        if let Some(parsed) = found {
            return Ok(parsed);
        }
        let parsed = std::rc::Rc::new(match macro2 {
            None => Self::new(definition.clone())?,
            Some(arguments) => Self::new_macro2(arguments.clone(), definition.clone())?,
        });
        PARSED.with(|cache| {
            cache.borrow_mut().entry(hash).or_default().push((
                macro2.cloned(),
                definition.clone(),
                parsed.clone(),
            ))
        });
        Ok(parsed)
    }
    pub fn new(definition: TokenForest) -> Result<Self, AdapterError> {
        definition.validate()?;
        let mut spans = SpanStore::default();
        let tree = spans.import(&definition)?;
        let parsed = mbe::DeclarativeMacro::parse_macro_rules_stable(&tree, |context| {
            context.edition(spans.database())
        });
        let parse_span = spans.key(tree.top_subtree().delimiter.open)?;
        let parse_error = parsed.err().map(|error| MacroFailure {
            kind: FailureKind::DefinitionSyntax,
            span: Some(parse_span),
            message: error.to_string(),
        });
        Ok(Self {
            definition,
            parsed,
            parse_error,
            spans,
            transparency: crate::token_wire::Transparency::SemiTransparent,
        })
    }
    /// Component parser for actual macro2 argument/body groups. This grants no
    /// source or runtime authority; original source producers select both groups.
    pub fn new_macro2(
        arguments: Option<TokenForest>,
        definition: TokenForest,
    ) -> Result<Self, AdapterError> {
        definition.validate()?;
        let body_bytes = definition.encoded_len()?;
        let argument_bytes = arguments
            .as_ref()
            .map(TokenForest::encoded_len)
            .transpose()?
            .unwrap_or(0);
        body_bytes
            .checked_add(argument_bytes)
            .ok_or_else(|| AdapterError {
                code: "LRR-E-MACRO-OUTPUT",
                message: "macro2 definition groups exceed metadata limit".into(),
            })?;
        let mut spans = SpanStore::default();
        let arguments = arguments
            .as_ref()
            .map(|args| {
                args.validate()?;
                spans.import(args)
            })
            .transpose()?;
        let tree = spans.import(&definition)?;
        let parsed = mbe::DeclarativeMacro::parse_macro2(arguments.as_ref(), &tree, |context| {
            context.edition(spans.database())
        });
        let parse_span = spans.key(tree.top_subtree().delimiter.open)?;
        let parse_error = parsed.err().map(|error| MacroFailure {
            kind: FailureKind::DefinitionSyntax,
            span: Some(parse_span),
            message: error.to_string(),
        });
        Ok(Self {
            definition,
            parsed,
            parse_error,
            spans,
            transparency: crate::token_wire::Transparency::Opaque,
        })
    }
    pub fn definition(&self) -> &TokenForest {
        &self.definition
    }
    pub fn expand(
        &self,
        input: &TokenForest,
        call_site: Key,
    ) -> Result<Result<Expansion, MacroFailure>, AdapterError> {
        self.expand_with_token_limit(input, call_site, usize::MAX)
            .map(|(result, _)| result)
    }
    /// Count the same pinned transcription before creating its output token buffer.
    pub fn expand_with_token_limit(
        &self,
        input: &TokenForest,
        call_site: Key,
        token_limit: usize,
    ) -> Result<(Result<Expansion, MacroFailure>, TranscriptionStats), AdapterError> {
        self.expand_with_limits(input, call_site, token_limit, usize::MAX)
    }
    /// Exact schema2 byte and token admission before output materialization.
    pub fn expand_with_limits(
        &self,
        input: &TokenForest,
        call_site: Key,
        token_limit: usize,
        byte_limit: usize,
    ) -> Result<(Result<Expansion, MacroFailure>, TranscriptionStats), AdapterError> {
        self.expand_impl(input, call_site, None, None, token_limit, byte_limit)
    }
    /// Hygienic runtime entry: the source index supplies the exact expansion
    /// identity and definition owner. Legacy matcher fixtures use the unmarked
    /// component entry above; they do not prove runtime name resolution.
    pub fn expand_hygienic_with_limits(
        &self,
        input: &TokenForest,
        call_site: Key,
        site: DeclarativeSite,
        token_limit: usize,
        byte_limit: usize,
    ) -> Result<(Result<Expansion, MacroFailure>, TranscriptionStats), AdapterError> {
        self.expand_impl(input, call_site, Some(site), None, token_limit, byte_limit)
    }
    /// Hygienic entry for a definition carrying `#[rustc_macro_transparency]`:
    /// the attribute replaces the dialect's default transparency
    /// (`macro_rules` semi-opaque, `macro` opaque), as rustc's
    /// `Transparency::fallback` is overridden by the attribute.
    pub fn expand_hygienic_with_transparency(
        &self,
        input: &TokenForest,
        call_site: Key,
        site: DeclarativeSite,
        transparency: Option<crate::token_wire::Transparency>,
        token_limit: usize,
        byte_limit: usize,
    ) -> Result<(Result<Expansion, MacroFailure>, TranscriptionStats), AdapterError> {
        self.expand_impl(input, call_site, Some(site), transparency, token_limit, byte_limit)
    }
    fn expand_impl(
        &self,
        input: &TokenForest,
        call_site: Key,
        site: Option<DeclarativeSite>,
        transparency: Option<crate::token_wire::Transparency>,
        token_limit: usize,
        byte_limit: usize,
    ) -> Result<(Result<Expansion, MacroFailure>, TranscriptionStats), AdapterError> {
        if token_limit == 0 || byte_limit == 0 {
            return Err(AdapterError {
                code: "LRR-E-MACRO-OUTPUT",
                message: "output limits must be positive reductions of the fixed defaults".into(),
            });
        }
        if let Some(error) = &self.parse_error {
            return Ok((Err(error.clone()), TranscriptionStats::default()));
        }
        let mut spans = self.spans.clone();
        let input = spans.import(input)?;
        let marked = site
            .map(|site| {
                spans.prepare_marks(
                    &self.definition,
                    call_site,
                    site,
                    transparency.unwrap_or(self.transparency),
                )
            })
            .transpose()?;
        let call_site = spans.span(call_site)?;
        let wire_error = std::cell::RefCell::new(None);
        let marker = |span: &mut span::Span| {
            if let Some(marked) = &marked {
                if let Some(preview) = marked.get(span) {
                    *span = *preview;
                } else if wire_error.borrow().is_none() {
                    *wire_error.borrow_mut() = Some(AdapterError {
                        code: "LRR-E-MACRO-TOKEN-WIRE",
                        message: "unregistered template span for hygiene preview".into(),
                    });
                }
            }
        };
        let span_cost = |span| match spans.wire_cost(span) {
            Ok(cost) => Some(cost),
            Err(error) => {
                if wire_error.borrow().is_none() {
                    *wire_error.borrow_mut() = Some(error);
                }
                None
            }
        };
        let context_parent = |key| match spans.context_parent(key) {
            Ok(parent) => Some(parent),
            Err(error) => {
                if wire_error.borrow().is_none() {
                    *wire_error.borrow_mut() = Some(error);
                }
                None
            }
        };
        let budget = mbe::ByteBudget {
            byte_limit,
            span_cost: &span_cost,
            context_parent: &context_parent,
            preview_marker: &marker,
        };
        let (result, stats) = self.parsed.expand_with_limits(
            spans.database(),
            &input,
            marker,
            mbe::MacroCallStyle::FnLike,
            call_site,
            token_limit,
            &budget,
        );
        if let Some(error) = wire_error.into_inner() {
            return Err(error);
        }
        let stats = TranscriptionStats {
            preflight_calls: stats.preflight_calls,
            materializations: stats.materializations,
            admitted_tokens: stats.admitted_tokens,
            max_preflight_tokens: stats.max_preflight_tokens,
            admitted_bytes: stats.admitted_bytes,
            max_preflight_bytes: stats.max_preflight_bytes,
            materialized_leaves: stats.materialized_leaves,
            materialized_raw_peak: stats.materialized_raw_peak,
        };
        if let Some(error) = result.err {
            let kind = match &error.inner.1 {
                mbe::ExpandErrorKind::NoMatchingRule => FailureKind::NoMatchingRule,
                mbe::ExpandErrorKind::LeftoverTokens => FailureKind::LeftoverTokens,
                mbe::ExpandErrorKind::BindingError(_) => FailureKind::Binding,
                mbe::ExpandErrorKind::UnresolvedBinding(_) => FailureKind::UnresolvedBinding,
                mbe::ExpandErrorKind::UnexpectedToken => FailureKind::UnexpectedToken,
                mbe::ExpandErrorKind::LimitExceeded => FailureKind::Limit,
            };
            return Ok((
                Err(MacroFailure {
                    kind,
                    span: Some(spans.key(error.inner.0)?),
                    message: error.to_string(),
                }),
                stats,
            ));
        }
        let (output, arm) = result.value;
        let matched_arm = arm.ok_or_else(|| AdapterError {
            code: "LRR-E-MACRO-TOKEN-WIRE",
            message: "pinned successful expansion omitted matched arm".into(),
        })?;
        let tokens = spans.export(&output)?;
        if tokens.encoded_len()? != stats.admitted_bytes {
            return Err(AdapterError {
                code: "LRR-E-MACRO-TOKEN-WIRE",
                message: "preflight byte count differs from owned output codec".into(),
            });
        }
        Ok((
            Ok(Expansion {
                tokens,
                matched_arm,
            }),
            stats,
        ))
    }
}
