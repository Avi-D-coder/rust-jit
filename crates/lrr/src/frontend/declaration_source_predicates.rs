//! Actual declaration own-bound recipes through the ordinary source Type walk.
//! No body preparation, inherited predicate closure, elaboration or solving.
use super::{
    declaration_generic_types::{
        DeclarationGenericTypes, DeclarationGenericTypesKey, GenericTypesPending,
    },
    declaration_generics::DeclarationGenerics,
    source_types::WrittenTypeDeclaration,
    type_name_namespace::SourceTypeOwner,
};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, seals::SealKey,
        value::QueryValue,
    },
    syntax::{ast_adapter::range, signatures::GenericKind, spans::SourceRange},
    traits::{
        predicate::SourcePredicateRecipe,
        term::{ParamKey, TyTerm},
        upstream::{
            source_default_bounds::{DefaultBoundsContext, DefaultBoundsPending},
            source_definition_headers::{
                DefinitionHeaderKey, PreparedDefinitionHeader, SourceHeaderParent,
            },
        },
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct DeclarationSourcePredicatesKey {
    pub inventory: SealKey,
    pub definition: Digest,
}
impl StableEncode for DeclarationSourcePredicatesKey {
    const DOMAIN: &'static str = "lrr/v1/declaration-source-predicates-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.inventory);
        e.digest(self.definition);
    }
}
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum DeclarationSourcePredicatesPending {
    GenericTypes(GenericTypesPending),
    Source,
    Environment,
    AssociatedContext,
    SelfType,
    UnsupportedDeclaration,
    ParentGenerics(GenericTypesPending),
    ParentLayout,
    DefaultBounds(DefaultBoundsPending),
}
#[derive(Debug)]
pub struct PreparedDeclarationSourcePredicates {
    key: DeclarationSourcePredicatesKey,
    generic_types: Arc<QueryValue>,
    environment: Arc<QueryValue>,
    source_header: Option<Arc<QueryValue>>,
    parent_header: Option<Arc<QueryValue>>,
    parent_generic_types: Option<Arc<QueryValue>>,
    default_bounds: Option<Arc<QueryValue>>,
    parent_definition: Option<Digest>,
    parameter_rebase: BTreeMap<ParamKey, ParamKey>,
    self_type: Option<TyTerm>,
    parameter_ranges: BTreeMap<u32, SourceRange>,
    recipes: Vec<SourcePredicateRecipe>,
    pending: Option<DeclarationSourcePredicatesPending>,
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
impl PreparedDeclarationSourcePredicates {
    pub fn key(&self) -> &DeclarationSourcePredicatesKey {
        &self.key
    }
    pub fn generic_types(&self) -> &DeclarationGenericTypes {
        let QueryValue::DeclarationGenericTypes(generics) = self.generic_types.as_ref() else {
            unreachable!("actual generic Type query")
        };
        generics
    }
    pub fn declaration(&self) -> &DeclarationGenerics {
        self.generic_types().declaration()
    }
    pub fn pending(&self) -> Option<DeclarationSourcePredicatesPending> {
        self.pending
    }
    pub fn parent_definition(&self) -> Option<Digest> {
        self.parent_definition
    }
    pub(crate) fn self_type(&self) -> Option<&TyTerm> {
        self.self_type.as_ref()
    }
    pub(crate) fn parameter_range(&self, key: ParamKey) -> Option<SourceRange> {
        if key.definition != self.key.definition {
            return None;
        }
        self.parameter_ranges.get(&key.ordinal).copied()
    }
    pub(crate) fn source_header_root(&self) -> Option<&Arc<QueryValue>> {
        self.source_header.as_ref()
    }
    pub(crate) fn parent_header_root(&self) -> Option<&Arc<QueryValue>> {
        self.parent_header.as_ref()
    }
    pub(crate) fn parent_header(&self) -> Option<&PreparedDefinitionHeader> {
        match self.parent_header.as_deref() {
            Some(QueryValue::DefinitionHeaderPlan(header)) => Some(header),
            None => None,
            _ => unreachable!("actual parent source header"),
        }
    }
    /// Actual parent source parameter -> represented child source parameter.
    /// This map supplies no inherited predicates or associated output bounds.
    pub(crate) fn rebase_parent_parameter(&self, key: ParamKey) -> Option<ParamKey> {
        self.parameter_rebase.get(&key).copied()
    }
    /// Recipe-level pending remains explicit in each source requirement. This
    /// slice never turns an incomplete preparation into an empty environment.
    pub fn recipes(&self) -> Result<&[SourcePredicateRecipe], DeclarationSourcePredicatesPending> {
        if let Some(why) = self.pending {
            return Err(why);
        }
        Ok(&self.recipes)
    }
    pub(crate) fn prepare(
        db: &mut Database,
        key: &DeclarationSourcePredicatesKey,
    ) -> QueryPoll<Self> {
        let generic_types = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DeclarationGenericTypes(Box::new(DeclarationGenericTypesKey {
                inventory: key.inventory.clone(),
                definition: key.definition,
            }))
        ))
        .value;
        let environment = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::PreparedScopeImplHeaders(key.inventory.clone())
        ))
        .value;
        let mut result = Self {
            key: key.clone(),
            generic_types,
            environment,
            source_header: None,
            parent_header: None,
            parent_generic_types: None,
            default_bounds: None,
            parent_definition: None,
            parameter_rebase: BTreeMap::new(),
            self_type: None,
            parameter_ranges: BTreeMap::new(),
            recipes: Vec::new(),
            pending: None,
        };
        macro_rules! pending {
            ($why:expr) => {{
                result.pending = Some($why);
                return QueryPoll::Ready(Ok(result));
            }};
        }
        if let Some(why) = result.generic_types().pending() {
            pending!(DeclarationSourcePredicatesPending::GenericTypes(why));
        }
        let generic_types_root = Arc::clone(&result.generic_types);
        let QueryValue::DeclarationGenericTypes(generic_types) = generic_types_root.as_ref() else {
            unreachable!("actual generic Type source")
        };
        let declaration = generic_types.declaration();
        let Some(layout) = declaration.layout() else {
            pending!(DeclarationSourcePredicatesPending::Source);
        };
        let Some((source, header, context, parsed)) = declaration.source_parts() else {
            pending!(DeclarationSourcePredicatesPending::Source);
        };
        let syntax = match parsed.syntax_at(context.range, |node| ast::Item::can_cast(node.kind()))
        {
            Ok(Some(node)) => node,
            Ok(None) => pending!(DeclarationSourcePredicatesPending::Source),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        if !matches!(
            ast::Item::cast(syntax.clone()),
            Some(
                ast::Item::Struct(_)
                    | ast::Item::Enum(_)
                    | ast::Item::Union(_)
                    | ast::Item::Trait(_)
                    | ast::Item::Impl(_)
                    | ast::Item::Fn(_)
                    | ast::Item::Const(_)
                    | ast::Item::Static(_)
                    | ast::Item::TypeAlias(_)
            )
        ) {
            pending!(DeclarationSourcePredicatesPending::UnsupportedDeclaration);
        }
        // Retain actual own parameter locations. Inherited Sized defaults and
        // associated output bounds are not this declaration's own recipes.
        if let Some(parameters) = syntax.children().find_map(ast::GenericParamList::cast) {
            for (ordinal, parameter) in parameters.generic_params().enumerate() {
                let ordinal = match u32::try_from(ordinal) {
                    Ok(ordinal) => ordinal,
                    Err(_) => {
                        return QueryPoll::Ready(Err(fault("own parameter ordinal overflow")));
                    }
                };
                result
                    .parameter_ranges
                    .insert(ordinal, range(parameter.syntax().text_range()));
            }
        }
        if usize::try_from(layout.own).ok() != Some(result.parameter_ranges.len()) {
            pending!(DeclarationSourcePredicatesPending::Source);
        }
        let environment_root = Arc::clone(&result.environment);
        let QueryValue::PreparedScopeImplHeaders(inventory) = environment_root.as_ref() else {
            return QueryPoll::Ready(Err(fault("declaration predicate inventory query differs")));
        };
        if inventory.key() != &key.inventory {
            return QueryPoll::Ready(Err(fault("declaration predicate inventory differs")));
        }
        let Some(environment) = inventory.inventory_root().environment() else {
            pending!(DeclarationSourcePredicatesPending::Environment);
        };
        let owner = SourceTypeOwner {
            definition: key.definition,
            scope: &context.scope,
            header,
        };
        let sources = environment.sources().iter().collect::<Vec<_>>();
        let source_header_root = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::DefinitionHeaderPlan(Box::new(DefinitionHeaderKey {
                inventory: key.inventory.clone(),
                definition: key.definition
            },))
        ))
        .value;
        let QueryValue::DefinitionHeaderPlan(source_header) = source_header_root.as_ref() else {
            return QueryPoll::Ready(Err(fault("actual declaration source header differs")));
        };
        if !source_header
            .declaration()
            .matches_metadata(source, header, context)
        {
            return QueryPoll::Ready(Err(fault(
                "declaration predicate source header receipt differs",
            )));
        }
        result.source_header = Some(Arc::clone(&source_header_root));
        if source_header.pending().is_some() {
            pending!(DeclarationSourcePredicatesPending::AssociatedContext);
        }
        let self_type = if let SourceHeaderParent::Associated(parent) =
            source_header.source_parent()
        {
            result.parent_definition = Some(*parent);
            let parent_types_root = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::DeclarationGenericTypes(Box::new(DeclarationGenericTypesKey {
                    inventory: key.inventory.clone(),
                    definition: *parent
                },))
            ))
            .value;
            let parent_header_root = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::DefinitionHeaderPlan(Box::new(DefinitionHeaderKey {
                    inventory: key.inventory.clone(),
                    definition: *parent
                },))
            ))
            .value;
            result.parent_generic_types = Some(Arc::clone(&parent_types_root));
            result.parent_header = Some(Arc::clone(&parent_header_root));
            let QueryValue::DeclarationGenericTypes(parent_types) = parent_types_root.as_ref()
            else {
                return QueryPoll::Ready(Err(fault(
                    "actual associated parent generic Type query differs",
                )));
            };
            if let Some(why) = parent_types.pending() {
                pending!(DeclarationSourcePredicatesPending::ParentGenerics(why));
            }
            let QueryValue::DefinitionHeaderPlan(parent_header_plan) = parent_header_root.as_ref()
            else {
                return QueryPoll::Ready(Err(fault(
                    "actual associated parent header query differs",
                )));
            };
            if parent_header_plan.pending().is_some() {
                pending!(DeclarationSourcePredicatesPending::AssociatedContext);
            }
            let parent_declaration = parent_types.declaration();
            let Some((parent_source, parent_header, parent_context, parent_parsed)) =
                parent_declaration.source_parts()
            else {
                pending!(DeclarationSourcePredicatesPending::AssociatedContext);
            };
            if !parent_header_plan.declaration().matches_metadata(
                parent_source,
                parent_header,
                parent_context,
            ) || parent_header.tokens != header.context
                || usize::try_from(layout.inherited).ok()
                    != Some(parent_declaration.parameters().len())
                || layout.implicit_self != parent_declaration.has_trait_self()
            {
                pending!(DeclarationSourcePredicatesPending::ParentLayout);
            }
            let Some(parent_layout) = parent_declaration.layout() else {
                pending!(DeclarationSourcePredicatesPending::ParentLayout);
            };
            if parent_layout.inherited != 0 || parent_layout.implicit_self {
                pending!(DeclarationSourcePredicatesPending::ParentLayout);
            }
            for parameter in parent_declaration.parameters() {
                let Some(ordinal) = layout.own.checked_add(parameter.ordinal) else {
                    return QueryPoll::Ready(Err(fault("associated parameter ordinal overflow")));
                };
                let Some(child) = usize::try_from(ordinal)
                    .ok()
                    .and_then(|ordinal| declaration.parameters().get(ordinal))
                else {
                    pending!(DeclarationSourcePredicatesPending::ParentLayout);
                };
                if child.ordinal != ordinal
                    || child.kind != parameter.kind
                    || child.name != parameter.name
                    || child.bounds != parameter.bounds
                    || child.default != parameter.default
                    || child.const_type != parameter.const_type
                {
                    pending!(DeclarationSourcePredicatesPending::ParentLayout);
                }
                result.parameter_rebase.insert(
                    ParamKey {
                        definition: *parent,
                        ordinal: parameter.ordinal,
                    },
                    ParamKey {
                        definition: key.definition,
                        ordinal,
                    },
                );
            }
            let parent_syntax = match parent_parsed.syntax_at(parent_context.range, |node| {
                ast::Trait::can_cast(node.kind()) || ast::Impl::can_cast(node.kind())
            }) {
                Ok(Some(node)) => node,
                Ok(None) => pending!(DeclarationSourcePredicatesPending::AssociatedContext),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            if parent_declaration.has_trait_self() {
                if !ast::Trait::can_cast(parent_syntax.kind()) {
                    pending!(DeclarationSourcePredicatesPending::ParentLayout);
                }
                let Some(ordinal) = layout.own.checked_add(layout.inherited) else {
                    return QueryPoll::Ready(Err(fault("associated Self ordinal overflow")));
                };
                result.parameter_rebase.insert(
                    ParamKey {
                        definition: *parent,
                        ordinal: layout.inherited,
                    },
                    ParamKey {
                        definition: key.definition,
                        ordinal,
                    },
                );
                Some(TyTerm::Param(ParamKey {
                    definition: key.definition,
                    ordinal,
                }))
            } else {
                let Some(implementation) = ast::Impl::cast(parent_syntax.clone()) else {
                    pending!(DeclarationSourcePredicatesPending::AssociatedContext);
                };
                let Some(ty) = implementation.self_ty() else {
                    pending!(DeclarationSourcePredicatesPending::SelfType);
                };
                if super::source_types::symbolic_type_pending(&ty).is_some() {
                    pending!(DeclarationSourcePredicatesPending::SelfType);
                }
                let parent_owner = SourceTypeOwner {
                    definition: *parent,
                    scope: &parent_context.scope,
                    header: parent_header,
                };
                let frame = WrittenTypeDeclaration::declaration(
                    parent_owner,
                    parent_declaration.parameters(),
                    &parent_syntax,
                    None,
                )
                .rebase_parameters(&result.parameter_rebase);
                let Some(ty) = ready!(super::source_types::resolve_actual_type(
                    db,
                    environment.names(),
                    &sources,
                    &frame,
                    &ty
                )) else {
                    pending!(DeclarationSourcePredicatesPending::SelfType);
                };
                Some(ty)
            }
        } else if layout.inherited != 0
            || layout.implicit_self
            || !header.context.is_empty()
            || matches!(source_header.source_parent(), SourceHeaderParent::Pending)
        {
            pending!(DeclarationSourcePredicatesPending::AssociatedContext);
        } else if declaration.has_trait_self() {
            let ordinal = match u32::try_from(declaration.parameters().len()) {
                Ok(ordinal) => ordinal,
                Err(_) => return QueryPoll::Ready(Err(fault("declaration Self ordinal overflow"))),
            };
            Some(TyTerm::Param(ParamKey {
                definition: key.definition,
                ordinal,
            }))
        } else if let Some(implementation) = ast::Impl::cast(syntax.clone()) {
            let Some(ty) = implementation.self_ty() else {
                pending!(DeclarationSourcePredicatesPending::SelfType);
            };
            if super::source_types::symbolic_type_pending(&ty).is_some() {
                pending!(DeclarationSourcePredicatesPending::SelfType);
            }
            let frame =
                WrittenTypeDeclaration::declaration(owner, declaration.parameters(), &syntax, None);
            let Some(ty) = ready!(super::source_types::resolve_actual_type(
                db,
                environment.names(),
                &sources,
                &frame,
                &ty
            )) else {
                pending!(DeclarationSourcePredicatesPending::SelfType);
            };
            Some(ty)
        } else {
            None
        };
        let frame = WrittenTypeDeclaration::declaration(
            owner,
            declaration.parameters(),
            &syntax,
            self_type.as_ref(),
        );
        let mut written = ready!(super::source_predicates::prepare_own(
            db,
            environment.names(),
            &sources,
            &frame,
            &syntax,
            environment.language_item_core(frame.profile()),
            layout.own,
            self_type.as_ref()
        ));
        let mut defaults_by_context = Vec::new();
        let own_types = declaration
            .parameters()
            .iter()
            .filter(|parameter| {
                parameter.ordinal < layout.own && parameter.kind == GenericKind::Type
            })
            .collect::<Vec<_>>();
        let trait_self = ast::Trait::can_cast(syntax.kind());
        if !own_types.is_empty() || trait_self || !written.recipes().is_empty() {
            let defaults_root = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::SourceDefaultBounds(Box::new(key.clone())),
            ))
            .value;
            result.default_bounds = Some(Arc::clone(&defaults_root));
            let QueryValue::SourceDefaultBounds(defaults) = defaults_root.as_ref() else {
                return QueryPoll::Ready(Err(fault("actual default-bound query differs")));
            };
            if let Err(why) = written.order_explicit_sized(defaults) {
                pending!(DeclarationSourcePredicatesPending::DefaultBounds(why));
            }
            for parameter in own_types {
                let parameter = ParamKey {
                    definition: key.definition,
                    ordinal: parameter.ordinal,
                };
                let Some(site) = result.parameter_range(parameter) else {
                    pending!(DeclarationSourcePredicatesPending::Source);
                };
                match defaults.apply(
                    &TyTerm::Param(parameter),
                    (source.content, site),
                    DefaultBoundsContext::TypeParameter(parameter),
                    written.default_inputs(DefaultBoundsContext::TypeParameter(parameter)),
                ) {
                    Ok(implicit) => defaults_by_context
                        .push((DefaultBoundsContext::TypeParameter(parameter), implicit)),
                    Err(why) => pending!(DeclarationSourcePredicatesPending::DefaultBounds(why)),
                }
            }
            if trait_self {
                let Some(subject) = self_type.as_ref() else {
                    pending!(DeclarationSourcePredicatesPending::SelfType);
                };
                match defaults.apply(
                    subject,
                    (source.content, range(syntax.text_range())),
                    DefaultBoundsContext::TraitSelf,
                    written.default_inputs(DefaultBoundsContext::TraitSelf),
                ) {
                    Ok(implicit) => {
                        defaults_by_context.push((DefaultBoundsContext::TraitSelf, implicit))
                    }
                    Err(why) => pending!(DeclarationSourcePredicatesPending::DefaultBounds(why)),
                }
            }
        }
        result.self_type = self_type;
        result.recipes = written.with_default_sets(defaults_by_context);
        QueryPoll::Ready(Ok(result))
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-DECLARATION-SOURCE-PREDICATES", message.into())
}
impl StableEncode for PreparedDeclarationSourcePredicates {
    const DOMAIN: &'static str = "lrr/v4/actual-declaration-source-predicates";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.record(self.generic_types());
        let QueryValue::PreparedScopeImplHeaders(inventory) = self.environment.as_ref() else {
            unreachable!("captured environment")
        };
        e.digest(inventory.fingerprint());
        for root in [&self.source_header, &self.parent_header] {
            e.option(root, |e, value| {
                let QueryValue::DefinitionHeaderPlan(header) = value.as_ref() else {
                    unreachable!("actual source header")
                };
                e.record(header.as_ref());
            });
        }
        e.option(&self.parent_generic_types, |e, value| {
            let QueryValue::DeclarationGenericTypes(types) = value.as_ref() else {
                unreachable!("actual parent generic Types")
            };
            e.record(types.as_ref());
        });
        e.option(&self.default_bounds, |e, value| {
            e.digest(value.fingerprint())
        });
        e.option(&self.parent_definition, |e, definition| {
            e.digest(*definition)
        });
        e.map(&self.parameter_rebase, |e, source, target| {
            e.record(source);
            e.record(target);
        });
        e.option(&self.self_type, |e, ty| e.record(ty));
        e.map(&self.parameter_ranges, |e, ordinal, range| {
            e.u32(*ordinal);
            e.u32(range.start);
            e.u32(range.end);
        });
        e.sequence(&self.recipes, |e, recipe| e.record(recipe));
        e.option(&self.pending, |e, why| match why {
            DeclarationSourcePredicatesPending::GenericTypes(why) => {
                e.u32(0);
                e.u32(*why as u32);
            }
            DeclarationSourcePredicatesPending::Source => e.u32(1),
            DeclarationSourcePredicatesPending::Environment => e.u32(2),
            DeclarationSourcePredicatesPending::AssociatedContext => e.u32(3),
            DeclarationSourcePredicatesPending::SelfType => e.u32(4),
            DeclarationSourcePredicatesPending::UnsupportedDeclaration => e.u32(5),
            DeclarationSourcePredicatesPending::ParentGenerics(why) => {
                e.u32(6);
                e.u32(*why as u32);
            }
            DeclarationSourcePredicatesPending::ParentLayout => e.u32(7),
            DeclarationSourcePredicatesPending::DefaultBounds(why) => {
                e.u32(8);
                e.u8(*why as u8);
            }
        });
    }
}
