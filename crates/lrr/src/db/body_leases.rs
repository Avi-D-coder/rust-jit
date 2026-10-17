//! Coordinator-created source ownership for actual active/suspended VM frames.
//! Weak registry entries do not keep a finished/dropped execution alive.
use super::*;
use crate::{source::manifest::BodyKey, syntax::parse::ParsedFile};
use std::sync::Weak;

pub(super) struct BodyExecutionOwner {
    body: BodyKey,
    // These actual immutable inputs/parses remain charged by the existing
    // input/query pin accounting even after a source-universe replacement.
    _body: Arc<InputValue>,
    _locator: Arc<InputValue>,
    _source: Arc<InputValue>,
    _parsed: Arc<ParsedFile>,
}
pub(crate) struct BodyExecutionLease(Arc<BodyExecutionOwner>);
impl BodyExecutionLease {
    pub(crate) fn body(&self) -> &BodyKey {
        &self.0.body
    }
    pub(crate) fn require_current(&self, db: &mut Database) -> QueryPoll<()> {
        for (key, expected) in [
            (
                InputKey::DefinitionBody(self.0.body.definition),
                &self.0._body,
            ),
            (
                InputKey::DefinitionSource(self.0.body.definition),
                &self.0._locator,
            ),
        ] {
            if db
                .input(&key)
                .is_none_or(|actual| actual.fingerprint() != expected.fingerprint())
            {
                return QueryPoll::Ready(Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "retained frame source/body publication changed",
                )));
            }
        }
        match self.0._source.as_ref() {
            InputValue::RetainedSyntax(input) => {
                if !input.prepared().permits_body(self.0.body.definition) {
                    return QueryPoll::Ready(Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "retained syntax readiness belongs to another frame body",
                    )));
                }
                input.require_current(db)
            }
            InputValue::BodyEnvironment(_) => {
                let InputValue::DefinitionSource(parse) = self.0._locator.as_ref() else {
                    return QueryPoll::Ready(Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "local frame source locator differs",
                    )));
                };
                require_local_environment(db, &self.0._source, &self.0.body, parse)
            }
            InputValue::GeneratedSyntax(input) => match db.fetch(
                &super::super::provider::SyntaxProvider,
                QueryKey::DiscoverySeal(input.seal.clone()),
            ) {
                QueryPoll::Ready(Ok(outcome)) if matches!(outcome.value.as_ref(),QueryValue::DiscoverySeal(actual) if actual.key()==input.seal) => {
                    QueryPoll::Ready(Ok(()))
                }
                QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Err(fault(
                    "LRR-E-INTERNAL-SNAPSHOT",
                    "retained generated frame seal differs",
                ))),
                QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
            },
            InputValue::SourceBlob(_) => QueryPoll::Ready(Ok(())),
            _ => QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "retained frame source ownership variant differs",
            ))),
        }
    }
}
/// Body-specific authority for a genuinely admitted local frame. An actual
/// parent ParseKey is not a retained readiness permit for every nested body.
fn require_local_environment(
    db: &mut Database,
    source: &Arc<InputValue>,
    body: &BodyKey,
    parse: &crate::syntax::parse::ParseKey,
) -> QueryPoll<()> {
    let key = InputKey::BodyEnvironment(body.clone());
    let InputValue::BodyEnvironment(environment) = source.as_ref() else {
        return QueryPoll::Ready(Err(fault(
            "LRR-E-INTERNAL-SNAPSHOT",
            "local frame environment variant differs",
        )));
    };
    if environment.body() != body
        || !source.matches(&key)
        || db
            .input(&key)
            .is_none_or(|current| current.fingerprint() != source.fingerprint())
    {
        return QueryPoll::Ready(Err(fault(
            "LRR-E-INTERNAL-SNAPSHOT",
            "local frame environment publication changed",
        )));
    }
    // This checks exact body/index/parse and the private current LocalBodyContext
    // chain. Neither an owner digest nor an ancestor retained permit suffices.
    match environment
        .source()
        .require_execution_readiness(db, body, parse)
    {
        QueryPoll::Ready(Ok(())) => environment.macros().require_current(db),
        other => other,
    }
}
impl std::fmt::Debug for BodyExecutionLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("BodyExecutionLease")
            .field(self.body())
            .finish()
    }
}
pub(super) fn metadata(capacity: usize, entries: usize) -> Result<usize, Diagnostic> {
    let weak = capacity.checked_mul(std::mem::size_of::<Weak<BodyExecutionOwner>>());
    let owners = std::mem::size_of::<BodyExecutionOwner>()
        .checked_add(2 * std::mem::size_of::<usize>())
        .and_then(|n| entries.checked_mul(n));
    weak.and_then(|n| owners.and_then(|m| n.checked_add(m)))
        .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "body lease metadata overflow"))
}
impl Database {
    pub(crate) fn require_body_unowned(&self, definition: Digest) -> Result<(), Diagnostic> {
        self.require_coordinator()?;
        if self
            .body_leases
            .iter()
            .filter_map(Weak::upgrade)
            .any(|owner| owner.body.definition == definition)
        {
            return Err(fault(
                "LRR-E-BODY-ACTIVE",
                "definition still belongs to a retained active or suspended frame",
            ));
        }
        Ok(())
    }
    /// Fetch no producer or executable thunk. All immutable source preparation
    /// must finish before Frame::new/new_in_run mutates the guest store.
    pub(crate) fn pin_guest_body(&mut self, body: &BodyKey) -> QueryPoll<BodyExecutionLease> {
        if let Err(error) = self.require_coordinator() {
            return QueryPoll::Ready(Err(error));
        }
        let Some(handle) = self.input(&InputKey::DefinitionBody(body.definition)) else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "frame body input is absent",
            )));
        };
        if !matches!(handle.as_ref(),InputValue::DefinitionBody(handle) if handle.key()==*body) {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "frame body input is stale",
            )));
        }
        let Some(locator) = self.input(&InputKey::DefinitionSource(body.definition)) else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "frame source input is absent",
            )));
        };
        let InputValue::DefinitionSource(parse) = locator.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "frame source variant differs",
            )));
        };
        let outcome = match self.fetch(
            &super::super::provider::SyntaxProvider,
            QueryKey::ParseFile(parse.clone()),
        ) {
            QueryPoll::Ready(Ok(outcome)) => outcome,
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        };
        let QueryValue::ParseFile(parsed) = outcome.value.as_ref() else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "frame parse result differs",
            )));
        };
        if parsed.key() != *parse {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "frame parse key differs",
            )));
        }
        // Actual admitted local bodies carry their own opaque source receipt.
        // Select by the genuine owner input, never by a failed retained permit
        // or by trying another content namespace after an authority error.
        let local = self
            .input(&InputKey::LocalDeclarationOwner(body.definition))
            .is_some();
        let mut source = if local {
            match self.input(&InputKey::BodyEnvironment(body.clone())) {
                Some(environment) => Some(environment),
                None => {
                    return QueryPoll::Ready(Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "local frame requires its admitted owning body environment",
                    )));
                }
            }
        } else {
            None
        };
        for key in [
            InputKey::RetainedSyntax(parse.content),
            InputKey::GeneratedSyntax(parse.content),
            InputKey::SourceBlob(parse.content),
        ] {
            if local {
                break;
            }
            if let Some(value) = self.input(&key) {
                if !value.matches(&key) {
                    return QueryPoll::Ready(Err(fault(
                        "LRR-E-INTERNAL-SNAPSHOT",
                        "frame source binding differs",
                    )));
                }
                source = Some(value);
                break;
            }
        }
        let Some(source) = source else {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-INTERNAL-SNAPSHOT",
                "frame source ownership absent",
            )));
        };
        if local {
            match require_local_environment(self, &source, body, parse) {
                QueryPoll::Ready(Ok(())) => {}
                QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
            }
        }
        if let InputValue::RetainedSyntax(input) = source.as_ref()
            && !input.prepared().permits_body(body.definition)
        {
            return QueryPoll::Ready(Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "retained syntax readiness belongs to another frame body",
            )));
        }
        let result = (|| {
            let live = self
                .body_leases
                .iter()
                .filter(|owner| owner.strong_count() != 0)
                .count();
            let count = live
                .checked_add(1)
                .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "body execution lease count limit"))?;
            let mut registry = Vec::new();
            registry
                .try_reserve_exact(count)
                .map_err(|_| fault("LRR-E-METADATA-LIMIT", "body execution registry allocation"))?;
            registry.extend(
                self.body_leases
                    .iter()
                    .filter(|owner| owner.strong_count() != 0)
                    .cloned(),
            );
            let old = metadata(self.body_leases.capacity(), self.body_leases.len())?;
            let new = metadata(registry.capacity(), count)?;
            let _ = (old, new); // E-R5-006: no byte accounting.
            let owner = Arc::new(BodyExecutionOwner {
                body: body.clone(),
                _body: handle,
                _locator: locator,
                _source: source,
                _parsed: parsed.clone(),
            });
            registry.push(Arc::downgrade(&owner));
            self.body_leases = registry;
            Ok(BodyExecutionLease(owner))
        })();
        QueryPoll::Ready(result)
    }
}

#[cfg(test)]
#[path = "body_lease_local_readiness_tests.rs"]
mod tests;
