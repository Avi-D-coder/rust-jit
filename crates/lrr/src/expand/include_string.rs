//! Pure syntax/data include output over adopted source records; no filesystem authority.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    db::{
        dependency::{InputKey, InputValue},
        memo::Database,
        poll::InputRequest,
    },
    expand::{
        compile_input_identity::CompileInputKind,
        identity::{ExpansionKey, GrammarSlot, MacroIdentity},
        include_arguments::{IncludeArguments, IncludePathPoll},
        record::ExpansionRecord,
        scopes::MacroDefinition,
    },
    frontend::compile_input_catalog::CompileInputCatalog,
    source::include_paths::{IncludeAuthorities, IncludeKind, IncludeLocation},
    syntax::token_wire::{Key, TokenForest},
};
pub struct IncludeDataExpansion<'a> {
    pub catalog: &'a CompileInputCatalog,
    pub selected: &'a MacroDefinition,
    pub key: &'a ExpansionKey,
    pub original: &'a TokenForest,
    pub site: Key,
    pub authorities: &'a IncludeAuthorities,
    pub from: &'a IncludeLocation,
    pub max_wire_bytes: usize,
}
pub enum IncludeDataPoll {
    Ready(Result<ExpansionRecord, Diagnostic>),
    NeedInput(InputRequest),
    NeedExpansion { first_span: Key, last_span: Key },
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
impl IncludeDataExpansion<'_> {
    /// The caller's query collector observes the exact include input dependency.
    pub fn poll(&self, db: &mut Database) -> IncludeDataPoll {
        match self.work(db, None, None) {
            Ok(poll) => poll,
            Err(error) => IncludeDataPoll::Ready(Err(error)),
        }
    }
    /// Resolved paths must still refer to the original invocation and adopted producers.
    pub fn poll_resolved(
        &self,
        db: &mut Database,
        arguments: &IncludeArguments,
    ) -> IncludeDataPoll {
        match self.work(db, Some(arguments), None) {
            Ok(poll) => poll,
            Err(error) => IncludeDataPoll::Ready(Err(error)),
        }
    }
    pub fn poll_included(
        &self,
        db: &mut Database,
        arguments: &IncludeArguments,
        view: &crate::source::include_view::IncludedSourceView,
    ) -> IncludeDataPoll {
        match self.work(db, Some(arguments), Some(view)) {
            Ok(poll) => poll,
            Err(error) => IncludeDataPoll::Ready(Err(error)),
        }
    }
    fn syntax_error_origin(
        &self,
        mut error: Diagnostic,
        included: Option<&crate::source::include_view::IncludedSourceView>,
    ) -> Diagnostic {
        // Called only after adopted capture, definition, input and route validation.
        // Failure keeps its primary code/message and cannot adopt an expansion.
        match crate::source::include_view::IncludedSourceView::capture_diagnostic_origin(
            self.key,
            self.original,
            self.site,
            included,
        ) {
            Ok(origin) => match std::sync::Arc::into_inner(origin) {
                Some(origin) => error.macro_origin = Some(Box::new(origin)),
                None => error.origin_attachment_failure = Some("LRR-E-INTERNAL-SOURCE-MAP"),
            },
            Err(attachment) => error.origin_attachment_failure = Some(attachment.code),
        }
        error
    }
    fn work(
        &self,
        db: &mut Database,
        resolved: Option<&IncludeArguments>,
        included: Option<&crate::source::include_view::IncludedSourceView>,
    ) -> Result<IncludeDataPoll, Diagnostic> {
        self.key.validate()?;
        if let Some(view) = included {
            if view.location() != self.from || !view.matches_authorities(self.authorities) {
                return Err(fault("include source base view mismatch"));
            }
            match view.validate(db) {
                crate::db::poll::QueryPoll::Ready(result) => result?,
                crate::db::poll::QueryPoll::NeedInput(input) => {
                    return Ok(IncludeDataPoll::NeedInput(input));
                }
            }
        }

        if self.max_wire_bytes == 0 {
            return Err(fault("include string output limits are invalid"));
        }
        let descriptor = self
            .catalog
            .select(self.selected)?
            .ok_or_else(|| fault("include string requires a compiler-facet definition"))?;
        let wire = self.original.encode().map_err(|e| fault(&e.to_string()))?;
        if !matches!(
            descriptor.kind(),
            CompileInputKind::Include
                | CompileInputKind::IncludeStr
                | CompileInputKind::IncludeBytes
        ) || self.key.macro_identity
            != (MacroIdentity::Builtin {
                definition: self.selected.identity,
            })
            || self.key.definition_edition != self.selected.definition_edition
            || (descriptor.kind() != CompileInputKind::Include
                && self.key.grammar != GrammarSlot::Expression)
            || self.key.input_tokens != Digest::of_bytes(&wire)
        {
            return Err(fault("include string definition/input/grammar mismatch"));
        }
        if !self.original.spans().iter().any(|s| s.key() == self.site) {
            return Err(fault("include string site is absent from original input"));
        }
        let parsed;
        let arguments = match resolved {
            Some(arguments) => {
                match arguments.validate_resolution(
                    db,
                    descriptor,
                    self.original,
                    self.key.consumer_compile_env,
                ) {
                    crate::db::poll::QueryPoll::Ready(Ok(())) => {}
                    crate::db::poll::QueryPoll::Ready(Err(error)) => return Err(error),
                    crate::db::poll::QueryPoll::NeedInput(input) => {
                        return Ok(IncludeDataPoll::NeedInput(input));
                    }
                }
                arguments
            }
            None => {
                parsed = IncludeArguments::parse(descriptor, self.original)?;
                &parsed
            }
        };
        let routed = match included {
            Some(view) => {
                arguments.route_from_included(descriptor, self.authorities, view, self.site)
            }
            None => arguments.route(descriptor, self.authorities, self.from, self.site),
        }?;
        let request = match routed {
            IncludePathPoll::NeedExpansion {
                first_span,
                last_span,
            } => {
                return Ok(IncludeDataPoll::NeedExpansion {
                    first_span,
                    last_span,
                });
            }
            IncludePathPoll::Capture(request) => request,
        };
        let input = InputRequest::CaptureInclude {
            request,
            compile_environment: self.key.consumer_compile_env,
        };
        let Some(record) = db.input(&InputKey::IncludeRecord(input.fingerprint())) else {
            return Ok(IncludeDataPoll::NeedInput(input));
        };
        let InputValue::IncludeRecord {
            input: identity,
            captured,
        } = record.as_ref()
        else {
            return Err(fault("include string capture input variant mismatch"));
        };
        let InputRequest::CaptureInclude { request, .. } = &input else {
            return Err(fault("include string capture request variant mismatch"));
        };
        if *identity != input.fingerprint()
            || captured.request() != request.identity()
            || captured.kind()
                != match descriptor.kind() {
                    CompileInputKind::Include => IncludeKind::Syntax,
                    CompileInputKind::IncludeStr => IncludeKind::Utf8,
                    CompileInputKind::IncludeBytes => IncludeKind::Bytes,
                    _ => return Err(fault("include output kind is unsupported")),
                }
        {
            return Err(fault("include string capture identity/kind mismatch"));
        }
        let original = lrr_expand_adapter::token_wire::TokenForest::decode(&wire)
            .map_err(|e| fault(&e.to_string()))?;
        let site = lrr_expand_adapter::token_wire::Key(self.site.0);
        let output = match descriptor.kind() {
            CompileInputKind::Include => {
                let grammar = match self.key.grammar {
                    GrammarSlot::Items => lrr_expand_adapter::grammar::Grammar::Items,
                    GrammarSlot::Expression => lrr_expand_adapter::grammar::Grammar::Expression,
                    GrammarSlot::Pattern => lrr_expand_adapter::grammar::Grammar::Pattern,
                    GrammarSlot::Type => lrr_expand_adapter::grammar::Grammar::Type,
                    GrammarSlot::TraitItems => lrr_expand_adapter::grammar::Grammar::TraitItems,
                    GrammarSlot::ImplItems => lrr_expand_adapter::grammar::Grammar::ImplItems,
                    // rustc's include MacResult uses the default make_stmts,
                    // which first parses one expression and then wraps that expression.
                    // The record keeps its statement slot; the included file still
                    // must contain exactly the expression that supplies that slot.
                    GrammarSlot::Statements => lrr_expand_adapter::grammar::Grammar::Expression,
                };
                let text = captured.blob().text()?;
                let output = lrr_expand_adapter::token_wire::parse_source_in_context(
                    text,
                    lrr_expand_adapter::token_wire::SourceIdentity {
                        unit: lrr_expand_adapter::token_wire::Key(
                            captured.virtual_location().unit.0,
                        ),
                        content: lrr_expand_adapter::token_wire::Key(captured.blob().content().0),
                        path: captured.virtual_location().path.as_str().into(),
                    },
                    &original,
                    site,
                    lrr_expand_adapter::token_wire::Key(captured.identity().0),
                )
                .map_err(|e| {
                    self.syntax_error_origin(
                        Diagnostic::new(
                            e.code,
                            format!(
                                "included syntax at {}: {}; caller origin={}; include origin={}",
                                captured
                                    .location()
                                    .path
                                    .as_str()
                                    .chars()
                                    .take(128)
                                    .collect::<String>(),
                                e.message.chars().take(256).collect::<String>(),
                                Digest(request.from().origin.0).hex(),
                                Digest(request.include_origin().0).hex()
                            ),
                        ),
                        included,
                    )
                })?;
                lrr_expand_adapter::grammar::validate(&output, grammar).map_err(|e| {
                    self.syntax_error_origin(
                        Diagnostic::new(
                            "LRR-E-INCLUDE-PARSE",
                            format!(
                                "included syntax at {}: {}; caller origin={}; include origin={}",
                                captured
                                    .location()
                                    .path
                                    .as_str()
                                    .chars()
                                    .take(128)
                                    .collect::<String>(),
                                e.message.chars().take(256).collect::<String>(),
                                Digest(request.from().origin.0).hex(),
                                Digest(request.include_origin().0).hex()
                            ),
                        ),
                        included,
                    )
                })?;
                Ok(output)
            }

            CompileInputKind::IncludeStr => {
                let value = std::str::from_utf8(captured.blob().bytes())
                    .map_err(|_| fault("adopted include string is not UTF8"))?;
                lrr_expand_adapter::string_output::emit(&original, site, value, self.max_wire_bytes)
            }
            CompileInputKind::IncludeBytes => lrr_expand_adapter::bytes_output::emit(
                &original,
                site,
                captured.blob().bytes(),
                self.max_wire_bytes,
            ),
            _ => return Err(fault("include output kind is unsupported")),
        }
        .map_err(|e| Diagnostic::new(e.code, e.message))?;
        let output = output.encode().map_err(|e| fault(&e.to_string()))?;
        if output.len() > self.max_wire_bytes {
            return Err(fault("include output wire limit"));
        }
        Ok(IncludeDataPoll::Ready(ExpansionRecord::from_wire(
            self.key.clone(),
            output,
        )))
    }
}

// Existing string callers share the same checked capture/emission path.
pub use IncludeDataExpansion as IncludeStringExpansion;
pub use IncludeDataPoll as IncludeStringPoll;

/// Syntax and data includes share exact capture/adoption transport.
pub use IncludeDataExpansion as IncludeExpansion;
pub use IncludeDataPoll as IncludeExpansionPoll;

impl IncludeExpansion<'_> {
    /// Certify an adopted syntax expansion against its exact adopted capture and pure emitter.
    /// Generated namespace/discovery integration consumes this view later.
    pub fn adopted_source_view(
        &self,
        db: &mut Database,
        capture: &InputRequest,
        resolved: Option<&IncludeArguments>,
    ) -> crate::db::poll::QueryPoll<crate::source::include_view::IncludedSourceView> {
        self.source_view_with_base(db, capture, resolved, None)
    }
    pub fn adopted_source_view_included(
        &self,
        db: &mut Database,
        capture: &InputRequest,
        resolved: Option<&IncludeArguments>,
        view: &crate::source::include_view::IncludedSourceView,
    ) -> crate::db::poll::QueryPoll<crate::source::include_view::IncludedSourceView> {
        self.source_view_with_base(db, capture, resolved, Some(view))
    }
    fn source_view_with_base(
        &self,
        db: &mut Database,
        capture: &InputRequest,
        resolved: Option<&IncludeArguments>,
        included: Option<&crate::source::include_view::IncludedSourceView>,
    ) -> crate::db::poll::QueryPoll<crate::source::include_view::IncludedSourceView> {
        use crate::{
            db::poll::{ExpansionRequest, QueryPoll},
            source::include_view::{IncludedSourceView, ViewDependency},
        };
        let mut work = || -> Result<QueryPoll<IncludedSourceView>, Diagnostic> {
            capture.validate()?;
            let descriptor = self
                .catalog
                .select(self.selected)?
                .ok_or_else(|| fault("included source view requires a compiler definition"))?;
            let InputRequest::CaptureInclude {
                request,
                compile_environment,
            } = capture
            else {
                return Err(fault(
                    "included source view requires an exact capture request",
                ));
            };
            if descriptor.kind() != CompileInputKind::Include
                || request.kind() != IncludeKind::Syntax
                || *compile_environment != self.key.consumer_compile_env
                || !self.authorities.matches_request(request)
            {
                return Err(fault(
                    "included source view capture/definition/context mismatch",
                ));
            }
            let expansion_key = InputKey::ExpansionRecord(self.key.fingerprint());
            let Some(expansion) = db.input(&expansion_key) else {
                return Ok(QueryPoll::NeedInput(InputRequest::Expand(Box::new(
                    ExpansionRequest {
                        key: self.key.clone(),
                    },
                ))));
            };
            let InputValue::ExpansionRecord(adopted) = expansion.as_ref() else {
                return Err(fault("included source view expansion variant mismatch"));
            };
            let generated = self.work(db, resolved, included)?;
            let fresh = match generated {
                IncludeExpansionPoll::Ready(result) => result?,
                IncludeExpansionPoll::NeedInput(input) => return Ok(QueryPoll::NeedInput(input)),
                IncludeExpansionPoll::NeedExpansion { .. } => {
                    return Err(fault("included source view requires resolved path proof"));
                }
            };
            if !adopted.same_record(&fresh) {
                return Err(fault(
                    "included source view output does not match captured source",
                ));
            }
            let source_key = InputKey::IncludeRecord(capture.fingerprint());
            let Some(source) = db.input(&source_key) else {
                return Ok(QueryPoll::NeedInput(capture.clone()));
            };
            let InputValue::IncludeRecord { input, captured } = source.as_ref() else {
                return Err(fault("included source view capture variant mismatch"));
            };
            if *input != capture.fingerprint()
                || captured.request() != request.identity()
                || captured.kind() != IncludeKind::Syntax
            {
                return Err(fault("included source view capture identity mismatch"));
            }
            let tokens = adopted.tokens()?;
            if tokens.spans().is_empty()
                || tokens.spans().iter().any(|span| {
                    span.source.unit.0 != captured.virtual_location().unit.0
                        || span.source.content.0 != captured.blob().content().0
                        || span.source.path != captured.virtual_location().path.as_str()
                        || span.origin.0 != captured.identity().0
                })
            {
                return Err(fault("included source view token provenance mismatch"));
            }
            let mut dependencies = vec![
                ViewDependency {
                    key: expansion_key,
                    fingerprint: expansion.fingerprint(),
                    request: InputRequest::Expand(Box::new(ExpansionRequest {
                        key: self.key.clone(),
                    })),
                },
                ViewDependency {
                    key: source_key,
                    fingerprint: source.fingerprint(),
                    request: capture.clone(),
                },
            ];
            if let Some(resolved) = resolved {
                for key in resolved.producer_keys() {
                    let input_key = InputKey::ExpansionRecord(key.fingerprint());
                    let Some(input) = db.input(&input_key) else {
                        return Ok(QueryPoll::NeedInput(InputRequest::Expand(Box::new(
                            ExpansionRequest { key: key.clone() },
                        ))));
                    };
                    dependencies.push(ViewDependency {
                        key: input_key,
                        fingerprint: input.fingerprint(),
                        request: InputRequest::Expand(Box::new(ExpansionRequest {
                            key: key.clone(),
                        })),
                    });
                }
            }
            if let Some(view) = included {
                dependencies.extend_from_slice(view.dependencies());
            }
            Ok(QueryPoll::Ready(IncludedSourceView::new(
                self.key.clone(),
                tokens,
                source,
                expansion,
                dependencies,
                IncludedSourceView::capture_diagnostic_origin(
                    self.key,
                    self.original,
                    self.site,
                    included,
                )?,
            )))
        };
        match work() {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
