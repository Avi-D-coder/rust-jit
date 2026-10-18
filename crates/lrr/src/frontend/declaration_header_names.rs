//! Source-only names in an actual selected local declaration's header.
//! The declaring parent supplies the lexical proof; the child body stays cold.
use super::*;

impl PreparedSourceLocalPlan {
    pub(crate) fn selected_header_type_name(
        self: &Arc<Self>,
        db: &mut Database,
        proof: &crate::frontend::callables::ordinary_local::OrdinaryLocalAdmission,
        name: &ast::NameRef,
    ) -> QueryPoll<SignatureName> {
        checked!(proof.revalidate(db));
        ready!(self.require_current(db));
        let declaration = proof.item();
        let signature = proof.signature();
        let source = self.context.source();
        let definition = declaration.def_key.fingerprint();
        let span = range(name.syntax().text_range());
        let item = checked!(
            self.field_types
                .items
                .get(&definition)
                .ok_or_else(|| error("selected declaration actual header absent"))
        );
        if !Arc::ptr_eq(self, proof.parent())
            || proof.source() != &source.parsed().key()
            || signature.definition != definition
            || signature.header != declaration.header
            || signature.scope != declaration.scope
            || declaration.def_key.item_kind != ItemKind::Function
            || item.header != declaration.header.fingerprint()
            || item.declaration != declaration.declaration_span
            || item.scope != proof.scope()
            || !item.declaration.contains(span)
            || declaration
                .body
                .as_ref()
                .is_some_and(|body| body.block_range().contains(span))
        {
            return QueryPoll::Ready(Err(error("selected declaration header occurrence differs")));
        }
        let actual = checked!(
            source
                .parsed()
                .syntax_at(span, |node| ast::NameRef::can_cast(node.kind()))
        );
        if actual.as_ref() != Some(name.syntax()) {
            return QueryPoll::Ready(Err(error("selected declaration name syntax differs")));
        }
        // This direct ancestor locator proves that the NameRef belongs to this
        // Fn header, not an equal-spelled name in an inner item or its body.
        if !name
            .syntax()
            .ancestors()
            .find_map(ast::Fn::cast)
            .is_some_and(|function| range(function.syntax().text_range()) == item.declaration)
        {
            return QueryPoll::Ready(Err(error("selected declaration name owner differs")));
        }
        let (root, _) = self.entered.local_hygiene();
        let occurrence = LocalTypeUse {
            name: name.text().to_string(),
            range: span,
            context: checked!(source.identifier_context_or_original(span, root)),
            edition: checked!(source.parsed().token_edition(span)),
        };
        let field = HeaderField {
            parameter: None,
            range: span,
            tokens: Vec::new(),
            name: Some(occurrence),
            shape: None,
        };
        let header = HeaderTypeSite {
            owner: self.field_types.owner,
            item,
            field: &field,
            leaf: None,
        };
        let prefix = ready!(self.prepare_header_prefix(
            db,
            &header,
            super::super::DeclarationTypeSite::Header {
                definition,
                range: span
            }
        ));
        let lookup = match prefix.selection() {
            LocalTypePrefixSelection::Parameter(parameter) => {
                return QueryPoll::Ready(Ok(SignatureName::Parameter(*parameter)));
            }
            LocalTypePrefixSelection::Lookup(lookup) => lookup.clone(),
            LocalTypePrefixSelection::ModuleTail => {
                let Some(observation) = ready!(prefix.module_name(db)) else {
                    return QueryPoll::Ready(Ok(SignatureName::NeedOwnerContext(
                        signature.scope.clone(),
                    )));
                };
                if observation.completion()
                    != crate::frontend::current_module_names::ModuleNameCompletion::Complete
                {
                    return QueryPoll::Ready(Ok(SignatureName::NeedOwnerContext(
                        signature.scope.clone(),
                    )));
                }
                let Some(lookup) = observation.lookup() else {
                    return QueryPoll::Ready(Ok(SignatureName::NeedOwnerContext(
                        signature.scope.clone(),
                    )));
                };
                lookup.clone()
            }
            _ => {
                return QueryPoll::Ready(Ok(SignatureName::NeedOwnerContext(
                    signature.scope.clone(),
                )));
            }
        };
        let occurrence = checked!(
            prefix
                .use_name()
                .ok_or_else(|| error("selected header Type name absent"))
        );
        QueryPoll::Ready(
            crate::frontend::signature_names::classify_completed_type_lookup(
                occurrence.name(),
                occurrence.edition(),
                lookup,
            ),
        )
    }
}
