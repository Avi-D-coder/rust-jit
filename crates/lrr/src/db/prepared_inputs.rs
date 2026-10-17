//! Cold capture of a pure memo's actual input closure. Runtime checks inspect
//! only cached published stamps; they never fetch a query or plan source.
use super::{Database, fault};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    db::{
        dependency::{DependencyStamp, InputKey, missing_fingerprint},
        key::QueryKey,
        poll::QueryPoll,
        provider::Provider,
    },
};
use std::collections::{BTreeMap, BTreeSet};

const MAX_DEPENDENCIES: usize = 100_000;
const MAX_BYTES: usize = 32 * 1024 * 1024;

/// Minted only from a successful current pure memo. This is a currentness
/// receipt, not authority to perform a lookup or execute an operation.
#[derive(Debug)]
pub(crate) struct PreparedInputGuard {
    inputs: Vec<DependencyStamp>,
    bytes: usize,
    fingerprint: Digest,
}
fn guard_fingerprint(inputs: &[DependencyStamp]) -> Digest {
    let mut e = StableEncoder::new("lrr/v1/prepared-input-guard");
    e.sequence(inputs, |e, input| e.record(input));
    e.finish()
}
impl PreparedInputGuard {
    /// Cold composition of actual captured closures. No lookup, stamp refresh,
    /// provider execution or new authority is granted by combining them.
    pub(crate) fn join(guards: &[&Self]) -> Result<Self, Diagnostic> {
        if guards.len() > MAX_DEPENDENCIES {
            return Err(limit());
        }
        let mut inputs = BTreeMap::new();
        let mut visits = 0usize;
        let mut temporary_bytes = 0usize;
        let mut key_bytes = 0usize;
        for guard in guards {
            visits = visits
                .checked_add(guard.inputs.len())
                .filter(|n| *n <= MAX_DEPENDENCIES)
                .ok_or_else(limit)?;
            for dependency in &guard.inputs {
                let DependencyStamp::Input { key, fingerprint } = dependency else {
                    return Err(stale("prepared input join contains a derived demand"));
                };
                if let Some(previous) = inputs.get(key) {
                    if previous != fingerprint {
                        return Err(stale("prepared input join stamps conflict"));
                    }
                    continue;
                }
                temporary_bytes = temporary_bytes
                    .checked_add(std::mem::size_of::<(&InputKey, Digest)>() + 64)
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or_else(limit)?;
                key_bytes = key_bytes
                    .checked_add(dependency.heap_bytes())
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or_else(limit)?;
                inputs.insert(key, *fingerprint);
            }
        }
        let mut stamps = Vec::new();
        stamps
            .try_reserve_exact(inputs.len())
            .map_err(|_| limit())?;
        let bytes = stamps
            .capacity()
            .checked_mul(std::mem::size_of::<DependencyStamp>())
            .and_then(|n| n.checked_add(key_bytes))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        // Bound simultaneous transient index and final owned closure too.
        bytes
            .checked_add(temporary_bytes)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        stamps.extend(
            inputs
                .into_iter()
                .map(|(key, fingerprint)| DependencyStamp::Input {
                    key: key.clone(),
                    fingerprint,
                }),
        );
        Ok(Self {
            fingerprint: guard_fingerprint(&stamps),
            inputs: stamps,
            bytes,
        })
    }
    pub(crate) fn metadata_bytes(&self) -> usize {
        self.bytes
    }
    /// Cached once at private cold capture. It never grants lookup/execution
    /// authority or refreshes a stamp; shared phase attachment reads O(1).
    pub(crate) fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub(crate) fn require_current(&self, db: &Database) -> Result<(), Diagnostic> {
        for dependency in &self.inputs {
            let DependencyStamp::Input { key, fingerprint } = dependency else {
                return Err(stale("prepared input guard contains a derived demand"));
            };
            let actual = db
                .published_input(key)
                .map_or_else(missing_fingerprint, |(_, fingerprint)| fingerprint);
            if actual != *fingerprint {
                return Err(stale("prepared operation input changed"));
            }
        }
        Ok(())
    }
}
fn stale(message: &str) -> Diagnostic {
    fault("LRR-E-INTERNAL-SNAPSHOT", message)
}
fn limit() -> Diagnostic {
    crate::runtime::arena::resource()
}
enum CaptureFailure {
    Missing { key: QueryKey, expected: Digest },
    Invalid(Diagnostic),
}
impl From<Diagnostic> for CaptureFailure {
    fn from(error: Diagnostic) -> Self {
        Self::Invalid(error)
    }
}
/// Conservative cumulative temporary ownership, including keys that have
/// already left the worklist. Keep a missing-memo repair linear and bounded.
#[derive(Default)]
struct RepairBudget {
    bytes: usize,
    edges: usize,
}
impl RepairBudget {
    fn bytes(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or_else(limit)?;
        Ok(())
    }
    fn query(&mut self, key: &QueryKey) -> Result<(), Diagnostic> {
        // Work entry, eventual visited BTree row, and a transient fetch key.
        self.bytes(
            key.heap_bytes()
                .checked_mul(2)
                .and_then(|n| n.checked_add(2 * std::mem::size_of::<QueryKey>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<Digest>() + 64))
                .ok_or_else(limit)?,
        )
    }
}
impl Database {
    /// Traverse the successful memo DAG once at installation. Actual input
    /// stamps remain valid even if an intermediate memo is later evicted.
    /// Providers are pure; an uncacheable/suspended outcome cannot mint this.
    #[cfg(test)]
    pub(crate) fn capture_prepared_inputs(
        &self,
        key: &QueryKey,
        fingerprint: Digest,
    ) -> Result<PreparedInputGuard, Diagnostic> {
        self.capture_input_closure(key, fingerprint)
            .map_err(|failure| match failure {
                CaptureFailure::Missing { .. } => stale("prepared operation memo is absent"),
                CaptureFailure::Invalid(error) => error,
            })
    }
    fn capture_input_closure(
        &self,
        root: &QueryKey,
        fingerprint: Digest,
    ) -> Result<PreparedInputGuard, CaptureFailure> {
        let mut work = vec![(root, fingerprint)];
        let mut visited = BTreeSet::new();
        let mut inputs: BTreeMap<&InputKey, Digest> = BTreeMap::new();
        let mut edges = 0usize;
        let mut key_bytes = 0usize;
        while let Some((key, expected)) = work.pop() {
            let Some(memo) = self.memo(key) else {
                if key == root {
                    return Err(stale("prepared operation memo is absent").into());
                }
                if key.heap_bytes() > MAX_BYTES {
                    return Err(limit().into());
                }
                return Err(CaptureFailure::Missing {
                    key: key.clone(),
                    expected,
                });
            };
            if memo.fingerprint != expected {
                return Err(stale("prepared operation memo fingerprint differs").into());
            }
            if !visited.insert(key) {
                continue;
            }
            edges = edges
                .checked_add(memo.dependencies.len())
                .filter(|edges| *edges <= MAX_DEPENDENCIES)
                .ok_or_else(limit)?;
            work.try_reserve(memo.dependencies.len())
                .map_err(|_| limit())?;
            for dependency in &memo.dependencies {
                match dependency {
                    DependencyStamp::Input { key, fingerprint } => {
                        if let Some(previous) = inputs.insert(key, *fingerprint) {
                            if previous != *fingerprint {
                                return Err(stale("prepared input stamps conflict").into());
                            }
                        } else {
                            key_bytes = key_bytes
                                .checked_add(dependency.heap_bytes())
                                .filter(|bytes| *bytes <= MAX_BYTES)
                                .ok_or_else(limit)?;
                        }
                    }
                    DependencyStamp::Query { key, fingerprint } => work.push((key, *fingerprint)),
                }
            }
        }
        let mut stamps = Vec::new();
        stamps
            .try_reserve_exact(inputs.len())
            .map_err(|_| limit())?;
        let bytes = stamps
            .capacity()
            .checked_mul(std::mem::size_of::<DependencyStamp>())
            .and_then(|n| n.checked_add(key_bytes))
            .and_then(|n| n.checked_add(std::mem::size_of::<PreparedInputGuard>()))
            .filter(|bytes| *bytes <= MAX_BYTES)
            .ok_or_else(limit)?;
        stamps.extend(
            inputs
                .into_iter()
                .map(|(key, fingerprint)| DependencyStamp::Input {
                    key: key.clone(),
                    fingerprint,
                }),
        );
        let guard = PreparedInputGuard {
            fingerprint: guard_fingerprint(&stamps),
            inputs: stamps,
            bytes,
        };
        guard.require_current(self)?;
        Ok(guard)
    }
    /// Installation-only repair. A complete closure never fetches. Only an
    /// actual absent descendant can enter the bounded cold provider walk.
    pub(crate) fn prepare_input_guard(
        &mut self,
        provider: &impl Provider,
        key: &QueryKey,
        expected: Digest,
    ) -> QueryPoll<PreparedInputGuard> {
        match self.capture_input_closure(key, expected) {
            Ok(guard) => QueryPoll::Ready(Ok(guard)),
            Err(CaptureFailure::Invalid(error)) => QueryPoll::Ready(Err(error)),
            Err(CaptureFailure::Missing {
                key: missing,
                expected: missing_expected,
            }) => {
                // Preserve the precise observed missing edge. The owned walk
                // rechecks the root and every edge before returning any guard.
                self.repair_input_closure(provider, key, expected, missing, missing_expected)
            }
        }
    }
    fn repair_input_closure(
        &mut self,
        provider: &impl Provider,
        root: &QueryKey,
        expected: Digest,
        missing: QueryKey,
        missing_expected: Digest,
    ) -> QueryPoll<PreparedInputGuard> {
        macro_rules! checked {
            ($value:expr) => {
                match $value {
                    Ok(value) => value,
                    Err(error) => return QueryPoll::Ready(Err(error)),
                }
            };
        }
        let mut budget = RepairBudget::default();
        checked!(budget.query(root));
        checked!(budget.query(&missing));
        // This first cold fetch follows the actual observed missing edge, not
        // a guessed dependency or a refresh of all existing memo values.
        match self.fetch_prepared_dependency(provider, &missing, missing_expected) {
            QueryPoll::Ready(Ok(())) => {}
            QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
        }
        let mut work = Vec::new();
        checked!(work.try_reserve_exact(1).map_err(|_| limit()));
        work.push((root.clone(), expected));
        let mut visited = BTreeSet::new();
        let mut inputs = BTreeMap::<InputKey, Digest>::new();
        let mut input_heap = 0usize;
        while let Some((key, expected)) = work.pop() {
            if self.memo(&key).is_none() {
                if &key == root {
                    return QueryPoll::Ready(Err(stale("prepared operation memo is absent")));
                }
                match self.fetch_prepared_dependency(provider, &key, expected) {
                    QueryPoll::Ready(Ok(())) => {}
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            }
            let memo = match self.memo(&key) {
                Some(memo) if memo.fingerprint == expected => memo,
                Some(_) => {
                    return QueryPoll::Ready(Err(stale(
                        "prepared operation memo fingerprint differs",
                    )));
                }
                None => {
                    return QueryPoll::Ready(Err(stale(
                        "prepared dependency memo was not retained",
                    )));
                }
            };
            // Check each expected fingerprint even on repeated graph edges.
            if !visited.insert(key) {
                continue;
            }
            budget.edges = checked!(
                budget
                    .edges
                    .checked_add(memo.dependencies.len())
                    .filter(|n| *n <= MAX_DEPENDENCIES)
                    .ok_or_else(limit)
            );
            let query_edges = memo
                .dependencies
                .iter()
                .filter(|dependency| matches!(dependency, DependencyStamp::Query { .. }))
                .count();
            // Charge work/visited key ownership before reserving or cloning.
            for dependency in &memo.dependencies {
                if let DependencyStamp::Query { key, .. } = dependency {
                    checked!(budget.query(key));
                }
            }
            checked!(work.try_reserve_exact(query_edges).map_err(|_| limit()));
            for dependency in &memo.dependencies {
                match dependency {
                    DependencyStamp::Input { key, fingerprint } => {
                        if let Some(previous) = inputs.get(key) {
                            if previous != fingerprint {
                                return QueryPoll::Ready(Err(stale(
                                    "prepared input stamps conflict",
                                )));
                            }
                        } else {
                            let row_bytes = checked!(
                                dependency
                                    .heap_bytes()
                                    .checked_add(
                                        std::mem::size_of::<InputKey>()
                                            + std::mem::size_of::<Digest>()
                                            + 64
                                    )
                                    .ok_or_else(limit)
                            );
                            checked!(budget.bytes(row_bytes));
                            input_heap = checked!(
                                input_heap
                                    .checked_add(dependency.heap_bytes())
                                    .ok_or_else(limit)
                            );
                            inputs.insert(key.clone(), *fingerprint);
                        }
                    }
                    DependencyStamp::Query { key, fingerprint } => {
                        work.push((key.clone(), *fingerprint));
                    }
                }
            }
        }
        let mut stamps = Vec::new();
        let planned_bytes = checked!(
            inputs
                .len()
                .checked_mul(std::mem::size_of::<DependencyStamp>())
                .and_then(|n| n.checked_add(input_heap))
                .and_then(|n| n.checked_add(std::mem::size_of::<PreparedInputGuard>()))
                .ok_or_else(limit)
        );
        checked!(budget.bytes(planned_bytes));
        checked!(stamps.try_reserve_exact(inputs.len()).map_err(|_| limit()));
        let bytes = checked!(
            stamps
                .capacity()
                .checked_mul(std::mem::size_of::<DependencyStamp>())
                .and_then(|n| n.checked_add(input_heap))
                .and_then(|n| n.checked_add(std::mem::size_of::<PreparedInputGuard>()))
                .filter(|n| *n <= MAX_BYTES)
                .ok_or_else(limit)
        );
        checked!(budget.bytes(checked!(bytes.checked_sub(planned_bytes).ok_or_else(limit))));
        stamps.extend(
            inputs
                .into_iter()
                .map(|(key, fingerprint)| DependencyStamp::Input { key, fingerprint }),
        );
        let guard = PreparedInputGuard {
            fingerprint: guard_fingerprint(&stamps),
            inputs: stamps,
            bytes,
        };
        QueryPoll::Ready(guard.require_current(self).map(|()| guard))
    }
    fn fetch_prepared_dependency(
        &mut self,
        provider: &impl Provider,
        key: &QueryKey,
        expected: Digest,
    ) -> QueryPoll<()> {
        // Caller reaches here only for an actual absent descendant memo.
        match self.fetch(provider, key.clone()) {
            QueryPoll::Ready(Ok(outcome)) if !outcome.cacheable => {
                QueryPoll::Ready(Err(stale("prepared dependency is uncacheable")))
            }
            QueryPoll::Ready(Ok(outcome)) if outcome.fingerprint != expected => {
                QueryPoll::Ready(Err(stale("prepared operation memo fingerprint differs")))
            }
            QueryPoll::Ready(Ok(_)) => QueryPoll::Ready(Ok(())),
            QueryPoll::Ready(Err(error)) => QueryPoll::Ready(Err(error)),
            QueryPoll::NeedInput(input) => QueryPoll::NeedInput(input),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate as lrr;
    use crate::{
        core::profile::ProfileIdentity,
        db::{
            dependency::InputValue,
            poll::QueryPoll,
            provider::{Provided, Provider},
            trace::Action,
        },
        engine::telemetry::CacheBudgets,
    };
    use std::sync::Arc;
    include!("../../../../xtask/tests/support/constant_key.rs");
    include!("../../../../xtask/tests/support/constant_value.rs");
    fn key(n: u8) -> QueryKey {
        QueryKey::Constant(fixture_key(
            Digest::of_bytes(&[n]),
            vec![],
            Digest::default(),
            ProfileIdentity::current(),
        ))
    }
    fn positive() -> InputKey {
        InputKey::CompileEnv("prepared-positive".into())
    }
    fn negative() -> InputKey {
        InputKey::CompileEnv("prepared-absence".into())
    }
    struct Dependencies;
    impl Provider for Dependencies {
        fn execute(&self, db: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
            if requested == &key(1) {
                match db.fetch(self, key(2)) {
                    QueryPoll::Ready(Ok(_)) => {}
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
            } else {
                db.input(&positive());
                db.input(&negative());
            }
            Provided::pure(fixture_value(requested, 1))
        }
    }
    fn capture(db: &mut Database) -> PreparedInputGuard {
        let result = db
            .fetch(&Dependencies, key(1))
            .into_ready()
            .unwrap()
            .unwrap();
        db.capture_prepared_inputs(&key(1), result.fingerprint)
            .unwrap()
    }
    #[test]
    fn exact_transitive_inputs_survive_memo_eviction_and_reject_changed_absence() {
        let mut db = Database::default();
        db.trace.detailed = true;
        db.publish_inputs(BTreeMap::from([(
            positive(),
            InputValue::CompileEnv("v1".into()),
        )]))
        .unwrap();
        let guard = capture(&mut db);
        assert_eq!(guard.inputs.len(), 2);
        assert!(guard.metadata_bytes() >= std::mem::size_of::<PreparedInputGuard>());
        db.publish_inputs(BTreeMap::from([
            (positive(), InputValue::CompileEnv("v1".into())),
            (
                InputKey::CompileEnv("unrelated".into()),
                InputValue::CompileEnv("other".into()),
            ),
        ]))
        .unwrap();
        let events = db.trace.events.len();
        guard.require_current(&db).unwrap();
        assert_eq!(
            db.trace.events.len(),
            events,
            "installed guard must not query or plan"
        );
        // Current input rows are a mandatory pinned floor. Only the query
        // memos are eligible here; a zero budget would reject those real rows.
        let input_floor = db.cache_usage().unwrap().pinned_memo_bytes;
        db.evict_between_runs(CacheBudgets {
            query_bytes: input_floor,
            ..CacheBudgets::default()
        })
        .unwrap();
        assert!(db.memo(&key(1)).is_none());
        assert!(db.memo(&key(2)).is_none());
        let events = db.trace.events.len();
        guard.require_current(&db).unwrap();
        assert_eq!(db.trace.events.len(), events);
        db.publish_inputs(BTreeMap::from([
            (positive(), InputValue::CompileEnv("v1".into())),
            (
                InputKey::CompileEnv("unrelated".into()),
                InputValue::CompileEnv("other".into()),
            ),
            (negative(), InputValue::CompileEnv("now present".into())),
        ]))
        .unwrap();
        assert_eq!(
            guard.require_current(&db).unwrap_err().code,
            "LRR-E-INTERNAL-SNAPSHOT"
        );
    }
    #[test]
    fn positive_changes_and_wrong_root_fingerprint_cannot_refresh_guard() {
        let mut db = Database::default();
        db.publish_inputs(BTreeMap::from([(
            positive(),
            InputValue::CompileEnv("v1".into()),
        )]))
        .unwrap();
        let guard = capture(&mut db);
        assert!(
            db.capture_prepared_inputs(&key(1), Digest::of_bytes(b"different result"))
                .is_err()
        );
        db.publish_inputs(BTreeMap::from([(
            positive(),
            InputValue::CompileEnv("v2".into()),
        )]))
        .unwrap();
        assert_eq!(
            guard.require_current(&db).unwrap_err().code,
            "LRR-E-INTERNAL-SNAPSHOT"
        );
        assert!(
            db.capture_prepared_inputs(&key(1), db.memo(&key(1)).unwrap().fingerprint)
                .is_err()
        );
        let successor = capture(&mut db);
        successor.require_current(&db).unwrap();
        assert!(
            guard.require_current(&db).is_err(),
            "a new query cannot refresh a retained guard"
        );
    }

    fn ready<T>(poll: QueryPoll<T>) -> T {
        match poll {
            QueryPoll::Ready(Ok(value)) => value,
            QueryPoll::Ready(Err(error)) => panic!("prepared dependency control: {error:?}"),
            QueryPoll::NeedInput(input) => {
                panic!("prepared dependency control demanded: {input:?}")
            }
        }
    }
    fn evict_only_child(db: &mut Database) {
        // The caller retains the actual root Outcome. Cache ownership, not a
        // private memo mutation, makes only its child eligible for eviction.
        let pinned = db.cache_usage().unwrap().pinned_memo_bytes;
        let eviction = db
            .evict_between_runs(CacheBudgets {
                query_bytes: pinned,
                ..CacheBudgets::default()
            })
            .unwrap();
        assert!(eviction.victims.contains(&key(2)));
        assert!(db.memo(&key(1)).is_some());
        assert!(db.memo(&key(2)).is_none());
    }
    #[test]
    fn retained_current_root_cold_repairs_only_its_evicted_child() {
        let mut db = Database::default();
        db.trace.detailed = true;
        db.publish_inputs(BTreeMap::from([(
            positive(),
            InputValue::CompileEnv("v1".into()),
        )]))
        .unwrap();
        let root = ready(db.fetch(&Dependencies, key(1)));
        let old_guard = db
            .capture_prepared_inputs(&key(1), root.fingerprint)
            .unwrap();
        evict_only_child(&mut db);
        let events = db.trace.events.len();
        let repaired = ready(db.prepare_input_guard(&Dependencies, &key(1), root.fingerprint));
        for selected in [key(1), key(2)] {
            let providers = db.trace.events[events..]
                .iter()
                .filter(|event| event.key == selected && event.action == Action::ProviderRun)
                .count();
            assert_eq!(providers, usize::from(selected == key(2)));
        }
        assert!(Arc::ptr_eq(&root.value, &db.memo(&key(1)).unwrap().value));
        repaired.require_current(&db).unwrap();
        old_guard.require_current(&db).unwrap();
        let events = db.trace.events.len();
        let _fast = ready(db.prepare_input_guard(&Dependencies, &key(1), root.fingerprint));
        assert_eq!(
            db.trace.events.len(),
            events,
            "complete closure never fetches"
        );

        db.publish_inputs(BTreeMap::from([(
            positive(),
            InputValue::CompileEnv("v2".into()),
        )]))
        .unwrap();
        assert_eq!(
            repaired.require_current(&db).unwrap_err().code,
            "LRR-E-INTERNAL-SNAPSHOT"
        );
        match db.prepare_input_guard(&Dependencies, &key(1), root.fingerprint) {
            QueryPoll::Ready(Err(error)) => assert_eq!(
                error.message, "prepared operation input changed",
                "{error:?}"
            ),
            QueryPoll::Ready(Ok(_)) => panic!("cold repair refreshed stale leaf authority"),
            QueryPoll::NeedInput(input) => panic!("stale leaf demanded another input: {input:?}"),
        }
        assert!(old_guard.require_current(&db).is_err());
    }

    struct VersionedDependencies;
    fn capture_unit() -> Digest {
        Digest::of_bytes(b"prepared-query-compile-environment")
    }
    impl Provider for VersionedDependencies {
        fn execute(&self, db: &mut Database, requested: &QueryKey) -> QueryPoll<Provided> {
            if requested == &key(1) {
                match db.fetch(self, key(2)) {
                    QueryPoll::Ready(Ok(_)) => {}
                    QueryPoll::Ready(Err(error)) => return QueryPoll::Ready(Err(error)),
                    QueryPoll::NeedInput(input) => return QueryPoll::NeedInput(input),
                }
                return Provided::pure(fixture_value(requested, 1));
            }
            let version = db.input(&positive()).unwrap();
            let InputValue::CompileEnv(version) = version.as_ref() else {
                panic!("version input differs")
            };
            db.input(&negative());
            if version == "needs-capture"
                && db
                    .input(&InputKey::UnitCompileEnv(capture_unit()))
                    .is_none()
            {
                return QueryPoll::NeedInput(
                    crate::db::poll::InputRequest::CaptureCompileEnvironment {
                        unit: capture_unit(),
                    },
                );
            }
            if version == "uncacheable" {
                return QueryPoll::Ready(Ok(Provided {
                    value: fixture_value(requested, 1),
                    cacheable: false,
                }));
            }
            Provided::pure(fixture_value(
                requested,
                if version == "v2" { 2 } else { 1 },
            ))
        }
    }
    #[test]
    fn missing_child_changed_output_or_real_demand_cannot_mint_a_guard() {
        for next in ["v2", "needs-capture", "uncacheable"] {
            let mut db = Database::default();
            db.publish_inputs(BTreeMap::from([(
                positive(),
                InputValue::CompileEnv("v1".into()),
            )]))
            .unwrap();
            let root = ready(db.fetch(&VersionedDependencies, key(1)));
            let old = db
                .capture_prepared_inputs(&key(1), root.fingerprint)
                .unwrap();
            evict_only_child(&mut db);
            db.publish_inputs(BTreeMap::from([(
                positive(),
                InputValue::CompileEnv(next.into()),
            )]))
            .unwrap();
            match db.prepare_input_guard(&VersionedDependencies, &key(1), root.fingerprint) {
                QueryPoll::Ready(Err(error)) if next == "v2" => {
                    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT", "{error:?}");
                    assert_eq!(
                        error.message, "prepared operation memo fingerprint differs",
                        "{error:?}"
                    );
                }
                QueryPoll::Ready(Err(error)) if next == "uncacheable" => {
                    assert_eq!(error.code, "LRR-E-INTERNAL-SNAPSHOT", "{error:?}");
                    assert_eq!(
                        error.message, "prepared dependency is uncacheable",
                        "{error:?}"
                    );
                    assert!(db.memo(&key(2)).is_none());
                }
                QueryPoll::NeedInput(
                    crate::db::poll::InputRequest::CaptureCompileEnvironment { unit },
                ) if next == "needs-capture" => {
                    assert_eq!(unit, capture_unit());
                    assert!(
                        db.memo(&key(2)).is_none(),
                        "suspended provider must retain no memo"
                    );
                }
                QueryPoll::Ready(Err(error)) => panic!("unexpected repair refusal: {error:?}"),
                QueryPoll::Ready(Ok(_)) => panic!("changed/missing dependency minted a guard"),
                QueryPoll::NeedInput(input) => panic!("unexpected repair demand: {input:?}"),
            }
            assert!(old.require_current(&db).is_err());
        }
    }
}

#[cfg(test)]
#[path = "prepared_input_join_tests.rs"]
mod join_tests;
