//! Actual trait-declaration identity through the ordinary retained parser query.
//! This is a source header receipt, never a trait goal or catalog-completeness proof.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    syntax::{
        ast_adapter::tokens,
        outline::{HeaderRecipe, ScopeKey},
        parse::ParseKey,
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TraitDeclarationResult {
    Trait,
    MetadataPending,
    /// A source Type name has no actual Trait at its declaration location.
    /// This does not establish a failed trait goal or reject guest execution.
    NotTrait,
}
#[derive(Debug)]
struct Metadata {
    source: Arc<InputValue>,
    header: Arc<InputValue>,
    context: Arc<InputValue>,
}
#[derive(Debug)]
pub struct TraitDeclaration {
    definition: Digest,
    metadata: Option<Metadata>,
    parsed: Option<Arc<QueryValue>>,
    result: TraitDeclarationResult,
}
impl TraitDeclaration {
    pub fn definition(&self) -> Digest {
        self.definition
    }
    pub fn result(&self) -> TraitDeclarationResult {
        self.result
    }
    pub fn is_trait(&self) -> bool {
        self.result == TraitDeclarationResult::Trait
    }
    pub(crate) fn matches_owner(
        &self,
        definition: Digest,
        parse: &ParseKey,
        header: &HeaderRecipe,
        scope: &ScopeKey,
    ) -> bool {
        let Some(metadata) = &self.metadata else {
            return false;
        };
        matches!((metadata.source.as_ref(), metadata.header.as_ref(), metadata.context.as_ref()),
            (InputValue::DefinitionSource(actual_parse), InputValue::DefinitionHeader(actual_header), InputValue::DeclarationContext(context))
                if self.is_trait() && self.definition == definition && actual_parse == parse
                    && actual_header == header && context.scope == *scope)
    }
    pub(crate) fn matches_source(&self, parse: &ParseKey, scope: &ScopeKey) -> bool {
        let Some(metadata) = &self.metadata else {
            return false;
        };
        matches!((metadata.source.as_ref(), metadata.context.as_ref()),
            (InputValue::DefinitionSource(actual), InputValue::DeclarationContext(context))
                if self.is_trait() && actual == parse && context.scope == *scope)
    }
    pub(crate) fn prepare(db: &mut Database, definition: Digest) -> QueryPoll<Self> {
        let mut prepared = Self {
            definition,
            metadata: None,
            parsed: None,
            result: TraitDeclarationResult::MetadataPending,
        };
        let source = db.input(&InputKey::DefinitionSource(definition));
        let header = db.input(&InputKey::DefinitionHeader(definition));
        let context = db.input(&InputKey::DeclarationContext(definition));
        let (Some(source), Some(header), Some(context)) = (source, header, context) else {
            return QueryPoll::Ready(Ok(prepared));
        };
        let (
            InputValue::DefinitionSource(parse),
            InputValue::DefinitionHeader(header_data),
            InputValue::DeclarationContext(context_data),
        ) = (source.as_ref(), header.as_ref(), context.as_ref())
        else {
            return QueryPoll::Ready(Err(fault("trait declaration input variants differ")));
        };
        if parse.profile != header_data.profile {
            return QueryPoll::Ready(Err(fault("trait declaration source/profile differs")));
        }
        let parsed_owner = match db.fetch(&SyntaxProvider, QueryKey::ParseFile(parse.clone())) {
            QueryPoll::Ready(Ok(value)) => value.value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(parsed) = parsed_owner.as_ref() else {
            return QueryPoll::Ready(Err(fault("trait declaration parser query differs")));
        };
        let node =
            match parsed.syntax_at(context_data.range, |node| ast::Item::can_cast(node.kind())) {
                Ok(node) => node,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let Some(node) = node else {
            return QueryPoll::Ready(Err(fault("actual declaration syntax absent")));
        };
        prepared.result = if let Some(declaration) = ast::Trait::cast(node) {
            let excluded = declaration
                .assoc_item_list()
                .map(|list| list.syntax().text_range());
            if tokens(declaration.syntax(), excluded) != header_data.tokens {
                return QueryPoll::Ready(Err(fault("actual trait declaration header differs")));
            }
            TraitDeclarationResult::Trait
        } else {
            TraitDeclarationResult::NotTrait
        };
        prepared.metadata = Some(Metadata {
            source,
            header,
            context,
        });
        prepared.parsed = Some(parsed_owner);
        QueryPoll::Ready(Ok(prepared))
    }
}
/// The one memoized receipt consumed by the common source trait resolver.
/// False means pending trait identity, never a negative semantic trait result.
pub(crate) fn is_trait(db: &mut Database, definition: Digest) -> QueryPoll<bool> {
    match db.fetch(&SyntaxProvider, QueryKey::TraitDeclaration(definition)) {
        QueryPoll::Ready(Ok(value)) => match value.value.as_ref() {
            QueryValue::TraitDeclaration(declaration) if declaration.definition == definition => {
                QueryPoll::Ready(Ok(declaration.is_trait()))
            }
            _ => QueryPoll::Ready(Err(fault("trait declaration result differs"))),
        },
        QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-TRAIT-DECLARATION", message.into())
}
impl StableEncode for TraitDeclaration {
    const DOMAIN: &'static str = "lrr/v1/actual-trait-declaration";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.u32(match self.result {
            TraitDeclarationResult::Trait => 0,
            TraitDeclarationResult::MetadataPending => 1,
            TraitDeclarationResult::NotTrait => 2,
        });
        e.option(&self.metadata, |e, metadata| {
            e.digest(metadata.source.fingerprint());
            e.digest(metadata.header.fingerprint());
            e.digest(metadata.context.fingerprint());
        });
        e.option(&self.parsed, |e, parsed| {
            let QueryValue::ParseFile(parsed) = parsed.as_ref() else {
                unreachable!("private parser owner")
            };
            e.record(&parsed.key());
        });
    }
}
