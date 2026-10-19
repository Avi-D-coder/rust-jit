//! Closure creation consumes prepared binding accesses and local Type facts.
//! Query providers select source metadata; no pattern search or body executes.
use super::*;
use crate::{
    frontend::pattern_access::PatternAccessKey, vm::frame::closures_frame::join_capture_mode,
};

impl RunMachine {
    pub(super) fn prepare_closure_pattern_access(
        &mut self,
        db: &mut Database,
        id: super::super::install::RecipeId,
    ) -> QueryPoll<()> {
        let pc = self.frame.pc();
        if pc.phase != crate::ir::pc::Phase::Entry.code().expect("entry phase")
            || self
                .frame
                .body
                .nodes()
                .get(pc.node as usize)
                .is_none_or(|node| {
                    node.role
                        != crate::syntax::body_index::NodeRole::Expression(
                            crate::syntax::body_index::ExpressionKind::Closure,
                        )
                })
        {
            return QueryPoll::Ready(Ok(()));
        }
        let recipe = match self.installed.get_current(db, id) {
            Ok(v) => v,
            Err(e) => return QueryPoll::Ready(Err(e)),
        };
        let Some((captures, uses)) = recipe
            .recipe()
            .operations
            .iter()
            .skip(self.frame.resume_operation_index())
            .find_map(|op| match op {
                Op::MakeClosure {
                    captures,
                    pattern_uses,
                    ..
                } if !pattern_uses.is_empty() => Some((captures, pattern_uses)),
                _ => None,
            })
        else {
            return QueryPoll::Ready(Ok(()));
        };
        let mut modes = Vec::with_capacity(captures.len());
        for capture in captures.iter() {
            match self.frame.ordinary_capture_mode(capture, &self.store) {
                Ok(mode) => modes.push(mode),
                Err(e) => return QueryPoll::Ready(Err(e)),
            }
        }
        for usage in uses.iter() {
            let (root, source) = match self.frame.capture_source_type(usage.source, &self.store) {
                Ok(value) => value,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            let key = PatternAccessKey {
                owner: usage.plan.owner().clone(),
                root: source,
                projections: usage.projections.clone(),
            };
            let prepared = if let Some((value, guard)) = self.closures.accesses.get(&key) {
                if let Err(e) = guard.require_current(db) {
                    return QueryPoll::Ready(Err(e));
                }
                Arc::clone(value)
            } else {
                let query = QueryKey::PatternAccess(Box::new(key.clone()));
                let value = match db.fetch(&SyntaxProvider, query.clone()) {
                    QueryPoll::Ready(Ok(v)) => v,
                    QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                    QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                };
                let guard = match db.prepare_input_guard(&SyntaxProvider, &query, value.fingerprint)
                {
                    QueryPoll::Ready(Ok(v)) => Arc::new(v),
                    QueryPoll::Ready(Err(e)) => return QueryPoll::Ready(Err(e)),
                    QueryPoll::NeedInput(i) => return QueryPoll::NeedInput(i),
                };
                self.closures
                    .accesses
                    .insert(key, (Arc::clone(&value.value), guard));
                value.value
            };
            let QueryValue::PatternAccess(access) = prepared.as_ref() else {
                return QueryPoll::Ready(Err(call_fault("closure pattern access query differs")));
            };
            let mode = match self
                .frame
                .pattern_access_mode(root, access, &mut self.store)
            {
                Ok(v) => v,
                Err(e) => return QueryPoll::Ready(Err(e)),
            };
            let Some(ordinal) = captures
                .iter()
                .position(|capture| capture.source == usage.source)
            else {
                return QueryPoll::Ready(Err(call_fault("closure pattern access source absent")));
            };
            modes[ordinal] = join_capture_mode(modes[ordinal], mode);
        }
        self.frame.set_capture_modes(modes.into());
        QueryPoll::Ready(Ok(()))
    }
}
