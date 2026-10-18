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
    pub(super) fn actual_field_type(
        &self,
        db: &mut Database,
        sources: &[&PreparedSourceView],
        field: &FieldRecipe,
    ) -> QueryPoll<Option<TyTerm>> {
        let current = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::TypeSchema(self.schema.definition)
        ));
        if !matches!(current.value.as_ref(), QueryValue::TypeSchema(actual) if actual.as_ref() == self.schema)
            || self
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
            || !context.range.contains(field.type_range)
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
        let parameters: Vec<_> = self
            .schema
            .generics
            .iter()
            .filter(|parameter| parameter.kind == GenericKind::Type)
            .collect();
        if arguments.len() > parameters.len() {
            return QueryPoll::Ready(Err(schema_fault(
                "record Type arguments exceed its declaration parameters",
            )));
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
        let result = self.bind_record_constructor_with(name, plan, caller, |query| {
            let FieldTypeQuery::Compound(field) = query else {
                return self.current_query(db, sources, query);
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
            let term = crate::frontend::associated_types::substitute(&term, &bindings);
            let term = if crate::frontend::associated_types::contains_projection(&term) {
                match crate::frontend::associated_types::normalize(
                    db, self.graph, sources, members, &term,
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
            let written = match self.written_field_type(db, sources, &term) {
                QueryPoll::Ready(Ok(written)) => written,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
            QueryPoll::Ready(Ok(SchemaTypeName {
                written,
                ..SchemaTypeName::deferred()
            }))
        });
        match result {
            Ok(poll) => poll,
            Err(error) => QueryPoll::Ready(Err(error)),
        }
    }
}
