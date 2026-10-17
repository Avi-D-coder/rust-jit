//! Method calls: select the method from the receiver's reached type through
//! the run's impl member index (inherent impls first, then trait impls, as
//! rustc's probe for a nominal receiver), and record how the receiver
//! argument is formed. The call then proceeds as an ordinary source call.
use super::*;
use crate::{
    runtime::shape::TypeKey,
    vm::frame::{ReceiverAdjustment, ReceiverMode},
};

impl RunMachine {
    pub(super) fn select_method_call(
        &mut self,
        db: &mut Database,
        site: u32,
        name: &str,
        syntax: &[String],
        arguments: &[SlotId],
        receiver_place: SlotId,
    ) -> QueryPoll<SelectedArgumentCall> {
        let method = name.strip_prefix('.').unwrap_or(name);
        // Autoderef the receiver's reached type to its nominal declaration.
        let mut ty = match self.frame.place_type(receiver_place, &self.store) {
            Ok(ty) => ty,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        use crate::frontend::member_index::BuiltinSelf;
        let mut derefs = 0u32;
        // Mutability of each reference stripped by autoderef, outermost first
        // (`None` for a builtin box dereference).
        let mut references = Vec::new();
        let self_type = loop {
            let key = match self.store.type_descriptor(ty) {
                Ok(descriptor) => descriptor.key.clone(),
                Err(error) => return QueryPoll::Ready(Err(error)),
            };
            match key.as_ref() {
                TypeKey::Ref { target, mutable } => {
                    derefs += 1;
                    references.push(Some(*mutable));
                    ty = *target;
                    // rustc's probe tries each step's own methods before the
                    // next deref: a reference to a reference (`&&A`) has the
                    // methods of core's `&A` impls (`Self = &A`) at this step.
                    if let Ok(descriptor) = self.store.type_descriptor(ty)
                        && let TypeKey::Ref { mutable: inner, .. } = descriptor.key.as_ref()
                        && self.members.as_ref().is_some_and(|members| {
                            !members
                                .builtin_trait_functions(BuiltinSelf::Reference(*inner), method)
                                .is_empty()
                        })
                    {
                        break Err(BuiltinSelf::Reference(*inner));
                    }
                }
                // rustc's probe steps through a box by its builtin deref when
                // `Box<T>` itself has no method of this name.
                TypeKey::Nominal { identity, args, .. }
                    if self.store.owned_box() == Some(*identity)
                        && args.len() == 1
                        && self.members.as_ref().is_some_and(|members| {
                            members
                                .select_method(Ok(*identity), method)
                                .is_err_and(|error| error.code == "LRR-E-NAME-MISSING")
                        }) =>
                {
                    derefs += 1;
                    references.push(None);
                    ty = args[0];
                }
                TypeKey::Nominal { identity, .. } => break Ok(*identity),
                TypeKey::Primitive(_) if ty.0 == 16 => break Err(BuiltinSelf::Str),
                TypeKey::Primitive(_) => match self.store.descriptor_source_term(ty) {
                    Some(crate::traits::term::TyTerm::Primitive(primitive)) => {
                        break Err(BuiltinSelf::Primitive(primitive));
                    }
                    _ => {
                        return QueryPoll::Ready(Err(call_fault(&format!(
                            "method `{method}` receiver primitive is not selectable"
                        ))));
                    }
                },
                TypeKey::Slice { .. } => break Err(BuiltinSelf::Slice),
                TypeKey::Array { .. } => break Err(BuiltinSelf::Array),
                _ => {
                    return QueryPoll::Ready(Err(call_fault(&format!(
                        "method `{method}` on this receiver type awaits trait-object/closure methods"
                    ))));
                }
            }
        };
        let Some(members) = self.members.clone() else {
            return QueryPoll::Ready(Err(call_fault("method selection requires the impl index")));
        };
        // The shared probe rule (inherent, trait, blanket impls; the array
        // unsizing step, whose receiver is the autoref'd element window).
        let (function, unsize) = match members.select_method(self_type, method) {
            Ok(selected) => selected,
            Err(error) => return QueryPoll::Ready(Err(error)),
        };
        let callee = match self.member_callee(db, function) {
            QueryPoll::Ready(Ok(callee)) => callee,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let Some(receiver) = callee
            .signature()
            .parameters
            .first()
            .filter(|parameter| parameter.receiver)
        else {
            return QueryPoll::Ready(Err(Diagnostic::new(
                "LRR-E-CALL-CONTEXT",
                format!("`{method}` is an associated function, not a method"),
            )));
        };
        let mut mode = match receiver.ty.first().map(|token| token.text.as_str()) {
            Some("&") if receiver.ty.iter().any(|token| token.text == "mut") => {
                ReceiverMode::Mutable
            }
            Some("&") => ReceiverMode::Shared,
            _ => ReceiverMode::Value,
        };
        // `&self` through a shared reference passes that reference itself
        // (a copy), rather than dereferencing and reborrowing it.
        if unsize && mode == ReceiverMode::Value {
            return QueryPoll::Ready(Err(call_fault(&format!(
                "method `{method}` takes an unsized slice receiver by value"
            ))));
        }
        if !unsize && mode == ReceiverMode::Shared && references.last() == Some(&Some(false)) {
            derefs -= 1;
            mode = ReceiverMode::Value;
        }
        self.frame.method_receivers.insert(
            self.frame.call_identity(site),
            ReceiverAdjustment {
                derefs,
                mode,
                unsize,
            },
        );
        self.prepare_selected_argument_call(db, callee, syntax, arguments)
            .map(|mut call| {
                call.monomorphic = true;
                call
            })
    }
}
