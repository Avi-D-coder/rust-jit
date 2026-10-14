# H003 async facade reservations

The fixed catalog defines Future, IntoFuture, AsyncFn, AsyncFnMut and AsyncFnOnce
with canonical compiler-owned core identities. Both core and std qualified routes
and edition-specific prelude routes are installed into the existing ordinary
namespace index. Macro and ordinary facade namespaces share the existing stable
core/std unit encoding; the macro identities are unchanged.

Rust1.99's v1 prelude contains AsyncFn/AsyncFnMut/AsyncFnOnce in all supported
source editions; Future and IntoFuture join the 2024 prelude. The catalog preserves
that distinction. Sources: [core v1](https://doc.rust-lang.org/core/prelude/v1/index.html)
and [core rust2024](https://doc.rust-lang.org/core/prelude/rust_2024/index.html), both
reporting pinned 1.99.0/b940084d7 when checked.

Ordinary declarations and manual globs can shadow prelude names. Aliases preserve
the selected facade identity. Unknown producers remain NeedInput rather than
falling through to a facade. Type identities do not enter value or macro lookup.
Exact selection checks all definition provenance. A reached executable request
for a selected async identity returns LRR-E-FEATURE-ASYNC; ordinary user identities
remain ordinary. The catalog cannot register user definitions as facades.

The current acceptance exercises the installed ordinary namespace graph and the
facade execution guard. It does not claim an implemented general ResolveMember
candidate engine: M025 must retain reserved traits in candidate enumeration before
selecting a different glob-imported method, and M026 must use the execution guard.
Generated/dependency prelude selection requires M014/M016 and the owning token
editions. The remaining facade/prelude inventory is assigned to T060–T065.

Generator syntax is structurally opaque. The actual scalar source engine rejects
reached generator execution; a skipped generator body containing an invalid call
stays cold. compat/features.toml explicitly records generators as unsupported.
Full E2408 reference/aggregate pattern matching and off/auto/force evidence remains
pending. No async execution, full LRR E24 pass, release or Linux support is claimed.
