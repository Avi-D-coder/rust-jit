//! The actual declared result occurrence enters the shared header Type service.
//! This child borrows its parent's canonical header/generic evidence; it neither
//! indexes nor prepares the selected function's executable body.
use super::*;

impl PreparedSourceLocalPlan {
    pub(crate) fn prepare_return_type(
        self: &Arc<Self>,
        db: &mut Database,
        signature: &crate::syntax::signatures::FunctionSignature,
    ) -> QueryPoll<NamedTypePreparation> {
        ready!(self.require_current(db));
        let current = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::FunctionSignature(signature.definition)
        ));
        if !matches!(current.value.as_ref(), QueryValue::FunctionSignature(actual)
            if std::ptr::eq(actual.as_ref(), signature) || actual.as_ref() == signature)
        {
            return QueryPoll::Ready(Err(error("return Type current signature differs")));
        }
        // A block-local enum's discriminant constant (E-R5-013) declares no
        // result syntax: its type is the enum's repr integer, which the
        // signature states as a primitive.
        if let Some(crate::traits::term::TyTerm::Primitive(primitive)) = &signature.result_symbolic
            && self.context.facts().members.iter().any(|row| {
                row.item.is_discriminant_constant()
                    && row.item.def_key.fingerprint() == signature.definition
                    && row.item.header == signature.header
            })
        {
            return QueryPoll::Ready(
                crate::frontend::source_types::SourceType::selected_primitive(*primitive)
                    .map(NamedTypePreparation::Ready),
            );
        }
        let mut rows = self
            .context
            .facts()
            .declarations()
            .filter(|row| row.item.def_key.fingerprint() == signature.definition);
        let row = checked!(
            rows.next()
                .ok_or_else(|| error("return Type canonical local owner absent"))
        );
        if rows.next().is_some()
            || row.attribute_input.is_some()
            || !matches!(
                row.item.def_key.item_kind,
                ItemKind::Function | ItemKind::Constant | ItemKind::Static
            )
            || row.item.scope != signature.scope
            || row.item.header != signature.header
            || !matches!(db.input(&InputKey::LocalDeclarationOwner(signature.definition)).as_deref(),
                Some(InputValue::LocalDeclarationOwner(body)) if body == self.context.source().index().body())
            || !matches!(db.input(&InputKey::DefinitionSource(signature.definition)).as_deref(),
                Some(InputValue::DefinitionSource(parse)) if *parse == self.context.source().parsed().key())
            || !matches!(db.input(&InputKey::DeclarationContext(signature.definition)).as_deref(),
                Some(InputValue::DeclarationContext(actual))
                    if actual.scope == row.item.scope && actual.range == row.item.declaration_span)
        {
            return QueryPoll::Ready(Err(error("return Type current owning declaration differs")));
        }
        let key = InputKey::DefinitionHeader(signature.definition);
        if !matches!(db.input(&key).as_deref(), Some(InputValue::DefinitionHeader(header))
            if header == &row.item.header)
        {
            return QueryPoll::Ready(Err(error("return Type actual header input differs")));
        }
        let header_input = checked!(
            db.published_input(&key)
                .map(|(_, stamp)| stamp)
                .ok_or_else(|| error("return Type header stamp absent"))
        );
        let item = checked!(
            self.field_types
                .items
                .get(&signature.definition)
                .ok_or_else(|| error("return Type actual owning header absent"))
        );
        if item.header_input != header_input
            || item.header != signature.header.fingerprint()
            || item.scope != row.scope
            || item.declaration != row.item.declaration_span
        {
            return QueryPoll::Ready(Err(error("return Type retained owning header differs")));
        }
        let source = self.context.source();
        let parsed = source.parsed();
        let declaration = checked!(
            parsed
                .syntax_at(item.declaration, |node| ast::Item::can_cast(node.kind()))
                .and_then(|node| node
                    .ok_or_else(|| error("return Type actual declaration syntax absent")))
        );
        let (ty, excluded, implicit_unit) = match row.item.def_key.item_kind {
            ItemKind::Function => {
                let function = checked!(
                    ast::Fn::cast(declaration)
                        .ok_or_else(|| error("return Type actual function syntax absent"))
                );
                let result = function.ret_type();
                let implicit_unit = result.is_none();
                let ty = result.and_then(|result| result.ty());
                let excluded = function
                    .syntax()
                    .children()
                    .find_map(ast::BlockExpr::cast)
                    .map(|body| body.syntax().text_range());
                (ty, excluded, implicit_unit)
            }
            ItemKind::Constant => {
                let constant = checked!(
                    ast::Const::cast(declaration)
                        .ok_or_else(|| error("return Type actual const syntax absent"))
                );
                (
                    constant.ty(),
                    constant.body().map(|body| body.syntax().text_range()),
                    false,
                )
            }
            ItemKind::Static => {
                let static_ = checked!(
                    ast::Static::cast(declaration)
                        .ok_or_else(|| error("return Type actual static syntax absent"))
                );
                (
                    static_.ty(),
                    static_.body().map(|body| body.syntax().text_range()),
                    false,
                )
            }
            _ => return QueryPoll::Ready(Err(error("return Type declaration has no result"))),
        };
        // The retained header already excluded the executable child. Resolve
        // only this declaration's actual annotation, never its initializer.
        if row
            .item
            .body
            .as_ref()
            .is_some_and(|body| excluded.is_none_or(|actual| range(actual) != body.block_range()))
        {
            return QueryPoll::Ready(Err(error("return Type actual declaration body differs")));
        }
        if implicit_unit {
            if !signature.result.is_empty() {
                return QueryPoll::Ready(Err(error("return Type implicit unit recipe differs")));
            }
            return QueryPoll::Ready(
                crate::frontend::source_types::SourceType::selected_primitive(
                    crate::ir::op::PrimitiveType::Unit,
                )
                .map(NamedTypePreparation::Ready),
            );
        }
        let ty = checked!(ty.ok_or_else(|| error("return Type actual annotation is incomplete")));
        let result_range = range(ty.syntax().text_range());
        if !item.declaration.contains(result_range) {
            return QueryPoll::Ready(Err(error("return Type outside actual declaration")));
        }
        checked!(add(0, signature.header.estimated_bytes()));
        let syntax = tokens(ty.syntax(), None);
        if syntax != signature.result {
            return QueryPoll::Ready(Err(error("return Type actual annotation recipe differs")));
        }
        let (root, _) = self.entered.local_hygiene();
        let use_name = |name: ast::NameRef| -> Result<LocalTypeUse, Diagnostic> {
            let span = range(name.syntax().text_range());
            Ok(LocalTypeUse {
                name: name.text().to_string(),
                range: span,
                context: source.identifier_context_or_original(span, root)?,
                edition: parsed.token_edition(span)?,
            })
        };
        let name = if let ast::Type::PathType(path) = &ty {
            checked!(
                path.path()
                    .filter(|path| path.qualifier().is_none())
                    .and_then(|path| path.segment())
                    .filter(|segment| segment.generic_arg_list().is_none()
                        && segment.coloncolon_token().is_none())
                    .and_then(|segment| segment.name_ref())
                    .map(|name| {
                        let span = range(name.syntax().text_range());
                        Ok(LocalTypeUse {
                            name: name.text().to_string(),
                            range: span,
                            context: source.identifier_context_or_original(span, root)?,
                            edition: parsed.token_edition(span)?,
                        })
                    })
                    .transpose()
            )
        } else {
            None
        };
        // A compound result's leaf names are its actual occurrences here,
        // captured on this demand only (the plan retains no result rows).
        let shape = match name {
            Some(_) => None,
            None => checked!(HeaderShape::capture(&ty, &use_name)),
        };
        let field = HeaderField {
            parameter: None,
            range: result_range,
            tokens: syntax,
            name,
            shape,
        };
        let header = HeaderTypeSite {
            owner: self.field_types.owner,
            item,
            field: &field,
            leaf: None,
        };
        let site = super::super::DeclarationTypeSite::Return {
            definition: signature.definition,
            range: result_range,
        };
        if let Some(shape) = &field.shape {
            return self.prepare_compound_header_type(db, &header, shape, &site, &signature.scope);
        }
        self.prepare_header_type(db, header, site, &signature.scope)
    }
}
