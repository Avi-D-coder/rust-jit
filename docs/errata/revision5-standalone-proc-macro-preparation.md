# E-R5-007 — Standalone procedural-macro auxiliaries (under E-R5-006)

October 17, 2026. Consequence of E-R5-006 ("We're not doing the sandbox right now. We're not doing IO permissions or anything like that right now"), applied to the procedural-macro sections of revision 5 (Section 48.6, E-M013-001). Not a change to E-R5-005: macro output still enters the one hygienic expansion pipeline.

## What changes

- **Preparation stage.** `lrr run|test` compiles each `--extern [noprelude,]proc-macro:NAME=aux.rs` auxiliary during its own preparation stage (`cli/proc_macros.rs`), as compiletest does: `rustc +1.99.0 --crate-type proc-macro --extern proc_macro` with `RUSTC_BOOTSTRAP=1`, into a content-keyed cache beside the binary (`LRR_PROC_MACRO_CACHE` overrides the location). Revision 5 places this in a separate authorized `prepare` step.
- **No execution grant.** No `--allow-macro-exec` (or equivalent permission broker grant) is required to execute a compiled procedural macro. The macro still executes only in the external stdio-v1 macro server, never in the interpreter process.
- **Server admission.** The stdio-v1 server (`LRR_PROC_MACRO_SERVER` overrides it) is accepted by its exact build banner. The full `PreparedMacroServer` provenance record and echo probe of Section 48.6 / E-M013-001 are not required for now.
- **Manifest.** The schema4 tool manifest built for a standalone run carries the planner marker `STANDALONE_PLANNER_SELECTION`; manifest validation accepts it.

## Unchanged

- Expansion output follows the ordinary hygienic route (MacroRun/MacroSession, discovery, seals); each macro invocation executes exactly once per run (Type and Value seals share one run).
- No arbitrary native symbol is loaded into the interpreter.

## Reverting

When sandboxing and permissions return to scope, restore the separate preparation step, the execution grant, and server provenance checking; this erratum then lapses.
