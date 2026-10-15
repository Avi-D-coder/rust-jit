# Qualified range type namespace preparation

T024/T041 harness repair for the concrete V82 failure (`d41396/101`). The unchanged `issue-21475.rs` main requested its real Type discovery seal after the harness prepared only Value namespaces. This candidate prepares both namespaces in every actual owning module through `SourceDiscovery`, retaining the original root Value producer for runtime macros. Each published seal must survive startup; preparation must query no original body, thunk or constant initializer.

The original source, runtime result, endpoint queries, comparison counts and reached-phase assertions are unchanged. This is a one-leaf source-only repair, not a stock pass or interpreter acceptance claim. Validate the exact original case before resuming byte assertions. Do not rerun the already-green 400,000-byte byte-string main without a relevant change.
