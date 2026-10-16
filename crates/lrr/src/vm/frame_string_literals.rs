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
        if self
            .literal_source_context(destination)?
            .is_some_and(|expected| expected != proof.source())
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
        store.preflight_immutable_str_literal(proof.utf8().as_bytes(), needs_cell)?;
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
