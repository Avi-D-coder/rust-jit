//! Route evidence for drop glue's `Drop::drop` witness: it is selected through
//! the member index and entered as an ordinary callee frame of the dropping
//! frame (sharing its step/action observation), consumes no recipe call-site
//! action, and resumes the suspended cleanup activation on return or unwind.
use super::*;

fn run_to_end(source: &str) -> (Database, RunMachine, RunPoll) {
    let (mut db, mut run, macros) = borrows::tests::start_discovered_body(source, "main", 2024);
    let contexts = macros.contexts().unwrap();
    let poll = run.poll_with_runtime_macros(&mut db, &contexts);
    drop(contexts);
    (db, run, poll)
}
fn completed(source: &str) -> (Database, RunMachine, RunReport) {
    let (db, run, poll) = run_to_end(source);
    match poll {
        RunPoll::Completed(report) => (db, run, report),
        RunPoll::Fault(error) => panic!("faulted: {error:?}\n{source}"),
        RunPoll::Panicked(report) => panic!("panicked: {:?}\n{source}", report.panic),
        RunPoll::NeedInput(_) => panic!("requested input\n{source}"),
    }
}
const MAIN: &str = "struct D(i32);
fn main() {
    let _a = D(1);
    {
        let _b = D(2);
    }
}";

#[test]
fn drop_witness_enters_an_ordinary_callee_frame_without_a_call_site_action() {
    // The same program without and with a `Drop` impl: drop glue alone, and
    // drop glue entering one `D::drop` frame per dropped value.
    let (mut plain_db, plain, plain_report) = completed(MAIN);
    let with_drop = format!(
        "impl Drop for D {{
    fn drop(&mut self) {{
        let _seen = self.0;
    }}
}}
{MAIN}"
    );
    let (mut db, run, report) = completed(&with_drop);
    assert!(matches!(report.value, ValueSummary::Unit));
    assert_eq!(plain.peak_guest_frames(), 1);
    // main plus exactly one entered `D::drop` frame at a time.
    assert_eq!(run.peak_guest_frames(), 2);
    // Witness entry is not a recipe call exit: it publishes/consumes no
    // action beyond the program's own call sites (here its constructors).
    let (plain_actions, actions) = (plain.action_snapshot(), run.action_snapshot());
    assert_eq!(
        (actions.published, actions.consumed),
        (plain_actions.published, plain_actions.consumed)
    );
    // Each witness frame cleans its own function scope on return.
    assert_eq!(
        report.opcode_counters.return_scopes_cleaned,
        plain_report.opcode_counters.return_scopes_cleaned + 2
    );
    assert!(run.calls.is_empty() && run.returning.is_none() && run.unwinding.is_none());
    assert!(!run.frame.has_cleanup_activation());
    assert_eq!(run.call_bytes, 0);
    assert_eq!(
        report.opcode_counters.operations_started,
        report.opcode_counters.operations_completed
    );
    plain_db.finish_run().unwrap();
    db.finish_run().unwrap();
}

#[test]
fn panicking_drop_witness_unwinds_into_the_dropping_frames_remaining_owners() {
    let source = "struct Quiet;
impl Drop for Quiet {
    fn drop(&mut self) {}
}
struct Loud(u8);
impl Drop for Loud {
    fn drop(&mut self) {
        let _x = 255u8 + self.0;
    }
}
fn main() {
    let _q = Quiet;
    let _l = Loud(1);
}";
    let (mut db, run, poll) = run_to_end(source);
    let report = match poll {
        RunPoll::Panicked(report) => report,
        RunPoll::Fault(error) => panic!("panicking drop witness faulted: {error:?}"),
        _ => panic!("panicking drop witness did not publish its guest panic"),
    };
    // The witness body's own arithmetic overflow is the published panic.
    assert!(matches!(
        report.panic.kind,
        crate::vm::numeric::PanicKind::Overflow
    ));
    // `_l`'s witness panicked; `_q`'s witness ran (and returned) while
    // main's remaining owners were cleaned by unwinding.
    assert_eq!(run.peak_guest_frames(), 2);
    assert!(report.opcode_counters.panic_scopes_cleaned >= 2);
    assert!(report.opcode_counters.return_scopes_cleaned >= 1);
    assert!(run.calls.is_empty() && run.unwinding.is_none() && run.terminal);
    db.finish_run().unwrap();
}
