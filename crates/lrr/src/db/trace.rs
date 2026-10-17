use super::{dependency::DependencyStamp, key::QueryKey};
use serde::Serialize;
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    ProviderRun,
    MemoHit,
    ValidatedReuse,
    Invalidated,
    Cycle,
    Evicted,
    Suspended,
}
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub revision: u64,
    pub key: QueryKey,
    pub action: Action,
    pub reason: String,
    pub dependencies: Vec<DependencyStamp>,
}
#[derive(Debug, Default, Serialize)]
pub struct Trace {
    pub counts: BTreeMap<String, u64>,
    pub events: Vec<Event>,
    pub truncated: u64,
    #[serde(skip)]
    pub detailed: bool,
}
impl Trace {
    pub fn record(
        &mut self,
        revision: u64,
        key: QueryKey,
        action: Action,
        reason: &str,
        dependencies: Vec<DependencyStamp>,
    ) {
        let label = format!("{}:{action:?}", key.kind());
        let count = self.counts.entry(label).or_default();
        *count = count.saturating_add(1);
        if self.detailed {
            self.events.push(Event {
                revision,
                key,
                action,
                reason: reason.into(),
                dependencies,
            });
        }
    }
}
