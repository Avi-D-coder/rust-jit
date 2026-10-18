//! Actual nominal declaration field Type occurrences; no text wrapper parser.
use super::*;
use crate::{
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    source::changes::PreparedSourceView,
    syntax::ast_adapter::{range, tokens},
};
use ra_ap_syntax::{AstNode, ast};
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
macro_rules! checked {
    ($value:expr) => {
        match $value {
            Ok(value) => value,
            Err(error) => return QueryPoll::Ready(Err(error)),
        }
    };
}
impl SchemaNames<'_> {
    /// Plain trait obligations on one actual nominal Type parameter. More
    /// elaborate argument/equality obligations remain unanswered here; their
    /// resolution is not replaced by a primitive-name policy.
    pub(crate) fn source_parameter_bounds(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        ordinal: u32,
    ) -> QueryPoll<Option<Vec<crate::core::digest::Digest>>> {
        use ra_ap_syntax::ast::HasGenericArgs;
        let Some(parameter) =
            self.schema.generics.iter().find(|parameter| {
                parameter.ordinal == ordinal && parameter.kind == GenericKind::Type
            })
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let (_, owner) = ready!(self.actual_nominal_owner(db));
        let declaration =
            super::super::source_types::WrittenTypeDeclaration::nominal(self.schema, &owner);
        let key = crate::traits::term::ParamKey {
            definition: self.schema.definition,
            ordinal,
        };
        let mut traits = Vec::new();
        for (subject, written_subject, bound) in
            super::super::source_types::actual_declaration_bounds(&declaration, &owner)
        {
            let selected = match subject {
                Some(TyTerm::Param(subject)) => subject == key,
                Some(_) => false,
                None => {
                    let Some(ast::Type::PathType(subject)) = written_subject else {
                        continue;
                    };
                    let Some(path) = subject.path() else {
                        continue;
                    };
                    path.qualifier().is_none()
                        && path.segment().is_some_and(|segment| {
                            segment.generic_arg_list().is_none()
                                && segment
                                    .name_ref()
                                    .is_some_and(|name| name.text() == parameter.name)
                        })
                }
            };
            if !selected || bound.question_mark_token().is_some() || bound.lifetime().is_some() {
                continue;
            }
            let Some(ast::Type::PathType(bound_type)) = bound.ty() else {
                return QueryPoll::Ready(Ok(None));
            };
            let Some(path) = bound_type.path() else {
                return QueryPoll::Ready(Ok(None));
            };
            // Associated bindings and trait arguments need their shared
            // equality/argument obligations, not a plain marker candidate.
            if path
                .segment()
                .and_then(|segment| segment.generic_arg_list())
                .is_some()
            {
                return QueryPoll::Ready(Ok(None));
            }
            let Some((identity, arguments)) = ready!(super::super::source_types::projection_trait(
                db,
                self.graph,
                sources,
                &declaration,
                &path
            )) else {
                return QueryPoll::Ready(Ok(None));
            };
            if !arguments.is_empty() {
                return QueryPoll::Ready(Ok(None));
            }
            traits.push(identity);
        }
        QueryPoll::Ready(Ok(Some(traits)))
    }

    /// Actual declaration terms retained for source constraints even while a
    /// projection still needs the constructed value's generic application.
    /// These terms are not runtime field/materialization authority.
    pub(crate) fn source_variant_fields(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        selected: u32,
    ) -> QueryPoll<Vec<Option<TyTerm>>> {
        let Some(variant) = self.schema.variants.get(selected as usize) else {
            return QueryPoll::Ready(Err(schema_fault("source field variant absent")));
        };
        let mut terms = Vec::new();
        for field in &variant.fields {
            terms.push(ready!(self.actual_field_type(db, sources, field)));
        }
        QueryPoll::Ready(Ok(terms))
    }

    fn actual_nominal_owner(
        &self,
        db: &mut Database,
    ) -> QueryPoll<(
        std::sync::Arc<crate::syntax::parse::ParsedFile>,
        ra_ap_syntax::SyntaxNode,
    )> {
        let current = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::TypeSchema(self.schema.definition)
        ));
        if !matches!(current.value.as_ref(), QueryValue::TypeSchema(actual) if actual.as_ref() == self.schema)
        {
            return QueryPoll::Ready(Err(schema_fault("nominal declaration schema differs")));
        }
        let header = db.input(&InputKey::DefinitionHeader(self.schema.definition));
        let context = db.input(&InputKey::DeclarationContext(self.schema.definition));
        let parse = db.input(&InputKey::DefinitionSource(self.schema.definition));
        let (
            Some(InputValue::DefinitionHeader(header)),
            Some(InputValue::DeclarationContext(context)),
            Some(InputValue::DefinitionSource(parse)),
        ) = (header.as_deref(), context.as_deref(), parse.as_deref())
        else {
            return QueryPoll::Ready(Err(schema_fault(
                "field Type current declaration inputs absent",
            )));
        };
        if header != &self.schema.header
            || context.scope != self.schema.scope
            || parse.profile != self.schema.header.profile
        {
            return QueryPoll::Ready(Err(schema_fault(
                "field Type actual declaration header/scope differs",
            )));
        }
        let parsed = ready!(db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())));
        let QueryValue::ParseFile(parsed) = parsed.value.as_ref() else {
            return QueryPoll::Ready(Err(schema_fault("field Type actual parse outcome differs")));
        };
        let owner =
            checked!(parsed.syntax_at(context.range, |node| ast::Item::can_cast(node.kind())));
        let Some(owner) = owner.filter(|node| {
            matches!(
                ast::Item::cast(node.clone()),
                Some(ast::Item::Struct(_) | ast::Item::Enum(_) | ast::Item::Union(_))
            )
        }) else {
            return QueryPoll::Ready(Err(schema_fault(
                "field Type actual nominal declaration absent",
            )));
        };
        QueryPoll::Ready(Ok((std::sync::Arc::clone(parsed), owner)))
    }

    pub(super) fn actual_field_type(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        field: &FieldRecipe,
    ) -> QueryPoll<Option<TyTerm>> {
        if self
            .schema
            .variants
            .iter()
            .flat_map(|variant| &variant.fields)
            .filter(|actual| *actual == field)
            .count()
            != 1
        {
            return QueryPoll::Ready(Err(schema_fault(
                "field Type differs from its actual nominal schema",
            )));
        }
        let (parsed, owner) = ready!(self.actual_nominal_owner(db));
        if !range(owner.text_range()).contains(field.type_range) {
            return QueryPoll::Ready(Err(schema_fault(
                "field Type is outside its actual declaration",
            )));
        }
        // The retained TypeSchema already validated this exact parse/header/
        // context and captured its generics. Reuse that current proof instead
        // of retokenizing the complete header for each selected field.
        let ty =
            checked!(parsed.syntax_at(field.type_range, |node| ast::Type::can_cast(node.kind())));
        let Some(ty) = ty.and_then(ast::Type::cast) else {
            return QueryPoll::Ready(Err(schema_fault(
                "field Type actual syntax occurrence absent",
            )));
        };
        if range(ty.syntax().text_range()) != field.type_range
            || tokens(ty.syntax(), None) != field.ty
        {
            return QueryPoll::Ready(Err(schema_fault(
                "field Type actual occurrence tokens differ",
            )));
        }
        super::super::source_types::resolve_actual_type(
            db,
            self.graph,
            sources,
            &super::super::source_types::WrittenTypeDeclaration::nominal(self.schema, &owner),
            &ty,
        )
    }
}

impl SchemaNames<'_> {
    /// Selected caller Type arguments instantiate this declaration's actual
    /// field terms before the shared associated-type normalizer runs.
    pub(crate) fn bind_record_constructor_current_with_arguments(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        name: Option<&str>,
        plan: &crate::syntax::constructors::ConstructorPlan,
        caller: &ScopeKey,
        arguments: &[Option<TyTerm>],
        members: &crate::frontend::member_index::MemberIndex,
    ) -> QueryPoll<NominalConstructorBinding> {
        let bindings = checked!(self.argument_bindings(arguments));
        let result = self.bind_record_constructor_with(name, plan, caller, |query| {
            self.instantiated_query(db, sources, query, &bindings, members)
        });
        match result {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }

    fn argument_bindings(
        &self,
        arguments: &[Option<TyTerm>],
    ) -> Result<BTreeMap<crate::traits::term::ParamKey, TyTerm>, Diagnostic> {
        let parameters: Vec<_> = self
            .schema
            .generics
            .iter()
            .filter(|parameter| parameter.kind == GenericKind::Type)
            .collect();
        if arguments.len() > parameters.len() {
            return Err(schema_fault(
                "nominal Type arguments exceed its declaration parameters",
            ));
        }
        let bindings: BTreeMap<_, _> = parameters
            .iter()
            .zip(arguments)
            .filter_map(|(parameter, argument)| {
                argument.as_ref().map(|argument| {
                    (
                        crate::traits::term::ParamKey {
                            definition: self.schema.definition,
                            ordinal: parameter.ordinal,
                        },
                        argument.clone(),
                    )
                })
            })
            .collect();
        Ok(bindings)
    }

    fn instantiated_query(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        query: FieldTypeQuery<'_>,
        bindings: &BTreeMap<crate::traits::term::ParamKey, TyTerm>,
        members: &crate::frontend::member_index::MemberIndex,
    ) -> QueryPoll<SchemaTypeName> {
        let field = match query {
            FieldTypeQuery::Compound(field) | FieldTypeQuery::Name(_, Some(field)) => field,
            FieldTypeQuery::Name(_, None) => return self.current_query(db, sources, query),
        };
        // Keep local fields on their actual HeaderTypeSite prefix. Their
        // pending projection integration never grants a module fallback.
        if self.schema.scope.owner.is_some() {
            return self.current_query(db, sources, query);
        }
        let term = match self.actual_field_type(db, sources, field) {
            QueryPoll::Ready(Ok(Some(term))) => term,
            QueryPoll::Ready(Ok(None)) => {
                return QueryPoll::Ready(Ok(SchemaTypeName::deferred()));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let term = if crate::frontend::associated_types::contains_projection(&term) {
            match crate::frontend::associated_types::normalize_template(
                db, self.graph, sources, members, &term, bindings,
            ) {
                QueryPoll::Ready(Ok(Some(term))) => term,
                QueryPoll::Ready(Ok(None)) => {
                    return QueryPoll::Ready(Ok(SchemaTypeName::deferred()));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        } else {
            term
        };
        // Preserve the existing concrete primitive field representation;
        // aliases and normalized projections have the same primitive identity.
        if let TyTerm::Primitive(primitive) = &term {
            return QueryPoll::Ready(SchemaTypeName::metadata(
                super::super::signature_names::SignatureName::Primitive(*primitive),
            ));
        }
        let written = match self.written_field_type(db, sources, &term) {
            QueryPoll::Ready(Ok(written)) => written,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        QueryPoll::Ready(Ok(SchemaTypeName {
            written,
            ..SchemaTypeName::deferred()
        }))
    }

    pub(crate) fn bind_variant_current_with_arguments(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        name: Option<&str>,
        arguments: &[Option<TyTerm>],
        members: &crate::frontend::member_index::MemberIndex,
    ) -> QueryPoll<NominalBinding> {
        let bindings = checked!(self.argument_bindings(arguments));
        match self.bind_variant_with(name, |query| {
            self.instantiated_query(db, sources, query, &bindings, members)
        }) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }

    pub(crate) fn bind_named_field_current_with_arguments(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        variant: Option<&str>,
        member: &str,
        caller: &ScopeKey,
        arguments: &[Option<TyTerm>],
        members: &crate::frontend::member_index::MemberIndex,
    ) -> QueryPoll<NominalFieldBinding> {
        let bindings = checked!(self.argument_bindings(arguments));
        match self.bind_named_field_with(variant, member, caller, |query| {
            self.instantiated_query(db, sources, query, &bindings, members)
        }) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }

    pub(crate) fn bind_positional_field_current_with_arguments(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        variant: Option<&str>,
        field: u32,
        caller: &ScopeKey,
        arguments: &[Option<TyTerm>],
        members: &crate::frontend::member_index::MemberIndex,
    ) -> QueryPoll<NominalFieldBinding> {
        let bindings = checked!(self.argument_bindings(arguments));
        match self.bind_field_with(variant, FieldSelector::Positional(field), caller, |query| {
            self.instantiated_query(db, sources, query, &bindings, members)
        }) {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
