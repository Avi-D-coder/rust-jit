//! Pure binding access facts over the ordinary retained pattern plan.
//! This types potential binding projections; it never matches a guest value,
//! reads an enum discriminant or prepares any executable result/guard body.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{StableEncode, StableEncoder},
    },
    db::{
        key::QueryKey, memo::Database, poll::QueryPoll, provider::SyntaxProvider, value::QueryValue,
    },
    frontend::{
        callables::CallResolver,
        nominal_member_goals::{NominalMemberKey, NominalMemberSelection},
        pattern_names::{PatternSelection, select_source_pattern_name},
        types::BoundNominalVariant,
    },
    ir::{op::CaptureProjection, pc::PcKey},
    syntax::pattern_tree::{ConstructorForm, PatternNode, PatternTree},
    traits::term::{GenericArg, TyTerm},
    vm::pattern::{DefaultBindingMode, PatternContext, ReferenceKind, matcher::constructor_fields},
};
use std::sync::Arc;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct PatternAccessKey {
    pub(crate) owner: PcKey,
    pub(crate) root: TyTerm,
    pub(crate) projections: Vec<CaptureProjection>,
}
impl StableEncode for PatternAccessKey {
    const DOMAIN: &'static str = "lrr/v1/pattern-binding-access-key";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.owner);
        e.record(&self.root);
        e.sequence(&self.projections, encode_projection);
    }
}
pub(crate) fn encode_projection(e: &mut StableEncoder, p: &CaptureProjection) {
    match p {
        CaptureProjection::Deref => e.u8(0),
        CaptureProjection::Element => e.u8(1),
        CaptureProjection::Field { site, ordinal } => {
            e.u8(2);
            e.u32(site.0);
            e.option(ordinal, |e, value| e.u32(*value));
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) enum TypeStep {
    Deref,
    Field(u32),
    Element,
    Nominal {
        variant: Arc<BoundNominalVariant>,
        field: u32,
    },
}
#[derive(Debug, Clone)]
pub(crate) struct BindingAccess {
    pub(crate) steps: Vec<TypeStep>,
    pub(crate) mode: DefaultBindingMode,
}
#[derive(Debug)]
pub struct PreparedPatternAccess {
    key: PatternAccessKey,
    pub(crate) bindings: Vec<BindingAccess>,
    // Unknown patterns retain the old conservative capture, explicitly held.
    pub(crate) complete: bool,
}
impl PreparedPatternAccess {
    pub(crate) fn key(&self) -> &PatternAccessKey {
        &self.key
    }
}
impl StableEncode for PreparedPatternAccess {
    const DOMAIN: &'static str = "lrr/v1/pattern-binding-access";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.record(&self.key);
        e.bool(self.complete);
        e.sequence(&self.bindings, |e, access| {
            e.record(&access.mode);
            e.sequence(&access.steps, |e, step| match step {
                TypeStep::Deref => e.u8(0),
                TypeStep::Field(n) => {
                    e.u8(1);
                    e.u32(*n);
                }
                TypeStep::Element => e.u8(2),
                TypeStep::Nominal { variant, field } => {
                    e.u8(3);
                    e.record(variant.as_ref());
                    e.u32(*field);
                }
            });
        });
    }
}
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-ACCESS", message.into())
}
macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(v)) => v,
            QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
            QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
        }
    };
}
macro_rules! checked {
    ($value:expr) => {
        match $value {
            Ok(v) => v,
            Err(e) => return QueryPoll::Ready(Err(e)),
        }
    };
}

pub(crate) fn prepare(
    db: &mut Database,
    key: &PatternAccessKey,
) -> QueryPoll<PreparedPatternAccess> {
    let value = ready!(db.fetch(&SyntaxProvider, QueryKey::PatternPlan(key.owner.clone())));
    let QueryValue::PatternPlan(plan) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("pattern access plan differs")));
    };
    if plan.scrutinee().is_none() {
        return QueryPoll::Ready(Err(fault("pattern has no capture scrutinee")));
    }
    // The source/member context is built once for this pure query, never per
    // pattern name or during the installed VM operation.
    let owner = ready!(db.fetch(
        &SyntaxProvider,
        QueryKey::SourceLocalPlan(key.owner.body.clone())
    ));
    let QueryValue::SourceLocalPlan(owner) = owner.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("pattern access owner differs")));
    };
    let environment = checked!(
        owner
            .environment()
            .ok_or_else(|| fault("pattern access environment absent"))
    );
    let completed = environment.macros().source_environment();
    let sources: Vec<_> = completed.sources().iter().collect();
    let members = checked!(completed.member_index());
    let resolver =
        checked!(CallResolver::new(completed.names(), &sources)).with_members(Some(&members));
    let mut walker = Walker {
        key,
        resolver,
        graph: completed.names(),
        sources: &sources,
        output: PreparedPatternAccess {
            key: key.clone(),
            bindings: Vec::new(),
            complete: true,
        },
    };
    let mut ty = key.root.clone();
    let mut steps = Vec::new();
    for projection in &key.projections {
        match projection {
            CaptureProjection::Deref => match ty {
                TyTerm::Ref(_, _, inner) => {
                    ty = *inner;
                    steps.push(TypeStep::Deref);
                }
                _ => {
                    return QueryPoll::Ready(Err(fault(
                        "capture dereference needs its source pointee",
                    )));
                }
            },
            CaptureProjection::Element => {
                peel(&mut ty, &mut steps, None);
                match ty {
                    TyTerm::Array(inner, _) | TyTerm::Slice(inner) => {
                        ty = *inner;
                        steps.push(TypeStep::Element);
                    }
                    _ => {
                        return QueryPoll::Ready(Err(fault(
                            "capture index needs the ordinary element Type goal",
                        )));
                    }
                }
            }
            CaptureProjection::Field { site, ordinal } => {
                peel(&mut ty, &mut steps, None);
                if let (TyTerm::Tuple(items), Some(field)) = (&ty, ordinal) {
                    ty = checked!(
                        items
                            .get(*field as usize)
                            .cloned()
                            .ok_or_else(|| fault("capture tuple field absent"))
                    );
                    steps.push(TypeStep::Field(*field));
                } else {
                    let TyTerm::Nominal(definition, arguments) = &ty else {
                        return QueryPoll::Ready(Err(fault(
                            "capture field needs nominal source Type",
                        )));
                    };
                    let member_key = NominalMemberKey {
                        pc: PcKey {
                            node: site.0,
                            ..key.owner.clone()
                        },
                        definition: *definition,
                        arguments: arguments.clone(),
                        selection: ordinal.map_or(
                            NominalMemberSelection::Named,
                            NominalMemberSelection::Positional,
                        ),
                    };
                    let member = ready!(db.fetch(
                        &SyntaxProvider,
                        QueryKey::NominalMember(Box::new(member_key))
                    ));
                    let QueryValue::NominalMember(member) = member.value.as_ref() else {
                        return QueryPoll::Ready(Err(fault("capture field member differs")));
                    };
                    let field = checked!(
                        member
                            .field()
                            .ok_or_else(|| fault("capture field not selected"))
                    );
                    let variant_key = NominalMemberKey {
                        pc: key.owner.clone(),
                        definition: *definition,
                        arguments: arguments.clone(),
                        selection: NominalMemberSelection::Variant(None),
                    };
                    let variant = ready!(bound_variant(db, variant_key));
                    let args = type_arguments(arguments);
                    ty = checked!(
                        field
                            .field_type()
                            .symbolic_source(&args)
                            .ok_or_else(|| fault("capture field Type remains open"))
                    );
                    steps.push(TypeStep::Nominal {
                        variant,
                        field: field.field(),
                    });
                }
            }
        }
    }
    for tree in plan.binding_trees() {
        let tree = checked!(tree.cloned().map_err(Clone::clone));
        let context = checked!(PatternContext::new(plan.index().profile().edition));
        ready!(walker.walk(db, &tree, tree.root(), ty.clone(), steps.clone(), context));
    }
    QueryPoll::Ready(Ok(walker.output))
}
fn type_arguments(arguments: &[GenericArg]) -> Vec<TyTerm> {
    arguments
        .iter()
        .filter_map(|arg| match arg {
            GenericArg::Type(ty) => Some(ty.clone()),
            _ => None,
        })
        .collect()
}
fn peel(ty: &mut TyTerm, steps: &mut Vec<TypeStep>, mut context: Option<&mut PatternContext>) {
    while let TyTerm::Ref(_, mutable, inner) = ty {
        if let Some(context) = context.as_deref_mut() {
            *context = context.peel_implicit(if *mutable {
                ReferenceKind::Mutable
            } else {
                ReferenceKind::Shared
            });
        }
        *ty = inner.as_ref().clone();
        steps.push(TypeStep::Deref);
    }
}
fn bound_variant(db: &mut Database, key: NominalMemberKey) -> QueryPoll<Arc<BoundNominalVariant>> {
    let value = ready!(db.fetch(&SyntaxProvider, QueryKey::NominalMember(Box::new(key))));
    let QueryValue::NominalMember(member) = value.value.as_ref() else {
        return QueryPoll::Ready(Err(fault("pattern access member differs")));
    };
    QueryPoll::Ready(
        member
            .variant()
            .cloned()
            .ok_or_else(|| fault("pattern access variant absent")),
    )
}
struct Walker<'a> {
    key: &'a PatternAccessKey,
    resolver: CallResolver<'a>,
    graph: &'a crate::frontend::names::NameNamespaces,
    sources: &'a [&'a crate::source::changes::PreparedSourceView],
    output: PreparedPatternAccess,
}
impl Walker<'_> {
    fn select(
        &self,
        db: &mut Database,
        tree: &PatternTree,
        slot: u32,
    ) -> QueryPoll<PatternSelection> {
        let name = checked!(
            tree.names()
                .get(slot as usize)
                .ok_or_else(|| fault("pattern access name slot absent"))
        );
        let (_, selected) = ready!(select_source_pattern_name(
            &self.resolver,
            self.graph,
            self.sources,
            db,
            &self.key.owner,
            name
        ));
        QueryPoll::Ready(Ok(selected))
    }
    fn walk(
        &mut self,
        db: &mut Database,
        tree: &PatternTree,
        node: &PatternNode,
        mut ty: TyTerm,
        mut steps: Vec<TypeStep>,
        mut context: PatternContext,
    ) -> QueryPoll<()> {
        if matches!(
            node,
            PatternNode::Tuple { .. } | PatternNode::Array { .. } | PatternNode::Constructor { .. }
        ) {
            peel(&mut ty, &mut steps, Some(&mut context));
        }
        match node {
            PatternNode::Wildcard | PatternNode::Literal(_) | PatternNode::Range { .. } => {}
            PatternNode::Unsupported(_) => {
                self.output.complete = false;
                self.output.bindings.push(BindingAccess {
                    steps,
                    mode: DefaultBindingMode::Move,
                });
            }
            PatternNode::Binding {
                by_reference,
                mutable,
                edition,
                name,
                subpattern,
                ..
            } => {
                let binding = match name {
                    Some((slot, _)) => matches!(
                        ready!(self.select(db, tree, *slot)),
                        PatternSelection::Binding
                    ),
                    None => true,
                };
                if binding {
                    let mode = checked!(
                        checked!(context.with_edition(*edition))
                            .binding_mode(PatternContext::modifier(*by_reference, *mutable))
                    );
                    self.output.bindings.push(BindingAccess {
                        steps: steps.clone(),
                        mode,
                    });
                }
                if let Some(sub) = subpattern {
                    ready!(self.walk(db, tree, sub, ty, steps, context));
                }
            }
            PatternNode::Reference { edition, inner, .. } => {
                let TyTerm::Ref(_, _, target) = ty else {
                    return QueryPoll::Ready(Err(fault("reference pattern source Type differs")));
                };
                steps.push(TypeStep::Deref);
                context = checked!(checked!(context.with_edition(*edition)).explicit_reference());
                ready!(self.walk(db, tree, inner, *target, steps, context));
            }
            PatternNode::Or(items) => {
                for item in items {
                    ready!(self.walk(db, tree, item, ty.clone(), steps.clone(), context));
                }
            }
            PatternNode::Tuple { items, rest } => {
                let arity = match &ty {
                    TyTerm::Tuple(members) => members.len(),
                    TyTerm::Primitive(crate::ir::op::PrimitiveType::Unit) => 0,
                    _ => return QueryPoll::Ready(Err(fault("tuple pattern source Type differs"))),
                };
                for (field, item) in checked!(crate::vm::pattern::matcher::tuple_fields(
                    items, *rest, arity
                )) {
                    let TyTerm::Tuple(members) = &ty else {
                        continue;
                    };
                    let mut path = steps.clone();
                    path.push(TypeStep::Field(field));
                    ready!(self.walk(
                        db,
                        tree,
                        item,
                        members[field as usize].clone(),
                        path,
                        context
                    ));
                }
            }
            PatternNode::Array {
                items,
                rest_binding,
                ..
            } => {
                let element = match &ty {
                    TyTerm::Array(element, _) | TyTerm::Slice(element) => element.as_ref().clone(),
                    _ => {
                        return QueryPoll::Ready(Err(fault(
                            "sequence pattern source Type differs",
                        )));
                    }
                };
                for item in items {
                    let mut path = steps.clone();
                    path.push(TypeStep::Element);
                    ready!(self.walk(db, tree, item, element.clone(), path, context));
                }
                if let Some(rest) = rest_binding {
                    ready!(self.walk(db, tree, rest, ty, steps, context));
                }
            }
            PatternNode::Constructor { slot, form } => {
                if matches!(form, ConstructorForm::Path) {
                    return QueryPoll::Ready(Ok(()));
                }
                let selected = ready!(self.select(db, tree, *slot));
                let PatternSelection::Constructor { identity, variant, .. } = selected else {
                    return QueryPoll::Ready(Err(fault("binding constructor source absent")));
                };
                let definition = variant
                    .as_ref()
                    .map_or(identity, |variant| variant.enumeration);
                let TyTerm::Nominal(actual, arguments) = &ty else {
                    return QueryPoll::Ready(Err(fault("nominal pattern source Type differs")));
                };
                if *actual != definition {
                    return QueryPoll::Ready(Err(fault(
                        "nominal capture source/constructor differ",
                    )));
                }
                let variant_name = variant.as_ref().map(|v| v.name.clone());
                let member_key = NominalMemberKey {
                    pc: self.key.owner.clone(),
                    definition,
                    arguments: arguments.clone(),
                    selection: NominalMemberSelection::Variant(variant_name.clone()),
                };
                let value = ready!(db.fetch(
                    &SyntaxProvider,
                    QueryKey::NominalMember(Box::new(member_key))
                ));
                let QueryValue::NominalMember(member) = value.value.as_ref() else {
                    return QueryPoll::Ready(Err(fault("constructor access member differs")));
                };
                let bound = checked!(
                    member
                        .variant()
                        .cloned()
                        .ok_or_else(|| fault("constructor access variant absent"))
                );
                let name = checked!(
                    tree.names()
                        .get(*slot as usize)
                        .ok_or_else(|| fault("constructor name absent"))
                );
                let ordinals = ready!(super::pattern_names::field_ordinals(
                    db,
                    definition,
                    variant_name.as_deref(),
                    &name.form,
                    member.shape()
                ));
                let args = type_arguments(arguments);
                for (field, child) in checked!(constructor_fields(
                    form,
                    member.shape(),
                    bound.fields().len(),
                    &ordinals
                )) {
                    let mut path = steps.clone();
                    path.push(TypeStep::Nominal {
                        variant: Arc::clone(&bound),
                        field,
                    });
                    let field_type = checked!(
                        bound.fields()[field as usize]
                            .symbolic_source(&args)
                            .ok_or_else(|| fault("constructor access field Type remains open"))
                    );
                    ready!(self.walk(db, tree, child, field_type, path, context));
                }
            }
        }
        QueryPoll::Ready(Ok(()))
    }
}
