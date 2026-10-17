//! Bounded deterministic discovery transitions. Expansion and resolution are external.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncoder},
    },
    db::seals::{Seal, SealDomain},
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum DiscoveryKind {
    ParseSource,
    ApplyCfg,
    RegisterHeader,
    ResolveImport,
    ResolveMacro,
    Expand,
    Integrate,
    ScanNestedDeclarations,
}
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct DiscoveryJob {
    pub module_order: u32,
    pub source_offset: u32,
    pub kind: DiscoveryKind,
    pub identity: Digest,
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct JobTicket {
    job: DiscoveryJob,
    epoch: u64,
    request: Digest,
    attempt: u64,
}
impl JobTicket {
    pub fn job(&self) -> &DiscoveryJob {
        &self.job
    }
}
#[derive(Debug, Clone)]
pub struct DiscoveryLimits {
    pub jobs: usize,
    pub members: usize,
    pub metadata_bytes: usize,
}
impl Default for DiscoveryLimits {
    fn default() -> Self {
        Self {
            jobs: usize::MAX,
            members: usize::MAX,
            metadata_bytes: usize::MAX,
        }
    }
}
#[derive(Default)]
pub struct DiscoveryUpdate {
    pub jobs: Vec<DiscoveryJob>,
    pub members: Vec<Digest>,
    pub dependencies: Vec<(Digest, Digest)>,
}
#[derive(Debug)]
pub enum DiscoveryStep {
    Run(JobTicket),
    Incomplete {
        diagnostic: Diagnostic,
        blocked: Vec<(Digest, Vec<Digest>)>,
    },
    Sealed(Seal),
}
#[derive(Clone)]
enum State {
    Ready,
    Blocked(Vec<Digest>),
    Complete,
}
pub struct Discovery {
    domain: SealDomain,
    graph: Digest,
    epoch: u64,
    limits: DiscoveryLimits,
    jobs: BTreeMap<DiscoveryJob, State>,
    job_orders: BTreeMap<(DiscoveryKind, Digest), (u32, u32)>,
    members: BTreeSet<Digest>,
    dependencies: BTreeMap<Digest, Digest>,
    ready: BTreeSet<DiscoveryJob>,
    blocked: BTreeSet<DiscoveryJob>,
    blocked_bytes: usize,
    active: Option<JobTicket>,
    attempt: u64,
    seal: Option<Seal>,
}
fn error(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
fn limit() -> Diagnostic {
    error("LRR-E-DISCOVERY-LIMIT", "discovery metadata or count limit")
}
impl Discovery {
    pub fn new(
        domain: SealDomain,
        graph: Digest,
        epoch: u64,
        limits: DiscoveryLimits,
    ) -> Result<Self, Diagnostic> {
        if epoch == 0 {
            return Err(limit());
        }
        Ok(Self {
            domain,
            graph,
            epoch,
            limits,
            jobs: BTreeMap::new(),
            job_orders: BTreeMap::new(),
            members: BTreeSet::new(),
            dependencies: BTreeMap::new(),
            ready: BTreeSet::new(),
            blocked: BTreeSet::new(),
            blocked_bytes: 0,
            active: None,
            attempt: 0,
            seal: None,
        })
    }
    fn unsealed(&self) -> Result<(), Diagnostic> {
        if self.seal.is_some() {
            Err(error(
                "LRR-E-EXPANSION-SEAL-VIOLATION",
                "discovery membership is already sealed",
            ))
        } else {
            Ok(())
        }
    }
    fn preflight_growth(
        &self,
        jobs: usize,
        members: usize,
        dependencies: usize,
    ) -> Result<usize, Diagnostic> {
        let bytes = jobs
            .checked_mul(448)
            .and_then(|n| members.checked_mul(64).and_then(|v| n.checked_add(v)))
            .and_then(|n| dependencies.checked_mul(128).and_then(|v| n.checked_add(v)))
            .and_then(|n| n.checked_add(256))
            .ok_or_else(limit)?;
        let bytes = bytes.checked_add(self.blocked_bytes).ok_or_else(limit)?;
        Ok(bytes)
    }
    fn check_job(&self, job: &DiscoveryJob) -> Result<(), Diagnostic> {
        if self
            .job_orders
            .get(&(job.kind, job.identity))
            .is_some_and(|order| *order != (job.module_order, job.source_offset))
        {
            return Err(error(
                "LRR-E-CATALOG-INCOMPLETE",
                "discovery job identity has conflicting source order",
            ));
        }
        Ok(())
    }
    pub(crate) fn completed_job(&self, kind: DiscoveryKind, identity: Digest) -> bool {
        let Some((module_order, source_offset)) = self.job_orders.get(&(kind, identity)) else {
            return false;
        };
        matches!(
            self.jobs.get(&DiscoveryJob {
                module_order: *module_order,
                source_offset: *source_offset,
                kind,
                identity
            }),
            Some(State::Complete)
        )
    }
    pub(crate) fn preflight_enqueue_batch(
        &self,
        jobs: &mut Vec<DiscoveryJob>,
    ) -> Result<(), Diagnostic> {
        self.unsealed()?;
        jobs.sort_by_key(|j| (j.kind, j.identity));
        for pair in jobs.windows(2) {
            if (pair[0].kind, pair[0].identity) == (pair[1].kind, pair[1].identity)
                && pair[0] != pair[1]
            {
                return Err(error(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "new discovery job has conflicting source order",
                ));
            }
        }
        jobs.dedup();
        for job in jobs.iter() {
            self.check_job(job)?;
        }
        let additional = jobs.iter().filter(|j| !self.jobs.contains_key(*j)).count();
        self.preflight_growth(
            self.jobs.len().checked_add(additional).ok_or_else(limit)?,
            self.members.len(),
            self.dependencies.len(),
        )?;
        Ok(())
    }
    pub(crate) fn enqueue_batch(&mut self, mut jobs: Vec<DiscoveryJob>) -> Result<(), Diagnostic> {
        self.preflight_enqueue_batch(&mut jobs)?;
        for job in jobs {
            self.enqueue(job)?;
        }
        Ok(())
    }
    pub fn enqueue(&mut self, job: DiscoveryJob) -> Result<(), Diagnostic> {
        self.unsealed()?;
        self.check_job(&job)?;
        if self.jobs.contains_key(&job) {
            return Ok(());
        }
        self.preflight_growth(
            self.jobs.len().checked_add(1).ok_or_else(limit)?,
            self.members.len(),
            self.dependencies.len(),
        )?;
        self.job_orders.insert(
            (job.kind, job.identity),
            (job.module_order, job.source_offset),
        );
        self.ready.insert(job.clone());
        self.jobs.insert(job, State::Ready);
        Ok(())
    }
    fn ticket(&self, t: &JobTicket) -> Result<(), Diagnostic> {
        self.unsealed()?;
        if self.active.as_ref() != Some(t) {
            return Err(error(
                "LRR-E-CATALOG-INCOMPLETE",
                "stale discovery completion ticket",
            ));
        }
        Ok(())
    }
    pub fn block(&mut self, t: &JobTicket, mut waiting: Vec<Digest>) -> Result<(), Diagnostic> {
        self.ticket(t)?;
        if std::env::var("TMPDBG_BLOCK").is_ok() { eprintln!("TMPDBG block {:?} kind={:?} waiting={:?}\n{}", t.job.identity, t.job.kind, waiting, std::backtrace::Backtrace::force_capture()); }
        if waiting.is_empty() {
            return Err(limit());
        }
        waiting.sort();
        waiting.dedup();
        let current =
            self.preflight_growth(self.jobs.len(), self.members.len(), self.dependencies.len())?;
        let additional = waiting.capacity().checked_mul(32).ok_or_else(limit)?;
        current.checked_add(additional).ok_or_else(limit)?;
        self.blocked_bytes = self
            .blocked_bytes
            .checked_add(additional)
            .ok_or_else(limit)?;
        self.blocked.insert(t.job.clone());
        self.jobs.insert(t.job.clone(), State::Blocked(waiting));
        self.active = None;
        Ok(())
    }
    fn preflight_update(
        &self,
        t: &JobTicket,
        update: &mut DiscoveryUpdate,
    ) -> Result<usize, Diagnostic> {
        self.ticket(t)?;
        // Preflight supplied sizes before mutating indexed state; caller facts are
        // admitted by the source/expansion integrator, not interpreted here.
        update.jobs.sort_by_key(|j| (j.kind, j.identity));
        for pair in update.jobs.windows(2) {
            if (pair[0].kind, pair[0].identity) == (pair[1].kind, pair[1].identity)
                && pair[0] != pair[1]
            {
                return Err(error(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "new discovery job has conflicting source order",
                ));
            }
        }
        update.jobs.dedup();
        update.members.sort();
        update.members.dedup();
        update.dependencies.sort();
        for pair in update.dependencies.windows(2) {
            if pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1 {
                return Err(error(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "discovery input fingerprint changed within epoch",
                ));
            }
        }
        update.dependencies.dedup();
        for job in &update.jobs {
            self.check_job(job)?;
        }
        for (id, value) in &update.dependencies {
            if self.dependencies.get(id).is_some_and(|old| old != value) {
                return Err(error(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "discovery input fingerprint changed within epoch",
                ));
            }
        }
        let new_jobs = update
            .jobs
            .iter()
            .filter(|j| !self.jobs.contains_key(*j))
            .count();
        let new_members = update
            .members
            .iter()
            .filter(|m| !self.members.contains(*m))
            .count();
        let new_dependencies = update
            .dependencies
            .iter()
            .filter(|(id, _)| !self.dependencies.contains_key(id))
            .count();
        self.preflight_growth(
            self.jobs.len().checked_add(new_jobs).ok_or_else(limit)?,
            self.members
                .len()
                .checked_add(new_members)
                .ok_or_else(limit)?,
            self.dependencies
                .len()
                .checked_add(new_dependencies)
                .ok_or_else(limit)?,
        )?;
        Ok(new_members)
    }
    pub(crate) fn preflight_complete(
        &self,
        t: &JobTicket,
        update: &mut DiscoveryUpdate,
    ) -> Result<(), Diagnostic> {
        self.preflight_update(t, update).map(|_| ())
    }
    pub fn complete(
        &mut self,
        t: &JobTicket,
        mut update: DiscoveryUpdate,
    ) -> Result<(), Diagnostic> {
        let new_members = self.preflight_update(t, &mut update)?;
        // All fallible validation precedes mutation. Only newly admitted rows
        // allocate, and wakeup visits the blocked index rather than all jobs.
        for job in update.jobs {
            self.job_orders.insert(
                (job.kind, job.identity),
                (job.module_order, job.source_offset),
            );
            if !self.jobs.contains_key(&job) {
                self.ready.insert(job.clone());
                self.jobs.insert(job, State::Ready);
            }
        }
        self.members.extend(update.members);
        self.dependencies.extend(update.dependencies);
        self.jobs.insert(t.job.clone(), State::Complete);
        if new_members != 0 {
            for job in std::mem::take(&mut self.blocked) {
                self.jobs.insert(job.clone(), State::Ready);
                self.ready.insert(job);
            }
            self.blocked_bytes = 0;
        }
        self.active = None;
        Ok(())
    }
    /// Admit an immutable input and retry the same producer without completing it.
    pub(crate) fn preflight_resume_input(
        &self,
        t: &JobTicket,
        id: Digest,
        value: Digest,
    ) -> Result<(), Diagnostic> {
        self.preflight_update(
            t,
            &mut DiscoveryUpdate {
                dependencies: vec![(id, value)],
                ..Default::default()
            },
        )?;
        Ok(())
    }
    pub(crate) fn resume_input(
        &mut self,
        t: &JobTicket,
        id: Digest,
        value: Digest,
    ) -> Result<(), Diagnostic> {
        self.preflight_resume_input(t, id, value)?;
        self.dependencies.insert(id, value);
        self.jobs.insert(t.job.clone(), State::Ready);
        self.ready.insert(t.job.clone());
        self.active = None;
        Ok(())
    }
    pub(crate) fn preflight_namespace_change(&self) -> Result<(), Diagnostic> {
        self.unsealed()?;
        if self.active.is_some() {
            return Err(error(
                "LRR-E-DISCOVERY-TICKET",
                "namespace change has an active job",
            ));
        }
        Ok(())
    }
    pub(crate) fn namespace_changed(&mut self) -> Result<(), Diagnostic> {
        self.preflight_namespace_change()?;
        for job in std::mem::take(&mut self.blocked) {
            self.jobs.insert(job.clone(), State::Ready);
            self.ready.insert(job);
        }
        self.blocked_bytes = 0;
        Ok(())
    }
    pub fn poll(&mut self) -> Result<DiscoveryStep, Diagnostic> {
        if let Some(seal) = &self.seal {
            return Ok(DiscoveryStep::Sealed(seal.clone()));
        }
        if let Some(t) = &self.active {
            return Ok(DiscoveryStep::Run(t.clone()));
        }
        if let Some(job) = self.ready.first() {
            self.attempt = self.attempt.checked_add(1).ok_or_else(limit)?;
            let mut request = StableEncoder::new("lrr/v2/discovery-request");
            request.record(&self.domain);
            request.digest(self.graph);
            request.u64(self.epoch);
            let ticket = JobTicket {
                job: job.clone(),
                epoch: self.epoch,
                request: request.finish(),
                attempt: self.attempt,
            };
            self.ready.remove(&ticket.job);
            self.active = Some(ticket.clone());
            return Ok(DiscoveryStep::Run(ticket));
        }
        let mut blocked = self
            .jobs
            .iter()
            .filter_map(|(job, s)| {
                if let State::Blocked(wait) = s {
                    Some((job.identity, wait.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        blocked.sort();
        if !blocked.is_empty() {
            return Ok(DiscoveryStep::Incomplete {
                diagnostic: error(
                    "LRR-E-CATALOG-INCOMPLETE",
                    "fixed point retains unresolved import or macro producers",
                ),
                blocked,
            });
        }
        let mut e = StableEncoder::new("lrr/v2/discovery-members");
        e.u64(u64::try_from(self.members.len()).map_err(|_| limit())?);
        for member in &self.members {
            e.digest(*member);
        }
        let mut scanned = StableEncoder::new("lrr/v2/discovery-scanned-jobs");
        scanned.u64(u64::try_from(self.jobs.len()).map_err(|_| limit())?);
        for job in self.jobs.keys() {
            scanned.u32(job.module_order);
            scanned.u32(job.source_offset);
            scanned.u32(match job.kind {
                DiscoveryKind::ParseSource => 0,
                DiscoveryKind::ApplyCfg => 1,
                DiscoveryKind::RegisterHeader => 2,
                DiscoveryKind::ResolveImport => 3,
                DiscoveryKind::ResolveMacro => 4,
                DiscoveryKind::Expand => 5,
                DiscoveryKind::Integrate => 6,
                DiscoveryKind::ScanNestedDeclarations => 7,
            });
            scanned.digest(job.identity);
        }
        let seal = Seal::completed(
            self.domain.clone(),
            self.graph,
            self.epoch,
            e.finish(),
            scanned.finish(),
            self.dependencies.iter().map(|(a, b)| (*a, *b)).collect(),
        );
        self.seal = Some(seal.clone());
        Ok(DiscoveryStep::Sealed(seal))
    }
}

#[cfg(test)]
mod batch_tests {
    use super::*;
}

#[cfg(test)]
mod resume_tests {
    use super::*;
    #[test]
    fn discovery_input_resume_retries_same_producer_without_completion_or_fingerprint_change() {
        let mut work = Discovery::new(
            SealDomain::ImplCatalog {
                id: Digest([71; 32]),
            },
            Digest([72; 32]),
            1,
            DiscoveryLimits::default(),
        )
        .unwrap();
        let job = DiscoveryJob {
            module_order: 0,
            source_offset: 0,
            kind: DiscoveryKind::ScanNestedDeclarations,
            identity: Digest([73; 32]),
        };
        work.enqueue(job.clone()).unwrap();
        let DiscoveryStep::Run(first) = work.poll().unwrap() else {
            panic!("producer did not start")
        };
        let input = Digest([74; 32]);
        let value = Digest([75; 32]);
        work.resume_input(&first, input, value).unwrap();
        assert!(!work.completed_job(job.kind, job.identity));
        let DiscoveryStep::Run(second) = work.poll().unwrap() else {
            panic!("same producer did not resume")
        };
        assert_eq!(second.job(), &job);
        assert!(work.resume_input(&second, input, Digest([76; 32])).is_err());
        assert!(!work.completed_job(job.kind, job.identity));
        work.block(&second, vec![Digest([77; 32])]).unwrap();
        assert!(matches!(
            work.poll().unwrap(),
            DiscoveryStep::Incomplete { .. }
        ));
        assert!(work.seal.is_none());
    }
}
