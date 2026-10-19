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
        ast_adapter::{TokenRecipe, tokens},
        outline::{HeaderRecipe, ScopeKey},
        parse::ParseKey,
        signatures::{DefinitionParameter, GenericKind},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasAttrs, HasGenericParams, HasTypeBounds},
};
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
/// Header facts captured once from the selected ordinary trait declaration.
/// Written parameters exclude the implicit upstream Self slot. Bounds remain
/// source syntax: these facts do not establish applicability or resolve names.
#[derive(Debug)]
pub struct TraitHeader {
    generics: Vec<DefinitionParameter>,
    generic_ranges: Vec<SourceRange>,
    supertraits: Vec<TokenRecipe>,
    supertraits_range: Option<SourceRange>,
    predicates: Vec<TokenRecipe>,
    predicates_range: Option<SourceRange>,
    attributes: Vec<Vec<TokenRecipe>>,
    attribute_ranges: Vec<SourceRange>,
    auto_trait: bool,
    unsafe_trait: bool,
    alias: bool,
}
impl TraitHeader {
    pub fn generics(&self) -> &[DefinitionParameter] {
        &self.generics
    }
    /// Positions in the retained owning parse, preserving token hygiene/edition
    /// for subsequent source-name demands. These are not reconstructed text.
    pub fn generic_ranges(&self) -> &[SourceRange] {
        &self.generic_ranges
    }
    pub fn supertraits(&self) -> &[TokenRecipe] {
        &self.supertraits
    }
    pub fn supertraits_range(&self) -> Option<SourceRange> {
        self.supertraits_range
    }
    pub fn predicates(&self) -> &[TokenRecipe] {
        &self.predicates
    }
    pub fn predicates_range(&self) -> Option<SourceRange> {
        self.predicates_range
    }
    pub fn attributes(&self) -> &[Vec<TokenRecipe>] {
        &self.attributes
    }
    pub fn attribute_ranges(&self) -> &[SourceRange] {
        &self.attribute_ranges
    }
    pub fn is_auto(&self) -> bool {
        self.auto_trait
    }
    pub fn is_unsafe(&self) -> bool {
        self.unsafe_trait
    }
    pub fn is_alias(&self) -> bool {
        self.alias
    }
}
impl StableEncode for TraitHeader {
    const DOMAIN: &'static str = "lrr/v1/actual-trait-header";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.sequence(&self.generics, |e, p| {
            e.u32(p.ordinal);
            e.u32(match p.kind {
                GenericKind::Lifetime => 1,
                GenericKind::Type => 2,
                GenericKind::Const => 3,
            });
            e.string(&p.name);
            e.sequence(&p.bounds, |e, t| e.record(t));
            e.sequence(&p.default, |e, t| e.record(t));
            e.sequence(&p.const_type, |e, t| e.record(t));
        });
        e.sequence(&self.supertraits, |e, t| e.record(t));
        e.sequence(&self.predicates, |e, t| e.record(t));
        e.sequence(&self.attributes, |e, a| e.sequence(a, |e, t| e.record(t)));
        let encode_range = |e: &mut StableEncoder, r: &SourceRange| {
            e.u32(r.start);
            e.u32(r.end);
        };
        e.sequence(&self.generic_ranges, encode_range);
        e.option(&self.supertraits_range, encode_range);
        e.option(&self.predicates_range, encode_range);
        e.sequence(&self.attribute_ranges, encode_range);
        e.bool(self.auto_trait);
        e.bool(self.unsafe_trait);
        e.bool(self.alias);
    }
}
#[derive(Debug)]
pub struct TraitDeclaration {
    definition: Digest,
    metadata: Option<Metadata>,
    parsed: Option<Arc<QueryValue>>,
    result: TraitDeclarationResult,
    header: Option<TraitHeader>,
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
    /// None is pending metadata or a non-trait, never an empty source catalog.
    pub fn header(&self) -> Option<&TraitHeader> {
        self.header.as_ref()
    }
    pub(crate) fn source(&self) -> Option<&ParseKey> {
        match self.metadata.as_ref()?.source.as_ref() {
            InputValue::DefinitionSource(parse) => Some(parse),
            _ => unreachable!("private checked source metadata"),
        }
    }
    pub(crate) fn scope(&self) -> Option<&ScopeKey> {
        match self.metadata.as_ref()?.context.as_ref() {
            InputValue::DeclarationContext(context) => Some(&context.scope),
            _ => unreachable!("private checked declaration metadata"),
        }
    }
    pub(crate) fn parsed(&self) -> Option<&crate::syntax::parse::ParsedFile> {
        match self.parsed.as_deref()? {
            QueryValue::ParseFile(parsed) => Some(parsed),
            _ => unreachable!("private checked parser owner"),
        }
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
            header: None,
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
            let (generics, _) = match parsed.declaration_generics(declaration.syntax()) {
                Ok(value) => value,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let supertraits = match parsed.type_tokens(
                declaration
                    .type_bound_list()
                    .map(|list| list.syntax().clone()),
            ) {
                Ok(value) => value,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let predicates = match parsed.declaration_predicates(declaration.syntax()) {
                Ok(value) => value,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            prepared.header = Some(TraitHeader {
                generics,
                generic_ranges: declaration
                    .generic_param_list()
                    .into_iter()
                    .flat_map(|list| list.generic_params())
                    .map(|p| crate::syntax::ast_adapter::range(p.syntax().text_range()))
                    .collect(),
                supertraits,
                supertraits_range: declaration
                    .type_bound_list()
                    .map(|list| crate::syntax::ast_adapter::range(list.syntax().text_range())),
                predicates,
                predicates_range: declaration
                    .where_clause()
                    .map(|clause| crate::syntax::ast_adapter::range(clause.syntax().text_range())),
                attributes: declaration
                    .attrs()
                    .map(|attr| tokens(attr.syntax(), None))
                    .collect(),
                attribute_ranges: declaration
                    .attrs()
                    .map(|attr| crate::syntax::ast_adapter::range(attr.syntax().text_range()))
                    .collect(),
                auto_trait: declaration.auto_token().is_some(),
                unsafe_trait: declaration.unsafe_token().is_some(),
                alias: declaration.eq_token().is_some(),
            });
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
    const DOMAIN: &'static str = "lrr/v2/actual-trait-declaration";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.definition);
        e.u32(match self.result {
            TraitDeclarationResult::Trait => 0,
            TraitDeclarationResult::MetadataPending => 1,
            TraitDeclarationResult::NotTrait => 2,
        });
        e.option(&self.header, |e, header| e.record(header));
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
