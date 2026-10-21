//! Retained source impl header syntax, before canonical Type/predicate lowering.
//! A scope inventory is not an ImplCatalogSeal or an applicability witness.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, seals::SealKey,
        value::QueryValue,
    },
    source::snapshot::SourcePath,
    syntax::{
        ast_adapter,
        outline::ScopeKey,
        parse::ParseKey,
        signatures::{DefinitionParameter, GenericKind, capture_impl_generics},
        spans::SourceRange,
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasGenericParams},
};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct ImplHeaderSyntaxKey {
    pub inventory: SealKey,
    /// Raw source attachment identity; never a canonical declaration DefKey.
    pub implementation: Digest,
}
impl StableEncode for ImplHeaderSyntaxKey {
    const DOMAIN: &'static str = "lrr/v1/impl-header-syntax-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.inventory);
        e.digest(self.implementation);
    }
}

#[derive(Debug)]
pub struct ImplHeaderSyntax {
    key: ImplHeaderSyntaxKey,
    syntax: crate::frontend::impl_index::ImplSyntaxSource,
    source: ParseKey,
    path: SourcePath,
    origin: Option<Digest>,
    scope: ScopeKey,
    header: Digest,
    owner: Option<super::header::CanonicalImplOwner>,
    self_type: SourceRange,
    trait_type: Option<SourceRange>,
    generics: Vec<DefinitionParameter>,
    predicates: Vec<SourceRange>,
    negative: bool,
    constant: bool,
    unsafe_impl: bool,
    default_impl: bool,
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-IMPL-HEADER", message.into())
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(request) => return QueryPoll::NeedInput(request),
        }
    };
}
impl ImplHeaderSyntax {
    pub fn key(&self) -> &ImplHeaderSyntaxKey {
        &self.key
    }
    pub(crate) fn parsed(&self) -> &crate::syntax::parse::ParsedFile {
        self.syntax.parsed()
    }
    pub fn source(&self) -> &ParseKey {
        &self.source
    }
    pub fn path(&self) -> &SourcePath {
        &self.path
    }
    pub fn generated_origin(&self) -> Option<Digest> {
        self.origin
    }
    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }
    pub fn canonical_owner(&self) -> Option<&super::header::CanonicalImplOwner> {
        self.owner.as_ref()
    }
    pub fn self_type(&self) -> SourceRange {
        self.self_type
    }
    pub fn trait_type(&self) -> Option<SourceRange> {
        self.trait_type
    }
    pub fn generics(&self) -> &[DefinitionParameter] {
        &self.generics
    }
    pub fn predicates(&self) -> &[SourceRange] {
        &self.predicates
    }
    pub fn modifiers(&self) -> (bool, bool, bool, bool) {
        (
            self.negative,
            self.constant,
            self.unsafe_impl,
            self.default_impl,
        )
    }

    pub(crate) fn prepare(db: &mut Database, key: &ImplHeaderSyntaxKey) -> QueryPoll<Self> {
        let inventory = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ));
        let QueryValue::PreparedScopeImplHeaders(inventory) = inventory.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("prepared impl inventory query differs")));
        };
        let Some(header) = inventory.header(key.implementation) else {
            return QueryPoll::Ready(Err(fault("impl header absent from actual scope inventory")));
        };
        let source = header.source().parse();
        let Some(syntax_source) = inventory.syntax_source(key.implementation) else {
            return QueryPoll::Ready(Err(fault(
                "impl header retained source syntax is not yet transported",
            )));
        };
        let parsed = syntax_source.parsed();
        if parsed.key() != *source {
            return QueryPoll::Ready(Err(fault("impl header owning parse differs")));
        }
        let selected = header.requirements();
        let syntax = match parsed.syntax_at(selected.range, |node| ast::Impl::can_cast(node.kind()))
        {
            Ok(Some(syntax)) => syntax,
            Ok(None) => return QueryPoll::Ready(Err(fault("actual impl syntax absent"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let implementation = ast::Impl::cast(syntax).expect("checked impl kind");
        let Some(self_type) = implementation.self_ty() else {
            return QueryPoll::Ready(Err(fault("impl header lacks its written self Type")));
        };
        let generics = match capture_impl_generics(&implementation) {
            Ok(generics) => generics,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let mut predicates = Vec::new();
        if let Some(clause) = implementation.where_clause() {
            let count = clause.predicates().count();
            if u32::try_from(count).is_err() {
                return QueryPoll::Ready(Err(fault("impl predicate ordinal overflow")));
            }
            if predicates.try_reserve_exact(count).is_err() {
                return QueryPoll::Ready(Err(fault("impl predicate allocation failed")));
            }
            predicates.extend(
                clause
                    .predicates()
                    .map(|p| ast_adapter::range(p.syntax().text_range())),
            );
        }
        QueryPoll::Ready(Ok(Self {
            key: key.clone(),
            syntax: syntax_source.clone(),
            source: source.clone(),
            path: header.source().path().clone(),
            origin: header.source().generated_origin(),
            scope: selected.scope.clone(),
            header: header.header_identity(),
            owner: inventory.canonical_owner(key.implementation).cloned(),
            self_type: ast_adapter::range(self_type.syntax().text_range()),
            trait_type: implementation
                .trait_()
                .map(|ty| ast_adapter::range(ty.syntax().text_range())),
            generics,
            predicates,
            negative: implementation.excl_token().is_some(),
            constant: implementation.const_token().is_some(),
            unsafe_impl: implementation.unsafe_token().is_some(),
            default_impl: implementation.default_token().is_some(),
        }))
    }
}
impl StableEncode for ImplHeaderSyntax {
    const DOMAIN: &'static str = "lrr/v3/impl-header-syntax";
    fn encode_fields(&self, e: &mut StableEncoder) {
        fn range(e: &mut StableEncoder, range: SourceRange) {
            e.u32(range.start);
            e.u32(range.end);
        }
        e.record(&self.key);
        e.record(&self.source);
        e.string(self.path.as_str());
        e.option(&self.origin, |e, origin| e.digest(*origin));
        e.record(&self.scope);
        e.digest(self.header);
        e.option(&self.owner, |e, owner| e.record(owner));
        range(e, self.self_type);
        e.option(&self.trait_type, |e, r| range(e, *r));
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
            e.option(&p.anonymous_region, |e, r| range(e, *r));
        });
        e.sequence(&self.predicates, |e, r| range(e, *r));
        e.bool(self.negative);
        e.bool(self.constant);
        e.bool(self.unsafe_impl);
        e.bool(self.default_impl);
    }
}
