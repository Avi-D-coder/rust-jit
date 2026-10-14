//! Each selected original-source test owns a fresh RunMachine and GuestStore.
use super::{
    run::{RunMachine, RunPoll, ValueSummary},
    test_catalog::TestCase,
};
use crate::{
    core::diagnostic::Diagnostic,
    db::{memo::Database, poll::QueryPoll},
    source::changes::PreparedSourceView,
};
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TestStatus {
    Passed,
    Failed,
    Ignored,
}
#[derive(Debug, serde::Serialize)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub diagnostic: Option<Diagnostic>,
}
impl TestResult {
    pub fn passed(&self) -> bool {
        matches!(self.status, TestStatus::Passed | TestStatus::Ignored)
    }
}
fn error(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
pub fn run_original(
    db: &mut Database,
    sources: &[PreparedSourceView],
    case: &TestCase,
    include_ignored: bool,
    fuel: u64,
) -> Result<TestResult, Diagnostic> {
    run_original_with_runtime_macros(db, sources, case, include_ignored, fuel, &[])
}
pub fn run_original_with_runtime_macros(
    db: &mut Database,
    sources: &[PreparedSourceView],
    case: &TestCase,
    include_ignored: bool,
    fuel: u64,
    contexts: &[super::runtime_macros::RuntimeMacroContext<'_>],
) -> Result<TestResult, Diagnostic> {
    run_test(db, sources, case, include_ignored, fuel, contexts, None)
}
pub(crate) fn run_sealed_test(
    db: &mut Database,
    sources: &[PreparedSourceView],
    case: &TestCase,
    fuel: u64,
    contexts: &[super::runtime_macros::RuntimeMacroContext<'_>],
    catalog: &super::test_catalog::TestCatalog,
) -> Result<TestResult, Diagnostic> {
    catalog.require_current(db)?;
    run_test(db, sources, case, true, fuel, contexts, Some(catalog))
}
fn run_test(
    db: &mut Database,
    sources: &[PreparedSourceView],
    case: &TestCase,
    include_ignored: bool,
    fuel: u64,
    contexts: &[super::runtime_macros::RuntimeMacroContext<'_>],
    catalog: Option<&super::test_catalog::TestCatalog>,
) -> Result<TestResult, Diagnostic> {
    if case.ignored && !include_ignored {
        return Ok(TestResult {
            name: case.name.clone(),
            status: TestStatus::Ignored,
            diagnostic: None,
        });
    }
    if let Some(diagnostic) = &case.attribute_error {
        return Ok(TestResult {
            name: case.name.clone(),
            status: TestStatus::Failed,
            diagnostic: Some(diagnostic.clone()),
        });
    }
    if case.unsupported_signature {
        return Ok(TestResult {
            name: case.name.clone(),
            status: TestStatus::Failed,
            diagnostic: Some(error(
                "LRR-E-UNSUPPORTED",
                "selected test requires async or owner-context integration",
            )),
        });
    }
    let Some(body) = case.body.clone() else {
        return Ok(TestResult {
            name: case.name.clone(),
            status: TestStatus::Failed,
            diagnostic: Some(error(
                "LRR-E-TEST-SIGNATURE",
                "selected test has no executable body",
            )),
        });
    };
    db.begin_run()?;
    let start = if let Some(catalog) = catalog {
        RunMachine::start_with_test_catalog(
            db,
            body,
            fuel,
            sources.iter().map(PreparedSourceView::share).collect(),
            catalog,
        )
    } else {
        RunMachine::start_with_sources(
            db,
            body,
            fuel,
            sources.iter().map(PreparedSourceView::share).collect(),
        )
    };
    let outcome = match start {
        QueryPoll::Ready(Ok(mut run)) => match run.poll_with_runtime_macros(db, contexts) {
            RunPoll::Completed(report) => {
                if matches!(report.value, ValueSummary::Unit) && !case.should_panic {
                    None
                } else {
                    Some(error(
                        "LRR-E-TEST-OUTCOME",
                        "selected test returned a non-unit value or did not panic as expected",
                    ))
                }
            }
            RunPoll::Panicked(report) => {
                if case.should_panic
                    && case
                        .panic_contains
                        .as_ref()
                        .is_none_or(|expected| report.panic.description().contains(expected))
                {
                    None
                } else {
                    Some(error("LRR-E-TEST-PANIC", "unexpected guest panic"))
                }
            }
            RunPoll::Fault(diagnostic) => Some(diagnostic),
            RunPoll::NeedInput(_) => Some(error(
                "LRR-E-CATALOG-INCOMPLETE",
                "selected test requires missing input/expansion integration",
            )),
        },
        QueryPoll::Ready(Err(diagnostic)) => Some(diagnostic),
        QueryPoll::NeedInput(_) => Some(error(
            "LRR-E-CATALOG-INCOMPLETE",
            "selected test body requires missing input",
        )),
    };
    db.finish_run()?;
    Ok(TestResult {
        name: case.name.clone(),
        status: if outcome.is_none() {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        },
        diagnostic: outcome,
    })
}
