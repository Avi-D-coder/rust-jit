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
pub struct PreparedImplHeadTypes {
    key: ImplHeaderSyntaxKey,
    syntax: Arc<QueryValue>,
    result: ImplHeadTypeResult,
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
        let source = db.input(&InputKey::DefinitionSource(definition));
        let header = db.input(&InputKey::DefinitionHeader(definition));
        let context = db.input(&InputKey::DeclarationContext(definition));
        let (Some(source), Some(header), Some(context)) = (source, header, context) else {
            pending!(DeclarationMetadata)
        };
        let (
            InputValue::DefinitionSource(source),
            InputValue::DefinitionHeader(header),
            InputValue::DeclarationContext(context),
        ) = (source.as_ref(), header.as_ref(), context.as_ref())
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
        let node = match syntax
            .parsed()
            .syntax_at(context.range, |node| ast::Impl::can_cast(node.kind()))
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
        // The ordinary runtime Type walk currently erases these regions. Do
        // not promote erased terms to symbolic trait-head evidence.
        if std::iter::once(&self_syntax)
            .chain(trait_syntax.iter())
            .any(|ty| {
                ty.syntax().descendants().any(|node| {
                    ast::RefType::can_cast(node.kind()) || ast::Lifetime::can_cast(node.kind())
                })
            })
        {
            pending!(SymbolicRegions);
        }
        if std::iter::once(&self_syntax)
            .chain(trait_syntax.iter())
            .any(|ty| {
                ty.syntax().descendants().any(|node| {
                    ast::DynTraitType::can_cast(node.kind())
                        || ast::ImplTraitType::can_cast(node.kind())
                        || ast::FnPtrType::cast(node).is_some_and(|function| {
                            function.abi().is_some() || function.unsafe_token().is_some()
                        })
                })
            })
        {
            // Runtime dyn views/ABI erasure are not complete symbolic identities.
            // Preserve actual source demand until their shared representation joins.
            pending!(SymbolicTypeIdentity);
        }
        let sources: Vec<_> = environment.sources().iter().collect();
        let owner = super::type_name_namespace::SourceTypeOwner {
            definition,
            scope: &context.scope,
            header,
        };
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
