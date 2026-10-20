//! Nominal structure in the source numeric constraint service: named and
//! positional field accesses, record and tuple-struct/variant constructors and
//! constructor patterns relate a component variable through the fields of the
//! declaration the ordinary services select. Field Types come only from the
//! shared nominal schema binder (`SchemaNames`, `BoundRecordConstructor`,
//! `bind_constructor`); constructor names only from the ordinary Value goal or
//! `CallResolver` lookup the dispatcher uses. A selection or field Type those
//! services cannot answer stays a missing goal.
use super::{Component, ConstraintDemand, Step, entry, fault};
use crate::{
    core::digest::Digest,
    db::{
        dependency::{InputKey, InputValue},
        key::QueryKey,
        memo::Database,
        poll::QueryPoll,
        provider::SyntaxProvider,
        value::QueryValue,
    },
    frontend::{
        nominal_constructors::{NominalConstructorTarget, bind_constructor},
        types::{
            BoundNominalVariant, FieldShape, NominalFieldBinding, NominalFieldType, SchemaNames,
        },
    },
    ir::{pc::NodeId, written_type::WrittenType},
    source::manifest::ItemKind,
    syntax::pattern_tree::{ConstructorForm, NameForm},
    traits::term::{GenericArg, TyTerm},
};
use std::sync::Arc;

macro_rules! ready {
    ($value:expr) => {
        match $value {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
    };
}

/// One declared field's Type as the shared binder answered it.
#[derive(Debug, Clone)]
pub(super) enum FieldType {
    Concrete(TyTerm),
    /// Index among the declaration's type parameters.
    Parameter(u32),
    /// A written structure around the declaration's type parameters.
    Written(Arc<WrittenType>),
    /// An actual declaration occurrence whose projection/application is not
    /// yet a materializable field Type. Preserve it without inventing a leaf.
    Deferred(Option<TyTerm>),
}
/// A declared field's written structure under the nominal Type's own type
/// arguments (`None` while a leaf is left to inference or an argument is
/// not a Type).
fn instantiate(written: &WrittenType, args: &[GenericArg]) -> Option<TyTerm> {
    let types = || {
        args.iter().filter_map(|arg| match arg {
            GenericArg::Type(term) => Some(term),
            _ => None,
        })
    };
    Some(match written {
        WrittenType::Parameter(index) => types().nth(*index as usize)?.clone(),
        WrittenType::Tuple(members) if members.is_empty() => written.source()?,
        WrittenType::Tuple(members) => TyTerm::Tuple(
            members
                .iter()
                .map(|member| instantiate(member, args))
                .collect::<Option<Vec<_>>>()?,
        ),
        WrittenType::Array(element, len) => TyTerm::Array(
            Box::new(instantiate(element, args)?),
            crate::traits::term::ConstTerm::Value(u64::from(*len)),
        ),
        WrittenType::Generic {
            identity,
            args: written,
            ..
        } => TyTerm::Nominal(
            *identity,
            written
                .iter()
                .map(|arg| instantiate(arg, args).map(GenericArg::Type))
                .collect::<Option<Vec<_>>>()?,
        ),
        WrittenType::Slice(element) => TyTerm::Slice(Box::new(instantiate(element, args)?)),
        WrittenType::Ref {
            region,
            mutable,
            target,
        } => TyTerm::Ref(
            region.clone(),
            *mutable,
            Box::new(instantiate(target, args)?),
        ),
        other => other.source()?,
    })
}
/// A declared field Type projected by a member's steps.
enum FieldProjection {
    /// A concrete leaf and the steps that remain below it.
    Concrete(TyTerm, Vec<Step>),
    /// The declaration's type parameter and the steps that remain below it.
    Parameter(u32, Vec<Step>),
    /// A written structure around the declaration's type parameters, reached
    /// with no steps remaining (`Box<List<T>>`).
    Structured(WrittenType),
    Unknown,
}
impl FieldType {
    /// Every projection path from this field's value to type parameter
    /// `parameter` (through tuples, arrays, slices and references).
    fn parameter_paths(&self, parameter: u32) -> Vec<Vec<Step>> {
        fn walk(
            written: &WrittenType,
            parameter: u32,
            prefix: &mut Vec<Step>,
            out: &mut Vec<Vec<Step>>,
        ) {
            match written {
                WrittenType::Parameter(index) if *index == parameter => out.push(prefix.clone()),
                WrittenType::Tuple(members) => {
                    for (position, member) in members.iter().enumerate() {
                        let Ok(position) = u32::try_from(position) else {
                            return;
                        };
                        prefix.push(Step::Field(position));
                        walk(member, parameter, prefix, out);
                        prefix.pop();
                    }
                }
                WrittenType::Array(element, _) | WrittenType::Slice(element) => {
                    prefix.push(Step::Element);
                    walk(element, parameter, prefix, out);
                    prefix.pop();
                }
                WrittenType::Ref { target, .. } => {
                    prefix.push(Step::Deref);
                    walk(target, parameter, prefix, out);
                    prefix.pop();
                }
                // A nominal's own type arguments (`Option<T>`, `Box<List<T>>`).
                WrittenType::Generic { args, .. } => {
                    for (position, arg) in args.iter().enumerate() {
                        let Ok(position) = u32::try_from(position) else {
                            return;
                        };
                        prefix.push(Step::TypeArgument(position));
                        walk(arg, parameter, prefix, out);
                        prefix.pop();
                    }
                }
                _ => {}
            }
        }
        match self {
            Self::Parameter(index) if *index == parameter => vec![Vec::new()],
            Self::Written(written) => {
                let mut out = Vec::new();
                walk(written, parameter, &mut Vec::new(), &mut out);
                out
            }
            _ => Vec::new(),
        }
    }
    /// Every (type parameter, projection path) occurrence in this field.
    fn parameters(&self) -> Vec<(u32, Vec<Step>)> {
        fn walk(written: &WrittenType, prefix: &mut Vec<Step>, out: &mut Vec<(u32, Vec<Step>)>) {
            match written {
                WrittenType::Parameter(index) => out.push((*index, prefix.clone())),
                WrittenType::Tuple(members) => {
                    for (position, member) in members.iter().enumerate() {
                        let Ok(position) = u32::try_from(position) else {
                            return;
                        };
                        prefix.push(Step::Field(position));
                        walk(member, prefix, out);
                        prefix.pop();
                    }
                }
                WrittenType::Array(element, _) | WrittenType::Slice(element) => {
                    prefix.push(Step::Element);
                    walk(element, prefix, out);
                    prefix.pop();
                }
                WrittenType::Ref { target, .. } => {
                    prefix.push(Step::Deref);
                    walk(target, prefix, out);
                    prefix.pop();
                }
                WrittenType::Generic { args, .. } => {
                    for (position, arg) in args.iter().enumerate() {
                        let Ok(position) = u32::try_from(position) else {
                            return;
                        };
                        prefix.push(Step::TypeArgument(position));
                        walk(arg, prefix, out);
                        prefix.pop();
                    }
                }
                _ => {}
            }
        }
        match self {
            Self::Parameter(index) => vec![(*index, Vec::new())],
            Self::Written(written) => {
                let mut out = Vec::new();
                walk(written, &mut Vec::new(), &mut out);
                out
            }
            Self::Concrete(_) => Vec::new(),
            Self::Deferred(_) => Vec::new(),
        }
    }
    /// This field's Type projected by `steps`, as far as its written
    /// structure states.
    fn project(&self, steps: &[Step]) -> FieldProjection {
        let mut written = match self {
            Self::Concrete(term) => return FieldProjection::Concrete(term.clone(), steps.to_vec()),
            Self::Parameter(index) => return FieldProjection::Parameter(*index, steps.to_vec()),
            Self::Written(written) => written.as_ref(),
            Self::Deferred(_) => return FieldProjection::Unknown,
        };
        let mut rest = steps;
        loop {
            if let WrittenType::Parameter(index) = written {
                return FieldProjection::Parameter(*index, rest.to_vec());
            }
            if !written.has_parameters() {
                return match written.source() {
                    Some(term) => FieldProjection::Concrete(term, rest.to_vec()),
                    None => FieldProjection::Unknown,
                };
            }
            let Some(step) = rest.first() else {
                return FieldProjection::Structured(written.clone());
            };
            written = match (step, written) {
                (Step::AutoDeref, WrittenType::Ref { target, .. }) => target,
                (Step::AutoDeref, _) => {
                    rest = &rest[1..];
                    continue;
                }
                (Step::Deref, WrittenType::Ref { target, .. }) => target,
                (Step::Field(position), WrittenType::Tuple(members)) => {
                    match members.get(*position as usize) {
                        Some(member) => member,
                        None => return FieldProjection::Unknown,
                    }
                }
                (Step::Element, WrittenType::Array(element, _) | WrittenType::Slice(element)) => {
                    element
                }
                (Step::TypeArgument(position), WrittenType::Generic { args, .. }) => {
                    match args.get(*position as usize) {
                        Some(arg) => arg,
                        None => return FieldProjection::Unknown,
                    }
                }
                // Indexing a one-argument container field (`Vec<T>`): its
                // element is that type argument.
                (Step::Element, WrittenType::Generic { args, .. }) if args.len() == 1 => &args[0],
                _ => return FieldProjection::Unknown,
            };
            if !matches!(step, Step::AutoDeref) {
                rest = &rest[1..];
            }
        }
    }
    fn of(field: &NominalFieldType) -> Self {
        match field {
            NominalFieldType::Concrete(source) => Self::Concrete(source.source().clone()),
            NominalFieldType::Parameter(index) => Self::Parameter(*index),
            NominalFieldType::Compound(written) => match written.has_parameters() {
                false => match written.source() {
                    Some(term) => Self::Concrete(term),
                    None => Self::Written(Arc::clone(written)),
                },
                true => Self::Written(Arc::clone(written)),
            },
        }
    }
    /// This field's Type under the nominal Type's arguments.
    fn under(&self, args: &[GenericArg]) -> Option<TyTerm> {
        match self {
            Self::Concrete(term) => Some(term.clone()),
            Self::Parameter(index) => args
                .iter()
                .filter_map(|arg| match arg {
                    GenericArg::Type(term) => Some(term),
                    _ => None,
                })
                .nth(*index as usize)
                .cloned(),
            Self::Written(written) => instantiate(written, args),
            Self::Deferred(_) => None,
        }
    }
}
/// A selected struct or enum-variant constructor and its declared fields.
#[derive(Debug)]
pub(super) struct Constructed {
    definition: Digest,
    enumeration: bool,
    variant: u32,
    shape: FieldShape,
    /// Declaration-order field names (`None` for positional fields).
    names: Vec<Option<String>>,
    fields: Vec<FieldType>,
    /// Actual declaration keys in Type-argument order. Region/const ordinal
    /// positions are not fabricated into Type parameter identities.
    type_parameters: Vec<crate::traits::term::ParamKey>,
}
impl Constructed {
    pub(super) fn arity(&self) -> usize {
        self.names.len()
    }
    /// How deeply the declared fields' Types nest.
    pub(super) fn depth(&self) -> usize {
        fn written(written: &WrittenType) -> usize {
            1 + match written {
                WrittenType::Tuple(members) => members.iter().map(written_depth).max().unwrap_or(0),
                WrittenType::Array(inner, _) | WrittenType::Slice(inner) => written_depth(inner),
                WrittenType::Ref { target, .. } => written_depth(target),
                WrittenType::Generic { args, .. } => {
                    args.iter().map(written_depth).max().unwrap_or(0)
                }
                _ => 0,
            }
        }
        fn written_depth(inner: &WrittenType) -> usize {
            written(inner)
        }
        self.fields
            .iter()
            .map(|field| match field {
                FieldType::Written(inner) => written(inner),
                FieldType::Concrete(term) => super::term_depth(term),
                FieldType::Parameter(_) => 1,
                FieldType::Deferred(Some(term)) => super::term_depth(term),
                FieldType::Deferred(None) => 1,
            })
            .max()
            .unwrap_or(0)
    }
    /// The declaration's own type parameter count.
    pub(super) fn type_parameter_count(&self) -> usize {
        self.type_parameters.len()
    }
    /// Whether some declared field of this constructor names type parameter
    /// `ordinal`.
    pub(super) fn names_parameter(&self, ordinal: u32) -> bool {
        let Some(position) = self
            .type_parameters
            .iter()
            .position(|key| key.ordinal == ordinal)
            .and_then(|position| u32::try_from(position).ok())
        else {
            return false;
        };
        self.fields.iter().any(|field| {
            field
                .parameters()
                .iter()
                .any(|(index, _)| *index == position)
        })
    }
    /// The type-parameter positions inside declaration field `field`.
    pub(super) fn field_parameters(&self, field: u32) -> Vec<(u32, Vec<Step>)> {
        self.fields
            .get(field as usize)
            .map(FieldType::parameters)
            .unwrap_or_default()
    }
    /// The constructed value's nominal Type with these type arguments.
    pub(super) fn nominal_type_with(&self, arguments: Vec<TyTerm>) -> TyTerm {
        TyTerm::Nominal(
            self.definition,
            arguments.into_iter().map(GenericArg::Type).collect(),
        )
    }
    /// The constructed value's nominal Type with each type argument a
    /// distinct variable of this declaration (position order), for a
    /// constructor whose arguments do not decide them.
    pub(super) fn open_type(&self) -> (TyTerm, Vec<crate::traits::term::ParamKey>) {
        let keys = self.type_parameters.clone();
        (
            TyTerm::Nominal(
                self.definition,
                keys.iter()
                    .map(|key| GenericArg::Type(TyTerm::Param(*key)))
                    .collect(),
            ),
            keys,
        )
    }
}
/// How a projection step relates to a constructor's fields.
pub(super) enum FieldMatch {
    /// The declaration-order field the step selects.
    Field(u32),
    /// Another variant of the same enum: no value of this constructor.
    Elsewhere,
    /// A step this constructor's Type cannot take.
    Unknown,
}
/// Which syntax selected a constructor (memo key within one demand).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum ConstructorSite {
    Expression(NodeId),
    Pattern { root: NodeId, slot: u32 },
}

impl<'a> Component<'a> {
    /// The completed environment's name graph and sources, or `None`.
    fn environment_parts(
        &mut self,
        db: &mut Database,
    ) -> QueryPoll<Option<Arc<crate::expand::source_discovery::CompletedSourceEnvironment>>> {
        self.environment(db)
    }
    /// The executing body's declaration scope (privacy of field selections).
    pub(super) fn caller_scope(
        &self,
        db: &mut Database,
    ) -> Option<crate::syntax::outline::ScopeKey> {
        match db
            .input(&InputKey::DeclarationContext(self.key.body.definition))
            .as_deref()
        {
            Some(InputValue::DeclarationContext(context)) => Some(context.scope.clone()),
            _ => None,
        }
    }
    /// The bound variant's schema-declared field names and field Types.
    fn constructed(
        &mut self,
        db: &mut Database,
        bound: &BoundNominalVariant,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let schema = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(bound.identity())) {
            QueryPoll::Ready(Ok(schema)) => schema,
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint constructor schema differs")));
        };
        let Some(variant) = schema.variants.get(bound.variant() as usize) else {
            return QueryPoll::Ready(Ok(None));
        };
        if variant.fields.len() != bound.fields().len() {
            return QueryPoll::Ready(Ok(None));
        }
        let fields = bound.fields().iter().map(FieldType::of).collect();
        QueryPoll::Ready(Ok(Some(Arc::new(Constructed {
            definition: bound.identity(),
            enumeration: bound.definition().item_kind == ItemKind::Enum,
            variant: bound.variant(),
            shape: variant.shape,
            names: variant
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect(),
            fields,
            type_parameters: schema
                .generics
                .iter()
                .filter(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
                .map(|parameter| crate::traits::term::ParamKey {
                    definition: schema.definition,
                    ordinal: parameter.ordinal,
                })
                .collect(),
        }))))
    }
    /// A selected Value/Type definition that constructs: a struct or an
    /// enum variant, bound with the binder constructor expressions use.
    fn constructor_definition(
        &mut self,
        db: &mut Database,
        identity: Digest,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let Some(environment) = ready!(self.environment_parts(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let graph = environment.names();
        let target = if let Some(variant) = graph.enum_variant(identity) {
            match NominalConstructorTarget::variant(db, &sources, variant) {
                Ok(target) => target,
                Err(error) if super::not_closed(&error) => return QueryPoll::Ready(Ok(None)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        } else {
            match crate::frontend::callables::selected_item_kind(
                db,
                sources.iter().copied(),
                identity,
            ) {
                Some(ItemKind::Struct)
                    if db
                        .input(&InputKey::LocalDeclarationOwner(identity))
                        .is_none() =>
                {
                    match NominalConstructorTarget::structure(db, &sources, identity) {
                        Ok(target) => target,
                        Err(error) if super::not_closed(&error) => {
                            return QueryPoll::Ready(Ok(None));
                        }
                        Err(error) => return QueryPoll::Ready(Err(error)),
                    }
                }
                // A struct declared in the executing body is no module
                // outline item: its published local declaration owns it.
                _ if db
                    .input(&InputKey::LocalDeclarationOwner(identity))
                    .is_some()
                    && matches!(
                        db.input(&InputKey::DefinitionHeader(identity)).as_deref(),
                        Some(InputValue::DefinitionHeader(header))
                            if header.tokens.iter().any(|token| token.text == "struct")
                    ) =>
                {
                    match ready!(NominalConstructorTarget::local(db, identity, None)) {
                        Some(target) => target,
                        None => return QueryPoll::Ready(Ok(None)),
                    }
                }
                _ => return QueryPoll::Ready(Ok(None)),
            }
        };
        self.bind_target(db, graph, &sources, &target)
    }
    /// The variant `Root::member` names when the entered Value goal selected
    /// the root `enumeration` lexically (a block-local or imported enum).
    fn rooted_constructor(
        &mut self,
        db: &mut Database,
        enumeration: Digest,
        member: &str,
        local: bool,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        // Only an enum has variants; a module or type root names other items.
        if !matches!(
            db.input(&InputKey::DefinitionHeader(enumeration)).as_deref(),
            Some(InputValue::DefinitionHeader(header))
                if header.tokens.iter().any(|token| token.text == "enum")
        ) {
            return QueryPoll::Ready(Ok(None));
        }
        let Some(environment) = ready!(self.environment_parts(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let graph = environment.names();
        // Only a declared variant constructs; another member of the enum is
        // an associated item.
        let schema = ready!(db.fetch(&SyntaxProvider, QueryKey::TypeSchema(enumeration)));
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint rooted enum schema differs")));
        };
        if schema.variant(Some(member)).is_err() {
            return QueryPoll::Ready(Ok(None));
        }
        let target = if local {
            match ready!(NominalConstructorTarget::local(
                db,
                enumeration,
                Some(member)
            )) {
                Some(target) => target,
                None => return QueryPoll::Ready(Ok(None)),
            }
        } else {
            let variant = crate::frontend::names::EnumVariantDefinition {
                enumeration,
                name: member.to_owned(),
            };
            match NominalConstructorTarget::variant(db, &sources, &variant) {
                Ok(target) => target,
                Err(error) if super::not_closed(&error) => return QueryPoll::Ready(Ok(None)),
                Err(error) => return QueryPoll::Ready(Err(error)),
            }
        };
        self.bind_target(db, graph, &sources, &target)
    }
    fn bind_target(
        &mut self,
        db: &mut Database,
        graph: &crate::frontend::names::NameNamespaces,
        sources: &[&crate::source::changes::PreparedSourceView],
        target: &NominalConstructorTarget,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let bound = match bind_constructor(db, graph, sources, target) {
            QueryPoll::Ready(Ok(bound)) => bound,
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let variant = Arc::clone(bound.variant());
        self.constructed(db, &variant)
    }
    /// The constructor a record expression, constructor call or constructor
    /// pattern selects, memoized for this demand.
    pub(super) fn constructor(
        &mut self,
        db: &mut Database,
        site: ConstructorSite,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        if let Some(found) = self.constructors.get(&site) {
            return QueryPoll::Ready(Ok(found.clone()));
        }
        let found = match site {
            ConstructorSite::Expression(node) => {
                let kind = match self.fact(node) {
                    Ok(fact) => fact.kind(),
                    Err(error) => return QueryPoll::Ready(Err(error)),
                };
                match kind {
                    crate::syntax::source_constraints::FactKind::Record => {
                        ready!(self.record_constructor(db, node))
                    }
                    crate::syntax::source_constraints::FactKind::Call => {
                        ready!(self.call_constructor(db, node))
                    }
                    // A fieldless constructor named as a value (`None`).
                    crate::syntax::source_constraints::FactKind::Path => {
                        ready!(self.selected_constructor(db, node))
                            .filter(|constructed| constructed.arity() == 0)
                    }
                    _ => None,
                }
            }
            ConstructorSite::Pattern { root, slot } => {
                ready!(self.pattern_constructor(db, root, slot))
            }
        };
        if let Some(constructed) = &found {
            self.note_depth(constructed.depth());
        }
        self.constructors.insert(site, found.clone());
        QueryPoll::Ready(Ok(found))
    }
    /// A record expression's selection through the ordinary prepared record
    /// constructor query (the same binding its execution uses).
    fn record_constructor(
        &mut self,
        db: &mut Database,
        node: NodeId,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let prepared = match db.fetch(
            &SyntaxProvider,
            QueryKey::BoundRecordConstructor(entry(self.key, node)),
        ) {
            QueryPoll::Ready(Ok(prepared)) => prepared,
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::BoundRecordConstructor(prepared) = prepared.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint record constructor differs")));
        };
        let Some(bound) = prepared.bound() else {
            return self.pending_record_shape(db, prepared);
        };
        let variant = bound.variant().clone();
        self.constructed(db, &variant)
    }
    /// Source constraint shape only: a pending runtime binding still has its
    /// exact schema, selected variant and actual field Type occurrences.
    fn pending_record_shape(
        &mut self,
        db: &mut Database,
        prepared: &crate::frontend::prepared_constructors::PreparedRecordConstructor,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let Some((selected, _)) = prepared.pending_target() else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(terms) = prepared.pending_fields() else {
            return QueryPoll::Ready(Ok(None));
        };
        let schema = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::TypeSchema(prepared.mapping().definition())
        ));
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("pending constraint schema differs")));
        };
        let Some(variant) = schema.variants.get(selected as usize) else {
            return QueryPoll::Ready(Err(fault("pending constraint variant absent")));
        };
        if crate::core::digest::StableEncode::fingerprint(schema.as_ref())
            != prepared.mapping().schema()
            || variant.fields.len() != terms.len()
        {
            return QueryPoll::Ready(Err(fault(
                "pending constraint field/schema identity differs",
            )));
        }
        let Some(environment) = ready!(self.environment_parts(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let parameters: Vec<_> = schema
            .generics
            .iter()
            .filter(|parameter| parameter.kind == crate::syntax::signatures::GenericKind::Type)
            .collect();
        let mut fields = Vec::new();
        for term in terms {
            let field = match term {
                Some(TyTerm::Param(key)) if key.definition == schema.definition => match parameters
                    .iter()
                    .position(|parameter| parameter.ordinal == key.ordinal)
                    .and_then(|index| u32::try_from(index).ok())
                {
                    Some(index) => FieldType::Parameter(index),
                    None => FieldType::Deferred(term.clone()),
                },
                Some(term)
                    if !crate::frontend::associated_types::contains_projection(term)
                        && (crate::frontend::call_contracts::closed_term(term)
                            || parameters.iter().enumerate().all(|(index, parameter)| {
                                u32::try_from(index).ok() == Some(parameter.ordinal)
                            })) =>
                {
                    match ready!(crate::frontend::written_terms::written_template(
                        db,
                        &sources,
                        term,
                        schema.definition
                    )) {
                        Some(written) => FieldType::Written(Arc::new(written)),
                        None => FieldType::Deferred(Some(term.clone())),
                    }
                }
                _ => FieldType::Deferred(term.clone()),
            };
            fields.push(field);
        }
        QueryPoll::Ready(Ok(Some(Arc::new(Constructed {
            definition: schema.definition,
            enumeration: variant.name.is_some(),
            variant: selected,
            shape: variant.shape,
            names: variant
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect(),
            fields,
            type_parameters: parameters
                .iter()
                .map(|parameter| crate::traits::term::ParamKey {
                    definition: schema.definition,
                    ordinal: parameter.ordinal,
                })
                .collect(),
        }))))
    }
    /// A call whose callee path selects a tuple struct or tuple variant.
    fn call_constructor(
        &mut self,
        db: &mut Database,
        call: NodeId,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let callee = match self.fact(call) {
            Ok(fact) => fact.operand(0),
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let Some(callee) = callee else {
            return QueryPoll::Ready(Ok(None));
        };
        if !matches!(
            self.fact(callee).map(|fact| fact.kind()),
            Ok(crate::syntax::source_constraints::FactKind::Path)
        ) {
            return QueryPoll::Ready(Ok(None));
        }
        self.selected_constructor(db, callee)
    }
    /// The constructor a value path selects through the ordinary Value goal.
    fn selected_constructor(
        &mut self,
        db: &mut Database,
        callee: NodeId,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let selected = ready!(db.fetch(
            &SyntaxProvider,
            QueryKey::EnteredValue(entry(self.key, callee))
        ));
        let QueryValue::EnteredValue(selected) = selected.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint callee query variant differs")));
        };
        // `Root::member` whose root the goal selected lexically: a variant
        // of that enum, or an associated item (no constructor).
        if let Some(member) = selected.rooted() {
            let Some(declaration) = selected.declaration() else {
                return QueryPoll::Ready(Ok(None));
            };
            let (identity, local) = (declaration.identity(), declaration.owner().is_some());
            return self.rooted_constructor(db, identity, member, local);
        }
        // An enum variant is a member of its enum, not a published
        // declaration of its own: the Value goal names it as that demand.
        let identity = match (selected.declaration(), selected.pending()) {
            (Some(declaration), None) => declaration.identity(),
            (
                None,
                Some(crate::frontend::source_value_goals::ValueGoalDemand::DeclarationPublication(
                    identity,
                )),
            ) => identity,
            _ => return QueryPoll::Ready(Ok(None)),
        };
        self.constructor_definition(db, identity)
    }
    /// A constructor pattern's name, selected by the shared pattern-name
    /// service the dispatcher uses (`frontend::pattern_names`): a block-local
    /// name through its entered lexical Value goal, any other through the
    /// body's lexical module context.
    fn pattern_constructor(
        &mut self,
        db: &mut Database,
        root: NodeId,
        slot: u32,
    ) -> QueryPoll<Option<Arc<Constructed>>> {
        let plan = self.plan;
        let Some(name) = plan
            .tree(root)
            .and_then(|tree| tree.names().get(slot as usize))
        else {
            return QueryPoll::Ready(Ok(None));
        };
        if let Some(node) = name.lexical {
            let selected = ready!(db.fetch(
                &SyntaxProvider,
                QueryKey::EnteredValue(entry(self.key, node))
            ));
            let QueryValue::EnteredValue(selected) = selected.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("constraint pattern goal variant differs")));
            };
            // A fresh binding, or a goal still awaiting its context.
            let (Some(declaration), None) = (selected.declaration(), selected.pending()) else {
                return QueryPoll::Ready(Ok(None));
            };
            if let Some(member) = selected.rooted() {
                let (identity, local) = (declaration.identity(), declaration.owner().is_some());
                return self.rooted_constructor(db, identity, member, local);
            }
            return self.constructor_definition(db, declaration.identity());
        }
        let Some(environment) = ready!(self.environment_parts(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let resolver =
            match crate::frontend::callables::CallResolver::new(environment.names(), &sources) {
                Ok(resolver) => resolver,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
        let selected = match crate::frontend::pattern_names::select_pattern_name(
            &resolver,
            environment.names(),
            &sources,
            db,
            self.key.body.definition,
            name,
        ) {
            QueryPoll::Ready(Ok((_, selected))) => selected,
            // The dispatcher refuses such a name only when it is reached.
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        match selected {
            crate::frontend::pattern_names::PatternSelection::Constructor { identity, .. } => {
                self.constructor_definition(db, identity)
            }
            _ => QueryPoll::Ready(Ok(None)),
        }
    }

    /// The projection step of declaration field `field`; `written` is the
    /// written name's member row of a record expression or pattern.
    pub(super) fn field_step(&self, constructed: &Constructed, field: u32) -> Option<Step> {
        if constructed.enumeration {
            return Some(Step::Variant {
                definition: constructed.definition,
                variant: constructed.variant,
                field,
            });
        }
        match constructed.shape {
            FieldShape::Tuple => Some(Step::Field(field)),
            FieldShape::Record => {
                let name = constructed.names.get(field as usize)?.as_deref()?;
                self.plan.member_row(name).map(Step::Named)
            }
            FieldShape::Unit => None,
        }
    }
    /// The declaration field a written record field name selects.
    pub(super) fn written_field(&self, constructed: &Constructed, row: u32) -> Option<u32> {
        let written = self.plan.member(row)?;
        let position = match constructed.shape {
            FieldShape::Record => constructed
                .names
                .iter()
                .position(|name| name.as_deref() == Some(written))?,
            // `S { 0: a }` names a positional field by its decimal ordinal.
            FieldShape::Tuple => written
                .parse::<usize>()
                .ok()
                .filter(|position| *position < constructed.arity())?,
            FieldShape::Unit => return None,
        };
        u32::try_from(position).ok()
    }
    /// Which declared field of this constructor a projection step selects.
    pub(super) fn field_match(&self, constructed: &Constructed, step: &Step) -> FieldMatch {
        match *step {
            Step::Variant {
                definition,
                variant,
                field,
            } if constructed.enumeration && definition == constructed.definition => {
                if variant != constructed.variant {
                    FieldMatch::Elsewhere
                } else if (field as usize) < constructed.arity() {
                    FieldMatch::Field(field)
                } else {
                    FieldMatch::Unknown
                }
            }
            Step::Field(position)
                if !constructed.enumeration
                    && constructed.shape == FieldShape::Tuple
                    && (position as usize) < constructed.arity() =>
            {
                FieldMatch::Field(position)
            }
            Step::Named(row) if !constructed.enumeration => {
                match self.written_field(constructed, row) {
                    Some(field) if constructed.shape == FieldShape::Record => {
                        FieldMatch::Field(field)
                    }
                    _ => FieldMatch::Unknown,
                }
            }
            _ => FieldMatch::Unknown,
        }
    }

    /// The constructed value's own type arguments as placeholders: each is
    /// decided by this same demand started at the value's type argument.
    fn dependent_arguments(&mut self, node: NodeId, constructed: &Constructed) -> Vec<GenericArg> {
        let definition =
            Digest::of_bytes(&[b"lrr/open-dependent".as_slice(), &node.0.to_le_bytes()].concat());
        (0..constructed.type_parameters.len())
            .filter_map(|ordinal| u32::try_from(ordinal).ok())
            .map(|ordinal| {
                let key = crate::traits::term::ParamKey {
                    definition,
                    ordinal,
                };
                self.dependents.insert(key, (node, ordinal));
                GenericArg::Type(TyTerm::Param(key))
            })
            .collect()
    }

    /// `node` constructs `constructed` from `operands` (declaration field,
    /// operand expression) and the current member is `node` itself: the
    /// selected field's operand and its declared Type relate to the rest of
    /// the projection; a type-parameter field also relates every operand of
    /// a field with that same parameter.
    pub(super) fn constructed_inward(
        &mut self,
        db: &mut Database,
        node: NodeId,
        constructed: &Constructed,
        operands: &[(u32, NodeId)],
    ) -> QueryPoll<()> {
        // The written constructor application constrains its fields before
        // numeric fallback, using the SAME indexed Type arguments call entry
        // consumes. No constructor/macro spelling chooses numeric semantics.
        if constructed.type_parameter_count() != 0 {
            let plan = ready!(db.fetch(&SyntaxProvider,
                QueryKey::SourceLocalPlan(self.key.body.clone())));
            let QueryValue::SourceLocalPlan(plan) = plan.value.as_ref() else {
                return QueryPoll::Ready(Err(fault("constructor argument source plan differs")));
            };
            let record = match self.fact(node) {
                Ok(fact) => fact.kind() == crate::syntax::source_constraints::FactKind::Record,
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            let selected_arguments = if record {
                // Same normalized target slots the installed constructor reads.
                let selected = ready!(db.fetch(&SyntaxProvider,QueryKey::BoundRecordConstructor(entry(self.key,node))));
                let QueryValue::BoundRecordConstructor(selected) = selected.value.as_ref() else {
                    return QueryPoll::Ready(Err(fault("constraint record arguments differ")));
                };
                (!selected.arguments().is_empty()).then(|| selected.arguments().iter().map(|argument|
                    argument.as_ref().map(|argument| argument.source().clone())).collect::<Vec<_>>())
            } else if let Some(selected) = ready!(super::super::explicit_type_arguments::select_path_arguments(
                db, plan, &entry(self.key, node),
                super::super::explicit_type_arguments::PathArgumentPosition::Constructor,
            )) {
                for guard in &selected.guards {
                    if let Err(error) = guard.require_current(db) {
                        return QueryPoll::Ready(Err(error));
                    }
                }
                Some(selected.arguments.iter().map(|argument|
                    argument.as_ref().map(|argument| argument.source().clone())).collect())
            } else {None};
            if let Some(selected) = selected_arguments {
                let mut arguments = self.dependent_arguments(node, constructed);
                for (argument, selected) in arguments.iter_mut().zip(&selected) {
                    if let Some(selected) = selected {
                        *argument = GenericArg::Type(selected.clone());
                    }
                }
                ready!(self.constrain_term(db, node,
                    &TyTerm::Nominal(constructed.definition, arguments),
                    ConstraintDemand::SelectedDeclaration));
            }
        }
        // A constructed value is not a reference; autoderef selects it.
        let mut via = self.via.clone();
        if via.steps.first() == Some(&Step::AutoDeref) {
            via = via.unwrap(Step::AutoDeref).expect("checked autoderef step");
        }
        let Some(first) = via.steps.first().copied() else {
            // The whole constructed value is an open variable: its Type is
            // the declaration applied to its own type arguments.
            if self.open {
                let args = self.dependent_arguments(node, constructed);
                self.candidate(node, TyTerm::Nominal(constructed.definition, args));
            } else {
                self.defer(node, ConstraintDemand::NumericFamily);
            }
            return QueryPoll::Ready(Ok(()));
        };
        match first {
            // The constructed Type's own argument: every field position of
            // that type parameter holds it.
            Step::TypeArgument(parameter) => {
                let rest = via.unwrap(first).expect("checked first step");
                // Apply numeric obligations only when this variable is
                // known to be numeric. A general open Type must not choose
                // a primitive merely because a trait also has a primitive
                // impl; the ordinary literal/source relations establish its
                // family first.
                if rest.steps.is_empty() {
                    let mut family = (!self.open).then_some(self.floating);
                    if self.open {
                        for (field, operand) in operands {
                            if matches!(constructed.fields.get(*field as usize),
                                Some(FieldType::Parameter(found)) if *found == parameter)
                            {
                                match super::numeric_initializer(self.plan, *operand) {
                                    Ok(Some(found)) => {
                                        if family.is_some_and(|old| old != found) {
                                            self.defer(node, ConstraintDemand::NumericFamily);
                                            return QueryPoll::Ready(Ok(()));
                                        }
                                        family = Some(found);
                                    }
                                    Ok(None) => {}
                                    Err(error) => return QueryPoll::Ready(Err(error)),
                                }
                            }
                        }
                    }
                    if let (Some(family), Some(key)) =
                        (family, constructed.type_parameters.get(parameter as usize))
                    {
                        ready!(self.nominal_numeric_parameter(db, node, *key, family));
                    }
                }
                for (operand_field, operand) in operands {
                    let Some(field_type) = constructed.fields.get(*operand_field as usize) else {
                        continue;
                    };
                    for mut path in field_type.parameter_paths(parameter) {
                        path.extend_from_slice(&rest.steps);
                        self.expression_via(
                            Some(*operand),
                            super::Via {
                                steps: path,
                                strict: false,
                            },
                        );
                    }
                }
                return QueryPoll::Ready(Ok(()));
            }
            // The iteration Item is selected from the constructed Type.
            Step::Item => return QueryPoll::Ready(Ok(())),
            _ => {}
        }
        let field = match self.field_match(constructed, &first) {
            FieldMatch::Field(field) => field,
            FieldMatch::Elsewhere => return QueryPoll::Ready(Ok(())),
            FieldMatch::Unknown => {
                self.defer(node, ConstraintDemand::ExpressionRelation);
                return QueryPoll::Ready(Ok(()));
            }
        };
        let rest = via.unwrap(first).expect("checked first step");
        for (operand_field, operand) in operands {
            if *operand_field == field {
                self.expression_via(Some(*operand), rest.clone());
            }
        }
        let Some(declared) = constructed.fields.get(field as usize) else {
            self.defer(node, ConstraintDemand::ExpressionRelation);
            return QueryPoll::Ready(Ok(()));
        };
        match declared.project(&rest.steps) {
            FieldProjection::Concrete(term, remaining) => {
                let current = std::mem::replace(
                    &mut self.via,
                    super::Via {
                        steps: remaining,
                        strict: rest.strict,
                    },
                );
                let result = self.constrain_term(db, node, &term, ConstraintDemand::ResultType);
                self.via = current;
                ready!(result);
            }
            // Every position of that parameter in every field holds the
            // same Type.
            FieldProjection::Parameter(parameter, remaining) => {
                for (operand_field, operand) in operands {
                    let Some(field_type) = constructed.fields.get(*operand_field as usize) else {
                        continue;
                    };
                    for mut path in field_type.parameter_paths(parameter) {
                        path.extend_from_slice(&remaining);
                        self.expression_via(
                            Some(*operand),
                            super::Via {
                                steps: path,
                                strict: false,
                            },
                        );
                    }
                }
            }
            // An open variable that is a Type built around the constructed
            // value's own parameters (`next: Option<Box<List<T>>>` of
            // `List { value: 1, next: None }`): the parameters are decided
            // by the constructed value's other relations.
            FieldProjection::Structured(written) if self.open => {
                let args = self.dependent_arguments(node, constructed);
                if let Some(term) = instantiate(&written, &args) {
                    self.candidate(node, term);
                } else {
                    self.defer(node, ConstraintDemand::ExpressionRelation);
                }
            }
            FieldProjection::Structured(_) | FieldProjection::Unknown => {
                self.defer(node, ConstraintDemand::ExpressionRelation)
            }
        }
        QueryPoll::Ready(Ok(()))
    }

    /// Projection of a declared nominal Type `definition<args>` by one field
    /// step, through the shared schema binder. `None` is a missing goal.
    pub(super) fn nominal_field(
        &mut self,
        db: &mut Database,
        definition: Digest,
        args: &[GenericArg],
        step: &Step,
    ) -> QueryPoll<Option<TyTerm>> {
        let Some(environment) = ready!(self.environment_parts(db)) else {
            return QueryPoll::Ready(Ok(None));
        };
        let Some(caller) = self.caller_scope(db) else {
            return QueryPoll::Ready(Ok(None));
        };
        let schema = match db.fetch(&SyntaxProvider, QueryKey::TypeSchema(definition)) {
            QueryPoll::Ready(Ok(schema)) => schema,
            QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::TypeSchema(schema) = schema.value.as_ref() else {
            return QueryPoll::Ready(Err(fault("constraint field schema differs")));
        };
        let sources: Vec<_> = environment.sources().iter().collect();
        let names =
            match SchemaNames::from_current_database(db, environment.names(), &sources, schema) {
                QueryPoll::Ready(Ok(names)) => names,
                QueryPoll::Ready(Err(error)) if super::not_closed(&error) => {
                    return QueryPoll::Ready(Ok(None));
                }
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            };
        let members = match environment.member_index() {
            Ok(members) => members,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let source_arguments: Vec<_> = args
            .iter()
            .filter_map(|argument| match argument {
                GenericArg::Type(term) => Some(Some(term.clone())),
                _ => None,
            })
            .collect();
        let enumeration = names.definition().item_kind == ItemKind::Enum;
        let binding = match *step {
            Step::Named(row) if !enumeration => {
                let Some(member) = self.plan.member(row) else {
                    return QueryPoll::Ready(Ok(None));
                };
                names.bind_named_field_current_with_arguments(
                    db,
                    &sources,
                    None,
                    member,
                    &caller,
                    &source_arguments,
                    &members,
                )
            }
            Step::Field(position) if !enumeration => names
                .bind_positional_field_current_with_arguments(
                    db,
                    &sources,
                    None,
                    position,
                    &caller,
                    &source_arguments,
                    &members,
                ),
            Step::Variant {
                definition: owner,
                variant,
                field,
            } if enumeration && owner == definition => {
                let Some(recipe) = schema.variants.get(variant as usize) else {
                    return QueryPoll::Ready(Ok(None));
                };
                let variant_name = recipe.name.as_deref();
                match recipe.shape {
                    FieldShape::Record => {
                        let Some(member) = recipe
                            .fields
                            .get(field as usize)
                            .and_then(|field| field.name.as_deref())
                        else {
                            return QueryPoll::Ready(Ok(None));
                        };
                        names.bind_named_field_current_with_arguments(
                            db,
                            &sources,
                            variant_name,
                            member,
                            &caller,
                            &source_arguments,
                            &members,
                        )
                    }
                    FieldShape::Tuple => names.bind_positional_field_current_with_arguments(
                        db,
                        &sources,
                        variant_name,
                        field,
                        &caller,
                        &source_arguments,
                        &members,
                    ),
                    FieldShape::Unit => return QueryPoll::Ready(Ok(None)),
                }
            }
            _ => return QueryPoll::Ready(Ok(None)),
        };
        let bound = match binding {
            QueryPoll::Ready(Ok(NominalFieldBinding::Ready(bound))) => bound,
            QueryPoll::Ready(Ok(NominalFieldBinding::Deferred { .. })) => {
                return QueryPoll::Ready(Ok(None));
            }
            // A member this Type does not declare (or does not expose here)
            // is not one of its fields: the access lies elsewhere.
            QueryPoll::Ready(Err(error))
                if super::not_closed(&error) || error.code == "LRR-E-TYPE-SCHEMA" =>
            {
                return QueryPoll::Ready(Ok(None));
            }
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let field = FieldType::of(bound.field_type()).under(args);
        if let Some(field) = &field {
            self.note_depth(super::term_depth(field));
        }
        QueryPoll::Ready(Ok(field))
    }

    /// The written field pattern of a constructor pattern for declaration
    /// field `field`, or `None` when the pattern leaves it to `..`.
    pub(super) fn constructor_subpattern<'p>(
        &self,
        constructed: &Constructed,
        form: &'p ConstructorForm,
        field: u32,
        written: Option<&[String]>,
    ) -> Option<&'p crate::syntax::pattern_tree::PatternNode> {
        match form {
            ConstructorForm::Path => None,
            ConstructorForm::Tuple { items, rest } => {
                let field = field as usize;
                match rest.map(|rest| rest as usize) {
                    Some(rest) if field >= rest => {
                        // Positions after `..` count from the declared end.
                        let from_end = constructed.arity().checked_sub(field)?;
                        let index = items.len().checked_sub(from_end)?;
                        (index >= rest).then(|| &items[index])
                    }
                    _ => items.get(field),
                }
            }
            ConstructorForm::Record { fields, .. } => {
                let written = written?;
                let index = written.iter().position(|name| {
                    self.plan
                        .member_row(name)
                        .and_then(|row| self.written_field(constructed, row))
                        == Some(field)
                })?;
                fields.get(index)
            }
        }
    }
    /// A retained occurrence step through a constructor pattern, resolved
    /// to the selected declaration's field step.
    pub(super) fn occurrence_constructor_step(
        &mut self,
        db: &mut Database,
        root: NodeId,
        slot: u32,
        written: u32,
    ) -> QueryPoll<Option<Step>> {
        let Some(constructed) =
            ready!(self.constructor(db, ConstructorSite::Pattern { root, slot }))
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let plan = self.plan;
        let Some(name) = plan
            .tree(root)
            .and_then(|tree| tree.names().get(slot as usize))
        else {
            return QueryPoll::Ready(Ok(None));
        };
        let field = match &name.form {
            NameForm::Tuple => Some(written),
            NameForm::Record(names) => names
                .get(written as usize)
                .and_then(|name| plan.member_row(name))
                .and_then(|row| self.written_field(&constructed, row)),
            _ => None,
        };
        QueryPoll::Ready(Ok(
            field.and_then(|field| self.field_step(&constructed, field))
        ))
    }
    /// `access` is `base.f`, the variable is the base's nominal Type
    /// `definition<args>` own type argument `position` (the projection
    /// `rest` below it): the declared Type of field `f` under the base's
    /// static nominal Type holds the variable wherever it names that type
    /// parameter; another field relates nothing.
    pub(super) fn type_argument_field(
        &mut self,
        db: &mut Database,
        access: NodeId,
        base: TyTerm,
        position: u32,
        rest: Vec<Step>,
    ) -> QueryPoll<()> {
        let step = match self.fact(access).map(|fact| (fact.field(), fact.member())) {
            Ok((Some(position), _)) => Step::Field(position),
            Ok((None, Some(row))) => Step::Named(row),
            _ => {
                self.defer(access, ConstraintDemand::ExpressionRelation);
                return QueryPoll::Ready(Ok(()));
            }
        };
        let TyTerm::Nominal(definition, args) = base else {
            self.defer(access, ConstraintDemand::ExpressionRelation);
            return QueryPoll::Ready(Ok(()));
        };
        // The variable's position is a marker parameter; every other
        // position is whatever inference decides.
        let marker = crate::traits::term::ParamKey {
            definition: Digest::default(),
            ordinal: u32::MAX,
        };
        let mut seen = 0usize;
        let args: Vec<GenericArg> = args
            .iter()
            .map(|arg| match arg {
                GenericArg::Type(_) => {
                    seen += 1;
                    GenericArg::Type(if seen - 1 == position as usize {
                        TyTerm::Param(marker)
                    } else {
                        super::methods::open()
                    })
                }
                other => other.clone(),
            })
            .collect();
        let Some(field) = ready!(self.nominal_field(db, definition, &args, &step)) else {
            self.defer(access, ConstraintDemand::ExpressionRelation);
            return QueryPoll::Ready(Ok(()));
        };
        for mut path in super::generics::parameter_paths(&field, &marker) {
            path.extend_from_slice(&rest);
            self.expression_via(
                Some(access),
                super::Via {
                    steps: path,
                    strict: false,
                },
            );
        }
        QueryPoll::Ready(Ok(()))
    }
    /// The written record names of a pattern's resolution slot.
    pub(super) fn pattern_record_names(&self, root: NodeId, slot: u32) -> Option<&'a [String]> {
        let plan: &'a crate::syntax::source_constraints::SourceConstraintPlan = self.plan;
        match &plan.tree(root)?.names().get(slot as usize)?.form {
            NameForm::Record(names) => Some(names),
            _ => None,
        }
    }

    /// A record expression member: its selected constructor's fields.
    pub(super) fn record_inward(&mut self, db: &mut Database, node: NodeId) -> QueryPoll<()> {
        let Some(constructed) = ready!(self.constructor(db, ConstructorSite::Expression(node)))
        else {
            self.defer(node, ConstraintDemand::ExpressionRelation);
            return QueryPoll::Ready(Ok(()));
        };
        let plan = self.plan;
        let fact = match self.fact(node) {
            Ok(fact) => fact,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let rows = plan.record_fields(node).unwrap_or(&[]);
        let mut operands = Vec::with_capacity(rows.len());
        for (ordinal, row) in rows.iter().enumerate() {
            if let (Some(field), Some(operand)) = (
                self.written_field(&constructed, *row),
                fact.operand(ordinal),
            ) {
                operands.push((field, operand));
            }
        }
        self.constructed_inward(db, node, &constructed, &operands)
    }
    /// A constructor pattern matched against the current member: the field
    /// pattern of the declared field the projection's first step selects.
    pub(super) fn walk_constructor(
        &mut self,
        db: &mut Database,
        root: NodeId,
        slot: u32,
        form: &ConstructorForm,
    ) -> QueryPoll<()> {
        let Some(first) = self.via.steps.first().copied() else {
            // A constructor's own value is never numeric.
            self.defer(root, ConstraintDemand::NumericFamily);
            return QueryPoll::Ready(Ok(()));
        };
        // A constructor pattern against a reference changes binding modes.
        if matches!(first, Step::AutoDeref | Step::Deref) {
            self.defer(root, ConstraintDemand::Binding);
            return QueryPoll::Ready(Ok(()));
        }
        let Some(constructed) =
            ready!(self.constructor(db, ConstructorSite::Pattern { root, slot }))
        else {
            self.defer(root, ConstraintDemand::Binding);
            return QueryPoll::Ready(Ok(()));
        };
        // The variable is one of the matched Type's own type arguments: every
        // field pattern whose declared Type names that parameter matches it.
        if let Step::TypeArgument(parameter) = first {
            let rest = self.via.unwrap(first).expect("checked first step");
            let written = self.pattern_record_names(root, slot);
            for field in 0..constructed.arity() {
                let Ok(field_number) = u32::try_from(field) else {
                    break;
                };
                let Some(item) =
                    self.constructor_subpattern(&constructed, form, field_number, written)
                else {
                    continue;
                };
                for mut path in constructed.fields[field].parameter_paths(parameter) {
                    path.extend_from_slice(&rest.steps);
                    let current = std::mem::replace(
                        &mut self.via,
                        super::Via {
                            steps: path,
                            strict: false,
                        },
                    );
                    let result = self.walk_current(db, root, item);
                    self.via = current;
                    ready!(result);
                }
            }
            return QueryPoll::Ready(Ok(()));
        }
        match self.field_match(&constructed, &first) {
            FieldMatch::Field(field) => {
                let rest = self.via.unwrap(first).expect("checked first step");
                let written = self.pattern_record_names(root, slot);
                if let Some(item) = self.constructor_subpattern(&constructed, form, field, written)
                {
                    let current = std::mem::replace(&mut self.via, rest);
                    let result = self.walk_current(db, root, item);
                    self.via = current;
                    ready!(result);
                }
            }
            FieldMatch::Elsewhere => {}
            FieldMatch::Unknown => self.defer(root, ConstraintDemand::Binding),
        }
        QueryPoll::Ready(Ok(()))
    }

    /// A constructor operand of declaration field `field` under the current
    /// member's projection: where that field's Type is one of the
    /// constructed Type's type parameters, the constructed value's own type
    /// argument holds the same projection.
    pub(super) fn type_argument_relation(
        &mut self,
        node: NodeId,
        constructed: &Constructed,
        field: u32,
    ) {
        for (parameter, path) in constructed.field_parameters(field) {
            if self.via.steps.starts_with(&path) {
                let rest = self.via.steps[path.len()..].to_vec();
                let mut steps = Vec::with_capacity(rest.len() + 1);
                steps.push(Step::TypeArgument(parameter));
                steps.extend(rest);
                self.expression_via(
                    Some(node),
                    super::Via {
                        steps,
                        strict: false,
                    },
                );
            }
        }
    }
}
