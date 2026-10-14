# M013 SDK macro-use and compiler-global visibility

M013 remains in progress. Explicit standalone preparation supplies one supported
SDK export inventory for core/std: six compiler-input definitions and four runtime
macro definitions. Export aliases preserve the actual core definition owner.
Selective macro-use validation sees both classes; unused selectors neither capture
files/environment variables nor execute runtime handlers. Dispatch remains separate
and selects exact immutable definitions, never just an invocation spelling.

Compiler-global visibility is a separate bounded, deterministic per-consumer-unit
index. Only configured standard facets supply the six compiler-input globals and
panic/assert runtime globals. no_implicit_prelude continues to suppress macro-use
imports, automatic dependency paths and ordinary prelude macros such as assert_eq.
Source textual/path bindings and unresolved producers retain precedence. A global
include still requires explicit coordinator capture, checked output adoption and
sealed discovery before an entry can execute.

Thirteen new controls cover mixed/all/selective/anonymous SDK macro-use, unused
imports, missing-export refusal, disabled imports, global include/concat/assert,
source shadowing and global panic execution. Eight CLI values and three selected
should_panic cases compare with isolated native macOS ARM64 Rust1.99.0 copies;
two native refusal witnesses distinguish unknown exports and disabled assert_eq.
The earlier mistaken assumption that no_implicit_prelude hides include/panic/assert
is retained as failing logs and native witnesses. Dedicated native probes also
check all six compiler-input spellings, panic/assert and assert_eq/assert_ne.

This is the supported inventory, not complete standard-library macro coverage.
Nested user/procedural argument macros, compile-expression runtime lowering,
Cargo/build-output/environment and per-unit preparation, generated/nested includes,
multiple source roots, world/native/procedural facets, full origin/invalidation/
quota/runtime/type integration, M014–M016, full MX027/MX028/JIT and release acceptance
remain pending. Linux remains a later port. No task or release completion is claimed.
