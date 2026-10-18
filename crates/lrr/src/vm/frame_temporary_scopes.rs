//! Runtime use of structural temporary annotations and selected call identity.
use super::*;
use crate::syntax::body_index::{ExpressionKind, NodeRole};

fn unresolved() -> Diagnostic {
    Diagnostic::new(
        "LRR-E-TEMPORARY-SCOPE",
        "temporary extension requires selected constructor or ordinary-call identity".into(),
    )
}
impl Frame {
    fn require_conditional_frontier(&self, node: NodeId) -> Result<(), Diagnostic> {
        let node = self
            .body
            .nodes()
            .get(node.0 as usize)
            .ok_or_else(|| invalid("temporary extension node absent"))?;
        let Some(extension) = &node.extended_to else {
            return Ok(());
        };
        if extension.constructors.is_empty() {
            return Ok(());
        }
        // Any selected ordinary call blocks extension through the whole path.
        // Unknown calls cannot certify a constructor; positive constructor
        // scope admission integrates with the later source constructor dispatcher.
        for guard in &extension.constructors {
            if self
                .ordinary_calls
                .get(guard.0 as usize)
                .ok_or_else(|| invalid("temporary constructor guard absent"))?
                .is_some()
            {
                return Ok(());
            }
        }
        Err(unresolved())
    }
    pub(crate) fn preflight_temporary_scope_entry(&self) -> Result<(), Diagnostic> {
        if self.pc.phase != crate::ir::pc::Phase::Entry.code()? {
            return Ok(());
        }
        let node = self
            .body
            .nodes()
            .get(self.pc.node as usize)
            .ok_or_else(|| invalid("temporary-scope PC node absent"))?;
        if node.role == NodeRole::Expression(ExpressionKind::Borrow) {
            for operand in &node.operands {
                self.require_conditional_frontier(*operand)?;
            }
        }
        Ok(())
    }
    pub(super) fn preflight_temporary_slot(&self, slot: SlotId) -> Result<(), Diagnostic> {
        if let Some(node) = self.slot(slot)?.conditional_extension {
            self.require_conditional_frontier(node)?;
        }
        Ok(())
    }
}

impl Frame {
    /// Read only the already reached reference context. A runtime descriptor
    /// cannot supply a missing nominal source identity; no query or type
    /// publication is performed while admitting this hint.
    pub(crate) fn borrow_target_descriptor(
        &self,
        reference: crate::ir::pc::SlotId,
        store: &GuestStore,
    ) -> Result<Option<crate::runtime::shape::TypeId>, Diagnostic> {
        use crate::{
            runtime::shape::{PrimitiveIdentity, TypeKey},
            traits::term::TyTerm,
        };
        let source = self.slot(reference)?.source.as_ref();
        let target_source = self.borrow_target_source_ref(reference)?;
        if let Some(descriptor) = self.hint(reference)? {
            let key = store.type_descriptor(descriptor)?.key.as_ref();
            // NumericHint is advisory on a forwarded reference operand. Its
            // primitive expectation belongs to the pointee, not the reference
            // result. Strict written annotations still require a Ref descriptor.
            if !self.slot(reference)?.strict_hint
                && matches!(key, TypeKey::Primitive(_))
                && matches!(descriptor, crate::runtime::shape::TypeId(2 | 4..=15))
            {
                Self::require_borrow_source_target(target_source, descriptor, store)?;
                return Ok(Some(descriptor));
            }
            let TypeKey::Ref { target, mutable } = key else {
                return Err(Diagnostic::new(
                    "LRR-E-OWNERSHIP",
                    "borrow result context is not a reference descriptor".into(),
                ));
            };
            if matches!(source, Some(TyTerm::Ref(_, written_mutable, _)) if written_mutable != mutable)
            {
                return Err(Diagnostic::new(
                    "LRR-E-OWNERSHIP",
                    "borrow reference context source access differs".into(),
                ));
            }
            Self::require_borrow_source_target(target_source, *target, store)?;
            // `&[T]` expects its operand only like an unsized rvalue (rustc's
            // `ExpectRvalueLikeUnsized`): the retained `[T]` view gives an
            // array operand its element Type, never a `[T]` descriptor.
            // `&dyn Trait` expects its operand only like an unsized rvalue:
            // the operand keeps its own concrete Type.
            if matches!(
                store.type_descriptor(*target)?.key.as_ref(),
                TypeKey::Slice { .. } | TypeKey::Dyn { .. }
            ) {
                return Ok(None);
            }
            return Ok(Some(*target));
        }
        // Existing grouping/control hint operations may retain the symbolic
        // view even when they have no numeric result descriptor. A closed
        // primitive target is still exact source context, not width guessing.
        match target_source {
            None | Some(TyTerm::Slice(_)) => Ok(None),
            Some(TyTerm::Primitive(primitive)) => {
                PrimitiveIdentity::from_recipe(*primitive).map(|identity| Some(identity.id()))
            }
            Some(TyTerm::Array(element, crate::traits::term::ConstTerm::Value(len)))
                if matches!(element.as_ref(), TyTerm::Primitive(_)) =>
            {
                let TyTerm::Primitive(primitive) = element.as_ref() else {
                    return Err(invalid("borrow context array element source differs"));
                };
                let element = PrimitiveIdentity::from_recipe(*primitive)?.id();
                let len = u32::try_from(*len)
                    .map_err(|_| invalid("borrow context array length exceeds its descriptor"))?;
                store
                    .find_type(&TypeKey::Array { element, len })
                    .map(Some)
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "LRR-E-TYPE-CONTEXT",
                            "borrow target requires its previously prepared array descriptor"
                                .into(),
                        )
                    })
            }
            // A non-generic nominal target: its own constructor or value
            // fixes its Type; the retained view carries the identity.
            Some(TyTerm::Nominal(_, arguments)) if arguments.is_empty() => Ok(None),
            _ => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "borrow target requires its retained symbolic descriptor binding".into(),
            )),
        }
    }
    fn borrow_target_source_ref(
        &self,
        reference: crate::ir::pc::SlotId,
    ) -> Result<Option<&crate::traits::term::TyTerm>, Diagnostic> {
        use crate::traits::term::TyTerm;
        match self.slot(reference)?.source.as_ref() {
            Some(TyTerm::Ref(_, _, target))
                if (Self::supported_source(target)
                    || matches!(target.as_ref(), TyTerm::Slice(element)
                        if Self::supported_element(element)))
                    && !matches!(target.as_ref(), TyTerm::Param(_)) =>
            {
                Ok(Some(target.as_ref()))
            }
            None => Ok(None),
            _ => Err(Diagnostic::new(
                "LRR-E-UNSUPPORTED",
                "borrow operand source requires symbolic target integration".into(),
            )),
        }
    }
    pub(crate) fn borrow_target_source(
        &self,
        reference: crate::ir::pc::SlotId,
    ) -> Result<Option<crate::traits::term::TyTerm>, Diagnostic> {
        Ok(self.borrow_target_source_ref(reference)?.cloned())
    }
}
