//! Demand-owned alias declarations; no name solving or executable body entry.
use super::{
    ast_adapter::{TokenRecipe, range, tokens},
    outline::{DeclarationContext, HeaderRecipe, ScopeKey},
    parse::ParsedFile,
    signatures::{DefinitionParameter, capture_generics, lower_type},
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    traits::term::TyTerm,
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams},
};
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TypeAliasRecipe {
    pub definition: Digest,
    pub scope: ScopeKey,
    pub header: HeaderRecipe,
    pub generics: Vec<DefinitionParameter>,
    pub target: Vec<TokenRecipe>,
    pub symbolic: Option<TyTerm>,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TYPE-CONTEXT", message.into())
}
impl ParsedFile {
    pub fn type_alias_recipe(
        &self,
        definition: Digest,
        header: &HeaderRecipe,
        context: &DeclarationContext,
    ) -> Result<TypeAliasRecipe, Diagnostic> {
        if header.profile != self.profile {
            return Err(fault("alias/source profile mismatch"));
        }
        let alias = self
            .syntax_at(context.range, |node| ast::TypeAlias::can_cast(node.kind()))?
            .and_then(ast::TypeAlias::cast)
            .filter(|alias| {
                range(alias.syntax().text_range()) == context.range
                    && tokens(alias.syntax(), None) == header.tokens
            })
            .ok_or_else(|| fault("alias declaration is not in its immutable source context"))?;
        let generics = capture_generics(alias.generic_param_list())?;
        let symbolic = alias
            .ty()
            .as_ref()
            .map(|ty| lower_type(ty, definition, &generics, 0))
            .transpose()?
            .flatten();
        let target = alias
            .ty()
            .map_or_else(Vec::new, |ty| tokens(ty.syntax(), None));
        let recipe = TypeAliasRecipe {
            definition,
            scope: context.scope.clone(),
            header: header.clone(),
            generics,
            target,
            symbolic,
        };
        Ok(recipe)
    }
}
impl StableEncode for TypeAliasRecipe {
    const DOMAIN: &'static str = "lrr/v3/type-alias-recipe";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.record(&self.scope);
        e.record(&self.header);
        e.sequence(&self.generics, |e, p| {
            e.u32(p.ordinal);
            e.u32(match p.kind {
                super::signatures::GenericKind::Lifetime => 1,
                super::signatures::GenericKind::Type => 2,
                super::signatures::GenericKind::Const => 3,
            });
            e.string(&p.name);
            e.sequence(&p.bounds, |e, t| e.record(t));
            e.sequence(&p.default, |e, t| e.record(t));
            e.sequence(&p.const_type, |e, t| e.record(t));
            e.option(&p.anonymous_region, |e, range| { e.u32(range.start); e.u32(range.end); });
        });
        // Predicates remain in the exact header;
        // target terms preserve the rigid definition/ordinal identities.
        e.sequence(&self.target, |e, t| e.record(t));
        e.option(&self.symbolic, |e, t| e.record(t));
    }
}
impl TypeAliasRecipe {
    pub fn estimated_bytes(&self) -> usize {
        self.checked_bytes().unwrap_or(usize::MAX)
    }
    fn checked_bytes(&self) -> Option<usize> {
        let mut bytes = std::mem::size_of::<Self>()
            .checked_add(self.scope.heap_bytes())?
            .checked_add(self.header.estimated_bytes())?
            .checked_add(
                self.generics
                    .capacity()
                    .checked_mul(std::mem::size_of::<DefinitionParameter>())?,
            )?
            .checked_add(
                self.target
                    .capacity()
                    .checked_mul(std::mem::size_of::<TokenRecipe>())?,
            )?;
        for token in &self.target {
            bytes = bytes.checked_add(token.text.capacity())?;
        }
        for parameter in &self.generics {
            bytes = bytes.checked_add(parameter.name.capacity())?;
            for tokens in [&parameter.bounds, &parameter.default, &parameter.const_type] {
                bytes = bytes.checked_add(
                    tokens
                        .capacity()
                        .checked_mul(std::mem::size_of::<TokenRecipe>())?,
                )?;
                for token in tokens {
                    bytes = bytes.checked_add(token.text.capacity())?;
                }
            }
        }
        bytes.checked_add(self.symbolic.as_ref().map_or(Some(0), |t| t.heap_bytes())?)
    }
}
