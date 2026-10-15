//! Exact source call certificates at reached runtime-macro protocol boundaries.
use super::*;
use crate::{
    db::dependency::{InputKey, InputValue},
    syntax::runtime_macro_scalars::{RuntimeMacroCall, RuntimeMacroCallPhase},
};
use std::sync::Arc;
impl RunMachine {
    pub(super) fn retain_runtime_call(
        &mut self,
        key: &PcKey,
        call: Arc<RuntimeMacroCall>,
    ) -> Result<(), Diagnostic> {
        if let Some(previous) = self.runtime_calls.get(key) {
            if previous.identity() != call.identity() {
                return Err(call_fault("runtime source call certificate changed"));
            }
            return Ok(());
        }
        // Conservatively charge shared witness storage at every reached phase,
        // including the actual owned capacities and map/key overhead.
        let bytes = call
            .allocated_bytes()?
            .checked_add(128)
            .and_then(|bytes| self.runtime_call_bytes.checked_add(bytes))
            .filter(|bytes| *bytes <= 4 * 1024 * 1024 && self.runtime_calls.len() < 4096)
            .ok_or_else(crate::runtime::arena::resource)?;
        self.runtime_calls.insert(key.clone(), call);
        self.runtime_call_bytes = bytes;
        Ok(())
    }
    pub(super) fn require_runtime_call_start(&self, db: &mut Database) -> QueryPoll<()> {
        let result = (|| {
            let call = self
                .frame
                .runtime_call
                .as_ref()
                .ok_or_else(|| call_fault("source macro call certificate absent"))?;
            call.require(
                self.frame.body.body(),
                self.frame.pc(),
                RuntimeMacroCallPhase::Prepare,
            )?;
            if let Some(parent) = call.parent()
                && !self
                    .pending
                    .contains_key(&(self.frame.pc().node, Some(parent)))
            {
                return Err(call_fault("nested macro call lost its selected parent"));
            }
            if !matches!(db.input(&InputKey::DefinitionSource(self.frame.pc().body.definition)).as_deref(),
                Some(InputValue::DefinitionSource(source)) if source == call.source())
            {
                return Err(call_fault("runtime macro call source publication differs"));
            }
            let source = self
                .sources
                .iter()
                .find(|source| {
                    &source.selection().outline.parse == call.source()
                        && source.outline().items.iter().any(|item| {
                            item.body
                                .as_ref()
                                .is_some_and(|body| body.key() == self.frame.pc().body)
                        })
                })
                .ok_or_else(|| call_fault("runtime macro call owning source absent"))?;
            let body = source
                .outline()
                .items
                .iter()
                .filter_map(|item| item.body.as_ref())
                .find(|body| body.key() == self.frame.pc().body)
                .ok_or_else(|| call_fault("runtime macro call owning body absent"))?;
            source.require_runtime_macro_call(body, &self.frame.body, self.frame.pc(), call)
        })();
        QueryPoll::Ready(result)
    }
    pub(super) fn require_runtime_call_operation(&self, op: &Op) -> Result<(), Diagnostic> {
        let Some(call) = &self.frame.runtime_call else {
            return Ok(());
        };
        let (site, phase) = match op {
            Op::CallPrepare {
                site,
                name,
                explicit,
                arguments,
            } => {
                if name != call.name()
                    || explicit != call.explicit()
                    || arguments != call.arguments()
                {
                    return Err(call_fault("source macro call prepare operands differ"));
                }
                (site, RuntimeMacroCallPhase::Prepare)
            }
            Op::CallAdvance { site, argument } => (site, RuntimeMacroCallPhase::Advance(*argument)),
            Op::CallSeal {
                site,
                name,
                explicit,
                arguments,
            } => {
                if name != call.name()
                    || explicit != call.explicit()
                    || arguments != call.arguments()
                {
                    return Err(call_fault("source macro call seal operands differ"));
                }
                (site, RuntimeMacroCallPhase::Seal)
            }
            _ => return Err(call_fault("non-call opcode at source call boundary")),
        };
        if site.0 != self.frame.pc().node {
            return Err(call_fault("source macro call indexed site differs"));
        }
        call.require(self.frame.body.body(), self.frame.pc(), phase)
    }
}
