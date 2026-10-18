//! Execute a verified recipe. No parser, resolver, compiler, or successor fetch.
use super::frame::{Frame, FrameStatus, invalid};
use crate::{
    core::diagnostic::Diagnostic,
    ir::{
        op::{DecodedLiteral, HintRecipe, LiteralKind, LiteralRecipe, Op, PrimitiveType},
        pc::PcKey,
        recipe::{ControlRecipe, RecipeExit, VerifiedRecipe},
    },
    runtime::{
        cell::{GuestStore, TicketId},
        shape::TypeId,
        value::ValueRepr,
    },
};
#[derive(Debug, serde::Serialize)]
pub struct GuestPanic {
    pub kind: super::numeric::PanicKind,
    pub message: Option<String>,
    pub pc: PcKey,
    pub origin: crate::ir::recipe::Origin,
}
impl GuestPanic {
    pub fn description(&self) -> &str {
        self.message
            .as_deref()
            .unwrap_or_else(|| self.kind.description())
    }
}
#[derive(Debug)]
pub enum StepOutcome {
    /// Bounded operation progress; PC and opcode entry charge remain retained.
    Dispatch,
    Backedge(PcKey),
    Next(PcKey),
    Call {
        action: super::action::CallAction,
        site: crate::ir::recipe::CallSiteId,
        resume: PcKey,
        result: crate::ir::pc::SlotId,
    },
    Return(TicketId),
    GuestPanic(GuestPanic),
}
/// rustc's integer/float fallback for a comparison whose operands are both
/// still-unresolved literals: nothing else constrains their shared type, so it
/// defaults to i32 (or f64 when either is a float literal). The Bool result
/// carries no operand type onward.
fn unconstrained_literal_fallback(left: ValueRepr, right: ValueRepr) -> Option<TypeId> {
    match (left.kind, right.kind) {
        (11 | 12, 11 | 12) if left.kind == 12 || right.kind == 12 => Some(TypeId(15)),
        (11, 11) => Some(TypeId(6)),
        _ => None,
    }
}
fn concrete(hint: &HintRecipe) -> Result<Option<TypeId>, Diagnostic> {
    let HintRecipe::Primitive(p) = hint else {
        return Ok(None);
    };
    Ok(Some(
        crate::runtime::shape::PrimitiveIdentity::from_recipe(*p)?.id(),
    ))
}
fn frame_hint(
    frame: &Frame,
    hint: &HintRecipe,
    required: bool,
    store: &GuestStore,
) -> Result<Option<TypeId>, Diagnostic> {
    if matches!(hint, HintRecipe::DefinitionResult) {
        frame.definition_result_descriptor(store)
    } else if matches!(hint, HintRecipe::DefinitionWrittenResult) {
        match frame.definition_written_result()? {
            Some(written) => written.find_in(store, &[]),
            None => Ok(None),
        }
    } else if let HintRecipe::DefinitionParameter(ordinal) = hint {
        concrete(&HintRecipe::Primitive(
            frame.definition_parameter_primitive(*ordinal)?,
        ))
    } else if let HintRecipe::DefinitionNumeric(context) = hint {
        concrete(&HintRecipe::Primitive(
            frame.definition_numeric_context(*context)?,
        ))
    } else if let HintRecipe::BorrowTarget { reference } = hint {
        frame.borrow_target_descriptor(*reference, store)
    } else if let HintRecipe::Parameter(key) = hint {
        let descriptor = frame.parameter_hint(*key)?;
        if required && descriptor.is_none() {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "generic local annotation requires a prepared runtime binding".into(),
            ));
        }
        Ok(descriptor)
    } else if let HintRecipe::Reference {
        mutable, target, ..
    } = hint
    {
        let target = crate::runtime::shape::PrimitiveIdentity::from_recipe(*target)?.id();
        Ok(store.find_type(&crate::runtime::shape::TypeKey::Ref {
            mutable: *mutable,
            target,
        }))
    } else if let HintRecipe::ArrayPrimitive { element, len } = hint {
        let element = crate::runtime::shape::PrimitiveIdentity::from_recipe(*element)?.id();
        Ok(store.find_type(&crate::runtime::shape::TypeKey::Array { element, len: *len }))
    } else if let HintRecipe::Written(written) = hint {
        written.find(store)
    } else {
        concrete(hint)
    }
}
/// A written annotation with an inference hole, or a slice/trait-object
/// pointee, names a structure rather than one descriptor: the reached
/// initializer must have that structure. A sized value under an unsized
/// written pointee needs an unsizing coercion, which is refused explicitly
/// rather than binding the value under a Type it does not have.
fn written_binding_admission(
    frame: &Frame,
    source: crate::ir::pc::SlotId,
    hint: &HintRecipe,
    store: &GuestStore,
) -> Result<(), Diagnostic> {
    let HintRecipe::Written(written) = hint else {
        return Ok(());
    };
    if written.exact() {
        return Ok(());
    }
    let value = store.admit_move(frame.cell(source)?)?;
    let admitted = match store.effective_type(value)? {
        Some(ty) => written.admits(ty, store)?,
        None => matches!(written.as_ref(), crate::ir::written_type::WrittenType::Infer),
    };
    // A typed `let` is a coercion site: `&[T; N]` meets `&[T]` by unsizing.
    if admitted || frame.written_unsizing(source, written, store)?.is_some() {
        return Ok(());
    }
    Err(Diagnostic::new(
        "LRR-E-UNSUPPORTED",
        "written annotation requires an unsizing coercion or partial-Type inference of its initializer, which awaits slice/trait-object reference integration".into(),
    ))
}
/// Descriptor publication is an executed VM action, never pure preflight work.
/// An initializer frame's root value takes its declared reference result
/// as context, as a function body's root block does
/// (`HintRecipe::DefinitionResult`), so its coercions (`&[u8; 2]` to
/// `&[u8]`) apply at the root (E-R5-007).
pub(crate) fn initializer_root_hint(
    frame: &mut Frame,
    store: &mut GuestStore,
) -> Result<(), Diagnostic> {
    let Some(child) = frame.body.nodes().first().and_then(|root| root.value) else {
        return Err(invalid("initializer root value absent"));
    };
    let hint = HintRecipe::DefinitionResult;
    frame_hint(frame, &hint, false, store)?;
    frame.preflight_hint(child)?;
    let descriptor = materialize_hint(frame, &hint, false, store)?;
    frame.declared_hint(child, descriptor)?;
    frame.set_source_view(child, frame.resolved_annotation_source(&hint)?)?;
    Ok(())
}
fn materialize_hint(
    frame: &Frame,
    hint: &HintRecipe,
    required: bool,
    store: &mut GuestStore,
) -> Result<Option<TypeId>, Diagnostic> {
    if matches!(hint, HintRecipe::DefinitionResult) {
        return frame
            .materialize_definition_result_descriptor(store)
            .map(Some);
    }
    if matches!(hint, HintRecipe::DefinitionWrittenResult) {
        let Some(written) = frame.definition_written_result()?.cloned() else {
            return Ok(None);
        };
        return written.materialize_in(store, &[]);
    }
    if let HintRecipe::Reference {
        mutable, target, ..
    } = hint
    {
        let target = crate::runtime::shape::PrimitiveIdentity::from_recipe(*target)?.id();
        return store
            .intern_type(crate::runtime::shape::TypeKey::Ref {
                mutable: *mutable,
                target,
            })
            .map(Some);
    }
    if let HintRecipe::ArrayPrimitive { element, len } = hint {
        let element = crate::runtime::shape::PrimitiveIdentity::from_recipe(*element)?.id();
        return store
            .intern_type(crate::runtime::shape::TypeKey::Array { element, len: *len })
            .map(Some);
    }
    if let HintRecipe::Written(written) = hint {
        return written.materialize(store);
    }
    frame_hint(frame, hint, required, store)
}
enum PreparedLiteral {
    String(super::frame::StringLiteralPublication),
    /// `b"..."`: a shared reference to the site's promoted `[u8; N]`.
    ByteString,
    Scalar(ValueRepr),
    /// Private immutable admission from the ordinary indexed literal's kind,
    /// source span and destination, never from a runtime descriptor.
    IntrinsicScalar {
        value: ValueRepr,
        primitive: PrimitiveType,
    },
    Integer {
        negative: bool,
        magnitude: u128,
        ty: Option<TypeId>,
        source: Option<PrimitiveType>,
    },
    Float {
        ty: Option<TypeId>,
        source: Option<PrimitiveType>,
    },
}
impl PreparedLiteral {
    fn intrinsic_source(&self) -> Option<PrimitiveType> {
        match self {
            Self::IntrinsicScalar { primitive, .. } => Some(*primitive),
            Self::Integer { source, .. } | Self::Float { source, .. } => *source,
            Self::Scalar(_) | Self::String(_) | Self::ByteString => None,
        }
    }
    fn ty(&self) -> Option<TypeId> {
        match self {
            Self::Scalar(value) | Self::IntrinsicScalar { value, .. } => {
                Some(TypeId(value.type_id))
            }
            Self::Integer { ty, .. } | Self::Float { ty, .. } => *ty,
            Self::String(_) | Self::ByteString => None,
        }
    }
}
fn prepare_literal(
    recipe: &LiteralRecipe,
    hint: Option<TypeId>,
) -> Result<PreparedLiteral, Diagnostic> {
    let kind = recipe.kind;
    let spelling = recipe.spelling.as_str();
    match kind {
        LiteralKind::Bool => match spelling {
            "true" => Ok(PreparedLiteral::Scalar(ValueRepr::boolean(true))),
            "false" => Ok(PreparedLiteral::Scalar(ValueRepr::boolean(false))),
            _ => Err(invalid("invalid bool literal spelling")),
        },
        LiteralKind::Integer | LiteralKind::NegativeInteger => {
            let (negative, suffix, magnitude) = match &recipe.decoded {
                Some(DecodedLiteral::Integer {
                    negative,
                    suffix,
                    magnitude,
                    ..
                }) => (*negative, *suffix, Some(*magnitude)),
                Some(DecodedLiteral::InvalidInteger {
                    negative,
                    suffix,
                    reason,
                    ..
                }) => {
                    if *reason == crate::ir::op::IntegerLexicalError::UnsupportedWidth {
                        return Err(Diagnostic::new(
                            "LRR-E-UNSUPPORTED",
                            "128-bit guest integers are outside the live profile".into(),
                        ));
                    }
                    (*negative, *suffix, None)
                }
                _ => return Err(invalid("integer has no pinned decoded payload")),
            };
            if negative != (kind == LiteralKind::NegativeInteger) {
                return Err(invalid("integer sign differs from decoded payload"));
            }
            if matches!(suffix, Some(PrimitiveType::I128 | PrimitiveType::U128)) {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "128-bit guest integers are outside the live profile".into(),
                ));
            }
            let magnitude = magnitude
                .ok_or_else(|| invalid("invalid integer literal spelling or magnitude"))?;
            let suffix = suffix
                .map(crate::runtime::shape::PrimitiveIdentity::from_recipe)
                .transpose()?
                .map(|p| p.id());
            let ty = suffix.or(hint);
            if let Some(ty) = ty {
                GuestStore::preview_integer(negative, magnitude, ty)?;
            }
            Ok(PreparedLiteral::Integer {
                negative,
                magnitude,
                ty,
                source: None,
            })
        }
        LiteralKind::Float => {
            let Some(DecodedLiteral::Float { decimal, suffix }) = &recipe.decoded else {
                return Err(invalid("float has no pinned decoded payload"));
            };
            let suffix = if let Some(suffix) = suffix {
                concrete(&HintRecipe::Primitive(*suffix))?
            } else {
                None
            };
            let ty = suffix.or(hint);
            if let Some(ty) = ty {
                GuestStore::preview_float(decimal, ty)?;
            }
            Ok(PreparedLiteral::Float { ty, source: None })
        }
        LiteralKind::Char => match recipe.decoded {
            Some(DecodedLiteral::Char(value)) => {
                Ok(PreparedLiteral::Scalar(ValueRepr::character(value)))
            }
            _ => Err(invalid("char has no pinned decoded payload")),
        },
        LiteralKind::Byte => match recipe.decoded {
            Some(DecodedLiteral::Byte(value)) => Ok(PreparedLiteral::Scalar(ValueRepr::unsigned(
                TypeId(9),
                u64::from(value),
            )?)),
            _ => Err(invalid("byte has no pinned decoded payload")),
        },
        _ => Err(Diagnostic::new(
            "LRR-E-UNSUPPORTED",
            "literal kind awaits numeric/source integration".into(),
        )),
    }
}
fn commit_literal(
    recipe: &LiteralRecipe,
    prepared: PreparedLiteral,
    store: &mut GuestStore,
) -> Result<TicketId, Diagnostic> {
    let (ticket, ty) = match prepared {
        PreparedLiteral::String(_) | PreparedLiteral::ByteString => {
            return Err(invalid(
                "string literal requires ordinary source publication",
            ));
        }
        PreparedLiteral::Scalar(value) | PreparedLiteral::IntrinsicScalar { value, .. } => {
            return store.ticket(value);
        }
        PreparedLiteral::Integer {
            negative,
            magnitude,
            ty,
            ..
        } => (
            store.integer_literal(negative, magnitude, recipe.spelling.clone())?,
            ty,
        ),
        PreparedLiteral::Float { ty, .. } => {
            let Some(DecodedLiteral::Float { decimal, .. }) = &recipe.decoded else {
                return Err(invalid("prepared float data absent"));
            };
            (store.float_literal(decimal.clone())?, ty)
        }
    };
    if let Some(ty) = ty {
        let value = store.materialize_ticket(ticket, ty)?;
        store.discard(ticket)?;
        Ok(value)
    } else {
        Ok(ticket)
    }
}
/// Ordinary Bool/Char/Byte literals and explicitly suffixed numeric literals
/// carry their written primitive identity. Generated literals keep their
/// separate validated expansion route; numeric defaults, variables and arbitrary
/// descriptors cannot mint this source view.
fn intrinsic_literal_preflight(
    frame: &Frame,
    literal: &LiteralRecipe,
    destination: crate::ir::pc::SlotId,
    prepared: PreparedLiteral,
) -> Result<PreparedLiteral, Diagnostic> {
    use crate::{
        syntax::body_index::{ExpressionKind, NodeRole},
        traits::term::TyTerm,
    };
    let primitive = match literal.kind {
        LiteralKind::Bool => PrimitiveType::Bool,
        LiteralKind::Char => PrimitiveType::Char,
        LiteralKind::Byte => PrimitiveType::U8,
        LiteralKind::Integer | LiteralKind::NegativeInteger | LiteralKind::Float => {
            match &literal.decoded {
                Some(DecodedLiteral::Integer {
                    suffix: Some(primitive),
                    ..
                })
                | Some(DecodedLiteral::Float {
                    suffix: Some(primitive),
                    ..
                }) => *primitive,
                _ => return Ok(prepared),
            }
        }
        _ => return Ok(prepared),
    };
    let node = frame
        .body
        .nodes()
        .get(frame.pc().node as usize)
        .ok_or_else(|| invalid("intrinsic literal indexed node absent"))?;
    let original_literal = node.role == NodeRole::Expression(ExpressionKind::Literal);
    let original_negative = literal.kind == LiteralKind::NegativeInteger
        && node.role == NodeRole::Expression(ExpressionKind::Other)
        && node.negative_integer_literal;
    if !original_literal && !original_negative {
        return Ok(prepared);
    }
    if frame.pc().phase != 0
        || node.value != Some(destination)
        || frame.body.span(node.id)? != literal.source
    {
        return Err(invalid(
            "intrinsic literal differs from its indexed source entry",
        ));
    }
    let descriptor = crate::runtime::shape::PrimitiveIdentity::from_recipe(primitive)?.id();
    if prepared.ty() != Some(descriptor) {
        return Err(Diagnostic::new(
            "LRR-E-TYPE-CONTEXT",
            "intrinsic literal differs from reached descriptor context".into(),
        ));
    }
    let source = TyTerm::Primitive(primitive);
    if frame
        .literal_source_context(destination)?
        .is_some_and(|expected| expected != &source)
    {
        return Err(Diagnostic::new(
            "LRR-E-TYPE-CONTEXT",
            "intrinsic literal differs from reached symbolic source context".into(),
        ));
    }
    frame.preflight_source_view(destination, Some(&source))?;
    match prepared {
        PreparedLiteral::Scalar(value) => Ok(PreparedLiteral::IntrinsicScalar { value, primitive }),
        PreparedLiteral::Integer {
            negative,
            magnitude,
            ty,
            ..
        } => {
            // Retain the numeric literal ticket/materialization path. Source
            // metadata does not allocate a scalar or skip numeric evaluation.
            Ok(PreparedLiteral::Integer {
                negative,
                magnitude,
                ty,
                source: Some(primitive),
            })
        }
        PreparedLiteral::Float { ty, .. } => Ok(PreparedLiteral::Float {
            ty,
            source: Some(primitive),
        }),
        PreparedLiteral::IntrinsicScalar { .. } => {
            Err(invalid("intrinsic literal source admission repeated"))
        }
        PreparedLiteral::String(_) | PreparedLiteral::ByteString => {
            Err(invalid("string literal entered scalar source admission"))
        }
    }
}
fn literal_preflight(
    frame: &Frame,
    store: &GuestStore,
    op: &Op,
) -> Result<Option<PreparedLiteral>, Diagnostic> {
    let Op::Literal {
        literal,
        hint,
        destination,
    } = op
    else {
        return Ok(None);
    };
    if literal.kind == LiteralKind::String {
        let Some(DecodedLiteral::String(proof)) = &literal.decoded else {
            return Err(invalid("string literal lacks source-certified payload"));
        };
        if *hint != HintRecipe::None {
            return Err(invalid(
                "string literal recipe cannot supply scalar/type modifiers",
            ));
        }
        return frame
            .preflight_string_literal(proof, *destination, store)
            .map(|ready| Some(PreparedLiteral::String(ready)));
    }
    if literal.kind == LiteralKind::ByteString {
        let Some(DecodedLiteral::ByteString(_)) = &literal.decoded else {
            return Err(invalid("byte string literal lacks its decoded bytes"));
        };
        return Ok(Some(PreparedLiteral::ByteString));
    }
    let explicit = frame_hint(frame, hint, true, store)?;
    let inherited = frame.hint(*destination)?;
    if explicit.is_some() && inherited.is_some() && explicit != inherited {
        return Err(invalid(
            "literal recipe conflicts with the reached parent hint",
        ));
    }
    let hint = explicit.or(inherited);
    let prepared = prepare_literal(literal, hint)?;
    frame.preflight_literal(*destination, prepared.ty(), hint, store)?;
    let prepared = intrinsic_literal_preflight(frame, literal, *destination, prepared)?;
    Ok(Some(prepared))
}
fn assertion_preflight(
    frame: &Frame,
    store: &GuestStore,
    op: &Op,
) -> Result<Option<bool>, Diagnostic> {
    let Op::Assert {
        condition,
        destination,
        ..
    } = op
    else {
        return Ok(None);
    };
    let value = store.preview_scalar(frame.cell(*condition)?, None)?;
    if value.kind != 2 || value.type_id != 2 {
        return Err(Diagnostic::new(
            "LRR-E-OP-TYPE",
            "assertion requires a concrete bool".into(),
        ));
    }
    let passed = value.payload != 0;
    if passed {
        frame.preflight_write_type(*destination, TypeId(1), store)?;
    }
    Ok(Some(passed))
}
fn place_preflight(
    frame: &Frame,
    store: &GuestStore,
    op: &Op,
) -> Result<Option<crate::runtime::place::PlacePanic>, Diagnostic> {
    use crate::runtime::place::PlaceFailure;
    let result = match op {
        Op::Borrow {
            place,
            access,
            destination,
        } => frame.preflight_borrow(*place, *access, *destination, store),
        Op::DerefPlace {
            reference,
            destination,
        } => frame.preflight_deref_place(*reference, *destination, store),
        Op::ReadPlace { place, destination } => {
            frame.preflight_read_place(*place, *destination, store)
        }
        Op::AssignPlace {
            place,
            source,
            destination,
        } => frame.preflight_assign_place(*place, *source, *destination, store),
        Op::ComparisonOperand {
            place,
            read,
            destination,
        } => frame.preflight_comparison_operand(*place, *read, *destination, store),
        _ => return Ok(None),
    };
    match result {
        Ok(()) => Ok(None),
        Err(PlaceFailure::RuntimeFault(error)) => Err(error),
        Err(PlaceFailure::GuestPanic(panic)) => Ok(Some(panic)),
    }
}
fn place_operation(
    frame: &mut Frame,
    store: &mut GuestStore,
    op: &Op,
) -> Result<bool, crate::runtime::place::PlaceFailure> {
    match op {
        Op::Borrow {
            place,
            access,
            destination,
        } => frame.borrow(*place, *access, *destination, store)?,
        Op::DerefPlace {
            reference,
            destination,
        } => frame.deref_place(*reference, *destination, store)?,
        Op::ReadPlace { place, destination } => frame.read_place(*place, *destination, store)?,
        Op::AssignPlace {
            place,
            source,
            destination,
        } => frame.assign_place(*place, *source, *destination, store)?,
        Op::MethodReceiver {
            site,
            place,
            scratch,
            destination,
        } => frame.method_receiver(site.0, *place, *scratch, *destination, store)?,
        Op::ComparisonOperand {
            place,
            read,
            destination,
        } => frame.comparison_operand(*place, *read, *destination, store)?,
        _ => return Ok(false),
    }
    Ok(true)
}
fn place_panic_outcome(
    frame: &mut Frame,
    recipe: &crate::ir::recipe::Recipe,
    panic: crate::runtime::place::PlacePanic,
) -> StepOutcome {
    // Element and range bounds checks are both rustc index panics; the
    // message carries rustc's exact formatted bounds text.
    let kind = super::numeric::PanicKind::IndexOutOfBounds;
    frame.status = FrameStatus::Panicked;
    StepOutcome::GuestPanic(GuestPanic {
        kind,
        message: Some(panic.message()),
        pc: recipe.key.clone(),
        origin: recipe.origin.clone(),
    })
}
fn state_preflight(frame: &Frame, store: &GuestStore, op: &Op) -> Result<(), Diagnostic> {
    match op {
        Op::SourceConstant { .. } => Err(invalid(
            "source constant bypassed its current-proof opcode boundary",
        )),
        Op::PrimitiveFloatingConstant {
            constant,
            destination,
        } => frame.preflight_primitive_floating_constant(constant, *destination, store),
        Op::ValuePlace {
            source,
            destination,
        }
        | Op::PromotedPlace {
            source,
            destination,
        } => frame.preflight_value_place(*source, *destination, store),
        Op::ArrayRepeat {
            source,
            count,
            destination,
        } => frame.preflight_array_repeat(*source, *count, *destination, store),
        // Always dispatched to its lang-item trait method.
        Op::IteratorCall { .. } => Ok(()),
        Op::PlaceTransfer {
            source,
            destination,
        } => frame.preflight_place_transfer(*source, *destination, store),
        Op::MakePlace {
            binding,
            destination,
        } => frame.preflight_make_place(*binding, *destination, store),
        Op::ParameterPlace {
            ordinal,
            destination,
        } => frame.preflight_parameter_place(*ordinal, *destination, store),
        Op::ConstructStart { guard } => frame.preflight_prepared_constructor(*guard),
        Op::ConstructField {
            guard,
            field,
            source,
        } => frame.preflight_constructor_field(*guard, *field, *source, store),
        Op::ConstructFinish { guard, destination } => {
            frame.preflight_constructor_finish(*guard, *destination, store)
        }
        Op::ValuePath { destination, .. } => frame.preflight_value_path(*destination, store),
        Op::MakeClosure {
            captures,
            destination,
            ..
        } => frame.preflight_make_closure(captures, *destination, store),
        Op::Declaration => frame.preflight_item(),
        Op::DeclareLocal { binding, hint, .. } => {
            frame_hint(frame, hint, true, store)?;
            frame.preflight_declaration(*binding)
        }
        Op::Pattern { operation } => frame.preflight_pattern(operation, store),
        Op::BindLocal {
            binding,
            source,
            hint,
            ..
        } => {
            // An unsizing initializer binds before its coercion; the written
            // `&[T]` Type then holds for the coerced value.
            let unsize = match hint {
                HintRecipe::Written(written) if !written.exact() => {
                    frame.written_unsizing(*source, written, store)?
                }
                _ => None,
            };
            frame.preflight_bind(
                *binding,
                *source,
                match unsize {
                    Some(_) => None,
                    None => frame_hint(frame, hint, true, store)?,
                },
                store,
            )?;
            written_binding_admission(frame, *source, hint, store)?;
            frame.preflight_binding_source(*source, hint)
        }
        Op::ReadLocal {
            binding,
            destination,
        } => frame.preflight_read_local(*binding, *destination, store),
        Op::AssignLocal {
            binding,
            source,
            destination,
        } => frame.preflight_assign(*binding, *source, *destination, store),
        Op::ResultTransfer {
            source,
            destination,
        } => {
            frame.certified_root_transfer_source(*source, *destination, store)?;
            frame.preflight_transfer(*source, *destination, store)
        }
        Op::Discard { source } => frame.preflight_discard(*source, store),
        Op::Unit { destination } => frame.preflight_write_type(*destination, TypeId(1), store),
        Op::ScopeEnter { scope } => frame.preflight_enter(*scope),
        Op::ScopeLeave { scope } => frame.preflight_cleanup_scope(*scope),
        Op::SetHint { child, hint } => {
            if frame.normalized_root_return_hint(*child, hint)?.is_some() {
                return frame.preflight_hint(*child);
            }
            frame_hint(
                frame,
                hint,
                Some(*child) != frame.body.nodes()[0].value,
                store,
            )?;
            frame.preflight_hint(*child)
        }
        Op::NumericHint {
            parent,
            child,
            left,
            operator,
        } => frame.preflight_numeric_hint(*parent, *child, *left, *operator, store),
        Op::CompoundHint {
            binding,
            child,
            kind,
        } => frame.preflight_compound_hint(*binding, *child, *kind, store),
        Op::LocalHint { binding, child } => frame.preflight_local_hint(*binding, *child, store),
        Op::BranchHint {
            parent,
            condition,
            yes,
            no,
            boolean,
        } => frame.preflight_branch_hint(*parent, *condition, *yes, *no, *boolean, store),
        Op::CheckParameters { count } => frame.check_parameters(*count),
        Op::ReadParameter {
            ordinal,
            destination,
        } => frame.preflight_parameter(*ordinal, *destination, store),
        Op::CallPrepare {
            site, arguments, ..
        } => frame.preflight_call_start(site.0, arguments),
        Op::CallAdvance { site, argument } => frame.preflight_call_advance(site.0, *argument),
        Op::CallSeal { .. } => {
            if !frame.call_ready || frame.call_arguments.is_some() {
                return Err(invalid(
                    "call seal requires an admitted unsealed source call",
                ));
            }
            frame.actions.preflight_publish()
        }
        Op::CheckUnit { source } => {
            let crate::runtime::cell::CellState::Live(value) = store.state(frame.cell(*source)?)?
            else {
                return Err(invalid("unit check requires a live result"));
            };
            if value.kind != 1 || value.type_id != 1 {
                return Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "if without else requires a unit arm result".into(),
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
fn exit_preflight(frame: &Frame, store: &GuestStore, exit: &RecipeExit) -> Result<(), Diagnostic> {
    match exit {
        RecipeExit::Call { .. } => {
            if !frame.call_ready || frame.call_arguments.is_none() {
                return Err(invalid("Call exit requires a sealed argument packet"));
            }
            frame.actions.preflight_publish()
        }
        RecipeExit::Next(_) | RecipeExit::Fault(_) | RecipeExit::MatchNoArm => Ok(()),
        RecipeExit::Backedge(pc) => {
            let node = &frame.body.nodes()[pc.node as usize];
            // A `for` loop's head enters one iteration, inside its own scope.
            let scope = if node.role
                == crate::syntax::body_index::NodeRole::Expression(
                    crate::syntax::body_index::ExpressionKind::For,
                ) {
                node.condition_scope
                    .ok_or_else(|| invalid("for iteration scope absent"))?
            } else {
                node.scope
            };
            frame.preflight_enter(scope)
        }
        RecipeExit::Branch { condition, .. } => {
            let value = store.preview_scalar(frame.cell(*condition)?, None)?;
            if value.kind != 2 || value.type_id != 2 {
                return Err(invalid("branch condition is not a concrete bool"));
            }
            Ok(())
        }
        RecipeExit::Control(ControlRecipe::Return { value }) => {
            frame.preflight_return(*value, store)
        }
        // A resumed control exit already moved its value; its cleanup
        // activation retains the admitted exit.
        RecipeExit::Control(ControlRecipe::Break { .. } | ControlRecipe::Continue { .. })
            if frame.has_control_cleanup() =>
        {
            Ok(())
        }
        RecipeExit::Control(ControlRecipe::Break { target, value, .. }) => {
            frame.preflight_control(*target, *value, false, store)
        }
        RecipeExit::Control(ControlRecipe::Continue { target }) => {
            frame.preflight_control(*target, None, true, store)
        }
        RecipeExit::Effect { .. } => Err(Diagnostic::new(
            "LRR-E-UNSUPPORTED",
            "call/effect dispatcher boundary awaits its ordered task".into(),
        )),
    }
}
/// Immutable result/source admission, retained across this opcode's step.
struct PreparedNumericResult {
    destination: crate::ir::pc::SlotId,
    ty: TypeId,
    source: Option<PrimitiveType>,
}
fn operator_operand_source(
    frame: &Frame,
    slot: crate::ir::pc::SlotId,
    ty: TypeId,
    forwarded: bool,
) -> Result<Option<PrimitiveType>, Diagnostic> {
    use crate::traits::term::TyTerm;
    let source = match frame.source_view_ref(slot)? {
        None => return Ok(None),
        Some(TyTerm::Primitive(primitive)) => *primitive,
        Some(TyTerm::Ref(_, false, target)) if forwarded => {
            let TyTerm::Primitive(primitive) = target.as_ref() else {
                return Err(Diagnostic::new(
                    "LRR-E-GENERIC-BINDING",
                    "builtin operator forwarding requires an admitted primitive source target"
                        .into(),
                ));
            };
            *primitive
        }
        Some(_) => {
            return Err(Diagnostic::new(
                "LRR-E-GENERIC-BINDING",
                "builtin operator cannot replace a rigid or nonprimitive operand source".into(),
            ));
        }
    };
    // Projection is source -> descriptor only; equal layout cannot mint source.
    if crate::runtime::shape::PrimitiveIdentity::from_recipe(source)?.id() != ty {
        return Err(Diagnostic::new(
            "LRR-E-GENERIC-BINDING",
            "builtin operator operand descriptor differs from its admitted primitive source".into(),
        ));
    }
    Ok(Some(source))
}
fn numeric_preflight(
    frame: &Frame,
    store: &GuestStore,
    op: &Op,
) -> Result<Option<PreparedNumericResult>, Diagnostic> {
    use super::numeric::{self, NumericFailure};
    let runtime = |error| match error {
        NumericFailure::RuntimeFault(error) => error,
        NumericFailure::GuestPanic(_) => invalid("type validation attempted arithmetic"),
    };
    let (destination, ty, source) = match op {
        Op::NumericBinary {
            operator,
            left,
            right,
            destination,
        } => {
            use crate::ir::op::NumericBinary as B;
            let left_slot = *left;
            let left_cell = frame.cell(left_slot)?;
            let right_cell = frame.cell(*right)?;
            let crate::runtime::cell::CellState::Live(left_raw) = store.state(left_cell)? else {
                return Err(invalid("left operand not live"));
            };
            let crate::runtime::cell::CellState::Live(right_raw) = store.state(right_cell)? else {
                return Err(invalid("right operand not live"));
            };
            // Non-primitive operands select the operator's trait method
            // before this preflight (`Frame::overloaded_operator`).
            for raw in [left_raw, right_raw] {
                let primitive_reference = raw.kind == 9
                    && matches!(
                        store.type_descriptor(TypeId(raw.type_id))?.key.as_ref(),
                        crate::runtime::shape::TypeKey::Ref { target, .. }
                            if target.0 != 16
                                && matches!(
                                    store.type_descriptor(*target)?.key.as_ref(),
                                    crate::runtime::shape::TypeKey::Primitive(_)
                                )
                    );
                if !(1..=7).contains(&raw.kind)
                    && raw.kind != 11
                    && raw.kind != 12
                    && !primitive_reference
                {
                    return Err(invalid(
                        "non-primitive operator operand bypassed its trait method dispatch",
                    ));
                }
            }
            let shift = matches!(operator, B::Shl | B::Shr);
            let comparison = matches!(operator, B::Eq | B::Ne | B::Lt | B::Le | B::Gt | B::Ge);
            let left_borrowed = if comparison {
                None
            } else {
                numeric::borrowed_operand(store, left_cell)?
            };
            let right_borrowed = if comparison {
                None
            } else {
                numeric::borrowed_operand(store, right_cell)?
            };
            let common = left_borrowed
                .map(|value| TypeId(value.type_id))
                .or(store.effective_type(left_raw)?)
                .or(right_borrowed.map(|value| TypeId(value.type_id)))
                .or(store.effective_type(right_raw)?)
                .or(if comparison {
                    unconstrained_literal_fallback(left_raw, right_raw)
                } else {
                    frame.hint(*destination)?
                });
            let left = if let Some(value) = left_borrowed {
                value
            } else {
                store.preview_scalar(
                    left_cell,
                    if shift {
                        frame.hint(*destination)?
                    } else {
                        common
                    },
                )?
            };
            // Concrete source count width is independent of the left/result.
            // Admission validates its range before charged numeric execution.
            let right_ty = TypeId(
                if let Some(value) = right_borrowed {
                    value
                } else {
                    store.preview_scalar(
                        right_cell,
                        if shift { frame.hint(*right)? } else { common },
                    )?
                }
                .type_id,
            );
            numeric::binary_types(*operator, TypeId(left.type_id), right_ty).map_err(runtime)?;
            let left_source = operator_operand_source(
                frame,
                left_slot,
                TypeId(left.type_id),
                left_borrowed.is_some(),
            )?;
            let right_source =
                operator_operand_source(frame, *right, right_ty, right_borrowed.is_some())?;
            // Closed builtin comparisons return Bool by their operator contract,
            // even if a suffixed literal has no retained numeric source tree.
            // Arithmetic may only forward actual admitted operand evidence.
            let source = if comparison {
                Some(PrimitiveType::Bool)
            } else if shift {
                left_source
            } else {
                left_source.or(right_source)
            };
            (
                *destination,
                if comparison {
                    TypeId(2)
                } else {
                    TypeId(left.type_id)
                },
                source,
            )
        }
        Op::NumericUnary {
            operator,
            source,
            destination,
        } => {
            let borrowed = numeric::borrowed_operand(store, frame.cell(*source)?)?;
            let value = if let Some(value) = borrowed {
                value
            } else {
                store.preview_scalar(frame.cell(*source)?, frame.hint(*destination)?)?
            };
            numeric::unary_type(*operator, TypeId(value.type_id)).map_err(runtime)?;
            let source =
                operator_operand_source(frame, *source, TypeId(value.type_id), borrowed.is_some())?;
            (*destination, TypeId(value.type_id), source)
        }
        Op::PrimitiveCast {
            source,
            target,
            destination,
        } => {
            let cell = frame.cell(*source)?;
            let value = match store.enum_cast_operand(cell)? {
                Some(discriminant) => discriminant,
                None => store.preview_scalar(cell, None)?,
            };
            let primitive = *target;
            let target = concrete(&HintRecipe::Primitive(primitive))?
                .ok_or_else(|| invalid("cast target absent"))?;
            numeric::cast_types(TypeId(value.type_id), target).map_err(runtime)?;
            // The result's Type is the written target primitive exactly.
            (*destination, target, Some(primitive))
        }
        _ => return Ok(None),
    };
    if matches!(op, Op::PrimitiveCast { .. }) {
        frame.preflight_write_type(destination, ty, store)?;
    } else {
        frame.preflight_primitive_operator_result(destination, ty, source, store)?;
    }
    Ok(Some(PreparedNumericResult {
        destination,
        ty,
        source,
    }))
}
fn numeric_operation(
    frame: &mut Frame,
    store: &mut GuestStore,
    op: &Op,
    prepared: Option<PreparedNumericResult>,
) -> Result<bool, super::numeric::NumericFailure> {
    use super::numeric::{self, NumericFailure};
    let (destination, result) = match op {
        Op::CompoundAssignPlace {
            operator,
            place,
            right,
            destination,
        } => {
            frame.compound_assign_place(*operator, *place, *right, *destination, store)?;
            return Ok(true);
        }
        Op::NumericBinary {
            operator,
            left,
            right,
            destination,
        } => {
            use crate::ir::op::NumericBinary as B;
            let left_cell = frame.cell(*left)?;
            let right_cell = frame.cell(*right)?;
            let crate::runtime::cell::CellState::Live(left_raw) = store.state(left_cell)? else {
                return Err(invalid("left operand not live").into());
            };
            let crate::runtime::cell::CellState::Live(right_raw) = store.state(right_cell)? else {
                return Err(invalid("right operand not live").into());
            };
            let shift = matches!(operator, B::Shl | B::Shr);
            let comparison = matches!(operator, B::Eq | B::Ne | B::Lt | B::Le | B::Gt | B::Ge);
            let left_borrowed = if comparison {
                None
            } else {
                numeric::borrowed_operand(store, left_cell)?
            };
            let right_borrowed = if comparison {
                None
            } else {
                numeric::borrowed_operand(store, right_cell)?
            };
            let common = left_borrowed
                .map(|value| TypeId(value.type_id))
                .or(store.effective_type(left_raw)?)
                .or(right_borrowed.map(|value| TypeId(value.type_id)))
                .or(store.effective_type(right_raw)?)
                .or(if comparison {
                    unconstrained_literal_fallback(left_raw, right_raw)
                } else {
                    frame.hint(*destination)?
                });
            let left = if let Some(value) = left_borrowed {
                value
            } else {
                frame.operand(
                    *left,
                    if shift {
                        frame.hint(*destination)?
                    } else {
                        common
                    },
                    store,
                )?
            };
            let right = if let Some(value) = right_borrowed {
                value
            } else {
                frame.operand(
                    *right,
                    if shift { frame.hint(*right)? } else { common },
                    store,
                )?
            };
            if comparison {
                frame.reserve_primitive_operator_result(*destination, store)?;
            }
            frame.prepare_write(*destination, store)?;
            (*destination, numeric::binary(*operator, left, right)?)
        }
        Op::NumericUnary {
            operator,
            source,
            destination,
        } => {
            let source =
                if let Some(value) = numeric::borrowed_operand(store, frame.cell(*source)?)? {
                    value
                } else {
                    frame.operand(*source, frame.hint(*destination)?, store)?
                };
            frame.prepare_write(*destination, store)?;
            (*destination, numeric::unary(*operator, source)?)
        }
        Op::PrimitiveCast {
            source,
            target,
            destination,
        } => {
            let cell = frame.cell(*source)?;
            let source = match store.enum_cast_operand(cell)? {
                Some(discriminant) => discriminant,
                None => store.read_scalar(cell)?,
            };
            frame.prepare_write(*destination, store)?;
            let target = concrete(&HintRecipe::Primitive(*target))?
                .ok_or_else(|| invalid("primitive cast target absent"))?;
            (*destination, numeric::cast(source, target)?)
        }
        _ => return Ok(false),
    };
    let prepared =
        prepared.ok_or_else(|| invalid("numeric operation bypassed immutable result admission"))?;
    if prepared.destination != destination || prepared.ty.0 != result.type_id {
        return Err(invalid("numeric result differs from immutable admission").into());
    }
    let ticket = store.ticket(result).map_err(NumericFailure::RuntimeFault)?;
    frame
        .write(destination, ticket, store)
        .map_err(NumericFailure::RuntimeFault)?;
    frame.set_source_view(
        destination,
        prepared.source.map(crate::traits::term::TyTerm::Primitive),
    )?;
    Ok(true)
}
fn execute(
    frame: &mut Frame,
    store: &mut GuestStore,
    op: &Op,
    prepared: Option<PreparedLiteral>,
) -> Result<(), Diagnostic> {
    match op {
        Op::SourceConstant { .. } => Err(invalid(
            "source constant bypassed its current-proof opcode boundary",
        )),
        Op::PrimitiveFloatingConstant {
            constant,
            destination,
        } => frame.write_primitive_floating_constant(constant, *destination, store),
        Op::ValuePlace {
            source,
            destination,
        } => frame.value_place(*source, *destination, store),
        Op::PromotedPlace {
            source,
            destination,
        } => frame.promoted_place(*source, *destination, store),
        Op::ArrayRepeat {
            source,
            count,
            destination,
        } => frame.array_repeat(*source, *count, *destination, store),
        Op::PlaceTransfer {
            source,
            destination,
        } => frame.transfer_place(*source, *destination, store),
        Op::MakePlace {
            binding,
            destination,
        } => frame.make_place(*binding, *destination, store),
        Op::ParameterPlace {
            ordinal,
            destination,
        } => frame.parameter_place(*ordinal, *destination, store),
        Op::ConstructStart { guard } => frame.start_prepared_constructor(*guard, store),
        Op::ConstructField {
            guard,
            field,
            source,
        } => frame.constructor_field(*guard, *field, *source, store),
        Op::ConstructFinish { guard, destination } => {
            frame.finish_constructor(*guard, *destination, store)
        }
        Op::ValuePath { destination, .. } => frame.write_value_path(*destination, store),
        Op::MakeClosure {
            captures,
            destination,
            ..
        } => frame.make_closure(captures, *destination, store),
        Op::Panic { .. } => Err(invalid(
            "explicit panic bypassed its guest outcome boundary",
        )),
        Op::Declaration => Ok(()),
        Op::CheckParameters { count } => frame.check_parameters(*count),
        Op::ReadParameter {
            ordinal,
            destination,
        } => frame.read_parameter(*ordinal, *destination, store),
        Op::CallPrepare {
            site, arguments, ..
        } => frame.start_call(site.0, arguments),
        Op::CallAdvance { site, argument } => frame.advance_call(site.0, *argument),
        Op::CallSeal { site, .. } => {
            frame.finish_call_progress(site.0)?;
            frame.call_arguments = Some(super::call::ArgumentPacket::seal(
                store,
                &frame.call_cells,
                &frame.call_types,
            )?);
            Ok(())
        }
        Op::Literal {
            literal: l,
            hint,
            destination,
        } => {
            let prepared =
                prepared.ok_or_else(|| invalid("literal bypassed immutable admission"))?;
            let prepared = match prepared {
                PreparedLiteral::String(publication) => {
                    let Some(DecodedLiteral::String(proof)) = &l.decoded else {
                        return Err(invalid("prepared string source payload disappeared"));
                    };
                    return frame.publish_string_literal(proof, publication, store);
                }
                PreparedLiteral::ByteString => {
                    let Some(DecodedLiteral::ByteString(bytes)) = &l.decoded else {
                        return Err(invalid("prepared byte string payload disappeared"));
                    };
                    return frame.publish_byte_string(bytes, *destination, store);
                }
                other => other,
            };
            let explicit = frame_hint(frame, hint, true, store)?;
            let inherited = frame.hint(*destination)?;
            if explicit.is_some() && inherited.is_some() && explicit != inherited {
                return Err(invalid(
                    "literal recipe conflicts with the reached parent hint",
                ));
            }
            let hint = explicit.or(inherited);
            let source = prepared.intrinsic_source();
            if matches!(&prepared, PreparedLiteral::IntrinsicScalar { .. }) {
                // Immutable admission proved this ordinary temporary needs a
                // new cell and one ticket. Reserve before publishing either.
                store.reserve_owning_read_result(true, false)?;
            }
            frame.set_hint(*destination, hint)?;
            if let PreparedLiteral::IntrinsicScalar { value, .. } = &prepared {
                let cell = frame.prepare_write(*destination, store)?;
                store.validate_initialization_value(cell, *value)?;
            }
            let ticket = commit_literal(l, prepared, store)?;
            frame.write(*destination, ticket, store)?;
            if let Some(primitive) = source {
                // Frame::write clears source metadata; attach only after the
                // normal single-use ticket has initialized its owning cell.
                frame.set_source_view(
                    *destination,
                    Some(crate::traits::term::TyTerm::Primitive(primitive)),
                )?;
            }
            Ok(())
        }
        Op::Unit { destination } | Op::Assert { destination, .. } => {
            let ticket = store.ticket(ValueRepr::unit())?;
            frame.write(*destination, ticket, store)
        }
        Op::FormatArguments { .. } => Err(invalid(
            "format operation executes through its resumable step",
        )),
        Op::NumericHint {
            parent,
            child,
            left,
            operator,
        } => frame.numeric_hint(*parent, *child, *left, *operator, store),
        Op::SetHint { child, hint } => {
            if let Some((primitive, descriptor)) =
                frame.normalized_root_return_hint(*child, hint)?
            {
                frame.declared_hint(*child, Some(descriptor))?;
                frame.set_source_view(
                    *child,
                    Some(crate::traits::term::TyTerm::Primitive(primitive)),
                )?;
                return Ok(());
            }
            let descriptor = materialize_hint(
                frame,
                hint,
                Some(*child) != frame.body.nodes()[0].value,
                store,
            )?;
            frame.declared_hint(*child, descriptor)?;
            // A written Type naming no single descriptor (`(_, u8)`, `&[u8]`)
            // stays a partial context for the value's own construction.
            if descriptor.is_none() {
                if let HintRecipe::Written(written) = hint {
                    frame.set_partial_hint(*child, std::sync::Arc::clone(written), None)?;
                } else if let HintRecipe::BorrowTarget { reference } = hint {
                    frame.borrow_partial_hint(*reference, *child, store)?;
                }
            }
            if let HintRecipe::BorrowTarget { reference } = hint {
                frame.soften_borrow_operand_hint(crate::ir::pc::NodeId(frame.pc().node), *child);
                frame.set_source_view(*child, frame.borrow_target_source(*reference)?)?;
            } else if matches!(
                hint,
                HintRecipe::Parameter(_)
                    | HintRecipe::DefinitionNumeric(_)
                    | HintRecipe::DefinitionParameter(_)
                    | HintRecipe::DefinitionResult
                    | HintRecipe::DefinitionWrittenResult
                    | HintRecipe::Reference { .. }
                    | HintRecipe::ArrayPrimitive { .. }
                    | HintRecipe::Written(_)
            ) {
                frame.set_source_view(*child, frame.resolved_annotation_source(hint)?)?;
            }
            Ok(())
        }
        Op::ResultTransfer {
            source,
            destination,
        } => {
            let source_cell = frame.cell(*source)?;
            let crate::runtime::cell::CellState::Live(value) = store.state(source_cell)? else {
                return Err(invalid("transfer source is not live"));
            };
            let source_view = frame.certified_root_transfer_source(*source, *destination, store)?;
            let unsize = frame.write_unsizing(*destination, value, store)?;
            // The coerced value has the destination's declared `&[T]` view.
            let source_view = match unsize {
                Some(_) => frame.source_view(*destination)?.or(source_view),
                None => source_view,
            };
            let destination_cell = frame.prepare_write(*destination, store)?;
            if unsize.is_none() {
                store.validate_initialization_value(destination_cell, value)?;
            }
            let ticket = store.move_from(source_cell)?;
            // A returned `&[T; N]` meets its declared `&[T]` result.
            if let Some(expected) = unsize {
                store.coerce_unsized_ticket(ticket, expected)?;
            }
            store.initialize(destination_cell, ticket)?;
            frame.set_source_view(*destination, source_view)
        }
        Op::CheckUnit { source } => {
            let crate::runtime::cell::CellState::Live(value) = store.state(frame.cell(*source)?)?
            else {
                return Err(invalid("unit check requires a live result"));
            };
            if value.kind != 1 || value.type_id != 1 {
                return Err(Diagnostic::new(
                    "LRR-E-OP-TYPE",
                    "if without else requires a unit arm result".into(),
                ));
            }
            Ok(())
        }
        Op::BranchHint {
            parent,
            condition,
            yes,
            no,
            boolean,
        } => frame.branch_hint(*parent, *condition, *yes, *no, *boolean, store),
        Op::DeclareLocal {
            binding,
            hint,
            writable,
        } => {
            let descriptor = materialize_hint(frame, hint, true, store)?;
            frame.declare_local(*binding, descriptor, *writable, store)?;
            frame.set_binding_source(*binding, hint)
        }
        Op::CompoundHint {
            binding,
            child,
            kind,
        } => frame.compound_hint(*binding, *child, *kind, store),
        Op::LocalHint { binding, child } => frame.local_hint(*binding, *child, store),
        Op::AssignLocal {
            binding,
            source,
            destination,
        } => frame.assign_local(*binding, *source, *destination, store),
        Op::Pattern { operation } => frame.execute_pattern(operation, store),
        Op::BindLocal {
            binding,
            source,
            hint,
            writable,
        } => {
            frame.preflight_binding_source(*source, hint)?;
            written_binding_admission(frame, *source, hint, store)?;
            let unsize = match hint {
                HintRecipe::Written(written) if !written.exact() => {
                    frame.written_unsizing(*source, written, store)?
                }
                _ => None,
            };
            let descriptor = match unsize {
                Some(_) => None,
                None => materialize_hint(frame, hint, true, store)?,
            };
            frame.bind(*binding, *source, descriptor, *writable, store)?;
            if let Some(expected) = unsize {
                frame.coerce_binding(*binding, expected, store)?;
            }
            frame.set_binding_source(*binding, hint)
        }
        Op::ReadLocal {
            binding,
            destination,
        } => frame.read_local(*binding, *destination, store),
        Op::Discard { source } => frame.discard(*source, store),
        Op::ScopeEnter { scope } => frame.enter(*scope),
        Op::ScopeLeave { scope } => frame.leave(*scope, store),
        Op::Borrow { .. }
        | Op::ProjectTuple { .. }
        | Op::ProjectIndex { .. }
        | Op::ProjectField { .. }
        | Op::DerefPlace { .. }
        | Op::ReadPlace { .. }
        | Op::MethodReceiver { .. }
        | Op::ComparisonOperand { .. }
        | Op::CompoundAssignPlace { .. }
        | Op::AssignPlace { .. } => Err(invalid(
            "place opcode bypassed its typed execution boundary",
        )),
        Op::IteratorCall { .. } => Err(invalid(
            "iterator call bypassed its trait method dispatch",
        )),
        Op::PointerCast {
            source,
            target,
            destination,
        } => frame.pointer_cast(*source, target, *destination, store),
        Op::NumericBinary { .. } | Op::NumericUnary { .. } | Op::PrimitiveCast { .. } => Err(
            invalid("numeric opcode bypassed its typed execution boundary"),
        ),
        Op::Unsupported(d) => Err(d.reached_unsupported()),
    }
}
fn increment(n: &mut u64) -> Result<(), Diagnostic> {
    *n = n
        .checked_add(1)
        .ok_or_else(|| invalid("VM counter overflow"))?;
    Ok(())
}
pub fn step(
    frame: &mut Frame,
    store: &mut GuestStore,
    verified: &VerifiedRecipe,
) -> Result<StepOutcome, Diagnostic> {
    step_with_constant(frame, store, verified, None)
}
pub(crate) fn step_with_constant(
    frame: &mut Frame,
    store: &mut GuestStore,
    verified: &VerifiedRecipe,
    constant: Option<&crate::engine::constant_array_publication::AdmittedConstantUse<'_>>,
) -> Result<StepOutcome, Diagnostic> {
    if frame.status != FrameStatus::Running {
        return Err(invalid("terminal frame cannot execute again"));
    }
    frame.require_store(store)?;
    let result = (|| {
        let recipe = verified.recipe();
        if recipe.key != frame.pc {
            return Err(invalid("installed recipe does not match frame PC"));
        }
        // A recipe is data. Refuse a missing/foreign source use before even
        // its ordinary ScopeEnter prefix can execute.
        for op in &recipe.operations {
            if let Op::Literal {
                literal,
                destination,
                ..
            } = op
                && let Some(DecodedLiteral::String(proof)) = &literal.decoded
            {
                proof.require_entry(&frame.body, frame.pc(), *destination, &literal.source)?;
            }
            if let Op::SourceConstant {
                constant: instruction,
                ..
            } = op
            {
                let proof = constant
                    .ok_or_else(|| invalid("source constant lacks a current admitted use"))?;
                if !proof.matches_instruction(instruction) {
                    return Err(invalid(
                        "source constant current use differs from immutable instruction",
                    ));
                }
                frame.preflight_constant_source(proof, store)?;
            }
        }
        frame.preflight_temporary_scope_entry()?;
        let resume = frame.resume_operation_index();
        if frame.has_copy_activation() {
            let op = recipe
                .operations
                .get(resume)
                .ok_or_else(|| invalid("copy operation absent from resumed recipe"))?;
            frame.check_copy_activation(resume, op)?;
        }
        if frame.has_cleanup_activation() && !frame.has_control_cleanup() {
            let op = recipe
                .operations
                .get(resume)
                .ok_or_else(|| invalid("cleanup operation absent from resumed recipe"))?;
            frame.check_cleanup_activation(resume, op)?;
        }
        for (index, op) in recipe.operations.iter().enumerate().skip(resume) {
            if let Op::SourceConstant {
                constant: instruction,
                ..
            } = op
            {
                let proof =
                    constant.ok_or_else(|| invalid("source constant current use disappeared"))?;
                if !frame.has_source_constant_activation() {
                    frame.start_source_constant_op(index, instruction, proof, store)?;
                }
                if !frame.advance_source_constant_op(index, op, proof, store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            if frame.has_source_constant_activation() {
                return Err(invalid("source constant resumed into another operation"));
            }
            frame.clear_rerun(index);
            if frame.has_copy_activation() {
                frame.check_copy_activation(index, op)?;
                if !frame.advance_aggregate_copy(store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            if frame.has_cleanup_activation() {
                frame.check_cleanup_activation(index, op)?;
                if !frame.advance_scope_cleanup(store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            // An operator suspended for its trait method resumes completed:
            // the method's result is already the operation's.
            if frame.has_operator_activation() {
                // A method receiver walking overloaded `Deref` steps resumes
                // with the referent of the reference the method returned.
                if frame.receiver_walk_returned() {
                    if !frame.resume_receiver_step(index, op, store)? {
                        return Ok(StepOutcome::Dispatch);
                    }
                    increment(&mut frame.counters.operations_completed)?;
                    continue;
                }
                // A place operation's overloaded `Deref` step returned its
                // reference: `*x` is complete; `x.f` runs again over the
                // dereferenced base.
                // A reference coercion step returned its reference.
                if frame.value_coercion_returned() {
                    let done = frame.resume_value_coercion(index, op, store)?;
                    increment(&mut frame.counters.operations_completed)?;
                    if done {
                        continue;
                    }
                    return Ok(StepOutcome::Dispatch);
                }
                if frame.place_deref_returned() {
                    if frame.resume_place_deref(index, op, store)? {
                        increment(&mut frame.counters.operations_completed)?;
                        continue;
                    }
                    // The suspended start completes as the dereference step;
                    // the operation starts again over the new base.
                    increment(&mut frame.counters.operations_completed)?;
                    return Ok(StepOutcome::Dispatch);
                }
                frame.complete_operator_operation(index, op)?;
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            // A format operation suspended for a trait-method `fmt` resumes:
            // it demands its next call or completes.
            if frame.has_format_activation() {
                if !frame.format_arguments_step(index, op, store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            let tag = usize::from(op.opcode_tag());
            let prepared = literal_preflight(frame, store, op)?;
            let assertion = assertion_preflight(frame, store, op)?;
            // A reference read into a destination expecting a reference to
            // another Type is a deref-coercion step; it is admitted when its
            // chain ends, not as the plain read.
            let read_op = matches!(
                op,
                Op::ReadLocal { .. } | Op::ReadParameter { .. } | Op::ReadPlace { .. }
            );
            let read_coercion = if read_op {
                frame.overloaded_operator(op, store)?
            } else {
                None
            };
            if read_coercion.is_none() {
                state_preflight(frame, store, op)?;
            }
            // rustc's deref coercion of a borrowed box (`&b` expected as
            // `&T` borrows `&*b`), before the borrow's own admission and
            // before an overloaded `Deref` step the rest of the chain needs.
            if let Op::Borrow {
                place, destination, ..
            } = op
            {
                frame.box_deref_coercion(*place, *destination, store)?;
            }
            // Non-primitive operands select the operator's trait method.
            let overloaded = if read_op {
                read_coercion
            } else {
                frame.overloaded_operator(op, store)?
            };
            let numeric_result = if overloaded.is_some() {
                None
            } else {
                numeric_preflight(frame, store, op)?
            };
            // A place operation suspended for its base's overloaded `Deref`
            // step is checked when it runs again over the dereferenced base.
            let deferred_place = overloaded.is_some()
                && matches!(
                    op,
                    Op::DerefPlace { .. }
                        | Op::ProjectField { .. }
                        | Op::ProjectTuple { .. }
                        | Op::ProjectIndex { .. }
                        | Op::Borrow { .. }
                );
            let (projection, projection_panic) = if deferred_place {
                (None, None)
            } else {
                match frame.prepare_projection(op, store) {
                    Ok(prepared) => (prepared, None),
                    Err(crate::runtime::place::PlaceFailure::RuntimeFault(error)) => {
                        return Err(error);
                    }
                    Err(crate::runtime::place::PlaceFailure::GuestPanic(panic)) => {
                        (None, Some(panic))
                    }
                }
            };
            let place_panic = if deferred_place {
                None
            } else {
                projection_panic.or(place_preflight(frame, store, op)?)
            };
            frame.count_step();
            increment(&mut frame.counters.operations_started)?;
            increment(&mut frame.counters.started_by_opcode[tag])?;
            if let Some(panic) = place_panic {
                return Ok(place_panic_outcome(frame, recipe, panic));
            }
            let panic = match op {
                Op::Panic { message } => Some((super::numeric::PanicKind::Explicit, message)),
                Op::Assert { message, .. } if assertion == Some(false) => {
                    Some((super::numeric::PanicKind::Assertion, message))
                }
                _ => None,
            };
            if let Some((kind, message)) = panic {
                frame.status = FrameStatus::Panicked;
                return Ok(StepOutcome::GuestPanic(GuestPanic {
                    kind,
                    message: Some(message.clone()),
                    pc: recipe.key.clone(),
                    origin: recipe.origin.clone(),
                }));
            }
            if let Some(kind) = overloaded {
                frame.begin_operator_call(index, op, kind, store)?;
                return Ok(StepOutcome::Dispatch);
            }
            // `format_args!`: placeholders outside the native family suspend
            // the operation for their trait's `fmt`.
            if matches!(op, Op::FormatArguments { .. }) {
                if !frame.format_arguments_step(index, op, store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            if frame.begin_aggregate_copy(index, op, store)? {
                if !frame.advance_aggregate_copy(store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            if frame.begin_scope_cleanup(index, op, store)? {
                if !frame.advance_scope_cleanup(store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                increment(&mut frame.counters.operations_completed)?;
                continue;
            }
            if let Some(prepared) = projection {
                frame.commit_projection(prepared)?;
            } else {
                match numeric_operation(frame, store, op, numeric_result) {
                    Ok(true) => {}
                    Ok(false) => match place_operation(frame, store, op) {
                        Ok(true) => {}
                        Ok(false) => execute(frame, store, op, prepared)?,
                        Err(crate::runtime::place::PlaceFailure::RuntimeFault(error)) => {
                            return Err(error);
                        }
                        Err(crate::runtime::place::PlaceFailure::GuestPanic(panic)) => {
                            return Ok(place_panic_outcome(frame, recipe, panic));
                        }
                    },
                    Err(super::numeric::NumericFailure::RuntimeFault(error)) => return Err(error),
                    Err(super::numeric::NumericFailure::GuestPanic(kind)) => {
                        frame.status = FrameStatus::Panicked;
                        return Ok(StepOutcome::GuestPanic(GuestPanic {
                            kind,
                            message: None,
                            pc: recipe.key.clone(),
                            origin: recipe.origin.clone(),
                        }));
                    }
                }
            }
            increment(&mut frame.counters.operations_completed)?;
        }
        exit_preflight(frame, store, &recipe.exit)?;
        frame.count_step();
        let outcome = match &recipe.exit {
            RecipeExit::Call {
                site,
                resume,
                result,
            } => {
                if !frame.call_ready || frame.call_arguments.is_none() {
                    return Err(invalid("Call exit has no sealed source argument packet"));
                }
                if frame.call_continuation.is_some() {
                    return Err(invalid("Call exit already has a suspended continuation"));
                }
                StepOutcome::Call {
                    action: frame.actions.publish_call(
                        recipe.key.clone(),
                        *site,
                        resume.clone(),
                        *result,
                        recipe.origin.clone(),
                    )?,
                    site: *site,
                    resume: resume.clone(),
                    result: *result,
                }
            }
            RecipeExit::Backedge(pc) => StepOutcome::Backedge(pc.clone()),
            RecipeExit::Next(pc) => StepOutcome::Next(pc.clone()),
            RecipeExit::Branch { condition, yes, no } => {
                let value = store.read_scalar(frame.cell(*condition)?)?;
                if value.kind != 2 || value.type_id != 2 {
                    return Err(invalid("branch condition is not a concrete bool"));
                }
                StepOutcome::Next(if value.payload == 1 { yes } else { no }.clone())
            }
            RecipeExit::Control(ControlRecipe::Return { value }) => {
                frame.preflight_return(*value, store)?;
                let cell = frame.cell(*value)?;
                frame.set_return_source_view(frame.certified_return_source(*value, store)?)?;
                StepOutcome::Return(store.move_from(cell)?)
            }
            RecipeExit::Control(ControlRecipe::Break {
                target,
                value,
                resume,
            }) => {
                if !frame.control_transfer(*target, *value, false, store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                StepOutcome::Next(resume.clone())
            }
            RecipeExit::Control(ControlRecipe::Continue { target }) => {
                let node = frame.body.labels()[target.0 as usize].node;
                // A `for` loop continues at its head (the next `next` call);
                // its entry would re-evaluate the iterated expression.
                let phase = if frame.body.nodes()[node.0 as usize].role
                    == crate::syntax::body_index::NodeRole::Expression(
                        crate::syntax::body_index::ExpressionKind::For,
                    ) {
                    crate::ir::pc::Phase::Commit
                } else {
                    crate::ir::pc::Phase::Entry
                };
                let entry = frame.body.pc(node, phase)?;
                if !frame.control_transfer(*target, None, true, store)? {
                    return Ok(StepOutcome::Dispatch);
                }
                StepOutcome::Backedge(entry)
            }
            RecipeExit::Fault(d) => return Err(d.reached_unsupported()),
            RecipeExit::MatchNoArm => {
                return Err(Diagnostic::new(
                    "LRR-E-MATCH-NO-ARM",
                    "no match arm accepted the reached scrutinee".into(),
                ));
            }
            _ => {
                return Err(Diagnostic::new(
                    "LRR-E-UNSUPPORTED",
                    "call/effect dispatcher boundary awaits its ordered task".into(),
                ));
            }
        };
        increment(&mut frame.counters.exits_completed)?;
        match &outcome {
            StepOutcome::Dispatch => return Err(invalid("Dispatch cannot be a recipe exit")),
            StepOutcome::Backedge(pc) => {
                increment(&mut frame.counters.backedges_completed)?;
                frame.pc = pc.clone();
            }
            StepOutcome::Next(pc) => frame.pc = pc.clone(),
            StepOutcome::Call { resume, result, .. } => {
                // Macro argument calls may return into a temporary rather than
                // the enclosing node's final slot. Bind the verified exit itself.
                frame.call_continuation = Some((resume.clone(), *result));
                frame.status = FrameStatus::Suspended;
            }
            StepOutcome::Return(_) => frame.status = FrameStatus::Returned,
            StepOutcome::GuestPanic(_) => frame.status = FrameStatus::Panicked,
        }
        Ok(outcome)
    })();
    if result.is_err() {
        frame.status = FrameStatus::Faulted;
    }
    result
}

#[cfg(test)]
#[path = "primitive_operator_result_source_tests.rs"]
mod primitive_operator_result_source_tests;
