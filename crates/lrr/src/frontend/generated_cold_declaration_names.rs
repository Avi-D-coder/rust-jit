//! Exact completed-output insertion into the common local declaration prefix.
use super::*;
use crate::db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue};

impl ColdDeclarationNames {
    pub(super) fn lookup_generated(&self, db: &mut Database,
        declaration: SourceTypeOwner<'_>, name: &ast::NameRef,
    ) -> Result<ColdNameStep, Diagnostic> {
        let Some(source) = db.input(&InputKey::DefinitionSource(declaration.definition)) else {
            return Ok(ColdNameStep::Pending);
        };
        let InputValue::DefinitionSource(parse) = source.as_ref() else {
            return Err(error("generated local Type source variant differs"));
        };
        let Some(input) = db.input(&InputKey::GeneratedSyntax(parse.content)) else {
            return Ok(ColdNameStep::Pending);
        };
        let InputValue::GeneratedSyntax(unit) = input.as_ref() else {
            return Err(error("generated local Type syntax variant differs"));
        };
        let record = unit.items.record();
        // Membership was prepared from the actual completed fragment and the
        // real original target. Neither token text nor source spans grant it.
        let Some((insertion, ordinal)) = self.outputs.get(&(record.descriptor().fingerprint(), declaration.definition)) else {
            return Ok(ColdNameStep::Pending);
        };
        self.require_current(db)?;
        self.require_scope_slot(db, insertion)?;
        if unit.key != *parse || &unit.scope != insertion || declaration.scope != insertion
            || unit.seal != self.seal || unit.items.epoch() != self.seal.epoch
            || !matches!(db.input(&InputKey::ExpansionRecord(record.key().fingerprint())).as_deref(),
                Some(InputValue::ExpansionRecord(current)) if current.same_record(record)) {
            return Err(error("generated local Type completion/source/insertion changed"));
        }
        let Some(actual) = unit.declaration(declaration.definition) else {
            return Err(error("generated local Type canonical declaration absent"));
        };
        let Some(header) = db.input(&InputKey::DefinitionHeader(declaration.definition)) else {
            return Ok(ColdNameStep::Pending);
        };
        let Some(context) = db.input(&InputKey::DeclarationContext(declaration.definition)) else {
            return Ok(ColdNameStep::Pending);
        };
        let (InputValue::DefinitionHeader(header), InputValue::DeclarationContext(context)) =
            (header.as_ref(), context.as_ref()) else {
                return Err(error("generated local Type canonical variants differ"));
            };
        if header != declaration.header || header != &actual.header
            || context.scope != *declaration.scope || context.scope != actual.scope
            || context.range != actual.declaration_span {
            return Err(error("generated local Type canonical owner/header differs"));
        }
        let captured = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return Err(error),
            QueryPoll::NeedInput(input) => return Ok(ColdNameStep::Demand(input)),
        };
        let QueryValue::ParseFile(parsed) = captured.as_ref() else {
            return Err(error("generated local Type parse variant differs"));
        };
        let occurrence = parsed.declaration_type_name(declaration.definition, header, context, name)?;
        occurrence.require(parse, declaration.definition, header, context, name.text())?;
        let origins = unit.items.project_source_spans(occurrence.range())?;
        let mut comparison = None;
        for origin in origins {
            let actual = unit.items.item_class(origin.context)?;
            if comparison.replace(actual).is_some_and(|prior| prior != actual) {
                return Err(error("generated local Type name has conflicting item contexts"));
            }
        }
        let Some(comparison) = comparison else {
            return Err(error("generated local Type identifier origin absent"));
        };
        let name = MacroName::local(name.text(), parsed.token_edition(occurrence.range())?)?;
        self.lookup_prefix(*ordinal, &name, comparison)
    }
}
