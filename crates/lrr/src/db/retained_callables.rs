//! Reached original-member projection from current retained source inputs.
//! This selects no name and publishes no source, declaration or executable code.
use super::*;
use crate::syntax::{
    outline::{BodyHandle, HeaderRecipe},
    parse::ParseKey,
};

/// Retains the actual input allocation, including its original source owner.
/// Existing input pin accounting charges this allocation after replacement.
#[derive(Clone, Debug)]
pub(crate) struct RetainedSourceLease(Arc<InputValue>);
impl RetainedSourceLease {
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        let InputValue::RetainedSyntax(input) = self.0.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "retained callable lease variant differs",
            )));
        };
        if db
            .input(&InputKey::RetainedSyntax(input.key().content))
            .is_none_or(|actual| actual.fingerprint() != self.0.fingerprint())
        {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "retained callable input was replaced",
            )));
        }
        input.require_current(db)
    }
}

pub(crate) struct RetainedOriginalDeclaration {
    parse: ParseKey,
    header: HeaderRecipe,
    body: BodyHandle,
    context: DeclarationContext,
    lease: RetainedSourceLease,
}
impl RetainedOriginalDeclaration {
    pub(crate) fn parse(&self) -> &ParseKey {
        &self.parse
    }
    pub(crate) fn header(&self) -> &HeaderRecipe {
        &self.header
    }
    pub(crate) fn body(&self) -> &BodyHandle {
        &self.body
    }
    pub(crate) fn context(&self) -> &DeclarationContext {
        &self.context
    }
    pub(crate) fn into_lease(self) -> RetainedSourceLease {
        self.lease
    }
}
impl Database {
    /// A macro-generated function whose locator names a current retained
    /// admission of it: project its retained rows exactly like an original
    /// member. None while the locator still names the generated unit itself.
    pub(crate) fn retained_generated_declaration(
        &mut self,
        definition: Digest,
    ) -> QueryPoll<Option<RetainedOriginalDeclaration>> {
        let Some(InputValue::DefinitionSource(parse)) = self
            .input(&InputKey::DefinitionSource(definition))
            .as_deref()
            .cloned()
        else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "generated callable locator absent",
            )));
        };
        let Some(input) = self.input(&InputKey::RetainedSyntax(parse.content)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let InputValue::RetainedSyntax(unit) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "generated retained callable source variant differs",
            )));
        };
        if unit.key() != &parse
            || unit.generated_unit().is_none() && unit.generated_associated().is_none()
        {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "generated retained callable source differs",
            )));
        }
        match unit.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
        let parsed = match self.fetch(
            &super::super::super::provider::SyntaxProvider,
            QueryKey::ParseFile(parse.clone()),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "generated retained callable parse result differs",
            )));
        };
        let result = (|| {
            let source = self
                .prepared_source(unit.prepared().source())?
                .ok_or_else(|| {
                    fault(
                        "LRR-E-CALL-CONTEXT",
                        "generated retained callable source lease absent",
                    )
                })?;
            let (_, expected) = super::generated_rows(unit, &source, parsed, definition)?;
            if !self.has_definition_rows(&expected) {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "generated retained callable requires its exact published rows",
                ));
            }
            let (
                Some(InputValue::DefinitionHeader(header)),
                Some(InputValue::DefinitionBody(body)),
                Some(InputValue::DeclarationContext(context)),
            ) = (
                expected.get(&InputKey::DefinitionHeader(definition)),
                expected.get(&InputKey::DefinitionBody(definition)),
                expected.get(&InputKey::DeclarationContext(definition)),
            )
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "generated retained callable row set differs",
                ));
            };
            Ok(Some(RetainedOriginalDeclaration {
                parse: parse.clone(),
                header: header.clone(),
                body: body.clone(),
                context: context.clone(),
                lease: RetainedSourceLease(input.clone()),
            }))
        })();
        QueryPoll::Ready(result)
    }
    /// The normal name graph still selects the original declaration. Only a
    /// genuine current retained publication may project its new body/locations.
    /// None means the locator still names the actual original source parse.
    pub(crate) fn retained_original_declaration(
        &mut self,
        source: &PreparedSourceView,
        definition: Digest,
    ) -> QueryPoll<Option<RetainedOriginalDeclaration>> {
        let Some(locator) = self.input(&InputKey::DefinitionSource(definition)) else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "original callable locator absent",
            )));
        };
        let InputValue::DefinitionSource(parse) = locator.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "original callable locator variant differs",
            )));
        };
        if parse == &source.selection().outline.parse {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(input) = self.input(&InputKey::RetainedSyntax(parse.content)) else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "original callable has no admitted retained source",
            )));
        };
        let InputValue::RetainedSyntax(unit) = input.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "original retained callable source variant differs",
            )));
        };
        if unit.key() != parse || unit.prepared().source() != source.selection() {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-CALL-CONTEXT",
                "original retained callable source differs",
            )));
        }
        match unit.require_current(self) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
        let parsed = match self.fetch(
            &super::super::super::provider::SyntaxProvider,
            QueryKey::ParseFile(parse.clone()),
        ) {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        };
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "retained callable parse result differs",
            )));
        };
        let result = (|| {
            let current = self.prepared_source(source.selection())?.ok_or_else(|| {
                fault(
                    "LRR-E-CALL-CONTEXT",
                    "retained callable original source lease absent",
                )
            })?;
            if !source.same_publication(&current) {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "retained callable belongs to another source publication",
                ));
            }
            let (_, expected) = rows(unit, source, parsed, definition)?;
            if !self.has_definition_rows(&expected) {
                return Err(fault(
                    "LRR-E-CALL-CONTEXT",
                    "retained callable requires its exact published declaration rows",
                ));
            }
            let (
                Some(InputValue::DefinitionHeader(header)),
                Some(InputValue::DefinitionBody(body)),
                Some(InputValue::DeclarationContext(context)),
            ) = (
                expected.get(&InputKey::DefinitionHeader(definition)),
                expected.get(&InputKey::DefinitionBody(definition)),
                expected.get(&InputKey::DeclarationContext(definition)),
            )
            else {
                return Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "retained callable row set differs",
                ));
            };
            Ok(Some(RetainedOriginalDeclaration {
                parse: parse.clone(),
                header: header.clone(),
                body: body.clone(),
                context: context.clone(),
                lease: RetainedSourceLease(input.clone()),
            }))
        })();
        QueryPoll::Ready(result)
    }
}
