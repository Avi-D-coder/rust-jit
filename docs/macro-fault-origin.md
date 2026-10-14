# Macro failure origins

M009 retains the first started expansion failure in each MacroSession. MacroFaultReport identifies the macro_expand phase, frozen demand context and suspended phase, consuming unit/epoch, tool artifact, invocation, call-site span and origin, parent origin, and scope. The original typed diagnostic is retained. It does not classify an external macro panic as a guest panic.

Protocol bytes and stderr are separate bounded previews, with explicit truncation flags. A failed preview/status observation has an explicit capture error; helper_reaped is unknown when observing it failed. Only one primary report is retained for the unit/epoch, so replay or further refusal cannot grow an unbounded report collection. The helper's existing 64KiB preview and diagnostic bounds apply.

The real macOS service fixture prepares only the synthetic proc-macro dependency of a live consumer with an invalid cold body. It publishes the checked schema4 manifest, opens separately authorized matched helpers, and exercises panic, abort, hang, oversized output, explicit panic diagnostics and noisy stderr. Each outcome rejects a SavedPhase with its state retained, installs no expansion query memo, terminates/reaps the helper, and replays the same report with one native attempt. Report JSON files retain the actual observations beside preparation evidence.

This proves the macro service and saved-phase rejection boundary. It does not prove the unfinished guest VM's should_panic handling, full CLI/service phase telemetry, complete displayed origin-chain resolution, or generated test execution. Those remain required later integration. M009 and the project remain in progress; Linux validation remains deferred.
