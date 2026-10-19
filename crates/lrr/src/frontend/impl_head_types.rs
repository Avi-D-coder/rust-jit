//! Actual impl head Types through the common declaration-context Type walk.
//! Generic/where predicates remain retained syntax, not applicability evidence.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    traits::{
        header::CanonicalImplOwner,
        header_syntax::ImplHeaderSyntaxKey,
        term::{TraitRef, TyTerm},
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ImplHeadPending {
    CanonicalOwner,
    SourceEnvironment,
    DeclarationMetadata,
    SelfType,
    TraitType,
    SymbolicRegions,
    SymbolicTypeIdentity,
}
#[derive(Debug)]
pub enum ImplHeadTypeResult {
    Resolved {
        self_type: TyTerm,
        trait_ref: Option<TraitRef>,
    },
    Pending(ImplHeadPending),
}
#[derive(Debug)]
pub(super) struct ImplDeclarationContext {
    source: Arc<InputValue>,
    header: Arc<InputValue>,
    context: Arc<InputValue>,
}
impl ImplDeclarationContext {
    pub(super) fn observe(db: &mut Database,
        syntax: &crate::traits::header_syntax::ImplHeaderSyntax,
        raw: crate::traits::header::SourceImplHeader<'_>,
    ) -> QueryPoll<Option<Self>> {
        let Some(owner) = syntax.canonical_owner() else { return QueryPoll::Ready(Ok(None)); };
        let definition = owner.identity();
        let source_input = db.input(&InputKey::DefinitionSource(definition));
        let header_input = db.input(&InputKey::DefinitionHeader(definition));
        let context_input = db.input(&InputKey::DeclarationContext(definition));
        let (Some(source_input), Some(header_input), Some(context_input)) =
            (source_input, header_input, context_input)
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let (
            InputValue::DefinitionSource(source),
            InputValue::DefinitionHeader(header),
            InputValue::DeclarationContext(context),
        ) = (
            source_input.as_ref(),
            header_input.as_ref(),
            context_input.as_ref(),
        )
        else {
            return QueryPoll::Ready(Err(fault("impl Type declaration input variants differ")));
        };
        if header.profile != syntax.source().profile
            || context.range != raw.requirements().range
            || context.scope.crate_key != syntax.scope().crate_key
            || context.scope.module_path != syntax.scope().module_path
            || header.tokens != raw.requirements().header
            || matches!(owner, CanonicalImplOwner::Original(_)) && source != syntax.source()
        {
            return QueryPoll::Ready(Err(fault(
                "impl Type actual declaration attachment differs",
            )));
        }
        QueryPoll::Ready(Ok(Some(Self { source: source_input, header: header_input, context: context_input })))
    }
    pub(super) fn matches_metadata(&self, parse: &crate::syntax::parse::ParseKey,
        recipe: &crate::syntax::outline::HeaderRecipe,
        declaration: &crate::syntax::outline::DeclarationContext,
    ) -> bool {
        matches!(self.source.as_ref(), InputValue::DefinitionSource(source) if source == parse)
            && matches!(self.header.as_ref(), InputValue::DefinitionHeader(header) if header == recipe)
            && matches!(self.context.as_ref(), InputValue::DeclarationContext(context) if context == declaration)
    }
    pub(super) fn owner(
        &self,
        definition: crate::core::digest::Digest,
    ) -> super::type_name_namespace::SourceTypeOwner<'_> {
        let InputValue::DefinitionHeader(header) = self.header.as_ref() else {
            unreachable!("verified declaration header")
        };
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("verified declaration context")
        };
        super::type_name_namespace::SourceTypeOwner {
            definition,
            header,
            scope: &context.scope,
        }
    }
    pub(super) fn range(&self) -> crate::syntax::spans::SourceRange {
        let InputValue::DeclarationContext(context) = self.context.as_ref() else {
            unreachable!("verified declaration context")
        };
        context.range
    }
}
#[derive(Debug)]
pub struct PreparedImplHeadTypes {
    key: ImplHeaderSyntaxKey,
    syntax: Arc<QueryValue>,
    result: ImplHeadTypeResult,
    declaration: Option<ImplDeclarationContext>,
}
impl PreparedImplHeadTypes {
    pub fn key(&self) -> &ImplHeaderSyntaxKey {
        &self.key
    }
    pub fn result(&self) -> &ImplHeadTypeResult {
        &self.result
    }
    pub fn syntax(&self) -> &crate::traits::header_syntax::ImplHeaderSyntax {
        let QueryValue::ImplHeaderSyntax(syntax) = self.syntax.as_ref() else {
            unreachable!("private source header owner")
        };
        syntax
    }
    pub(super) fn declaration(&self) -> Option<&ImplDeclarationContext> {
        self.declaration.as_ref()
    }
    pub(crate) fn prepare(db: &mut Database, key: &ImplHeaderSyntaxKey) -> QueryPoll<Self> {
        macro_rules! ready {
            ($value:expr) => {
                match $value {
                    QueryPoll::Ready(Ok(value)) => value,
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            };
        }
        let captured = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::ImplHeaderSyntax(Box::new(key.clone()))
        ));
        let syntax_owner = captured.value;
        let QueryValue::ImplHeaderSyntax(syntax) = syntax_owner.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl Type syntax query differs")));
        };
        macro_rules! pending {
            ($why:ident) => {
                return QueryPoll::Ready(Ok(Self {
                    key: key.clone(),
                    syntax: syntax_owner.clone(),
                    result: ImplHeadTypeResult::Pending(ImplHeadPending::$why),
                    declaration: None,
                }))
            };
        }
        let Some(owner) = syntax.canonical_owner() else {
            pending!(CanonicalOwner)
        };
        let definition = owner.identity();
        let inventory = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ));
        let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("impl Type inventory query differs")));
        };
        let Some(raw) = inventory.header(key.implementation) else {
            return QueryPoll::Ready(Err(fault("impl Type source header absent")));
        };
        let Some(environment) = inventory.inventory_root().environment() else {
            pending!(SourceEnvironment)
        };
        let Some(declaration_context) = ready!(ImplDeclarationContext::observe(db, syntax, raw)) else {
            pending!(DeclarationMetadata)
        };
        let owner = declaration_context.owner(definition);
        let context_range = declaration_context.range();
        let node = match syntax
            .parsed()
            .syntax_at(context_range, |node| ast::Impl::can_cast(node.kind()))
        {
            Ok(Some(node)) => node,
            Ok(None) => return QueryPoll::Ready(Err(fault("impl Type retained syntax absent"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let implementation = ast::Impl::cast(node).expect("checked impl kind");
        let Some(self_syntax) = implementation.self_ty() else {
            pending!(SelfType)
        };
        let trait_syntax = implementation.trait_();
        for ty in std::iter::once(&self_syntax).chain(trait_syntax.iter()) {
            match super::source_types::symbolic_type_pending(ty) {
                Some(super::source_types::SymbolicTypePending::Regions) => {
                    pending!(SymbolicRegions)
                }
                Some(super::source_types::SymbolicTypePending::Identity) => {
                    pending!(SymbolicTypeIdentity)
                }
                None => {}
            }
        }
        let sources: Vec<_> = environment.sources().iter().collect();
        let declaration = super::source_types::WrittenTypeDeclaration::implementation(
            owner,
            syntax.generics(),
            implementation.syntax(),
            None,
        );
        let Some(self_type) = ready!(super::source_types::resolve_actual_type(
            db,
            environment.names(),
            &sources,
            &declaration,
            &self_syntax
        )) else {
            pending!(SelfType)
        };
        let declaration = super::source_types::WrittenTypeDeclaration::implementation(
            owner,
            syntax.generics(),
            implementation.syntax(),
            Some(&self_type),
        );
        let trait_ref = match trait_syntax {
            None => None,
            Some(ast::Type::PathType(path)) => {
                let Some(path) = path.path() else {
                    pending!(TraitType)
                };
                if path
                    .syntax()
                    .descendants()
                    .any(|node| ast::AssocTypeArg::can_cast(node.kind()))
                {
                    pending!(TraitType);
                }
                let Some((trait_key, args)) = ready!(super::source_types::projection_trait(
                    db,
                    environment.names(),
                    &sources,
                    &declaration,
                    &path
                )) else {
                    pending!(TraitType)
                };
                Some(TraitRef {
                    trait_key,
                    self_ty: self_type.clone(),
                    args,
                })
            }
            Some(_) => {
                pending!(TraitType)
            }
        };
        QueryPoll::Ready(Ok(Self {
            key: key.clone(),
            syntax: syntax_owner.clone(),
            declaration: Some(declaration_context),
            result: ImplHeadTypeResult::Resolved {
                self_type,
                trait_ref,
            },
        }))
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-HEADER", message.into())
}
impl StableEncode for PreparedImplHeadTypes {
    const DOMAIN: &'static str = "lrr/v1/prepared-impl-head-types";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.syntax());
        match &self.result {
            ImplHeadTypeResult::Resolved {
                self_type,
                trait_ref,
            } => {
                e.u32(0);
                e.record(self_type);
                e.option(trait_ref, |e, value| e.record(value));
            }
            ImplHeadTypeResult::Pending(why) => {
                e.u32(1);
                e.u32(match why {
                    ImplHeadPending::CanonicalOwner => 0,
                    ImplHeadPending::SourceEnvironment => 1,
                    ImplHeadPending::DeclarationMetadata => 2,
                    ImplHeadPending::SelfType => 3,
                    ImplHeadPending::TraitType => 4,
                    ImplHeadPending::SymbolicRegions => 5,
                    ImplHeadPending::SymbolicTypeIdentity => 6,
                });
            }
        }
    }
}
