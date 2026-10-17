//! T054 exact source/type/current memo join, never a query or evaluator provider.
//! Guest descriptor binding is an explicit reached driver step, not source authority.
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    db::{dependency::DependencyStamp, key::QueryKey, memo::Database, value::QueryValue},
    frontend::{
        constant_result_type::CurrentConstantResultType,
        constant_source_admission::ConstantSourceAdmission,
        constants::{query_value::ConstantQueryValue, wire::ConstantWireType},
    },
    runtime::{
        cell::GuestStore,
        shape::{PrimitiveIdentity, TypeId, TypeKey},
    },
    source::changes::PreparedSourceView,
    traits::term::{ConstTerm, TyTerm},
};
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-CONST-CONSTRUCTION-SOURCE", message.into())
}
fn require_current_query(
    db: &Database,
    admission: &ConstantSourceAdmission,
    expected: &ConstantWireType,
    value: &ConstantQueryValue,
    length: Option<&ConstantQueryValue>,
) -> Result<(), Diagnostic> {
    let key = value.key();
    key.validate()?;
    let plan = admission.plan();
    if key.admission() != admission.identity()
        || key.definition() != plan.owner_definition()
        || key.selection() != plan.identity()
        || key.source() != plan.source().source()
        || key.cfg() != plan.source().cfg()
        || key.profile() != plan.profile()
        || key.result_type() != expected.fingerprint_checked()?
        || !key.matches_payload(value.payload())
        || key.universe() != Some(db.constant_input_universe()?)
    {
        return Err(fault(
            "constant query belongs to another current source/type/plan/input universe",
        ));
    }
    let entry = db
        .memo(&QueryKey::Constant(key.clone()))
        .ok_or_else(|| fault("exact current constant query memo is required"))?;
    let QueryValue::Constant(current) = entry.value.as_ref() else {
        return Err(fault("constant current memo value variant differs"));
    };
    let length_stamp = length
        .map(|value| -> Result<DependencyStamp, Diagnostic> {
            let key = QueryKey::Constant(value.key().clone());
            let current = db
                .memo(&key)
                .ok_or_else(|| fault("exact demanded length memo is required"))?;
            Ok(DependencyStamp::Query {
                key,
                fingerprint: current.fingerprint,
            })
        })
        .transpose()?;
    let dependency_count = admission
        .dependencies()
        .len()
        .checked_add(usize::from(length_stamp.is_some()))
        .ok_or_else(|| fault("constant memo dependency cardinality overflow"))?;
    if entry.verified_revision != db.revision()
        || current.key() != key
        || current.fingerprint() != value.fingerprint()
        || entry.fingerprint != entry.value.fingerprint()
        || entry.dependencies.len() != dependency_count
        || length_stamp
            .as_ref()
            .is_some_and(|stamp| !entry.dependencies.contains(stamp))
    {
        return Err(fault(
            "constant query memo is stale or its exact payload/dependencies differ",
        ));
    }
    for (key, fingerprint) in admission.dependencies() {
        if !entry.dependencies.contains(&DependencyStamp::Input {
            key: key.clone(),
            fingerprint: *fingerprint,
        }) || db
            .published_input(key)
            .is_none_or(|(_, actual)| actual != *fingerprint)
        {
            return Err(fault(
                "constant memo admitted dependency is absent or changed",
            ));
        }
    }
    Ok(())
}
/// Private borrowed source/type/query construction evidence. No mutable fields,
/// serialized data or descriptor/name/count constructor can forge this packet.
pub(crate) struct CurrentConstantConstruction<'a> {
    db: &'a Database,
    source: &'a PreparedSourceView,
    result: &'a CurrentConstantResultType,
    parent: &'a ConstantQueryValue,
    length: Option<(&'a ConstantSourceAdmission, &'a ConstantQueryValue)>,
    wire: ConstantWireType,
    source_term: TyTerm,
    recipe: SelectedLayout,
    bytes: usize,
    byte_limit: usize,
}
#[derive(Clone, Copy)]
enum SelectedLayout {
    Scalar(crate::ir::op::PrimitiveType),
    Array {
        element: crate::ir::op::PrimitiveType,
        len: u32,
    },
}
impl<'a> CurrentConstantConstruction<'a> {
    pub(crate) fn capture_current(
        db: &'a Database,
        source: &'a PreparedSourceView,
        result: &'a CurrentConstantResultType,
        parent: &'a ConstantQueryValue,
        length: Option<(&'a ConstantSourceAdmission, &'a ConstantQueryValue)>,
        byte_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if byte_limit == 0 {
            return Err(fault(
                "constant construction metadata budget exceeds profile",
            ));
        }
        let bytes = std::mem::size_of::<Self>()
            .checked_add(if result.array().is_some() {
                std::mem::size_of::<ConstantWireType>() + std::mem::size_of::<TyTerm>()
            } else {
                0
            })
            .and_then(|n| n.checked_add(result.metadata_bytes()))
            .and_then(|n| n.checked_add(parent.metadata_bytes().ok()?))
            .and_then(|n| {
                n.checked_add(length.map_or(Some(0), |(a, v)| {
                    a.metadata_bytes().checked_add(v.metadata_bytes().ok()?)
                })?)
            })
            .filter(|n| *n <= byte_limit)
            .ok_or_else(|| fault("constant construction retained metadata limit"))?;
        result.revalidate(db, source)?;
        let (wire, source_term, recipe) = if let Some(scalar) = result.scalar() {
            if length.is_some() {
                return Err(fault("scalar constant supplied an unrelated length query"));
            }
            let p = scalar.primitive();
            (
                scalar.wire_type(),
                TyTerm::Primitive(p),
                SelectedLayout::Scalar(p),
            )
        } else if let Some((element, demand)) = result.array() {
            let (child, query) = length
                .ok_or_else(|| fault("exact demanded current array-length query is required"))?;
            let selected = result.admission().reselect_array_length_current(
                db,
                source,
                demand,
                byte_limit
                    .checked_sub(bytes)
                    .ok_or_else(|| fault("constant child transient metadata limit"))?,
            )?;
            child.revalidate(db, source)?;
            if child.identity() != selected.identity()
                || demand.parent_plan_identity() != result.admission().plan().identity()
                || demand.source() != source.selection()
                || demand.definition() != result.admission().plan().owner_definition()
                || demand.context() != result.admission().plan().owner_context()
                || Some(demand.body()) != result.admission().plan().owner_body()
            {
                return Err(fault(
                    "array length child is not the exact source-selected parent demand",
                ));
            }
            require_current_query(db, child, &demand.expected_wire(), query, None)?;
            let length = query.payload().decode_value()?;
            let count = length
                .scalar()
                .ok_or_else(|| fault("array length query is not scalar"))?;
            count.validate_scalar()?;
            if count.kind != 5 || count.type_id != 13 {
                return Err(fault("array length requires exact concrete usize"));
            }
            let len = u32::try_from(count.payload)
                .map_err(|_| fault("array length exceeds guest descriptor width"))?;
            let primitive = element.primitive();
            let wire = ConstantWireType::Array {
                element: Box::new(element.wire_type()),
                length: count.payload,
            };
            wire.fingerprint_checked()?;
            (
                wire,
                TyTerm::Array(
                    Box::new(TyTerm::Primitive(primitive)),
                    ConstTerm::Value(count.payload),
                ),
                SelectedLayout::Array {
                    element: primitive,
                    len,
                },
            )
        } else {
            return Err(fault(
                "selected constant result has no closed scalar/array source readiness",
            ));
        };
        require_current_query(
            db,
            result.admission(),
            &wire,
            parent,
            length.map(|(_, q)| q),
        )?;
        // Canonical immutable content is checked after all source/current memo
        // admission. This creates no guest handle, field, owner or descriptor.
        parent.payload().decode_value()?;
        Ok(Self {
            db,
            source,
            result,
            parent,
            length,
            wire,
            source_term,
            recipe,
            bytes,
            byte_limit,
        })
    }
    pub(crate) fn revalidate(&self) -> Result<(), Diagnostic> {
        self.result.revalidate(self.db, self.source)?;
        if let Some((child, query)) = self.length {
            let (_, demand) = self
                .result
                .array()
                .ok_or_else(|| fault("array source readiness disappeared"))?;
            let selected = self.result.admission().reselect_array_length_current(
                self.db,
                self.source,
                demand,
                self.byte_limit
                    .checked_sub(self.bytes)
                    .ok_or_else(|| fault("constant child transient metadata limit"))?,
            )?;
            if selected.identity() != child.identity() {
                return Err(fault("array source length child changed"));
            }
            child.revalidate(self.db, self.source)?;
            require_current_query(self.db, child, &demand.expected_wire(), query, None)?;
        }
        require_current_query(
            self.db,
            self.result.admission(),
            &self.wire,
            self.parent,
            self.length.map(|(_, q)| q),
        )
    }
    pub(crate) fn database(&self) -> &Database {
        self.db
    }
    pub(crate) fn source(&self) -> &PreparedSourceView {
        self.source
    }
    pub(crate) fn result(&self) -> &CurrentConstantResultType {
        self.result
    }
    pub(crate) fn parent(&self) -> &ConstantQueryValue {
        self.parent
    }
    pub(crate) fn source_term(&self) -> &TyTerm {
        &self.source_term
    }
    pub(crate) fn wire(&self) -> &ConstantWireType {
        &self.wire
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    /// Exact source-owned structural descriptor preparation. It reads no caller
    /// descriptor/type spelling and executes no guest initializer or query.
    pub(crate) fn bind_guest_descriptor<'b>(
        &'b self,
        store: &mut GuestStore,
    ) -> Result<SelectedGuestConstantType<'b, 'a>, Diagnostic> {
        self.revalidate()?;
        let (ty, expected) = match self.recipe {
            SelectedLayout::Scalar(p) => {
                let id = PrimitiveIdentity::from_recipe(p)?;
                let ty = id.id();
                (ty, TypeKey::Primitive(id))
            }
            SelectedLayout::Array { element, len } => {
                let element = PrimitiveIdentity::from_recipe(element)?.id();
                let expected = TypeKey::Array { element, len };
                let ty = store.intern_type(expected.clone())?;
                (ty, expected)
            }
        };
        if store.type_descriptor(ty)?.key.as_ref() != &expected {
            return Err(fault("selected structural guest descriptor differs"));
        }
        Ok(SelectedGuestConstantType {
            construction: self,
            ty,
            expected,
            session: store.pattern_binding_session(),
            epoch: store.budget().epoch(),
        })
    }
}
/// Borrows the actual current source/query proof; a TypeId is data, not authority.
pub(crate) struct SelectedGuestConstantType<'b, 'a> {
    construction: &'b CurrentConstantConstruction<'a>,
    ty: TypeId,
    expected: TypeKey,
    session: crate::runtime::cell::StoreSession,
    epoch: u32,
}
impl SelectedGuestConstantType<'_, '_> {
    pub(crate) fn construction(&self) -> &CurrentConstantConstruction<'_> {
        self.construction
    }
    pub(crate) fn ty(&self) -> TypeId {
        self.ty
    }
    pub(crate) fn revalidate(&self, store: &GuestStore) -> Result<(), Diagnostic> {
        self.construction.revalidate()?;
        if store.pattern_binding_session() != self.session
            || store.budget().epoch() != self.epoch
            || store.type_descriptor(self.ty)?.key.as_ref() != &self.expected
        {
            return Err(fault(
                "selected guest descriptor belongs to another run or layout",
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "constant_construction_tests.rs"]
pub(crate) mod tests;
