//! Actual entered associated-value/call qualifiers, selected by the ordinary
//! body Type service and retained with their genuine source occurrences.
//! Occurrence rows alone are not completed member/initializer applications:
//! unsupported full generic packets remain explicit preparation demands.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    db::{memo::Database, poll::QueryPoll},
    frontend::{
        entered_call_goals::{EnteredGenericArgument, written_argument},
        source_locals::PreparedSourceLocalPlan,
    },
    ir::pc::{NodeId, PcKey, Phase},
    syntax::{
        ast_adapter::range,
        body_index::{ExpressionKind, NodeRole},
        spans::SourceRange,
        token_wire::Key,
    },
};
use ra_ap_syntax::{AstNode, ast};
use std::sync::Arc;

fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-ITEM-APPLICATION", message.into())
}
/// An actual source occurrence is not a completed lexical Type selection.
/// The common Type/member service must discharge this demand before closing
/// an initializer application. No descriptor or caller environment does so.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AssociatedConstApplicationDemand {
    LexicalTypeAndSubstitution,
}
/// Every actual generic-argument AST is kept, including nested arguments,
/// lifetimes, consts and associated bindings. The observation supplies the
/// existing canonical indexed Type site or its explicit pending demand.
pub(crate) struct ItemGenericArgumentOccurrence {
    syntax: ast::GenericArg,
    observation: EnteredGenericArgument,
}
impl ItemGenericArgumentOccurrence {
    pub(crate) fn syntax(&self) -> &ast::GenericArg {
        &self.syntax
    }
    pub(crate) fn observation(&self) -> &EnteredGenericArgument {
        &self.observation
    }
}
/// Privately constructed source occurrence, not a selected item application.
/// `qualifier` may name a module or Type: only the ordinary resolver decides.
/// Shared Rowan handles retain genuine structure, never reparsed display text.
pub(crate) struct AssociatedConstPathOccurrence {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    qualifier: ast::Path,
    member: ast::NameRef,
    member_context: Option<Key>,
    member_edition: u16,
    arguments: Box<[ItemGenericArgumentOccurrence]>,
    fingerprint: Digest,
    bytes: usize,
}
impl AssociatedConstPathOccurrence {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn qualifier(&self) -> &ast::Path {
        &self.qualifier
    }
    pub(crate) fn member(&self) -> &ast::NameRef {
        &self.member
    }
    pub(crate) fn member_context(&self) -> Option<Key> {
        self.member_context
    }
    pub(crate) fn member_edition(&self) -> u16 {
        self.member_edition
    }
    pub(crate) fn arguments(&self) -> &[ItemGenericArgumentOccurrence] {
        &self.arguments
    }
    fn qualifier_has_arguments(&self) -> bool {
        let qualifier = self.qualifier.syntax().text_range();
        // The Type arguments of an anchor's trait (`<T as Tr<A>>`) apply to the
        // trait, not to the Self Type the qualifier selects.
        let anchor_trait = self
            .qualifier
            .segment()
            .and_then(|segment| match segment.kind() {
                Some(ast::PathSegmentKind::Type { trait_ref, .. }) => {
                    trait_ref.map(|path| path.syntax().text_range())
                }
                _ => None,
            });
        self.arguments.iter().any(|argument| {
            let at = argument.syntax.syntax().text_range();
            qualifier.contains_range(at) && !anchor_trait.is_some_and(|tr| tr.contains_range(at))
        })
    }
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn estimated_bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn demand(&self) -> AssociatedConstApplicationDemand {
        AssociatedConstApplicationDemand::LexicalTypeAndSubstitution
    }
    /// Reuse checks the existing source/body/context/environment authority;
    /// this does not upgrade the occurrence to a Type/member lookup result.
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        self.plan.require_current(db)
    }
}
impl StableEncode for AssociatedConstPathOccurrence {
    const DOMAIN: &'static str = "lrr/v1/associated-constant-path-occurrence";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.source_identity());
        let qualifier = range(self.qualifier.syntax().text_range());
        let member = range(self.member.syntax().text_range());
        e.u32(qualifier.start);
        e.u32(qualifier.end);
        e.u32(member.start);
        e.u32(member.end);
        e.option(&self.member_context, |e, context| {
            e.digest(Digest(context.0))
        });
        e.u16(self.member_edition);
        e.sequence(&self.arguments, |e, argument| {
            e.record(&argument.observation)
        });
    }
}
/// Cold source preparation only. An owning immutable query may retain this
/// result; installed execution must consume that retained plan, never call
/// this helper to rediscover an operand. `None` means no qualifier occurred,
/// not that an associated constant or source Type was absent.
pub(crate) fn prepare_occurrence(
    db: &mut Database,
    plan: &Arc<PreparedSourceLocalPlan>,
    key: &PcKey,
) -> QueryPoll<Option<AssociatedConstPathOccurrence>> {
    let plan = Arc::clone(plan);
    match plan.require_current(db) {
        QueryPoll::Ready(Ok(())) => {}
        QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
        QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
    }
    QueryPoll::Ready((|| {
        let source = plan.context().source();
        let node = NodeId(key.node);
        if key.phase != 0
            || source.body().key() != key.body
            || source.index().body() != &key.body
            || source
                .index()
                .nodes()
                .get(node.0 as usize)
                .is_none_or(|indexed| {
                    indexed.id != node || indexed.role != NodeRole::Expression(ExpressionKind::Path)
                })
        {
            return Err(fault("associated value occurrence indexed source differs"));
        }
        let Some(syntax) =
            source
                .parsed()
                .entered_node_syntax(source.body(), source.index(), node)?
        else {
            return Err(fault("associated value occurrence syntax absent"));
        };
        let expression = ast::PathExpr::cast(syntax)
            .ok_or_else(|| fault("associated value occurrence is not its actual path"))?;
        let path = expression
            .path()
            .ok_or_else(|| fault("associated value path absent"))?;
        let Some(qualifier) = path.qualifier() else {
            return Ok(None);
        };
        let member = path
            .segment()
            .and_then(|segment| segment.name_ref())
            .ok_or_else(|| fault("associated value member name absent"))?;
        let path_range = range(path.syntax().text_range());
        let member_range = range(member.syntax().text_range());
        if !source.body().range.contains(path_range) || !path_range.contains(member_range) {
            return Err(fault("associated value occurrence escapes actual body"));
        }
        let member_context = source.identifier_context(member_range)?;
        let member_edition = source.parsed().token_edition(member_range)?;
        let mut arguments = Vec::new();
        // Walk only this actual path once, not the body or parsed file. Nested
        // GenericArg handles preserve their real parent structure and order;
        // no application ordinal or erasure is inferred from this inventory.
        for syntax in path
            .syntax()
            .descendants()
            .filter_map(ast::GenericArg::cast)
        {
            let observation = written_argument(&plan, syntax.clone())?;
            if !path_range.contains(observation.range()) {
                return Err(fault(
                    "associated value generic argument escapes actual path",
                ));
            }
            arguments
                .try_reserve(1)
                .map_err(|_| crate::runtime::arena::resource())?;
            arguments.push(ItemGenericArgumentOccurrence {
                syntax,
                observation,
            });
        }
        let bytes = std::mem::size_of::<AssociatedConstPathOccurrence>()
            .checked_add(
                arguments
                    .len()
                    .checked_mul(std::mem::size_of::<ItemGenericArgumentOccurrence>())
                    .ok_or_else(crate::runtime::arena::resource)?,
            )
            .ok_or_else(crate::runtime::arena::resource)?;
        let mut occurrence = AssociatedConstPathOccurrence {
            key: key.clone(),
            plan: Arc::clone(&plan),
            qualifier,
            member,
            member_context,
            member_edition,
            arguments: arguments.into_boxed_slice(),
            fingerprint: Digest([0; 32]),
            bytes,
        };
        occurrence.fingerprint = StableEncode::fingerprint(&occurrence);
        Ok(Some(occurrence))
    })())
}

/// Immutable qualifier Type selection, shared by associated function entry
/// and associated constant reads. Open parameters name the genuine caller
/// binder and are instantiated by its ordinary reached GenericEnv.
pub struct PreparedAssociatedItemQualifier {
    key: PcKey,
    plan: Arc<PreparedSourceLocalPlan>,
    occurrence: Option<AssociatedConstPathOccurrence>,
    source: Option<crate::frontend::explicit_type_arguments::ExplicitType>,
    /// The Type arguments written on an anchor's trait (`<T as Tr<A, B>>`),
    /// each selected at its own indexed Type occurrence; `None` for an
    /// argument no Type is selected for.
    trait_arguments: Vec<Option<crate::frontend::explicit_type_arguments::ExplicitType>>,
    trait_qualifier: Option<Digest>,
    member_traits: Option<Box<[SourceTraitBound]>>,
}
struct SourceTraitBound {
    identity: Option<Digest>,
    arguments: Vec<crate::traits::term::GenericArg>,
    syntax: ra_ap_syntax::SyntaxNode,
    complete: bool,
    owning_trait: bool,
}
/// Source member scope of a parameter. This borrowed view never substitutes
/// the reached type's unrelated impls for actual declared trait bounds.
pub(crate) struct AssociatedConstMemberContext<'a> {
    bounds: &'a [SourceTraitBound],
}
impl AssociatedConstMemberContext<'_> {
    pub(crate) fn owner(&self) -> Result<Option<Digest>, Diagnostic> {
        self.bounds
            .iter()
            .find(|bound| bound.owning_trait)
            .map(Self::selected)
            .transpose()
    }
    pub(crate) fn bounds(&self) -> impl Iterator<Item = Result<Digest, Diagnostic>> + '_ {
        self.bounds
            .iter()
            .filter(|bound| !bound.owning_trait)
            .map(Self::selected)
    }
    fn selected(bound: &SourceTraitBound) -> Result<Digest, Diagnostic> {
        if !bound.complete || !bound.arguments.is_empty() {
            return Err(fault(
                "associated constant trait application requires its full source argument packet",
            ));
        }
        bound
            .identity
            .ok_or_else(|| fault("associated constant bound trait source is pending"))
    }
}
impl std::fmt::Debug for PreparedAssociatedItemQualifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedAssociatedItemQualifier")
            .field("key", &self.key)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}
impl PreparedAssociatedItemQualifier {
    pub(crate) fn key(&self) -> &PcKey {
        &self.key
    }
    pub(crate) fn retained_plan(&self) -> &Arc<PreparedSourceLocalPlan> {
        &self.plan
    }
    pub(crate) fn source(&self) -> Option<&crate::frontend::explicit_type_arguments::ExplicitType> {
        self.source.as_ref()
    }
    pub(crate) fn trait_arguments(
        &self,
    ) -> &[Option<crate::frontend::explicit_type_arguments::ExplicitType>] {
        &self.trait_arguments
    }
    pub(crate) fn member(&self) -> Option<&ast::NameRef> {
        self.occurrence
            .as_ref()
            .map(|occurrence| occurrence.member())
    }
    pub(crate) fn permits_trait_self_inference(&self) -> bool {
        self.trait_qualifier.is_some()
            && self
                .occurrence
                .as_ref()
                .is_some_and(|occurrence| !occurrence.qualifier_has_arguments())
    }
    pub(crate) fn member_traits(&self) -> Option<AssociatedConstMemberContext<'_>> {
        self.member_traits
            .as_deref()
            .map(|bounds| AssociatedConstMemberContext { bounds })
    }
    pub(crate) fn pending(&self) -> Option<AssociatedConstApplicationDemand> {
        (self.occurrence.is_some() && self.source.is_none())
            .then_some(AssociatedConstApplicationDemand::LexicalTypeAndSubstitution)
    }
    pub(crate) fn metadata_bytes(&self) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(self.occurrence.as_ref().map_or(Some(0), |occurrence| {
                occurrence
                    .estimated_bytes()
                    .checked_sub(std::mem::size_of::<AssociatedConstPathOccurrence>())
            })?)?
            .checked_add(
                self.source
                    .as_ref()
                    .map_or(Some(0), |source| source.heap_bytes().ok())?,
            )?
            .checked_add(
                self.trait_arguments
                    .iter()
                    .try_fold(
                        self.trait_arguments
                            .capacity()
                            .checked_mul(std::mem::size_of::<Option<crate::frontend::explicit_type_arguments::ExplicitType>>())?,
                        |bytes, argument| {
                            bytes.checked_add(
                                argument
                                    .as_ref()
                                    .map_or(Some(0), |argument| argument.heap_bytes().ok())?,
                            )
                        },
                    )?,
            )?
            .checked_add(self.member_traits.as_ref().map_or(Some(0), |bounds| {
                let mut bytes = bounds
                    .len()
                    .checked_mul(std::mem::size_of::<SourceTraitBound>())?;
                for bound in bounds.iter() {
                    bytes =
                        bytes.checked_add(bound.arguments.capacity().checked_mul(
                            std::mem::size_of::<crate::traits::term::GenericArg>(),
                        )?)?;
                    for argument in &bound.arguments {
                        if let crate::traits::term::GenericArg::Type(term) = argument {
                            bytes = bytes.checked_add(term.heap_bytes()?)?;
                        }
                    }
                }
                Some(bytes)
            })?)
    }
}
impl StableEncode for PreparedAssociatedItemQualifier {
    const DOMAIN: &'static str = "lrr/v4/prepared-associated-item-qualifier";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.digest(self.plan.source_identity());
        e.option(&self.occurrence, |e, occurrence| e.record(occurrence));
        e.option(&self.source, |e, source| e.record(source));
        e.sequence(&self.trait_arguments, |e, argument| {
            e.option(argument, |e, argument| e.record(argument))
        });
        e.option(&self.trait_qualifier, |e, identity| e.digest(*identity));
        e.option(&self.member_traits, |e, bounds| {
            e.sequence(bounds.as_ref(), |e, bound| {
                e.option(&bound.identity, |e, identity| e.digest(*identity));
                e.bool(bound.complete);
                e.bool(bound.owning_trait);
                let at = range(bound.syntax.text_range());
                e.u32(at.start);
                e.u32(at.end);
                e.sequence(&bound.arguments, |e, argument| e.record(argument));
            })
        });
    }
}
pub(crate) fn prepare(
    db: &mut Database,
    key: &PcKey,
) -> QueryPoll<PreparedAssociatedItemQualifier> {
    use crate::{
        db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
        traits::term::TyTerm,
    };
    macro_rules! polled {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let selected = polled!(db.fetch(&SyntaxProvider,QueryKey::SourceLocalPlan(key.body.clone())));
    let QueryValue::SourceLocalPlan(plan)=selected.value.as_ref()else{return QueryPoll::Ready(Err(fault("associated qualifier source plan variant differs")));};
    if !selected.cacheable || plan.context().source().index().body()!=&key.body {
        return QueryPoll::Ready(Err(fault("associated qualifier actual source body differs")));
    }
    let plan=Arc::clone(plan);
    let occurrence=polled!(prepare_occurrence(db,&plan,key));
    let mut source = None;
    let mut trait_qualifier = None;
    let mut anchor_traits = None;
    let mut trait_arguments = Vec::new();
    if let Some(occurrence) = &occurrence {
        // Full argument application is a separate source goal. Retain every
        // actual occurrence instead of erasing unsupported Const/Region kinds
        // or mistaking a generic base for its completed application.
        if !occurrence.qualifier_has_arguments() {
            let anchor =
                occurrence
                    .qualifier()
                    .segment()
                    .and_then(|segment| match segment.kind() {
                        Some(ast::PathSegmentKind::Type {
                            type_ref,
                            trait_ref,
                        }) => Some((segment.type_anchor(), type_ref, trait_ref)),
                        _ => None,
                    });
            if let Some((anchor, type_ref, trait_ref)) = anchor {
                if let Some(ty) = type_ref {
                    if let Some(site) = match indexed_type_site(&plan, &ty) {
                        Ok(site) => site,
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    } {
                        source = if matches!(ty, ast::Type::PathType(_)) {
                            polled!(prepare_named_qualifier(db, &plan, NodeId(site.node))).0
                        } else {
                            polled!(crate::frontend::explicit_type_arguments::select_argument(
                                db,
                                &plan,
                                key,
                                &site,
                                range(ty.syntax().text_range())
                            ))
                            .map(|selected| selected.source)
                        };
                    }
                }
                if anchor.is_some_and(|anchor| anchor.as_token().is_some()) {
                    // The second genuine Type child is the trait in an `as`
                    // anchor; TypeAnchor::path_type alone could select the
                    // first child instead when the self Type is a path too.
                    let mut selected_trait = None;
                    // Each written Type argument of the trait is selected at its
                    // own indexed occurrence (lifetimes and consts take no slot).
                    let written_arguments: Vec<Option<ast::Type>> = trait_ref
                        .as_ref()
                        .and_then(|ty| ty.path())
                        .and_then(|path| path.segment())
                        .and_then(|segment| ast::HasGenericArgs::generic_arg_list(&segment))
                        .into_iter()
                        .flat_map(|list| list.generic_args().collect::<Vec<_>>())
                        .filter_map(|argument| match argument {
                            ast::GenericArg::TypeArg(argument) => Some(argument.ty()),
                            _ => None,
                        })
                        .collect();
                    for written in written_arguments {
                        let selected = match written {
                            Some(ty) => match indexed_type_site(&plan, &ty) {
                                Ok(Some(site)) => polled!(
                                    crate::frontend::explicit_type_arguments::select_argument(
                                        db,
                                        &plan,
                                        key,
                                        &site,
                                        range(ty.syntax().text_range())
                                    )
                                )
                                .map(|selected| selected.source),
                                Ok(None) => None,
                                Err(error) => return QueryPoll::Ready(Err(error)),
                            },
                            None => None,
                        };
                        trait_arguments.push(selected);
                    }
                    let trait_ref_has_arguments = trait_ref
                        .as_ref()
                        .and_then(|ty| ty.path())
                        .and_then(|path| path.segment())
                        .and_then(|segment| ast::HasGenericArgs::generic_arg_list(&segment))
                        .is_some();
                    if let Some(trait_ref) = trait_ref {
                        let ty = ast::Type::PathType(trait_ref.clone());
                        if let Some(site) = match indexed_type_site(&plan, &ty) {
                            Ok(site) => site,
                            Err(error) => return QueryPoll::Ready(Err(error)),
                        } {
                            selected_trait =
                                polled!(prepare_named_qualifier(db, &plan, NodeId(site.node))).1;
                        }
                        if let Some(identity) = selected_trait {
                            let mut context = Vec::new();
                            if context.try_reserve_exact(1).is_err() {
                                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
                            }
                            context.push(SourceTraitBound {
                                identity: Some(identity),
                                arguments: Vec::new(),
                                syntax: trait_ref.syntax().clone(),
                                complete: true,
                                owning_trait: true,
                            });
                            anchor_traits = Some(context.into_boxed_slice());
                        }
                    }
                    // A trait written with Type arguments (`<T as Tr<A>>`) names its
                    // identity through the full application goal, not here; the
                    // anchor's Self Type is the qualifier's own either way.
                    let trait_with_arguments = trait_ref_has_arguments;
                    if selected_trait.is_none() && !trait_with_arguments {
                        source = None;
                    }
                }
            } else {
                (source, trait_qualifier) =
                    polled!(prepare_named_qualifier(db, &plan, NodeId(key.node)));
            }
        }
    }
    let member_traits = match (anchor_traits, source.as_ref().map(|source| source.source())) {
        (Some(context), _) => Some(context),
        (None, Some(TyTerm::Param(parameter))) => {
            Some(polled!(parameter_member_traits(db, &plan, *parameter)).into_boxed_slice())
        }
        _ => None,
    };
    QueryPoll::Ready(Ok(PreparedAssociatedItemQualifier {
        key: key.clone(),
        plan,
        occurrence,
        source,
        trait_arguments,
        trait_qualifier,
        member_traits,
    }))
}

/// The exact ordinary indexed Type occurrence of an anchor child. This is
/// the same source-relative Syntax site that explicit Type arguments use.
fn indexed_type_site(
    plan: &Arc<PreparedSourceLocalPlan>,
    ty: &ast::Type,
) -> Result<Option<PcKey>, Diagnostic> {
    let owner = plan.context().source();
    let actual = range(ty.syntax().text_range());
    if !owner.body().range.contains(actual) {
        return Err(fault("associated anchor Type escapes actual body"));
    }
    let relative = SourceRange::checked(
        actual
            .start
            .checked_sub(owner.body().range.start)
            .ok_or_else(crate::runtime::arena::resource)?,
        actual
            .end
            .checked_sub(owner.body().range.start)
            .ok_or_else(crate::runtime::arena::resource)?,
    )?;
    let Some(node) = owner
        .index()
        .nodes_at_range(relative)
        .find(|node| node.role == NodeRole::Syntax)
    else {
        return Ok(None);
    };
    let projected = SourceRange::checked(
        owner
            .body()
            .range
            .start
            .checked_add(node.range.start)
            .ok_or_else(crate::runtime::arena::resource)?,
        owner
            .body()
            .range
            .start
            .checked_add(node.range.end)
            .ok_or_else(crate::runtime::arena::resource)?,
    )?;
    if projected != actual || node.range != relative {
        return Err(fault("associated anchor indexed Type projection differs"));
    }
    Ok(Some(owner.index().pc(node.id, Phase::Entry)?))
}

/// Named qualifier and genuine anchor leaf use one ordinary entered-Type
/// finalizer. A selected trait is not a concrete Type or a default Self.
fn prepare_named_qualifier(
    db: &mut Database,
    plan: &Arc<PreparedSourceLocalPlan>,
    node: NodeId,
) -> QueryPoll<(Option<crate::frontend::explicit_type_arguments::ExplicitType>,Option<Digest>)> {
    use crate::db::{key::QueryKey,provider::SyntaxProvider,value::QueryValue};
    let key=match plan.context().source().index().pc(node,Phase::Entry){Ok(key)=>key,Err(error)=>return QueryPoll::Ready(Err(error))};
    let selected=match db.fetch(&SyntaxProvider,QueryKey::PreparedBodyTypeApplication(key.clone())){
        QueryPoll::Ready(Ok(value))=>value,QueryPoll::Ready(Err(error))=>return QueryPoll::Ready(Err(error)),QueryPoll::NeedInput(input)=>return QueryPoll::NeedInput(input),
    };
    let QueryValue::PreparedBodyTypeApplication(application)=selected.value.as_ref()else{return QueryPoll::Ready(Err(fault("named qualifier application query variant differs")));};
    if !selected.cacheable || application.key()!=&key || !Arc::ptr_eq(application.retained_plan().context(),plan.context()) {
        return QueryPoll::Ready(Err(fault("named qualifier actual source application differs")));
    }
    QueryPoll::Ready(Ok((application.source().cloned(),application.trait_qualifier())))
}

/// The declared member scope of a source parameter, without inspecting the
/// reached type's unrelated impls. Bound syntax stays genuine and retained;
/// unsupported trait argument applications are never collapsed to an identity.
fn parameter_member_traits(
    db: &mut Database,
    plan: &Arc<PreparedSourceLocalPlan>,
    parameter: crate::traits::term::ParamKey,
) -> QueryPoll<Vec<SourceTraitBound>> {
    use crate::{
        db::{key::QueryKey, provider::SyntaxProvider, value::QueryValue},
        frontend::source_types::{
            WrittenTypeDeclaration, actual_declaration_bounds, projection_trait,
            resolve_actual_type,
        },
        traits::term::TyTerm,
    };
    use ra_ap_syntax::ast::HasGenericParams;
    macro_rules! polled {
        ($value:expr) => {
            match $value {
                QueryPoll::Ready(Ok(value)) => value,
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        };
    }
    let source = plan.context().source();
    let captured = polled!(db.fetch(
        &SyntaxProvider,
        QueryKey::FunctionSignature(source.body().def_key)
    ));
    let QueryValue::FunctionSignature(signature) = captured.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("qualifier bound signature variant differs")));
    };
    if !captured.cacheable
        || signature.definition != parameter.definition
        || signature.header != *source.header()
        || signature.scope != source.context().scope
    {
        return QueryPoll::Ready(Err(fault("qualifier bound source signature differs")));
    }
    let syntax = match source.parsed().syntax_at(source.context().range, |node| {
        ast::Item::can_cast(node.kind())
    }) {
        Ok(Some(syntax)) => syntax,
        Ok(None) => {
            return QueryPoll::Ready(Err(fault("qualifier bound actual declaration absent")));
        }
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let Some(environment) = plan.environment() else {
        return QueryPoll::Ready(Err(fault("qualifier bound environment absent")));
    };
    let completed = environment.macros().source_environment();
    let members = match completed.member_index() {
        Ok(members) => members,
        Err(error) => return QueryPoll::Ready(Err(error)),
    };
    let views: Vec<_> = completed.sources().iter().collect();
    let declaration = WrittenTypeDeclaration::function_syntax(signature, &syntax);
    let mut bounds = Vec::new();
    if signature
        .generics
        .iter()
        .any(|declared| declared.ordinal == parameter.ordinal && declared.name == "Self")
    {
        if let Some(owner) = signature
            .scope
            .owner
            .filter(|owner| members.is_trait(*owner))
        {
            let actual_owner = syntax
                .parent()
                .and_then(|items| items.parent())
                .and_then(ast::Trait::cast);
            let complete = actual_owner.as_ref().is_some_and(|owner| {
                owner
                    .generic_param_list()
                    .is_none_or(|list| list.generic_params().next().is_none())
            });
            if bounds.try_reserve(1).is_err() {
                return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
            }
            bounds.push(SourceTraitBound {
                identity: Some(owner),
                arguments: Vec::new(),
                syntax: actual_owner.map_or_else(|| syntax.clone(), |owner| owner.syntax().clone()),
                complete,
                owning_trait: true,
            });
        }
    }
    for (subject, written_subject, bound) in actual_declaration_bounds(&declaration, &syntax) {
        let subject = match subject {
            Some(subject) => Some(subject),
            None => match written_subject {
                Some(subject) => polled!(resolve_actual_type(
                    db,
                    completed.names(),
                    &views,
                    &declaration,
                    &subject
                )),
                None => None,
            },
        };
        if subject.as_ref() != Some(&TyTerm::Param(parameter)) {
            continue;
        }
        match crate::syntax::source_bound_polarity::polarity(&bound) {
            crate::syntax::source_bound_polarity::SourceBoundPolarity::Negative => {
                bounds.push(SourceTraitBound { identity: None, arguments: Vec::new(),
                    syntax: bound.syntax().clone(), complete: false, owning_trait: false });
                continue;
            },
            crate::syntax::source_bound_polarity::SourceBoundPolarity::Positive => {},
            _ => continue,
        }
        let Some(ast::Type::PathType(ty)) = bound.ty() else {
            continue;
        };
        let Some(path) = ty.path() else {
            continue;
        };
        let selected = polled!(projection_trait(
            db,
            completed.names(),
            &views,
            &declaration,
            &path,
            &TyTerm::Param(parameter)
        ));
        let (identity, arguments) = selected.map_or((None, Vec::new()), |(identity, arguments)| {
            (members.is_trait(identity).then_some(identity), arguments)
        });
        let has_arguments = path
            .syntax()
            .descendants()
            .any(|node| ast::GenericArg::can_cast(node.kind()));
        let complete = identity.is_some() && !has_arguments && arguments.is_empty();
        if bounds.try_reserve(1).is_err() {
            return QueryPoll::Ready(Err(crate::runtime::arena::resource()));
        }
        bounds.push(SourceTraitBound {
            identity,
            arguments,
            syntax: path.syntax().clone(),
            complete,
            owning_trait: false,
        });
    }
    QueryPoll::Ready(Ok(bounds))
}
