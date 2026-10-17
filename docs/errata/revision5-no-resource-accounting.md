# E-R5-006 — Simple interpreter: rustc limits only, no sandbox

October 16, 2026. Explicit user direction, superseding the conflicting parts of revision 5:

> "The only limits that this should have should come from Rust C limits. If you can't justify it there by finding it in the compiler, it shouldn't be here. We're not doing the sandbox right now. We're not doing IO permissions or anything like that right now. We're just implementing a super simple interpreter and testing it against the upstream Rust C tests. After that, we're implementing the JIT."
>
> "We're not counting memory … We don't need to count time. We don't need to count memory. That's what the OS is for, not what our sandbox does."

## What changes

- **Limits.** A limit may exist only if it reproduces a limit rustc itself enforces, cited by its source location in the pinned compiler (`rustc/rust`). Examples: `recursion_limit` (default 128, overridden by `#![recursion_limit]`; `compiler/rustc_session` / `rustc_expand`), `type_length_limit`, `pattern_complexity_limit`. Everything else is removed rather than raised:
  - guest fuel and wall-time counting, accounted guest memory, handle/frame/cleanup/output caps (Section 26.2 runtime rows);
  - compile-side byte and count caps: per-file and snapshot byte limits, called-body node limits, query recursion caps, Section 50.6's per-request expansion caps, and the scattered implementation caps (`100_000` counts, `N * 1024 * 1024` byte budgets, `256` counts, retained-admission accounting).
  
  Checked arithmetic that prevents an overflow panic remains acceptable; it must not act as a ceiling. The OS bounds memory and time.
- **Sandbox, permissions and capabilities** (permission broker, IO grants, capability authority on source inputs, runtime filesystem/env authority) are out of scope for now. Do not extend them; do not let them block language work. Source reads follow rustc (`#[path]`, `include!` read the paths rustc would read).
- **Goal and order.** A simple, correct interpreter measured against the unchanged upstream rustc runtime tests; the JIT follows interpreter acceptance. E-R5-005 (one compositional macro/language pipeline) still applies.

## How to apply

When an existing limit or authority check blocks a correct program, delete it (with any test that only asserted the limit) instead of raising the number or adding smarter accounting. Do not add new caps. Keep the failure modes in AGENTS.md about shared semantics; they are unaffected.

Existing limit code is not yet removed crate-wide; it is repair debt removed as encountered or by a dedicated sweep.
