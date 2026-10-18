//! Ordinary intrinsic static str publication. The opaque parser receipt carries
//! source authority; descriptor lookup and backing storage cannot invent it.
use super::*;
use crate::{
    runtime::shape::TypeKey, syntax::string_literal::PreparedStringLiteral, traits::term::TyTerm,
};
/// Private immutable preflight result, consumed by this same literal operation.
pub(crate) struct StringLiteralPublication {
    entry: PcKey,
    destination: SlotId,
    source: TyTerm,
    needs_cell: bool,
}
impl Frame {
    pub(crate) fn preflight_string_literal(
        &self,
        proof: &PreparedStringLiteral,
        destination: SlotId,
        store: &GuestStore,
    ) -> Result<StringLiteralPublication, Diagnostic> {
        proof.require_entry(
            &self.body,
            self.pc(),
            destination,
            &self.body.span(NodeId(self.pc().node))?,
        )?;
        let key = TypeKey::Ref {
            mutable: false,
            target: TypeId(16),
        };
        if let Some(hint) = self.hint(destination)? {
            if store.type_descriptor(hint)?.key.as_ref() != &key {
                return Err(Diagnostic::new(
                    "LRR-E-TYPE-CONTEXT",
                    "string literal conflicts with reached descriptor constraint".into(),
                ));
            }
        }
        // A `&'static str` literal satisfies a `&'r str` expectation for any
        // region: regions are not runtime data.
        if self
            .literal_source_context(destination)?
            .is_some_and(|expected| {
                crate::frontend::signature_subst::erase_regions(expected)
                    != crate::frontend::signature_subst::erase_regions(proof.source())
            })
        {
            return Err(Diagnostic::new(
                "LRR-E-TYPE-CONTEXT",
                "string literal conflicts with declared source constraint".into(),
            ));
        }
        let actual = store.find_type(&key);
        self.preflight_literal(destination, actual, self.hint(destination)?, store)?;
        self.preflight_source_view(destination, Some(proof.source()))?;
        let source = self
            .clone_source(Some(proof.source()))?
            .ok_or_else(|| invalid("intrinsic string source absent"))?;
        let needs_cell = self.slot(destination)?.cell.is_none();
        store.preflight_immutable_str_literal(proof.utf8().as_bytes())?;
        Ok(StringLiteralPublication {
            entry: self.pc().clone(),
            destination,
            source,
            needs_cell,
        })
    }
    pub(crate) fn publish_string_literal(
        &mut self,
        proof: &PreparedStringLiteral,
        publication: StringLiteralPublication,
        store: &mut GuestStore,
    ) -> Result<(), Diagnostic> {
        if self.pc() != &publication.entry
            || self.slot(publication.destination)?.cell.is_none() != publication.needs_cell
            || &publication.source != proof.source()
        {
            return Err(invalid(
                "string literal publication differs from immutable admission",
            ));
        }
        proof.require_entry(
            &self.body,
            self.pc(),
            publication.destination,
            &self.body.span(NodeId(self.pc().node))?,
        )?;
        let reserved =
            store.reserve_immutable_str_literal(proof.utf8().as_bytes(), publication.needs_cell)?;
        let ty = store
            .find_type(&TypeKey::Ref {
                mutable: false,
                target: TypeId(16),
            })
            .ok_or_else(|| invalid("reserved str reference descriptor absent"))?;
        // Source/type/scope/storage admission precedes the first owning cell.
        self.preflight_write_type(publication.destination, ty, store)?;
        let cell = self.prepare_write(publication.destination, store)?;
        store.validate_constructor_destination(cell, ty)?;
        let ticket = store.commit_immutable_str(reserved)?;
        self.write(publication.destination, ticket, store)?;
        self.set_source_view(publication.destination, Some(publication.source))
    }
}

/// The code of a value outside the natively rendered family: it formats
/// through its trait's `fmt` (`vm/frame_format_calls.rs`).
pub(super) const FORMAT_TRAIT_CODE: &str = "LRR-E-FORMAT-TRAIT";
/// The value in `cell`: an array is its Debug list of element entries.
pub(super) fn format_cell(
    store: &GuestStore,
    cell: CellId,
) -> Result<crate::ir::format::FormatValue, Diagnostic> {
    if let Some(elements) = store.array_element_cells(cell)? {
        return format_list(store, elements);
    }
    // `<Box<T> as Debug/Display>` forwards to `T`.
    if let Some(contents) = store.boxed_cell_contents(cell)? {
        return format_cell(store, contents);
    }
    // Any other aggregate formats through its trait.
    if let crate::runtime::cell::CellState::Live(value) = store.state(cell)?
        && value.kind == 8
    {
        return Err(Diagnostic::new(
            FORMAT_TRAIT_CODE,
            "this value formats through its trait's `fmt` impl".into(),
        ));
    }
    format_value(store, store.read_format_scalar(cell)?)
}
/// `<[T] as Debug>` / `<[T; N] as Debug>`: `debug_list().entries(iter)`.
fn format_list(
    store: &GuestStore,
    elements: Vec<CellId>,
) -> Result<crate::ir::format::FormatValue, Diagnostic> {
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(elements.len())
        .map_err(|_| crate::runtime::arena::resource())?;
    for element in elements {
        entries.push(format_cell(store, element)?);
    }
    Ok(crate::ir::format::FormatValue::List(entries))
}

/// The closed formattable family: scalars, str, arrays and slices of them,
/// and references to them.
pub(crate) fn format_value(
    store: &GuestStore,
    value: crate::runtime::value::ValueRepr,
) -> Result<crate::ir::format::FormatValue, Diagnostic> {
    use crate::{
        ir::format::FormatValue,
        runtime::{cell::str_storage::ReferenceView, shape::Primitive},
    };
    if value.kind == 9 {
        if let Some(elements) = store.referent_element_cells(value)? {
            return format_list(store, elements);
        }
        if let Some(contents) = store.referent_box_contents(value)? {
            return format_cell(store, contents);
        }
        // `<&T as Display/Debug>` forwards to the referent.
        if let Some(referent) = store.referent_location_cell(value)? {
            return format_cell(store, referent);
        }
        return match store.reference_view(value)? {
            ReferenceView::Str(text) => Ok(FormatValue::Str(text.to_owned())),
            ReferenceView::Value(inner) => format_value(store, inner),
        };
    }
    if let Some(contents) = store.box_contents_cell(value)? {
        return format_cell(store, contents);
    }
    let unsupported = || {
        Diagnostic::new(
            FORMAT_TRAIT_CODE,
            "this value formats through its trait's `fmt` impl".into(),
        )
    };
    Ok(
        match crate::runtime::shape::primitive(crate::runtime::shape::TypeId(value.type_id))
            .map_err(|_| unsupported())?
        {
            Primitive::Unit => FormatValue::Unit,
            Primitive::Bool => FormatValue::Bool(value.payload == 1),
            Primitive::Char => FormatValue::Char(
                u32::try_from(value.payload)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(unsupported)?,
            ),
            Primitive::Signed(bits) => {
                FormatValue::Signed(store.integer_bits(value)? as i128, u32::from(bits))
            }
            Primitive::Unsigned(bits) => {
                FormatValue::Unsigned(store.integer_bits(value)?, u32::from(bits))
            }
            Primitive::F32 => FormatValue::F32(f32::from_bits(value.payload as u32)),
            Primitive::F64 => FormatValue::F64(f64::from_bits(value.payload)),
            Primitive::Str => return Err(unsupported()),
        },
    )
}
