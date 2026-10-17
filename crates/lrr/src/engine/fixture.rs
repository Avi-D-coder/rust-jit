//! Section 28 fixture contracts. Parsing and checking never execute a guest,
//! compiler, query or broker; absent observations are failures, including zero minima.
use crate::core::diagnostic::Diagnostic;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
fn fault(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("LRR-E-FIXTURE", message.into())
}
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureMode {
    Off,
    Auto,
    Force,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureManifest {
    pub schema_version: u32,
    pub id: String,
    pub category: String,
    pub source: String,
    pub entry: String,
    pub modes: Vec<FixtureMode>,
    pub oracle: bool,
    pub expect: FixtureExpectation,
    #[serde(default)]
    pub laziness: FixtureLaziness,
    #[serde(default)]
    pub force: FixtureForce,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureExpectation {
    pub status: String,
    pub stdout: String,
    pub stderr: String,
    pub diagnostic_code: Option<String>,
    pub phase: Option<String>,
    pub cleanup_ran: Option<bool>,
    pub panic_contains: Option<String>,
    pub expected_drop_log: Option<Vec<String>>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureLaziness {
    #[serde(default)]
    pub no_body_preparation: Vec<String>,
    #[serde(default)]
    pub no_marker_preparation: Vec<String>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureForce {
    #[serde(default)]
    pub minimum_native_entries: u64,
    #[serde(default)]
    pub minimum_native_scalar_ops: u64,
}
/// E-R5-006: fixture text has no size ceiling; only NUL and required
/// non-empty fields are refused.
fn checked_text(value: &str, nonempty: bool) -> Result<(), Diagnostic> {
    if value.contains('\0') || (nonempty && value.is_empty()) {
        return Err(fault("fixture text is empty or contains NUL"));
    }
    Ok(())
}
fn unique_names(names: &[String]) -> Result<(), Diagnostic> {
    let mut seen = BTreeSet::new();
    for name in names {
        checked_text(name, true)?;
        if !seen.insert(name) {
            return Err(fault("duplicate fixture observation name"));
        }
    }
    Ok(())
}
impl FixtureManifest {
    pub fn parse(text: &str) -> Result<Self, Diagnostic> {
        let manifest: Self = toml::from_str(text)
            .map_err(|error| fault(format!("invalid fixture manifest: {error}")))?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub(crate) fn validate(&self) -> Result<(), Diagnostic> {
        if self.schema_version != 1 {
            return Err(fault("unsupported fixture schema"));
        }
        checked_text(&self.id, true)?;
        checked_text(&self.category, true)?;
        if !self
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(fault("invalid stable fixture ID"));
        }
        checked_text(&self.source, true)?;
        if self.source.starts_with('/')
            || self.source.contains('\\')
            || self
                .source
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
        {
            return Err(fault("fixture source must stay inside its directory"));
        }
        checked_text(&self.entry, true)?;
        for part in self.entry.split("::") {
            crate::expand::scopes::MacroName::parse(part, 2024)
                .map_err(|_| fault("invalid fixture entry path"))?;
        }
        if self.oracle && matches!(self.category.as_str(), "relaxed" | "profile-fault") {
            return Err(fault(
                "relaxed/profile-fault fixtures cannot require a native-pass oracle",
            ));
        }
        if self.oracle && self.entry != "case" {
            return Err(fault("native oracle fixture requires root entry case"));
        }
        if self.modes.is_empty()
            || self.modes.len() > 3
            || self.modes.iter().collect::<BTreeSet<_>>().len() != self.modes.len()
        {
            return Err(fault("fixture modes must be nonempty and unique"));
        }
        checked_text(&self.expect.status, true)?;
        checked_text(&self.expect.stdout, false)?;
        checked_text(&self.expect.stderr, false)?;
        for value in [&self.expect.diagnostic_code, &self.expect.phase]
            .into_iter()
            .flatten()
        {
            checked_text(value, true)?;
        }
        if let Some(value) = &self.expect.panic_contains {
            checked_text(value, false)?;
        }
        if self.expect.panic_contains.is_some() && self.expect.diagnostic_code.is_some() {
            return Err(fault(
                "fixture cannot conflate guest panic and runtime fault",
            ));
        }
        if let Some(log) = &self.expect.expected_drop_log {
            for event in log {
                checked_text(event, false)?;
            }
        }
        unique_names(&self.laziness.no_body_preparation)?;
        unique_names(&self.laziness.no_marker_preparation)?;
        Ok(())
    }
    /// Checks explicit observations for one requested mode. This is not execution.
    pub fn check(
        &self,
        mode: FixtureMode,
        observed: &FixtureObservation,
    ) -> Result<(), Diagnostic> {
        self.validate()?;
        if !self.modes.contains(&mode) {
            return Err(fault("requested mode is outside fixture contract"));
        }
        if observed.mode != Some(mode) {
            return Err(fault("missing or mismatched executed mode"));
        }
        match observed
            .outcome
            .ok_or_else(|| fault("missing typed fixture outcome"))?
        {
            FixtureOutcome::Completed
                if observed.diagnostic_code.is_some() || observed.panic_message.is_some() =>
            {
                return Err(fault(
                    "completed fixture has a contradictory panic/fault record",
                ));
            }
            FixtureOutcome::GuestPanic
                if observed.panic_message.is_none() || observed.diagnostic_code.is_some() =>
            {
                return Err(fault(
                    "guest panic requires its message and cannot carry a runtime fault",
                ));
            }
            FixtureOutcome::Fault
                if observed.diagnostic_code.is_none()
                    || observed.panic_message.is_some()
                    || matches!(self.expect.status.as_str(), "passed" | "panicked")
                    || self.expect.panic_contains.is_some() =>
            {
                return Err(fault(
                    "runtime fault cannot satisfy a guest panic or passed expectation",
                ));
            }
            _ => {}
        }
        equal(observed.status.as_ref(), &self.expect.status, "status")?;
        equal(observed.stdout.as_ref(), &self.expect.stdout, "stdout")?;
        equal(observed.stderr.as_ref(), &self.expect.stderr, "stderr")?;
        if let Some(expected) = &self.expect.diagnostic_code {
            equal(
                observed.diagnostic_code.as_ref(),
                expected,
                "diagnostic_code",
            )?;
        }
        if let Some(expected) = &self.expect.phase {
            equal(observed.phase.as_ref(), expected, "phase")?;
        }
        if let Some(expected) = &self.expect.cleanup_ran {
            equal(observed.cleanup_ran.as_ref(), expected, "cleanup_ran")?;
        }
        if let Some(expected) = &self.expect.panic_contains {
            let actual = observed
                .panic_message
                .as_ref()
                .ok_or_else(|| fault("missing panic observation"))?;
            if observed.diagnostic_code.is_some() || !actual.contains(expected) {
                return Err(fault(
                    "guest panic does not match; faults cannot satisfy panic",
                ));
            }
        }
        if let Some(expected) = &self.expect.expected_drop_log {
            equal(observed.drop_log.as_ref(), expected, "drop_log")?;
        }
        for name in &self.laziness.no_body_preparation {
            equal(observed.body_preparations.get(name), &0, "body preparation")?;
        }
        for name in &self.laziness.no_marker_preparation {
            let actual = observed
                .markers
                .get(name)
                .ok_or_else(|| fault(format!("missing marker observation: {name}")))?;
            equal(
                actual.thunk_executions.as_ref(),
                &0,
                "marker thunk execution",
            )?;
            equal(
                actual.installed_operations.as_ref(),
                &0,
                "marker installed operation",
            )?;
            equal(
                actual.native_operations.as_ref(),
                &0,
                "marker native operation",
            )?;
        }
        if mode == FixtureMode::Force {
            minimum(
                observed.native_entries,
                self.force.minimum_native_entries,
                "native entries",
            )?;
            minimum(
                observed.native_scalar_ops,
                self.force.minimum_native_scalar_ops,
                "native scalar operations",
            )?;
        }
        Ok(())
    }
}
fn equal<T: PartialEq>(actual: Option<&T>, expected: &T, name: &str) -> Result<(), Diagnostic> {
    match actual {
        Some(actual) if actual == expected => Ok(()),
        Some(_) => Err(fault(format!("mismatched fixture observation: {name}"))),
        None => Err(fault(format!("missing fixture observation: {name}"))),
    }
}
fn minimum(actual: Option<u64>, expected: u64, name: &str) -> Result<(), Diagnostic> {
    match actual {
        Some(actual) if actual >= expected => {}
        Some(_) => return Err(fault(format!("fixture minimum not reached: {name}"))),
        None => return Err(fault(format!("missing fixture observation: {name}"))),
    }
    Ok(())
}
/// None means unobserved. Collectors must not substitute inferred zero counts.
#[derive(Debug, Default)]
pub struct FixtureObservation {
    pub outcome: Option<FixtureOutcome>,
    pub mode: Option<FixtureMode>,
    pub status: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub diagnostic_code: Option<String>,
    pub phase: Option<String>,
    pub cleanup_ran: Option<bool>,
    pub panic_message: Option<String>,
    pub drop_log: Option<Vec<String>>,
    pub body_preparations: BTreeMap<String, u64>,
    pub markers: BTreeMap<String, MarkerObservation>,
    pub native_entries: Option<u64>,
    pub native_scalar_ops: Option<u64>,
}
#[derive(Debug, Default)]
pub struct MarkerObservation {
    pub thunk_executions: Option<u64>,
    pub installed_operations: Option<u64>,
    pub native_operations: Option<u64>,
}

/// Deterministic IDs across a complete supplied fixture batch. This owns contracts,
/// not passed results, captured source or execution authority.
pub struct FixtureCatalog {
    manifests: BTreeMap<String, FixtureManifest>,
}
impl FixtureCatalog {
    pub fn new(manifests: Vec<FixtureManifest>) -> Result<Self, Diagnostic> {
        let mut index = BTreeMap::new();
        for manifest in manifests {
            manifest.validate()?;
            if index.contains_key(&manifest.id) {
                return Err(fault(format!("duplicate fixture ID: {}", manifest.id)));
            }
            index.insert(manifest.id.clone(), manifest);
        }
        Ok(Self { manifests: index })
    }
    pub fn manifests(&self) -> impl Iterator<Item = &FixtureManifest> {
        self.manifests.values()
    }
    pub fn get(&self, id: &str) -> Option<&FixtureManifest> {
        self.manifests.get(id)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FixtureOutcome {
    Completed,
    GuestPanic,
    Fault,
}
