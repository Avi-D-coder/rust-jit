//! Canonical declaration facts of a genuine lexical owner. Binding preparation
//! is deferred until entry; this plan is not an executable-body or impl seal.
use super::{
    ast_adapter::TokenRecipe,
    entered_macros::{DeclarationFacts, EnteredMacroFacts},
    local_bindings::{BindingBudget, BindingForm},
    outline::{BodyHandle, ScopeKey},
    parse::ParseKey,
};
use crate::core::{
    diagnostic::Diagnostic,
    digest::{Digest, StableEncoder},
};
use ra_ap_syntax::ast::{self, HasLoopBody};
use std::sync::Arc;

fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}

enum BindingRequest {
    Receiver(ast::SelfParam),
    Pattern {
        pattern: Option<ast::Pat>,
        scope: u32,
        visible_from: u64,
        form: BindingForm,
        required: Option<&'static str>,
    },
    For(ast::ForExpr),
    MatchArm(ast::MatchArm),
}

/// Ordered requests captured by the single declaration/scope traversal. AST
/// handles share the real immutable syntax tree; no pattern plan is built.
#[derive(Default)]
pub(super) struct DeferredBindings {
    requests: Vec<BindingRequest>,
}
impl DeferredBindings {
    fn push(&mut self, request: BindingRequest) -> Result<(), Diagnostic> {
        self.requests
            .try_reserve(1)
            .map_err(|_| crate::runtime::arena::resource())?;
        self.requests.push(request);
        Ok(())
    }
    pub(super) fn receiver(&mut self, receiver: ast::SelfParam) -> Result<(), Diagnostic> {
        self.push(BindingRequest::Receiver(receiver))
    }
    pub(super) fn pattern(
        &mut self,
        pattern: Option<ast::Pat>,
        scope: u32,
        visible_from: u64,
        form: BindingForm,
        required: Option<&'static str>,
    ) -> Result<(), Diagnostic> {
        self.push(BindingRequest::Pattern {
            pattern,
            scope,
            visible_from,
            form,
            required,
        })
    }
    pub(super) fn for_expression(&mut self, expression: ast::ForExpr) -> Result<(), Diagnostic> {
        self.push(BindingRequest::For(expression))
    }
    pub(super) fn match_arm(&mut self, arm: ast::MatchArm) -> Result<(), Diagnostic> {
        self.push(BindingRequest::MatchArm(arm))
    }
    fn apply(
        &self,
        body: &BodyHandle,
    ) -> Result<Vec<super::local_bindings::LocalBindingRecipe>, Diagnostic> {
        let mut budget = BindingBudget::default();
        let mut bindings = Vec::new();
        for request in &self.requests {
            match request {
                BindingRequest::Receiver(receiver) => {
                    budget.receiver(receiver, body, &mut bindings)?
                }
                BindingRequest::Pattern {
                    pattern,
                    scope,
                    visible_from,
                    form,
                    required,
                } => {
                    if let Some(pattern) = pattern {
                        budget.collect(
                            pattern,
                            body,
                            *scope,
                            *visible_from,
                            *form,
                            &mut bindings,
                        )?;
                    } else if let Some(message) = required {
                        return Err(error(message));
                    }
                }
                BindingRequest::For(expression) => {
                    if expression.iterable().is_none() || expression.loop_body().is_none() {
                        return Err(error("for iterator/body boundary missing"));
                    }
                }
                BindingRequest::MatchArm(arm) => {
                    if arm.pat().is_none()
                        || arm.expr().is_none()
                        || arm.fat_arrow_token().is_none()
                    {
                        return Err(error("match pattern/body boundary missing"));
                    }
                }
            }
        }
        Ok(bindings)
    }
}

/// Pure syntax metadata, owning no runtime state. Construction is confined to
/// the existing canonical lexical collector; callers cannot supply rows.
pub struct DeclarationScopePlan {
    parse: ParseKey,
    body: BodyHandle,
    scope: ScopeKey,
    cfg: Digest,
    facts: Arc<DeclarationFacts>,
    deferred: DeferredBindings,
    fingerprint: Digest,
}
impl std::fmt::Debug for DeclarationScopePlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeclarationScopePlan")
            .field("parse", &self.parse)
            .field("body", &self.body)
            .field("scope", &self.scope)
            .field("cfg", &self.cfg)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}
impl DeclarationScopePlan {
    pub(super) fn new(
        parse: ParseKey,
        body: BodyHandle,
        cfg: Digest,
        scope: ScopeKey,
        header_context: &[TokenRecipe],
        facts: DeclarationFacts,
        deferred: DeferredBindings,
    ) -> Result<Self, Diagnostic> {
        // Everything captured below is a deterministic function of this exact
        // parser identity, owner, cfg and containing/header context. Retained
        // parser keys include actual generated syntax identity and editions.
        let mut key = StableEncoder::new("lrr/v1/canonical-declaration-scope-plan");
        key.record(&parse);
        key.record(&body.key());
        key.u32(body.range.start);
        key.u32(body.range.end);
        key.u32(body.block_offset);
        key.bool(body.entry_blocked);
        key.record(&scope);
        key.digest(cfg);
        key.sequence(header_context, |key, token| key.record(token));
        let fingerprint = key.finish();
        Ok(Self {
            parse,
            body,
            scope,
            cfg,
            facts: Arc::new(facts),
            deferred,
            fingerprint,
        })
    }
    pub(crate) fn parse(&self) -> &ParseKey {
        &self.parse
    }
    pub(crate) fn body(&self) -> &BodyHandle {
        &self.body
    }
    pub(crate) fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub(crate) fn cfg(&self) -> Digest {
        self.cfg
    }
    /// This borrowed metadata contains no prepared bindings. It grants no
    /// body entry or negative trait/applicability conclusion.
    pub(crate) fn declarations(&self) -> &DeclarationFacts {
        &self.facts
    }
    pub(crate) fn declaration_facts(&self) -> &Arc<DeclarationFacts> {
        &self.facts
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn prepare_entered(&self) -> Result<EnteredMacroFacts, Diagnostic> {
        self.body.require_entry()?;
        let bindings = self.deferred.apply(&self.body)?;
        Ok(EnteredMacroFacts::from_declarations(
            bindings,
            Arc::clone(&self.facts),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{digest::StableEncode, profile::ProfileIdentity},
        source::snapshot::{CfgSet, SourceBlob},
        syntax::parse::ParsedFile,
    };

    #[test]
    fn unchanged_local_drop_headers_keep_canonical_identity_without_binding_preparation() {
        const ORIGINAL: &str = include_str!(
            "../../../../fixtures/upstream/rustc-checkout/tests/ui/zero-sized/zero-size-type-destructors.rs"
        );
        for edition in [2015, 2018, 2021, 2024] {
            let cfg = CfgSet::guest_default();
            let parsed = ParsedFile::parse(
                &SourceBlob::new(ORIGINAL.as_bytes().to_vec()).unwrap(),
                ProfileIdentity {
                    edition,
                    ..ProfileIdentity::current()
                },
            )
            .unwrap();
            let outline = parsed.outline(Digest([187; 32]), vec![], &cfg).unwrap();
            let function = outline
                .items
                .iter()
                .find(|item| item.def_key.declared_name == "foo")
                .unwrap();
            let body = function.body.as_ref().unwrap();
            let cold = parsed
                .declaration_scope_plan(body, &cfg, &function.scope)
                .unwrap();
            assert_eq!(cold.parse(), &parsed.key());
            assert_eq!(cold.body(), body);
            assert_eq!(cold.scope(), &function.scope);
            assert_eq!(cold.cfg(), cfg.fingerprint());
            assert!(!cold.deferred.requests.is_empty());
            let definitions: Vec<_> = cold
                .declarations()
                .declarations()
                .map(|row| {
                    (
                        row.item.def_key.fingerprint(),
                        row.item.header.fingerprint(),
                        row.item.body.clone(),
                    )
                })
                .collect();
            assert_eq!(cold.declarations().items.len(), 2);
            assert_eq!(cold.declarations().members.len(), 1);
            let impl_ = cold
                .declarations()
                .items
                .iter()
                .find(|row| row.item.def_key.item_kind == crate::source::manifest::ItemKind::Impl)
                .unwrap();
            let drop = &cold.declarations().members[0].item;
            assert_eq!(drop.def_key.declared_name, "drop");
            assert_eq!(drop.scope.owner, impl_.item.def_key.enclosing_impl);
            assert_eq!(
                drop.body.as_ref().unwrap().def_key,
                drop.def_key.fingerprint()
            );
            let repeated = parsed
                .declaration_scope_plan(body, &cfg, &function.scope)
                .unwrap();
            assert_eq!(cold.fingerprint(), repeated.fingerprint());
            let entered = cold.prepare_entered().unwrap();
            let entered_again = cold.prepare_entered().unwrap();
            assert!(Arc::ptr_eq(
                cold.declaration_facts(),
                entered.declaration_facts()
            ));
            assert!(Arc::ptr_eq(
                entered.declaration_facts(),
                entered_again.declaration_facts()
            ));
            assert_ne!(entered.bindings.as_ptr(), entered_again.bindings.as_ptr());
            assert_eq!(
                entered
                    .declarations()
                    .map(|row| {
                        (
                            row.item.def_key.fingerprint(),
                            row.item.header.fingerprint(),
                            row.item.body.clone(),
                        )
                    })
                    .collect::<Vec<_>>(),
                definitions
            );
            assert_eq!(
                entered
                    .bindings
                    .iter()
                    .map(|row| row.name.as_str())
                    .collect::<Vec<_>>(),
                ["_x"]
            );
            assert_eq!(entered.bindings[0].scope, 0);
            assert_eq!(cold.fingerprint(), repeated.fingerprint());
            assert_eq!(cold.declarations().items.len(), 2);
        }
    }
}
