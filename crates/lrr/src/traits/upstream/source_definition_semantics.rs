//! Demanded source-proven scalar queries, not an eager complete fact factory.
//! Missing compiler producers remain typed dependencies of the actual query.
use super::source_definition_headers::{
    DefinitionHeaderKey, DefinitionHeaderRole as Role, PreparedDefinitionHeader, SourceHeaderParent,
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
    frontend::impl_trait_identity::ImplTraitIdentityResult,
    traits::predicate::SourcePredicateRecipe,
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

pub type SourceDefinitionSemanticsKey = DefinitionHeaderKey;
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum SourceSemantic<T> {
    Known(T),
    /// The pinned compiler's query is invalid for this actual definition role.
    NotApplicable,
    Pending(DefinitionSemanticPending),
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum DefinitionSemanticPending {
    Header,
    Parent,
    ParentIdentity,
    Constness,
    TraitItemIdentity,
    TargetFeatureAttributes,
    ClauseElaboration,
    ConstConditions,
    OpaqueTypeWalk,
    NestedBodyIdentities,
    AliasArgumentLayouts,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum DefinitionBooleanQuery {
    RequiresSizedSelf,
    FnIsConst,
    ClosureIsConst,
    AliasHasConstConditions,
    IsConditionallyConst,
    HasTargetFeatures,
    IsImplTraitInTrait,
    HasItemDefinition,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
pub(crate) enum SourceAliasDomain {
    FreeType,
    FreeConst,
    ProjectionType,
    ProjectionConst,
    InherentType,
    InherentConst,
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
enum SourceConstness {
    NotConst,
    Conditional,
    Always,
}

#[derive(Debug)]
pub struct PreparedSourceDefinitionSemantics {
    key: SourceDefinitionSemanticsKey,
    header: Arc<QueryValue>,
    parent_root: Option<Arc<QueryValue>>,
    trait_root: Option<Arc<QueryValue>>,
    booleans: [SourceSemantic<bool>; 8],
    parent: SourceSemantic<Digest>,
    constness: SourceSemantic<SourceConstness>,
    opaque_domain: SourceSemantic<()>,
    alias_domain: SourceSemantic<SourceAliasDomain>,
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
impl PreparedSourceDefinitionSemantics {
    pub(crate) fn key(&self) -> &SourceDefinitionSemanticsKey {
        &self.key
    }
    pub(crate) fn header(&self) -> &PreparedDefinitionHeader {
        let QueryValue::DefinitionHeaderPlan(header) = self.header.as_ref() else {
            unreachable!("captured semantic header")
        };
        header
    }
    pub(crate) fn header_root(&self) -> &Arc<QueryValue> {
        &self.header
    }
    pub(crate) fn boolean(&self, query: DefinitionBooleanQuery) -> SourceSemantic<bool> {
        self.booleans[query as usize]
    }
    pub(crate) fn parent(&self) -> SourceSemantic<Digest> {
        self.parent
    }
    pub(crate) fn alias_domain(&self) -> SourceSemantic<SourceAliasDomain> {
        self.alias_domain
    }
    /// Actual source alias role is not the compiler's instantiated argument layout.
    pub(crate) fn alias_argument_layouts(&self) -> SourceSemantic<()> {
        match self.alias_domain {
            SourceSemantic::Known(_) => {
                SourceSemantic::Pending(DefinitionSemanticPending::AliasArgumentLayouts)
            }
            SourceSemantic::NotApplicable => SourceSemantic::NotApplicable,
            SourceSemantic::Pending(why) => SourceSemantic::Pending(why),
        }
    }
    pub(crate) fn const_conditions(&self) -> SourceSemantic<&[SourcePredicateRecipe]> {
        match self.boolean(DefinitionBooleanQuery::IsConditionallyConst) {
            SourceSemantic::Known(true) => {
                SourceSemantic::Pending(DefinitionSemanticPending::ConstConditions)
            }
            SourceSemantic::Known(false) | SourceSemantic::NotApplicable => {
                SourceSemantic::NotApplicable
            }
            SourceSemantic::Pending(why) => SourceSemantic::Pending(why),
        }
    }
    pub(crate) fn opaque_types_defined_by(&self) -> SourceSemantic<&[Digest]> {
        match self.opaque_domain {
            SourceSemantic::Known(()) => {
                SourceSemantic::Pending(DefinitionSemanticPending::OpaqueTypeWalk)
            }
            SourceSemantic::NotApplicable => SourceSemantic::NotApplicable,
            SourceSemantic::Pending(why) => SourceSemantic::Pending(why),
        }
    }
    pub(crate) fn opaque_types_and_coroutines_defined_by(&self) -> SourceSemantic<&[Digest]> {
        match self.opaque_domain {
            SourceSemantic::Known(()) => {
                SourceSemantic::Pending(DefinitionSemanticPending::NestedBodyIdentities)
            }
            SourceSemantic::NotApplicable => SourceSemantic::NotApplicable,
            SourceSemantic::Pending(why) => SourceSemantic::Pending(why),
        }
    }
    pub(crate) fn prepare(
        db: &mut Database,
        key: &SourceDefinitionSemanticsKey,
    ) -> QueryPoll<Self> {
        use DefinitionBooleanQuery as Q;
        use DefinitionSemanticPending as P;
        use SourceSemantic::{Known, NotApplicable, Pending};
        let header = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(key.clone()))
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            header,
            parent_root: None,
            trait_root: None,
            booleans: [Pending(P::Header); 8],
            parent: Pending(P::Header),
            constness: Pending(P::Header),
            opaque_domain: Pending(P::Header),
            alias_domain: Pending(P::Header),
        };
        if result.header().pending().is_some() {
            return QueryPoll::Ready(Ok(result));
        }
        let role = result.header().role();
        let source_parent = result.header().source_parent().clone();
        // A lexical ScopeKey is not an authenticated compiler module DefId.
        result.parent = match source_parent {
            SourceHeaderParent::Associated(parent) => Known(parent),
            SourceHeaderParent::Lexical(_) | SourceHeaderParent::Foreign(_) => {
                Pending(P::ParentIdentity)
            }
            SourceHeaderParent::Pending => Pending(P::Parent),
        };
        let Some((_, _, context, parsed)) = result.header().declaration().source_parts() else {
            return QueryPoll::Ready(Err(fault("semantic declaration source absent")));
        };
        let node = match parsed.syntax_at(context.range, |node| ast::Item::can_cast(node.kind())) {
            Ok(Some(node)) => node,
            Ok(None) => return QueryPoll::Ready(Err(fault("semantic declaration syntax absent"))),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        // Independent header/generic metadata only; never parent predicates/body.
        if let SourceHeaderParent::Associated(parent) = source_parent {
            result.parent_root = Some(
                ready!(db.fetch(
                    &SyntaxProvider,
                    QueryKey::SourceDefinitionSemantics(Box::new(DefinitionHeaderKey {
                        inventory: key.inventory.clone(),
                        definition: parent,
                    }))
                ))
                .value,
            );
        }
        let parent = match result.parent_root.as_deref() {
            Some(QueryValue::SourceDefinitionSemantics(parent)) => Some(parent),
            None => None,
            _ => return QueryPoll::Ready(Err(fault("semantic parent query differs"))),
        };
        let parent_role = parent.map(|parent| parent.header().role());
        let parent_membership = parent.and_then(|parent| parent.header().impl_membership());
        let parent_ready = parent.is_none_or(|parent| parent.header().pending().is_none());
        let attributes = result.header().attributes();
        let comptime = attributes
            .iter()
            .any(|attribute| attribute.name == "rustc_comptime");
        let written_const = node
            .children_with_tokens()
            .filter_map(|element| element.into_token())
            .any(|token| token.text() == "const");
        let written_constness = if comptime {
            SourceConstness::Always
        } else if written_const {
            SourceConstness::Conditional
        } else {
            SourceConstness::NotConst
        };
        let constness = match role {
            Role::Trait | Role::Impl => Known(written_constness),
            Role::Function => match &source_parent {
                SourceHeaderParent::Foreign(_) => Known(SourceConstness::NotConst),
                SourceHeaderParent::Associated(_) if !parent_ready => Pending(P::Parent),
                SourceHeaderParent::Associated(_) => match parent_role {
                    Some(Role::Trait)
                        if attributes
                            .iter()
                            .any(|a| a.name == "rustc_non_const_trait_method") =>
                    {
                        Known(SourceConstness::NotConst)
                    }
                    Some(Role::Trait) => parent.unwrap().constness,
                    Some(Role::Impl) => match parent_membership {
                        Some(ImplTraitIdentityResult::Trait(_)) => parent.unwrap().constness,
                        Some(ImplTraitIdentityResult::Inherent)
                            if written_constness == SourceConstness::NotConst =>
                        {
                            parent.unwrap().constness
                        }
                        Some(ImplTraitIdentityResult::Inherent) => Known(written_constness),
                        _ => Pending(P::Parent),
                    },
                    _ => Pending(P::Parent),
                },
                SourceHeaderParent::Lexical(_) => Known(written_constness),
                SourceHeaderParent::Pending => Pending(P::Parent),
            },
            _ => NotApplicable,
        };
        result.constness = constness;
        result.booleans[Q::RequiresSizedSelf as usize] = Pending(P::ClauseElaboration);
        result.booleans[Q::FnIsConst as usize] = if role == Role::Function {
            map(constness, |value| value != SourceConstness::NotConst)
        } else {
            Known(false)
        };
        // These are source item definitions, never anonymous closure definitions.
        result.booleans[Q::ClosureIsConst as usize] = NotApplicable;
        result.booleans[Q::IsImplTraitInTrait as usize] = Known(false);
        let mut conditional = match role {
            Role::Trait => map(constness, |value| value != SourceConstness::NotConst),
            Role::Impl => match result.header().impl_membership() {
                Some(ImplTraitIdentityResult::Inherent) => {
                    map(constness, |value| value == SourceConstness::Conditional)
                }
                Some(ImplTraitIdentityResult::Trait(_))
                    if constness == Known(SourceConstness::NotConst)
                        || constness == Known(SourceConstness::Always) =>
                {
                    Known(false)
                }
                Some(ImplTraitIdentityResult::Trait(_)) => Pending(P::Constness),
                _ => Pending(P::Parent),
            },
            Role::TypeAlias if matches!(source_parent, SourceHeaderParent::Associated(_)) => {
                if !parent_ready {
                    Pending(P::Parent)
                } else {
                    match parent_role {
                        Some(Role::Trait) => parent.unwrap().boolean(Q::IsConditionallyConst),
                        Some(Role::Impl) => match parent_membership {
                            Some(ImplTraitIdentityResult::Inherent) => Known(false),
                            Some(ImplTraitIdentityResult::Trait(_)) => {
                                parent.unwrap().boolean(Q::IsConditionallyConst)
                            }
                            _ => Pending(P::Parent),
                        },
                        _ => Pending(P::Parent),
                    }
                }
            }
            Role::Function => match &source_parent {
                SourceHeaderParent::Associated(_) if !parent_ready => Pending(P::Parent),
                SourceHeaderParent::Associated(_) => match parent_role {
                    Some(Role::Trait) => and(
                        map(constness, |value| value == SourceConstness::Conditional),
                        parent.unwrap().boolean(Q::IsConditionallyConst),
                    ),
                    Some(Role::Impl) => match parent_membership {
                        Some(ImplTraitIdentityResult::Inherent) => {
                            map(constness, |value| value == SourceConstness::Conditional)
                        }
                        Some(ImplTraitIdentityResult::Trait(_)) => and(
                            Pending(P::TraitItemIdentity),
                            parent.unwrap().boolean(Q::IsConditionallyConst),
                        ),
                        _ => Pending(P::Parent),
                    },
                    _ => Pending(P::Parent),
                },
                _ => map(constness, |value| value == SourceConstness::Conditional),
            },
            _ => Known(false),
        };
        // Const trait-impl conditionality consults the actual implemented trait.
        if role == Role::Impl
            && constness == Known(SourceConstness::Conditional)
            && let Some(ImplTraitIdentityResult::Trait(trait_definition)) =
                result.header().impl_membership()
        {
            let root = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceDefinitionSemantics(Box::new(DefinitionHeaderKey {
                    inventory: key.inventory.clone(),
                    definition: trait_definition,
                }))
            ))
            .value;
            let QueryValue::SourceDefinitionSemantics(trait_) = root.as_ref() else {
                return QueryPoll::Ready(Err(fault("semantic implemented trait query differs")));
            };
            conditional = trait_.boolean(Q::IsConditionallyConst);
            result.trait_root = Some(root);
        }
        result.booleans[Q::IsConditionallyConst as usize] = conditional;
        result.booleans[Q::AliasHasConstConditions as usize] = if role == Role::TypeAlias
            && matches!(source_parent, SourceHeaderParent::Associated(_))
        {
            conditional
        } else {
            NotApplicable
        };
        result.booleans[Q::HasTargetFeatures as usize] =
            if matches!(role, Role::Function | Role::Static) {
                if result
                    .header()
                    .attributes()
                    .iter()
                    .any(|a| a.name == "target_feature")
                {
                    Pending(P::TargetFeatureAttributes)
                } else {
                    Known(false)
                }
            } else {
                NotApplicable
            };
        result.booleans[Q::HasItemDefinition as usize] = if !parent_ready {
            Pending(P::Parent)
        } else {
            match parent_role {
                Some(Role::Trait) => match role {
                    Role::TypeAlias => Known(
                        ast::TypeAlias::cast(node.clone())
                            .is_some_and(|alias| alias.ty().is_some()),
                    ),
                    Role::Constant => Known(result.header().initializer_equals() == Some(true)),
                    Role::Function => Known(
                        result
                            .header()
                            .source_function()
                            .is_some_and(|function| function.body.is_some()),
                    ),
                    _ => NotApplicable,
                },
                Some(Role::Impl)
                    if matches!(role, Role::Function | Role::Constant | Role::TypeAlias) =>
                {
                    match parent_membership {
                        Some(ImplTraitIdentityResult::Trait(_)) => Known(true),
                        Some(ImplTraitIdentityResult::Inherent) => NotApplicable,
                        _ => Pending(P::Parent),
                    }
                }
                None if role == Role::Impl => match result.header().impl_membership() {
                    Some(ImplTraitIdentityResult::Trait(_)) => Known(true),
                    Some(ImplTraitIdentityResult::Inherent) => NotApplicable,
                    _ => Pending(P::Parent),
                },
                _ => NotApplicable,
            }
        };
        result.opaque_domain = if matches!(
            role,
            Role::Function | Role::Static | Role::Constant | Role::TypeAlias
        ) {
            Known(()) // applicability only; neither list is declared empty
        } else {
            NotApplicable
        };
        result.alias_domain = match (role, parent_role) {
            (Role::TypeAlias, None) => Known(SourceAliasDomain::FreeType),
            (Role::Constant, None) => Known(SourceAliasDomain::FreeConst),
            (Role::TypeAlias, Some(Role::Trait)) => Known(SourceAliasDomain::ProjectionType),
            (Role::Constant, Some(Role::Trait)) => Known(SourceAliasDomain::ProjectionConst),
            (Role::TypeAlias | Role::Constant, Some(Role::Impl)) => match parent_membership {
                Some(ImplTraitIdentityResult::Inherent) => Known(if role == Role::TypeAlias {
                    SourceAliasDomain::InherentType
                } else {
                    SourceAliasDomain::InherentConst
                }),
                Some(ImplTraitIdentityResult::Trait(_)) => NotApplicable,
                _ => Pending(P::Parent),
            },
            _ => NotApplicable,
        };
        if !parent_ready && matches!(source_parent, SourceHeaderParent::Associated(_)) {
            result.alias_domain = Pending(P::Parent);
        }
        QueryPoll::Ready(Ok(result))
    }
}
fn map<T>(value: SourceSemantic<T>, f: impl FnOnce(T) -> bool) -> SourceSemantic<bool> {
    match value {
        SourceSemantic::Known(value) => SourceSemantic::Known(f(value)),
        SourceSemantic::NotApplicable => SourceSemantic::NotApplicable,
        SourceSemantic::Pending(why) => SourceSemantic::Pending(why),
    }
}
fn and(a: SourceSemantic<bool>, b: SourceSemantic<bool>) -> SourceSemantic<bool> {
    use SourceSemantic::*;
    match (a, b) {
        (Known(false), _) | (_, Known(false)) => Known(false),
        (Known(true), value) | (value, Known(true)) => value,
        (Pending(why), _) | (_, Pending(why)) => Pending(why),
        _ => NotApplicable,
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-SOURCE-DEFINITION-SEMANTICS", message.into())
}
impl StableEncode for PreparedSourceDefinitionSemantics {
    const DOMAIN: &'static str = "lrr/v1/source-definition-semantics";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.header());
        for root in [&self.parent_root, &self.trait_root] {
            e.option(root, |e, root| {
                let QueryValue::SourceDefinitionSemantics(parent) = root.as_ref() else {
                    unreachable!("semantic parent root")
                };
                e.record(parent.as_ref());
            });
        }
        fn encode<T>(
            e: &mut StableEncoder,
            value: SourceSemantic<T>,
            known: impl FnOnce(&mut StableEncoder, T),
        ) {
            match value {
                SourceSemantic::Known(value) => {
                    e.u32(0);
                    known(e, value);
                }
                SourceSemantic::NotApplicable => e.u32(1),
                SourceSemantic::Pending(why) => {
                    e.u32(2);
                    e.u32(why as u32);
                }
            }
        }
        for value in self.booleans {
            encode(e, value, |e, value| e.bool(value));
        }
        encode(e, self.parent, |e, value| e.digest(value));
        encode(e, self.constness, |e, value| e.u32(value as u32));
        encode(e, self.opaque_domain, |_, ()| {});
        encode(e, self.alias_domain, |e, value| e.u32(value as u32));
    }
}
pub(crate) struct CurrentSourceDefinitionSemantics {
    value: Arc<QueryValue>,
    guard: PreparedInputGuard,
}
impl CurrentSourceDefinitionSemantics {
    pub(crate) fn capture(
        db: &mut Database,
        key: &SourceDefinitionSemanticsKey,
    ) -> QueryPoll<Self> {
        let query = QueryKey::SourceDefinitionSemantics(Box::new(key.clone()));
        let outcome = ready!(db.fetch(&SyntaxProvider, query.clone()));
        if !outcome.cacheable
            || !matches!(outcome.value.as_ref(), QueryValue::SourceDefinitionSemantics(plan) if plan.key()==key)
        {
            return QueryPoll::Ready(Err(fault("semantic source memo differs")));
        }
        let guard = ready!(db.prepare_input_guard(&SyntaxProvider, &query, outcome.fingerprint));
        QueryPoll::Ready(Ok(Self {
            value: outcome.value,
            guard,
        }))
    }
    pub(crate) fn plan(&self) -> &PreparedSourceDefinitionSemantics {
        let QueryValue::SourceDefinitionSemantics(plan) = self.value.as_ref() else {
            unreachable!("captured semantic source query")
        };
        plan
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        self.guard.require_current(db)
    }
}
