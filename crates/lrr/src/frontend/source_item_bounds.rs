//! Actual trait-associated output syntax and parent where-bound candidates.
//! This is source preparation, not compiler item_bounds, elaboration or solving.
use super::{
    declaration_source_predicates::{
        DeclarationSourcePredicatesKey, DeclarationSourcePredicatesPending,
        PreparedDeclarationSourcePredicates,
    },
    source_types::WrittenTypeDeclaration,
    trait_associated_types::{TraitAssociatedTypePending, TraitAssociatedTypesKey},
    type_name_namespace::SourceTypeOwner,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey,
        memo::{Database, PreparedInputGuard},
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    syntax::{ast_adapter::range, signatures::GenericKind, spans::SourceRange},
    traits::{
        predicate::{PredicatePending, PredicateResult, SourcePredicateRecipe},
        term::{ConstTerm, GenericArg, ParamKey, ProjectionKey, RegionTerm, TraitRef, TyTerm},
        upstream::{
            source_definition_headers::DefinitionHeaderRole,
            source_lang_identity::SourceLangIdentityKey,
        },
    },
};
use ra_ap_syntax::{
    AstNode,
    ast::{self, HasName},
};
use std::{collections::BTreeMap, sync::Arc};

pub type SourceItemBoundsKey = DeclarationSourcePredicatesKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SourceItemBoundsPending {
    Declaration(DeclarationSourcePredicatesPending),
    NotTraitAssociatedType,
    Member(TraitAssociatedTypePending),
    GenericLayout,
    ParentSource,
    Environment,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
#[repr(u32)]
pub enum SizednessRole {
    Sized = 0,
    MetaSized = 1,
    PointeeSized = 2,
}
impl SizednessRole {
    fn source_name(self) -> &'static str {
        match self {
            Self::Sized => "Sized",
            Self::MetaSized => "MetaSized",
            Self::PointeeSized => "PointeeSized",
        }
    }
    fn compiler_name(self) -> &'static str {
        match self {
            Self::Sized => "sized",
            Self::MetaSized => "meta_sized",
            Self::PointeeSized => "pointee_sized",
        }
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ItemBoundsDefaultDependency {
    /// Real RustcNoImplicitBounds/default-trait option inputs are not exposed.
    Configuration,
    /// Missing selected source role is a dependency, never complete absence.
    LanguageIdentity(SizednessRole),
}
/// Source association only. Even a direct target is not a completed compiler
/// bound: PredicateFilter/elaboration/defaults remain separate producers.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ParentItemBoundAssociation {
    Target { nested_projection_depth: u32 },
    GatRemappingRequired { nested_projection_depth: u32 },
    OtherSubject,
    Pending(PredicatePending),
}
#[derive(Debug)]
pub struct ParentItemBoundCandidate {
    pub origin: Digest,
    pub recipe: SourcePredicateRecipe,
    pub association: ParentItemBoundAssociation,
}
#[derive(Debug)]
pub struct PreparedSourceItemBounds {
    key: SourceItemBoundsKey,
    source: Arc<QueryValue>,
    members: Option<Arc<QueryValue>>,
    environment: Option<Arc<QueryValue>>,
    languages: Vec<Arc<QueryValue>>,
    subject: Option<TyTerm>,
    site: Option<(Digest, SourceRange)>,
    written: Vec<SourcePredicateRecipe>,
    parent_candidates: Vec<ParentItemBoundCandidate>,
    source_pending: Option<SourceItemBoundsPending>,
    parent_pending: Option<SourceItemBoundsPending>,
    default_dependencies: Vec<ItemBoundsDefaultDependency>,
    sizedness_identities: BTreeMap<SizednessRole, Digest>,
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}
fn argument(kind: GenericKind, key: ParamKey) -> GenericArg {
    match kind {
        GenericKind::Type => GenericArg::Type(TyTerm::Param(key)),
        GenericKind::Lifetime => GenericArg::Region(RegionTerm::Param(key)),
        GenericKind::Const => GenericArg::Const(ConstTerm::Param(key)),
    }
}
impl PreparedSourceItemBounds {
    pub(crate) fn key(&self) -> &SourceItemBoundsKey {
        &self.key
    }
    pub(crate) fn source(&self) -> &PreparedDeclarationSourcePredicates {
        let QueryValue::DeclarationSourcePredicates(source) = self.source.as_ref() else {
            unreachable!("actual item-bound declaration")
        };
        source
    }
    pub(crate) fn source_pending(&self) -> Option<SourceItemBoundsPending> {
        self.source_pending
    }
    pub(crate) fn projection_subject(&self) -> Option<&TyTerm> {
        self.subject.as_ref()
    }
    pub(crate) fn output_site(&self) -> Option<(Digest, SourceRange)> {
        self.site
    }
    /// Written output rows are retained independently of default/parent holds.
    /// Nested Pending recipes are source requirements, never successful bounds.
    pub(crate) fn written_recipes(
        &self,
    ) -> Result<&[SourcePredicateRecipe], SourceItemBoundsPending> {
        self.source_pending.map_or(Ok(&self.written), Err)
    }
    pub(crate) fn parent_candidates(
        &self,
    ) -> Result<&[ParentItemBoundCandidate], SourceItemBoundsPending> {
        self.source_pending
            .or(self.parent_pending)
            .map_or(Ok(&self.parent_candidates), Err)
    }
    pub(crate) fn default_dependencies(&self) -> &[ItemBoundsDefaultDependency] {
        &self.default_dependencies
    }
    /// None here denotes the corresponding LanguageIdentity dependency.
    pub(crate) fn sizedness_identity(&self, role: SizednessRole) -> Option<Digest> {
        self.sizedness_identities.get(&role).copied()
    }
    pub(crate) fn prepare(db: &mut Database, key: &SourceItemBoundsKey) -> QueryPoll<Self> {
        let source = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DeclarationSourcePredicates(Box::new(key.clone()))
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            source,
            members: None,
            environment: None,
            languages: Vec::new(),
            subject: None,
            site: None,
            written: Vec::new(),
            parent_candidates: Vec::new(),
            source_pending: None,
            parent_pending: None,
            default_dependencies: vec![ItemBoundsDefaultDependency::Configuration],
            sizedness_identities: BTreeMap::new(),
        };
        macro_rules! pending {
            ($why:expr) => {{
                result.source_pending = Some($why);
                return QueryPoll::Ready(Ok(result));
            }};
        }
        if let Some(why) = result.source().pending() {
            pending!(SourceItemBoundsPending::Declaration(why));
        }
        let source_root = Arc::clone(&result.source);
        let QueryValue::DeclarationSourcePredicates(source) = source_root.as_ref() else {
            unreachable!()
        };
        let Some(parent_header) = source.parent_header() else {
            pending!(SourceItemBoundsPending::NotTraitAssociatedType);
        };
        if parent_header.role() != DefinitionHeaderRole::Trait {
            pending!(SourceItemBoundsPending::NotTraitAssociatedType);
        }
        let Some(parent) = source.parent_definition() else {
            pending!(SourceItemBoundsPending::ParentSource);
        };
        let declaration = source.declaration();
        let Some((parse, header, context, parsed)) = declaration.source_parts() else {
            pending!(SourceItemBoundsPending::GenericLayout);
        };
        let alias =
            match parsed.syntax_at(context.range, |node| ast::TypeAlias::can_cast(node.kind())) {
                Ok(Some(node)) => ast::TypeAlias::cast(node).expect("selected actual TypeAlias"),
                Ok(None) => pending!(SourceItemBoundsPending::NotTraitAssociatedType),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let Some(name) = alias.name() else {
            pending!(SourceItemBoundsPending::GenericLayout);
        };
        let edition = match parsed.token_edition(range(name.syntax().text_range())) {
            Ok(edition) => edition,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let name = match super::names::owned_name(name.text(), edition) {
            Ok(name) => name,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let item_key = super::member_index::associated_name_key(name.as_str());
        let members = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::TraitAssociatedTypes(Box::new(TraitAssociatedTypesKey {
                inventory: key.inventory.clone(),
                trait_definition: parent
            },))
        ))
        .value;
        result.members = Some(Arc::clone(&members));
        let QueryValue::TraitAssociatedTypes(members) = members.as_ref() else {
            return QueryPoll::Ready(Err(fault("actual trait member query differs")));
        };
        match members.lookup(item_key) {
            Ok(actual) if actual == key.definition => {}
            Ok(_) => {
                return QueryPoll::Ready(Err(fault(
                    "actual associated declaration differs from source output owner",
                )));
            }
            Err(why) => pending!(SourceItemBoundsPending::Member(why)),
        }
        let Some(layout) = declaration.layout() else {
            pending!(SourceItemBoundsPending::GenericLayout);
        };
        if !layout.implicit_self {
            pending!(SourceItemBoundsPending::GenericLayout);
        }
        let Some(self_type) = source.self_type() else {
            pending!(SourceItemBoundsPending::GenericLayout);
        };
        let parent_declaration = parent_header.declaration();
        let mut rebase = BTreeMap::new();
        let mut parent_args = Vec::new();
        for parameter in parent_declaration.parameters() {
            let from = ParamKey {
                definition: parent,
                ordinal: parameter.ordinal,
            };
            let Some(to) = source.rebase_parent_parameter(from) else {
                pending!(SourceItemBoundsPending::GenericLayout);
            };
            rebase.insert(from, to);
            parent_args.push(argument(parameter.kind, to));
        }
        let parent_self = match u32::try_from(parent_declaration.parameters().len()) {
            Ok(ordinal) => ParamKey {
                definition: parent,
                ordinal,
            },
            Err(_) => return QueryPoll::Ready(Err(fault("actual parent Self ordinal overflow"))),
        };
        let Some(child_self) = source.rebase_parent_parameter(parent_self) else {
            pending!(SourceItemBoundsPending::GenericLayout);
        };
        rebase.insert(parent_self, child_self);
        let own_args = declaration
            .parameters()
            .iter()
            .filter(|parameter| parameter.ordinal < layout.own)
            .map(|parameter| {
                argument(
                    parameter.kind,
                    ParamKey {
                        definition: key.definition,
                        ordinal: parameter.ordinal,
                    },
                )
            })
            .collect();
        let trait_ref = TraitRef {
            trait_key: parent,
            self_ty: self_type.clone(),
            args: parent_args,
        };
        // item_key remains a lookup key; the retained Q77 proves the actual ID.
        let subject = TyTerm::Projection(Box::new(ProjectionKey {
            trait_ref: trait_ref.clone(),
            item: item_key,
            item_args: own_args,
        }));
        result.subject = Some(subject.clone());
        result.site = Some((parse.content, range(alias.syntax().text_range())));
        let environment = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ))
        .value;
        result.environment = Some(Arc::clone(&environment));
        let QueryValue::PreparedScopeImplHeaders(inventory) = environment.as_ref() else {
            return QueryPoll::Ready(Err(fault("item-bound source inventory differs")));
        };
        let Some(environment) = inventory.inventory_root().environment() else {
            pending!(SourceItemBoundsPending::Environment);
        };
        let sources = environment.sources().iter().collect::<Vec<_>>();
        let syntax = alias.syntax().clone();
        let owner = SourceTypeOwner {
            definition: key.definition,
            scope: &context.scope,
            header,
        };
        let frame = WrittenTypeDeclaration::declaration(
            owner,
            declaration.parameters(),
            &syntax,
            Some(self_type),
        );
        let core = environment.language_item_core(frame.profile());
        result.written = ready!(super::source_predicates::prepare_output_bounds(
            db,
            environment.names(),
            &sources,
            &frame,
            &alias,
            &subject,
            core
        ));
        if let Some((parent_parse, parent_recipe, parent_context, parent_parsed)) =
            parent_declaration.source_parts()
        {
            match parent_parsed.syntax_at(parent_context.range, |node| {
                ast::Trait::can_cast(node.kind())
            }) {
                Ok(Some(parent_syntax)) => {
                    let parent_owner = SourceTypeOwner {
                        definition: parent,
                        scope: &parent_context.scope,
                        header: parent_recipe,
                    };
                    let parent_frame = WrittenTypeDeclaration::declaration(
                        parent_owner,
                        parent_declaration.parameters(),
                        &parent_syntax,
                        Some(self_type),
                    )
                    .rebase_parameters(&rebase);
                    let candidates = ready!(super::source_predicates::prepare_parent_where_bounds(
                        db,
                        environment.names(),
                        &sources,
                        &parent_frame,
                        &parent_syntax,
                        core
                    ));
                    for recipe in candidates {
                        let association =
                            match association(&recipe, &trait_ref, item_key, layout.own) {
                                Ok(association) => association,
                                Err(error) => return QueryPoll::Ready(Err(error)),
                            };
                        result.parent_candidates.push(ParentItemBoundCandidate {
                            origin: parent_parse.content,
                            recipe,
                            association,
                        });
                    }
                }
                Ok(None) => result.parent_pending = Some(SourceItemBoundsPending::ParentSource),
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        } else {
            result.parent_pending = Some(SourceItemBoundsPending::ParentSource);
        }
        // Role lookup is a source query only. Default selection is held until
        // genuine crate/options inputs and the full pinned policy are exposed.
        for role in [
            SizednessRole::Sized,
            SizednessRole::MetaSized,
            SizednessRole::PointeeSized,
        ] {
            let identity = ready!(super::language_items::marker_trait(
                db,
                &sources,
                frame.profile(),
                core,
                role.source_name()
            ));
            let Some(identity) = identity else {
                result
                    .default_dependencies
                    .push(ItemBoundsDefaultDependency::LanguageIdentity(role));
                continue;
            };
            let language = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceLangIdentity(Box::new(SourceLangIdentityKey {
                    inventory: key.inventory.clone(),
                    definition: identity
                },))
            ))
            .value;
            let QueryValue::SourceLangIdentity(actual) = language.as_ref() else {
                return QueryPoll::Ready(Err(fault(
                    "actual sizedness lang identity query differs",
                )));
            };
            if matches!(actual.classification(), Ok(Some((name, _))) if name == role.compiler_name())
            {
                result.sizedness_identities.insert(role, identity);
            } else {
                result
                    .default_dependencies
                    .push(ItemBoundsDefaultDependency::LanguageIdentity(role));
            }
            result.languages.push(language);
        }
        QueryPoll::Ready(Ok(result))
    }
}

fn association(
    recipe: &SourcePredicateRecipe,
    expected: &TraitRef,
    item: Digest,
    own_parameters: u32,
) -> Result<ParentItemBoundAssociation, Diagnostic> {
    let mut subject = match recipe {
        SourcePredicateRecipe::Explicit {
            result:
                PredicateResult::Implemented(reference)
                | PredicateResult::Relaxed {
                    trait_ref: reference,
                    ..
                },
            ..
        } => &reference.self_ty,
        SourcePredicateRecipe::Explicit {
            result: PredicateResult::TypeOutlives { ty, .. },
            ..
        } => ty,
        SourcePredicateRecipe::Explicit {
            result: PredicateResult::Pending(why),
            ..
        } => return Ok(ParentItemBoundAssociation::Pending(*why)),
        _ => return Ok(ParentItemBoundAssociation::OtherSubject),
    };
    let mut depth = 0u32;
    while let TyTerm::Projection(projection) = subject {
        if projection.trait_ref == *expected && projection.item == item {
            return Ok(if own_parameters == 0 && projection.item_args.is_empty() {
                ParentItemBoundAssociation::Target {
                    nested_projection_depth: depth,
                }
            } else {
                ParentItemBoundAssociation::GatRemappingRequired {
                    nested_projection_depth: depth,
                }
            });
        }
        subject = &projection.trait_ref.self_ty;
        depth = depth
            .checked_add(1)
            .ok_or_else(|| fault("actual nested projection depth overflow"))?;
    }
    Ok(ParentItemBoundAssociation::OtherSubject)
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-ITEM-BOUNDS", message.into())
}

#[derive(Debug)]
pub(crate) struct CurrentSourceItemBounds {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceItemBounds {
    pub(crate) fn capture(db: &mut Database, key: &SourceItemBoundsKey) -> QueryPoll<Self> {
        let query = QueryKey::SourceItemBounds(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(), QueryValue::SourceItemBounds(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("source item-bound memo differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceItemBounds {
        let QueryValue::SourceItemBounds(plan) = self.value.as_ref() else {
            unreachable!()
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
impl StableEncode for PreparedSourceItemBounds {
    const DOMAIN: &'static str = "lrr/v1/source-associated-item-bound-recipes";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.source());
        e.option(&self.members, |e, value| {
            let QueryValue::TraitAssociatedTypes(value) = value.as_ref() else {
                unreachable!()
            };
            e.record(value.as_ref());
        });
        e.option(&self.environment, |e, value| {
            let QueryValue::PreparedScopeImplHeaders(value) = value.as_ref() else {
                unreachable!()
            };
            e.digest(value.fingerprint());
        });
        e.sequence(&self.languages, |e, value| {
            let QueryValue::SourceLangIdentity(value) = value.as_ref() else {
                unreachable!()
            };
            e.record(value.as_ref());
        });
        e.option(&self.subject, |e, subject| e.record(subject));
        e.option(&self.site, |e, (origin, source)| {
            e.digest(*origin);
            e.u32(source.start);
            e.u32(source.end);
        });
        e.sequence(&self.written, |e, recipe| e.record(recipe));
        e.sequence(&self.parent_candidates, |e, candidate| {
            e.digest(candidate.origin);
            e.record(&candidate.recipe);
            match candidate.association {
                ParentItemBoundAssociation::Target {
                    nested_projection_depth,
                } => {
                    e.u32(0);
                    e.u32(nested_projection_depth);
                }
                ParentItemBoundAssociation::GatRemappingRequired {
                    nested_projection_depth,
                } => {
                    e.u32(1);
                    e.u32(nested_projection_depth);
                }
                ParentItemBoundAssociation::OtherSubject => e.u32(2),
                ParentItemBoundAssociation::Pending(_) => e.u32(3), // exact reason is in the retained recipe
            }
        });
        for pending in [&self.source_pending, &self.parent_pending] {
            e.option(pending, |e, why| match why {
                SourceItemBoundsPending::Declaration(_) => e.u32(0), // exact cause retained in Q78
                SourceItemBoundsPending::NotTraitAssociatedType => e.u32(1),
                SourceItemBoundsPending::Member(why) => {
                    e.u32(2);
                    e.u32(*why as u32);
                }
                SourceItemBoundsPending::GenericLayout => e.u32(3),
                SourceItemBoundsPending::ParentSource => e.u32(4),
                SourceItemBoundsPending::Environment => e.u32(5),
            });
        }
        e.sequence(&self.default_dependencies, |e, why| match why {
            ItemBoundsDefaultDependency::Configuration => e.u32(0),
            ItemBoundsDefaultDependency::LanguageIdentity(role) => {
                e.u32(1);
                e.u32(*role as u32);
            }
        });
        e.map(&self.sizedness_identities, |e, role, identity| {
            e.u32(*role as u32);
            e.digest(*identity);
        });
    }
}
