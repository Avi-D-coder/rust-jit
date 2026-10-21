//! The single transactional pattern matcher. Every pattern owner (match arms
//! and guards, `let`, `let`-`else`, `if let`/`while let` clauses and patterns
//! emitted by macros) runs this one non-consuming search over the retained
//! `PatternTree`. Testing reads guest addresses only; ownership changes happen
//! later, in `Frame::commit_pattern`, after the whole pattern has matched.
use super::{DefaultBindingMode, PatternContext, PatternDemand, ReferenceKind};
use crate::{
    core::diagnostic::Diagnostic,
    ir::pc::NodeId,
    runtime::{
        cell::GuestStore,
        place::{Place, PlaceFailure},
        shape::{Primitive, TypeId, TypeKey},
        value::ValueRepr,
    },
    syntax::{
        body_index::ScopeId,
        pattern_tree::{ConstructorForm, PatternLiteral, PatternNode, PatternTree, RangeEndpoint},
    },
    traits::term::TyTerm,
};
use std::{collections::BTreeMap, sync::Arc};

fn unsupported(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-UNSUPPORTED", message.into())
}
fn mismatch(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-TYPE", message.into())
}
fn transaction(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-PATTERN-TRANSACTION", message.into())
}
pub(crate) fn place_fault(failure: PlaceFailure) -> Diagnostic {
    match failure {
        PlaceFailure::RuntimeFault(error) => error,
        PlaceFailure::GuestPanic(_) => transaction("pattern address reported a bounds panic"),
    }
}
pub(crate) fn copy_place(place: &Place) -> Place {
    Place {
        root: place.root,
        projections: place.projections.clone(),
    }
}

pub(crate) fn tuple_fields<'a>(items: &'a [PatternNode], rest: Option<u32>, arity: usize) -> Result<Vec<(u32, &'a PatternNode)>, Diagnostic> {
                let fixed = items.len();
                let before = match rest {
                    Some(rest) if fixed <= arity => rest as usize,
                    None if fixed == arity => fixed,
                    _ => {
                        return Err(mismatch(
                            "tuple-struct pattern arity differs from its constructor",
                        )
                        .into());
                    }
                };
                let mut fields = Vec::with_capacity(fixed);
                for (ordinal, item) in items.iter().enumerate() {
                    let field = if ordinal < before {
                        ordinal
                    } else {
                        arity - (fixed - ordinal)
                    };
                    fields.push((
                        u32::try_from(field).map_err(|_| mismatch("field overflow"))?,
                        item,
                    ));
                }
                Ok(fields)
}

/// Declaration-ordinal mapping shared by runtime matching and type-only
/// binding access preparation. It never examines a guest value.
pub(crate) fn constructor_fields<'a>(
    form: &'a ConstructorForm,
    shape: crate::frontend::types::FieldShape,
    arity: usize,
    ordinals: &[u32],
) -> Result<Vec<(u32, &'a PatternNode)>, Diagnostic> {
        let fields = match form {
            ConstructorForm::Path => {
                if shape != crate::frontend::types::FieldShape::Unit {
                    return Err(mismatch("path pattern names a tuple or record constructor").into());
                }
                Vec::new()
            }
            ConstructorForm::Tuple { items, rest } => {
                if shape != crate::frontend::types::FieldShape::Tuple {
                    return Err(mismatch(
                        "tuple-struct pattern names a unit or record constructor",
                    )
                    .into());
                }
                tuple_fields(items, *rest, arity)?
            }
            ConstructorForm::Record { fields, .. } => {
                if ordinals.len() != fields.len() {
                    return Err(transaction("record pattern field resolution differs").into());
                }
                ordinals.iter().copied().zip(fields.iter()).collect()
            }
        };
    Ok(fields)
}

/// The ordinary namespace resolution of one pattern name slot, prepared by
/// the dispatcher once per PC through the same Value/Type routes ordinary
/// paths, callees and record constructors use.
#[derive(Clone)]
pub(crate) enum PatternTarget {
    /// A fresh binding (plain identifiers only).
    Binding,
    /// A nominal constructor: its bound variant, its declared shape and, for
    /// a record pattern, the declaration ordinal of each written field.
    Constructor {
        variant: Arc<crate::frontend::types::BoundNominalVariant>,
        shape: crate::frontend::types::FieldShape,
        ordinals: Box<[u32]>,
    },
    /// Source-selected constructor whose fields need the actually reached
    /// goal's generic application. No runtime field authority yet.
    ConstructorTemplate {
        template: Arc<crate::frontend::nominal_constructors::ConstructorTemplate>,
        ordinals: Box<[u32]>,
    },
    /// An evaluated constant's actual run-owned storage, never a copied owner.
    Constant(crate::runtime::cell::pattern_constants::ConstantRoot),
    /// Source-selected constant; evaluated only if this search reaches it.
    Initializer(crate::core::digest::Digest),
    /// This name's resolution was refused; the refusal faults only if the
    /// matcher actually reaches the name (cold alternatives stay cold).
    Refused(Diagnostic),
}

/// One binding selected by a successful search, not yet published.
pub(crate) struct Staged {
    pub(crate) binding: NodeId,
    pub(crate) place: Place,
    pub(crate) source: Option<TyTerm>,
    pub(crate) mode: DefaultBindingMode,
    pub(crate) mutable: bool,
}

/// An owner's open transaction. The scrutinee keeps its single owner (the
/// evaluated temporary or the original place); the transaction retains only
/// its checked address.
pub(crate) struct OpenPattern {
    pub(crate) owner: NodeId,
    pub(crate) scope: ScopeId,
    pub(crate) root: Place,
    pub(crate) root_source: Option<TyTerm>,
    pub(crate) tree: Option<Arc<PatternTree>>,
    pub(crate) solution: u64,
    pub(crate) matched: bool,
    pub(crate) staged: Vec<Staged>,
    pub(crate) exposed: Vec<NodeId>,
    pub(crate) nominal_tree: Option<crate::core::digest::Digest>,
    pub(crate) nominal_bindings: BTreeMap<(u32, TypeId, TyTerm), PreparedPatternNominal>,
}

/// One stack per frame, shared by every pattern owner.
#[derive(Default)]
pub(crate) struct OpenPatterns {
    active: Vec<OpenPattern>,
}
impl OpenPatterns {
    pub(crate) fn open(&mut self, pattern: OpenPattern) {
        // Re-entering an owner (a loop iteration, or an arm left by control
        // flow) retires its previous incarnation and everything opened above it.
        if let Some(index) = self
            .active
            .iter()
            .position(|active| active.owner == pattern.owner)
        {
            self.active.truncate(index);
        }
        self.active.push(pattern);
    }
    pub(crate) fn current(&self, owner: NodeId) -> Result<&OpenPattern, Diagnostic> {
        self.find(owner)
            .ok_or_else(|| transaction("pattern owner has no open transaction"))
    }
    pub(crate) fn find(&self, owner: NodeId) -> Option<&OpenPattern> {
        self.active
            .iter()
            .rev()
            .find(|active| active.owner == owner)
    }
    pub(crate) fn current_mut(&mut self, owner: NodeId) -> Result<&mut OpenPattern, Diagnostic> {
        self.active
            .iter_mut()
            .rev()
            .find(|active| active.owner == owner)
            .ok_or_else(|| transaction("pattern owner has no open transaction"))
    }
    pub(crate) fn close(&mut self, owner: NodeId) -> Result<OpenPattern, Diagnostic> {
        let index = self
            .active
            .iter()
            .rposition(|active| active.owner == owner)
            .ok_or_else(|| transaction("pattern owner has no open transaction"))?;
        let closed = self.active.remove(index);
        self.active.truncate(index);
        Ok(closed)
    }
    pub(crate) fn retire_scope(&mut self, scope: ScopeId) {
        if let Some(index) = self.active.iter().position(|active| active.scope == scope) {
            self.active.truncate(index);
        }
    }
}

struct Goal<'t> {
    place: Place,
    source: Option<TyTerm>,
    node: &'t PatternNode,
    context: PatternContext,
}
enum Next<'a, 't> {
    Done,
    Goals(&'a [Goal<'t>], &'a Next<'a, 't>),
}

#[derive(Clone, Copy, PartialEq)]
enum Number {
    Integer(i128),
    Unsigned(u128),
    Float(f64),
    Char(u32),
    Bool(bool),
}
impl Number {
    fn integer_parts(negative: bool, magnitude: u128) -> Result<Self, Diagnostic> {
        if negative {
            if magnitude > 1u128 << 127 {
                return Err(mismatch("negative integer pattern has no signed Rust type"));
            }
            Ok(Self::Integer(0u128.wrapping_sub(magnitude) as i128))
        } else {
            Ok(Self::Unsigned(magnitude))
        }
    }
    /// None is an unordered float comparison (NaN): it satisfies no test.
    fn compare(self, other: Self) -> Result<Option<std::cmp::Ordering>, Diagnostic> {
        Ok(match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => Some(a.cmp(&b)),
            (Self::Unsigned(a), Self::Unsigned(b)) => Some(a.cmp(&b)),
            (Self::Integer(a), Self::Unsigned(b)) => Some(if a < 0 {
                std::cmp::Ordering::Less
            } else {
                (a as u128).cmp(&b)
            }),
            (Self::Unsigned(a), Self::Integer(b)) => Some(if b < 0 {
                std::cmp::Ordering::Greater
            } else {
                a.cmp(&(b as u128))
            }),
            (Self::Char(a), Self::Char(b)) => Some(a.cmp(&b)),
            (Self::Bool(a), Self::Bool(b)) => Some(a.cmp(&b)),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(&b),
            _ => return Err(mismatch("pattern literal type differs from the scrutinee")),
        })
    }
}

#[derive(Clone)]
pub(crate) struct PreparedPatternNominal {
    /// Closed source application used by the guarded member query.
    pub(crate) source: TyTerm,
    pub(crate) variant: Arc<crate::frontend::types::BoundNominalVariant>,
    pub(crate) guard: Arc<crate::db::memo::PreparedInputGuard>,
}
/// One non-consuming search completion or suspension. Every pattern owner
/// and preparation kind retains the same transaction and operation index.
pub(crate) enum SearchOutcome {
    Complete(Option<Vec<Staged>>),
    Prepare(PatternDemand),
}
enum SearchFailure {
    Fault(Diagnostic),
    Prepare(PatternDemand),
}
impl From<Diagnostic> for SearchFailure {
    fn from(error: Diagnostic) -> Self {
        Self::Fault(error)
    }
}

/// The non-consuming search. `skip` counts earlier complete solutions to pass
/// over, so a retry after a false guard continues in written order.
pub(crate) struct Search<'s> {
    store: &'s GuestStore,
    names: Option<&'s [PatternTarget]>,
    skip: u64,
    nominal_bindings: Option<&'s BTreeMap<(u32, TypeId, TyTerm), PreparedPatternNominal>>,
    pub(crate) staged: Vec<Staged>,
}
impl<'s> Search<'s> {
    pub(crate) fn run(
        store: &'s GuestStore,
        names: Option<&'s [PatternTarget]>,
        nominal_bindings: Option<&'s BTreeMap<(u32, TypeId, TyTerm), PreparedPatternNominal>>,
        skip: u64,
        root: &Place,
        root_source: Option<&TyTerm>,
        tree: &PatternTree,
        edition: u16,
    ) -> Result<SearchOutcome, Diagnostic> {
        let mut search = Self {
            store,
            names,
            nominal_bindings,
            skip,
            staged: Vec::new(),
        };
        let goal = Goal {
            place: copy_place(root),
            source: root_source.cloned(),
            node: tree.root(),
            context: PatternContext::new(edition)?,
        };
        match search.solve(&goal, &Next::Done) {
            Ok(true) => Ok(SearchOutcome::Complete(Some(search.staged))),
            Ok(false) => Ok(SearchOutcome::Complete(None)),
            Err(SearchFailure::Prepare(demand)) => Ok(SearchOutcome::Prepare(demand)),
            Err(SearchFailure::Fault(error)) => Err(error),
        }
    }
    fn proceed(&mut self, next: &Next<'_, '_>) -> Result<bool, SearchFailure> {
        match next {
            Next::Done => {
                if self.skip == 0 {
                    Ok(true)
                } else {
                    self.skip -= 1;
                    Ok(false)
                }
            }
            Next::Goals(goals, rest) => match goals.split_first() {
                None => self.proceed(rest),
                Some((goal, tail)) => self.solve(goal, &Next::Goals(tail, rest)),
            },
        }
    }
    fn solve(&mut self, goal: &Goal<'_>, next: &Next<'_, '_>) -> Result<bool, SearchFailure> {
        let mark = self.staged.len();
        let matched = self.solve_inner(goal, next)?;
        if !matched {
            self.staged.truncate(mark);
        }
        Ok(matched)
    }
    fn place_type(&self, place: &Place) -> Result<TypeId, Diagnostic> {
        self.store.place_type_id(place).map_err(place_fault)
    }
    /// An owning scrutinee cell may still hold an unresolved numeric literal
    /// (its type is fixed only by a later use). It is a scalar, never a
    /// reference, and the literal tests compare it by value.
    fn unresolved_literal(&self, place: &Place) -> Result<Option<ValueRepr>, Diagnostic> {
        let crate::runtime::place::PlaceRoot::Cell(cell) = place.root else {
            return Ok(None);
        };
        if !place.projections.is_empty() {
            return Ok(None);
        }
        Ok(match self.store.state(cell)? {
            crate::runtime::cell::CellState::Live(value) if matches!(value.kind, 11 | 12) => {
                Some(value)
            }
            _ => None,
        })
    }
    /// Default binding modes: a non-reference pattern peels each actual
    /// reference layer and moves the default mode toward `ref`/`ref mut`.
    fn peel(
        &self,
        mut place: Place,
        mut source: Option<TyTerm>,
        mut context: PatternContext,
        keep_str_reference: bool,
    ) -> Result<(Place, Option<TyTerm>, PatternContext, Option<TypeId>), Diagnostic> {
        loop {
            if self.unresolved_literal(&place)?.is_some() {
                return Ok((place, source, context, None));
            }
            let ty = self.place_type(&place)?;
            let TypeKey::Ref { mutable, target } = self.store.type_descriptor(ty)?.key.as_ref()
            else {
                return Ok((place, source, context, Some(ty)));
            };
            if keep_str_reference && *target == TypeId(16) {
                return Ok((place, source, context, Some(ty)));
            }
            context = context.peel_implicit(if *mutable {
                ReferenceKind::Mutable
            } else {
                ReferenceKind::Shared
            });
            place = self.store.dereference_place(&place).map_err(place_fault)?;
            source = match source {
                Some(TyTerm::Ref(_, _, pointee)) => Some(*pointee),
                _ => None,
            };
        }
    }
    fn value(&self, place: &Place) -> Result<ValueRepr, Diagnostic> {
        if let Some(value) = self.unresolved_literal(place)? {
            return Ok(value);
        }
        self.store.read_place(place).map_err(place_fault)
    }
    fn number(&self, value: ValueRepr) -> Result<Number, Diagnostic> {
        Ok(match value.kind {
            2 => Number::Bool(value.payload == 1),
            3 => Number::Char(
                u32::try_from(value.payload).map_err(|_| mismatch("char payload overflow"))?,
            ),
            4 => Number::Integer(i128::from(value.payload as i64)),
            5 => Number::Unsigned(u128::from(value.payload)),
            17 if value.type_id == 17 => {
                Number::Integer(self.store.wide_integer_bits(value)? as i128)
            }
            17 => Number::Unsigned(self.store.wide_integer_bits(value)?),
            6 => Number::Float(f64::from(f32::from_bits(value.payload as u32))),
            7 => Number::Float(f64::from_bits(value.payload)),
            11 => {
                let (negative, magnitude) = self
                    .store
                    .integer_parts(value)?
                    .ok_or_else(|| mismatch("integer literal parts absent"))?;
                Number::integer_parts(negative, magnitude)?
            }
            12 => {
                return Err(unsupported(
                    "unsuffixed float scrutinee requires a reached type context",
                ));
            }
            _ => return Err(mismatch("literal pattern requires a scalar scrutinee")),
        })
    }
    fn literal(&self, literal: &PatternLiteral, value: ValueRepr) -> Result<Number, Diagnostic> {
        let concrete = (value.kind != 11 && value.kind != 12).then_some(TypeId(value.type_id));
        let suffix = |suffix: &Option<crate::ir::op::PrimitiveType>| -> Result<(), Diagnostic> {
            if let (Some(suffix), Some(actual)) = (suffix, concrete) {
                let expected = crate::runtime::shape::PrimitiveIdentity::from_recipe(*suffix)?.id();
                if expected != actual {
                    return Err(mismatch(
                        "suffixed pattern literal differs from the scrutinee type",
                    ));
                }
            }
            Ok(())
        };
        Ok(match literal {
            PatternLiteral::Integer {
                negative,
                magnitude,
                suffix: written,
            } => {
                suffix(written)?;
                if matches!(value.kind, 6 | 7) {
                    return Err(mismatch(
                        "integer pattern literal against a float scrutinee",
                    ));
                }
                if let Some(ty) = concrete.filter(|ty| matches!(ty.0, 17 | 18)) {
                    let bits = GuestStore::integer_literal_bits(*negative, *magnitude, ty)?;
                    if ty.0 == 17 {
                        Number::Integer(bits as i128)
                    } else {
                        Number::Unsigned(bits)
                    }
                } else {
                    Number::integer_parts(*negative, *magnitude)?
                }
            }
            PatternLiteral::Byte(byte) => {
                if concrete.is_some_and(|ty| ty != TypeId(9)) {
                    return Err(mismatch("byte pattern literal requires a u8 scrutinee"));
                }
                Number::Integer(i128::from(*byte))
            }
            PatternLiteral::Float {
                negative,
                decimal,
                suffix: written,
            } => {
                suffix(written)?;
                let parsed = if value.kind == 6 {
                    f64::from(
                        decimal
                            .parse::<f32>()
                            .map_err(|_| mismatch("float pattern literal is invalid"))?,
                    )
                } else {
                    decimal
                        .parse::<f64>()
                        .map_err(|_| mismatch("float pattern literal is invalid"))?
                };
                Number::Float(if *negative { -parsed } else { parsed })
            }
            PatternLiteral::Bool(value) => Number::Bool(*value),
            PatternLiteral::Char(value) => Number::Char(u32::from(*value)),
            PatternLiteral::Str(_) | PatternLiteral::Bytes(_) => {
                return Err(unsupported(
                    "string literal pattern requires str comparison integration",
                ));
            }
        })
    }
    fn equals(&self, literal: &PatternLiteral, value: ValueRepr) -> Result<bool, Diagnostic> {
        let actual = self.number(value)?;
        let expected = self.literal(literal, value)?;
        Ok(actual.compare(expected)? == Some(std::cmp::Ordering::Equal))
    }
    fn within(
        &self,
        start: Option<&Result<RangeEndpoint, String>>,
        end: Option<&Result<RangeEndpoint, String>>,
        inclusive: bool,
        value: ValueRepr,
    ) -> Result<bool, SearchFailure> {
        use std::cmp::Ordering;
        let actual = self.number(value)?;
        let compare =
            |endpoint: &Result<RangeEndpoint, String>| -> Result<Option<Ordering>, SearchFailure> {
                match endpoint.as_ref().map_err(|message| unsupported(message))? {
                    RangeEndpoint::Literal(literal) => actual
                        .compare(self.literal(literal, value)?)
                        .map_err(Into::into),
                    RangeEndpoint::Constant(slot) => match self.target(*slot)? {
                        PatternTarget::Constant(bound) => actual
                            .compare(self.number(self.store.pattern_constant_value(*bound)?)?)
                            .map_err(Into::into),
                        PatternTarget::Refused(refusal) => Err(refusal.clone().into()),
                        _ => Err(mismatch("range pattern bound is not a constant").into()),
                    },
                }
            };
        if let Some(start) = start {
            match compare(start)? {
                Some(Ordering::Less) | None => return Ok(false),
                _ => {}
            }
        }
        if let Some(end) = end {
            match compare(end)? {
                Some(Ordering::Less) => {}
                Some(Ordering::Equal) if inclusive => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }
    fn target(&self, slot: u32) -> Result<&'s PatternTarget, SearchFailure> {
        let target = self
            .names
            .and_then(|names| names.get(slot as usize))
            .ok_or_else(|| {
                Diagnostic::new(
                    "LRR-E-PATTERN-BINDING",
                    "pattern name requires its ordinary namespace resolution".into(),
                )
            })?;
        match target {
            PatternTarget::Initializer(identity) => Err(SearchFailure::Prepare(
                PatternDemand::Initializer(*identity),
            )),
            _ => Ok(target),
        }
    }
    /// Compare an evaluated value without consuming either owner. References
    /// peel only until the actual constant type, retaining reference constants.
    fn constant(
        &mut self,
        goal: &Goal<'_>,
        expected: crate::runtime::cell::pattern_constants::ConstantRoot,
        next: &Next<'_, '_>,
    ) -> Result<bool, SearchFailure> {
        let expected_value = self.store.pattern_constant_value(expected)?;
        let mut place = copy_place(&goal.place);
        while self.unresolved_literal(&place)?.is_none() {
            let ty = self.place_type(&place)?;
            if ty.0 == expected_value.type_id
                || !matches!(
                    self.store.type_descriptor(ty)?.key.as_ref(),
                    TypeKey::Ref { .. }
                )
            {
                break;
            }
            place = self.store.dereference_place(&place).map_err(place_fault)?;
        }
        let scalar_equal = |actual, expected| {
            Ok(self.number(actual)?.compare(self.number(expected)?)?
                == Some(std::cmp::Ordering::Equal))
        };
        let equal = if matches!(expected_value.kind, 2..=7 | 11 | 12 | 17) {
            scalar_equal(self.value(&place)?, expected_value)?
        } else {
            self.store
                .compare_pattern_constant(&place, expected, scalar_equal)
                .map_err(place_fault)?
        };
        if equal { self.proceed(next) } else { Ok(false) }
    }
    /// A nominal pattern: peel references, check the descriptor's nominal
    /// identity, compare the active variant without reading or moving the
    /// value, then test the selected declaration-order fields in place.
    fn constructor(
        &mut self,
        goal: &Goal<'_>,
        variant: &crate::frontend::types::BoundNominalVariant,
        fields: &[(u32, &PatternNode)],
        next: &Next<'_, '_>,
    ) -> Result<bool, SearchFailure> {
        let (place, _, context, ty) = self.peel(
            copy_place(&goal.place),
            goal.source.clone(),
            goal.context,
            false,
        )?;
        let ty = ty.ok_or_else(|| mismatch("nominal pattern requires a nominal scrutinee"))?;
        if !matches!(self.store.type_descriptor(ty)?.key.as_ref(),
            TypeKey::Nominal { identity, .. } if *identity == variant.identity())
        {
            return Err(mismatch("nominal pattern names another type than its scrutinee").into());
        }
        if self.store.place_variant(&place).map_err(place_fault)? != Some(variant.variant()) {
            return Ok(false);
        }
        let mut goals = Vec::with_capacity(fields.len());
        // A generic declaration's field Types are instantiated by the
        // scrutinee descriptor's own type arguments.
        let args: Vec<_> = match self.store.type_descriptor(ty)?.key.as_ref() {
            TypeKey::Nominal { args, .. } => args.to_vec(),
            _ => Vec::new(),
        };
        for (ordinal, node) in fields {
            let source = variant
                .field_source(*ordinal, self.store, &args)
                .ok_or_else(|| mismatch("nominal pattern field is absent"))?;
            goals.push(Goal {
                place: self
                    .store
                    .project_nominal_field(&place, variant, *ordinal)
                    .map_err(place_fault)?,
                source: Some(source),
                node,
                context,
            });
        }
        self.proceed(&Next::Goals(&goals, next))
    }
    fn template_constructor(
        &mut self,
        goal: &Goal<'_>,
        slot: u32,
        template: &crate::frontend::nominal_constructors::ConstructorTemplate,
        fields: &[(u32, &PatternNode)],
        next: &Next<'_, '_>,
    ) -> Result<bool, SearchFailure> {
        let (place, source, _, ty) = self.peel(
            copy_place(&goal.place),
            goal.source.clone(),
            goal.context,
            false,
        )?;
        let ty = ty.ok_or_else(|| mismatch("nominal pattern requires a nominal scrutinee"))?;
        if !matches!(self.store.type_descriptor(ty)?.key.as_ref(),
            TypeKey::Nominal {identity, schema, ..}
                if *identity == template.identity && *schema == template.schema)
        {
            return Err(mismatch(
                "pattern source template differs from the actual nominal descriptor",
            )
            .into());
        }
        if self.store.place_variant(&place).map_err(place_fault)? != Some(template.variant) {
            return Ok(false);
        }
        let source = source
            .or_else(|| self.store.descriptor_source_term(ty))
            .ok_or_else(|| {
                unsupported("reached nominal pattern requires its source argument facts")
            })?;
        if let Some(prepared) = self
            .nominal_bindings
            .and_then(|bindings| bindings.get(&(slot, ty, source.clone())))
        {
            return self.constructor(goal, &prepared.variant, fields, next);
        }
        Err(SearchFailure::Prepare(PatternDemand::Nominal {
            slot,
            ty,
            source,
        }))
    }
    fn named_constructor(
        &mut self,
        goal: &Goal<'_>,
        slot: u32,
        form: &ConstructorForm,
        next: &Next<'_, '_>,
    ) -> Result<bool, SearchFailure> {
        let target = self.target(slot)?;
        let (shape, arity, ordinals) = match target {
            PatternTarget::Refused(error) => return Err(error.clone().into()),
            PatternTarget::Initializer(identity) => {
                return Err(SearchFailure::Prepare(PatternDemand::Initializer(
                    *identity,
                )));
            }
            PatternTarget::Constant(value) if matches!(form, ConstructorForm::Path) => {
                return self.constant(goal, *value, next);
            }
            PatternTarget::Constructor {
                variant,
                shape,
                ordinals,
            } => (*shape, variant.fields().len(), ordinals.as_ref()),
            PatternTarget::ConstructorTemplate { template, ordinals } => {
                (template.shape, template.arity, ordinals.as_ref())
            }
            _ => {
                return Err(
                    mismatch("pattern path does not name a constructor of this form").into(),
                );
            }
        };
        let fields = constructor_fields(form, shape, arity, ordinals)?;
        match target {
            PatternTarget::Constructor { variant, .. } => {
                self.constructor(goal, variant, &fields, next)
            }
            PatternTarget::ConstructorTemplate { template, .. } => {
                self.template_constructor(goal, slot, template, &fields, next)
            }
            _ => Err(transaction("pattern constructor target changed during search").into()),
        }
    }
    fn solve_inner(&mut self, goal: &Goal<'_>, next: &Next<'_, '_>) -> Result<bool, SearchFailure> {
        match goal.node {
            PatternNode::Wildcard => self.proceed(next),
            PatternNode::Unsupported(message) => Err(unsupported(message).into()),
            PatternNode::Binding {
                binding,
                by_reference,
                mutable,
                edition,
                name,
                subpattern,
                ..
            } => {
                if let Some((slot, written)) = name {
                    match self.target(*slot)? {
                        PatternTarget::Refused(error) => return Err(error.clone().into()),
                        PatternTarget::Initializer(identity) => {
                            return Err(SearchFailure::Prepare(PatternDemand::Initializer(
                                *identity,
                            )));
                        }
                        PatternTarget::Binding => {}
                        PatternTarget::Constant(value) => {
                            return self.constant(goal, *value, next);
                        }
                        PatternTarget::Constructor {
                            variant,
                            shape: crate::frontend::types::FieldShape::Unit,
                            ..
                        } => return self.constructor(goal, variant, &[], next),
                        PatternTarget::ConstructorTemplate { template, .. }
                            if template.shape == crate::frontend::types::FieldShape::Unit =>
                        {
                            return self.template_constructor(goal, *slot, template, &[], next);
                        }
                        PatternTarget::ConstructorTemplate { .. }
                        | PatternTarget::Constructor { .. } => {
                            return Err(mismatch(&format!(
                                "pattern identifier `{written}` names a tuple or record constructor"
                            ))
                            .into());
                        }
                    }
                }
                let binding = binding.ok_or_else(|| unsupported(
                    "actual pattern binding requires resolved alternative, mode and hygiene facts",
                ))?;
                let context = goal.context.with_edition(*edition)?;
                let modifier = PatternContext::modifier(*by_reference, *mutable);
                let mode = context.binding_mode(modifier)?;
                self.staged.push(Staged {
                    binding,
                    place: copy_place(&goal.place),
                    source: goal.source.clone(),
                    mode,
                    mutable: *mutable && !*by_reference,
                });
                match subpattern {
                    Some(sub) => self.solve(
                        &Goal {
                            place: copy_place(&goal.place),
                            source: goal.source.clone(),
                            node: sub,
                            context: goal.context,
                        },
                        next,
                    ),
                    None => self.proceed(next),
                }
            }
            PatternNode::Literal(PatternLiteral::Bytes(expected)) => {
                let (place, _, _, ty) =
                    self.peel(copy_place(&goal.place), None, goal.context, false)?;
                let ty = ty.ok_or_else(|| mismatch("byte string pattern requires a sequence"))?;
                let (element, length) = match self.store.type_descriptor(ty)?.key.as_ref() {
                    TypeKey::Array { element, len } => (*element, *len),
                    TypeKey::Slice { element } => (
                        *element,
                        self.store.slice_place_len(&place).map_err(place_fault)?,
                    ),
                    _ => {
                        return Err(
                            mismatch("byte string pattern requires an array or slice").into()
                        );
                    }
                };
                if element != TypeId(9) {
                    return Err(mismatch("byte string pattern requires u8 elements").into());
                }
                if length as usize != expected.len() {
                    return Ok(false);
                }
                for (index, expected) in expected.iter().enumerate() {
                    let index = ValueRepr::unsigned(
                        TypeId(13),
                        u64::try_from(index)
                            .map_err(|_| mismatch("byte pattern index overflow"))?,
                    )?;
                    let member = self
                        .store
                        .project_index(&place, index)
                        .map_err(place_fault)?;
                    let value = self.value(&member)?;
                    if value != ValueRepr::unsigned(TypeId(9), u64::from(*expected))? {
                        return Ok(false);
                    }
                }
                self.proceed(next)
            }
            PatternNode::Literal(PatternLiteral::Str(expected)) => {
                // A string literal pattern has type &str: peel references
                // down to the &str and compare its actual shared bytes.
                let (place, _, _, _) =
                    self.peel(copy_place(&goal.place), None, goal.context, true)?;
                let value = self.value(&place)?;
                let crate::runtime::cell::str_storage::ReferenceView::Str(actual) =
                    self.store.reference_view(value)?
                else {
                    return Err(mismatch("string literal pattern requires a &str scrutinee").into());
                };
                if actual == expected {
                    self.proceed(next)
                } else {
                    Ok(false)
                }
            }
            PatternNode::Literal(literal) => {
                let (place, _, _, _) =
                    self.peel(copy_place(&goal.place), None, goal.context, false)?;
                let value = self.value(&place)?;
                if self.equals(literal, value)? {
                    self.proceed(next)
                } else {
                    Ok(false)
                }
            }
            PatternNode::Range {
                start,
                end,
                inclusive,
            } => {
                let (place, _, _, _) =
                    self.peel(copy_place(&goal.place), None, goal.context, false)?;
                let value = self.value(&place)?;
                if self.within(start.as_ref(), end.as_ref(), *inclusive, value)? {
                    self.proceed(next)
                } else {
                    Ok(false)
                }
            }
            PatternNode::Tuple { items, rest } => {
                let (place, source, context, ty) = self.peel(
                    copy_place(&goal.place),
                    goal.source.clone(),
                    goal.context,
                    false,
                )?;
                let ty = ty.ok_or_else(|| mismatch("tuple pattern requires a tuple scrutinee"))?;
                let arity = match self.store.type_descriptor(ty)?.key.as_ref() {
                    TypeKey::Tuple(fields) => fields.len(),
                    TypeKey::Primitive(identity)
                        if crate::runtime::shape::primitive(identity.id())? == Primitive::Unit =>
                    {
                        0
                    }
                    _ => return Err(mismatch("tuple pattern requires a tuple scrutinee").into()),
                };
                let fixed = items.len();
                let (before, after) = match rest {
                    Some(rest) => {
                        if fixed > arity {
                            return Err(mismatch(
                                "tuple pattern has more fields than its scrutinee",
                            )
                            .into());
                        }
                        (*rest as usize, fixed - *rest as usize)
                    }
                    None => {
                        if fixed != arity {
                            return Err(
                                mismatch("tuple pattern arity differs from its scrutinee").into()
                            );
                        }
                        (fixed, 0)
                    }
                };
                let mut goals = Vec::with_capacity(fixed);
                for (ordinal, item) in items.iter().enumerate() {
                    let field = if ordinal < before {
                        ordinal
                    } else {
                        arity - after + (ordinal - before)
                    };
                    let field_u32 =
                        u32::try_from(field).map_err(|_| mismatch("tuple field overflow"))?;
                    goals.push(Goal {
                        place: self
                            .store
                            .project_tuple(&place, field_u32)
                            .map_err(place_fault)?,
                        source: match &source {
                            Some(TyTerm::Tuple(members)) => members.get(field).cloned(),
                            _ => None,
                        },
                        node: item,
                        context,
                    });
                }
                self.proceed(&Next::Goals(&goals, next))
            }
            PatternNode::Array {
                items,
                rest,
                rest_binding,
            } => {
                let (place, source, context, ty) = self.peel(
                    copy_place(&goal.place),
                    goal.source.clone(),
                    goal.context,
                    false,
                )?;
                let ty = ty.ok_or_else(|| mismatch("array pattern requires an array scrutinee"))?;
                // A `[T]` scrutinee (through a slice reference) has its
                // window's runtime length; a pattern of another length is
                // refuted rather than ill-typed.
                let (length, unsized_slice) = match self.store.type_descriptor(ty)?.key.as_ref() {
                    TypeKey::Array { len, .. } => (*len as usize, false),
                    TypeKey::Slice { .. } => (
                        self.store.slice_place_len(&place).map_err(place_fault)? as usize,
                        true,
                    ),
                    _ => return Err(mismatch("array pattern requires an array scrutinee").into()),
                };
                let fixed = items.len();
                let before = match rest {
                    Some(rest) if fixed <= length => *rest as usize,
                    None if fixed == length => fixed,
                    _ if unsized_slice => return Ok(false),
                    _ => {
                        return Err(
                            mismatch("array pattern length differs from its scrutinee").into()
                        );
                    }
                };
                let mut goals = Vec::with_capacity(fixed + 1);
                for (ordinal, item) in items.iter().enumerate() {
                    let index = if ordinal < before {
                        ordinal
                    } else {
                        length - (fixed - ordinal)
                    };
                    let index = ValueRepr::unsigned(
                        TypeId(13),
                        u64::try_from(index).map_err(|_| mismatch("array index overflow"))?,
                    )?;
                    goals.push(Goal {
                        place: self
                            .store
                            .project_index(&place, index)
                            .map_err(place_fault)?,
                        source: match &source {
                            Some(TyTerm::Array(element, _) | TyTerm::Slice(element)) => {
                                Some(element.as_ref().clone())
                            }
                            _ => None,
                        },
                        node: item,
                        context,
                    });
                }
                // `xs @ ..` binds the element window between the fixed items:
                // a `[T]` place (borrowed as `&[T]` under a reference mode) of
                // a slice scrutinee, a sized `[T; K]` sub-array place of an
                // array scrutinee (a fresh array by value, `&[T; K]` by
                // reference).
                if let Some(binding) = rest_binding {
                    let start =
                        u32::try_from(before).map_err(|_| mismatch("slice rest overflow"))?;
                    let end = u32::try_from(length - (fixed - before))
                        .map_err(|_| mismatch("slice rest overflow"))?;
                    let (place, source) = if unsized_slice {
                        (
                            self.store
                                .project_range(
                                    &place,
                                    crate::runtime::place::SliceRange {
                                        start: u64::from(start),
                                        end: Some(u64::from(end)),
                                        inclusive: false,
                                    },
                                )
                                .map_err(place_fault)?,
                            match &source {
                                Some(TyTerm::Slice(element)) => {
                                    Some(TyTerm::Slice(element.clone()))
                                }
                                _ => None,
                            },
                        )
                    } else {
                        (
                            self.store
                                .project_subarray(&place, start, end)
                                .map_err(place_fault)?,
                            match &source {
                                Some(TyTerm::Array(element, _)) => Some(TyTerm::Array(
                                    element.clone(),
                                    crate::traits::term::ConstTerm::Value(u64::from(end - start)),
                                )),
                                _ => None,
                            },
                        )
                    };
                    goals.push(Goal {
                        place,
                        source,
                        node: binding,
                        context,
                    });
                }
                self.proceed(&Next::Goals(&goals, next))
            }
            PatternNode::Reference {
                mutable,
                edition,
                inner,
            } => {
                let context = goal.context.with_edition(*edition)?.explicit_reference()?;
                if self.unresolved_literal(&goal.place)?.is_some() {
                    return Err(mismatch("reference pattern requires a reference scrutinee").into());
                }
                let ty = self.place_type(&goal.place)?;
                let TypeKey::Ref {
                    mutable: actual, ..
                } = self.store.type_descriptor(ty)?.key.as_ref()
                else {
                    return Err(mismatch("reference pattern requires a reference scrutinee").into());
                };
                if actual != mutable {
                    return Err(mismatch(
                        "reference pattern mutability differs from its scrutinee",
                    )
                    .into());
                }
                let place = self
                    .store
                    .dereference_place(&goal.place)
                    .map_err(place_fault)?;
                let source = match &goal.source {
                    Some(TyTerm::Ref(_, _, pointee)) => Some(pointee.as_ref().clone()),
                    _ => None,
                };
                self.solve(
                    &Goal {
                        place,
                        source,
                        node: inner,
                        context,
                    },
                    next,
                )
            }
            PatternNode::Constructor { slot, form } => {
                self.named_constructor(goal, *slot, form, next)
            }
            PatternNode::Or(alternatives) => {
                for alternative in alternatives.iter() {
                    let matched = self.solve(
                        &Goal {
                            place: copy_place(&goal.place),
                            source: goal.source.clone(),
                            node: alternative,
                            context: goal.context,
                        },
                        next,
                    )?;
                    if matched {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}
