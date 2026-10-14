# M010 original item macro inputs

`service::item_macro_inputs` prepares owned inputs after source-aware macro
selection. It does not resolve names, execute tools, or publish query results.
Attribute preparation materializes active outer attributes, removes the selected
attribute, preserves the item/body and later attributes, and supplies the original
argument contents with only the outer argument delimiter removed. Derive preparation
filters fields/variants using the admitted cfg facts, removes derive annotations,
and extracts ordered unresolved paths through the pinned token bridge. Absolute
paths, raw source spelling and original first/last span keys are retained.

Input identities use explicit stable encoding of owner, parse/profile/cfg/item,
selected attribute/index/kind and the exact encoded item/argument forests. Changed
owner, contexts, spans or token input cannot reuse the prepared-input identity.
These identities are source input facts; the macro service still validates its
complete invocation identity, consumer environment, epoch and matched server.
`MacroSession::request_key` now constructs the procedural demand from the exact
loaded artifact/export/form and frozen consumer context. It validates all three
registered site spans, item grammar, and the definition edition against the
prepared producer. `fulfil` uses this same construction before coordinator start.
The procedural input fingerprint includes the selected definition/call/mixed span
keys, scope and parent origin. Equal token tables with different selected sites
cannot share a demand. Engine generation 5 invalidates generation 4 records.
Key inspection executes no macro. Frozen export identities survive helper failure
so a cached fault is returned without another invocation. Acceptance checks absent exports, stale epochs,
unregistered sites, wrong forms, and 2015/2018/2021 producers with 2024 call sites.

Builtin derive identity must be established by the resolver before this API is
called; source spelling alone grants no intrinsic or execution authority.

The unit acceptance exercises attribute arguments, owner binding, deterministic
repetition, qualified/absolute/raw derive path order, original span keys, wrong
annotation/index and inactive item refusal. Actual matched-server attribute input
agrees with native preprocessing, and all six native member-cfg cases now use the
assembler before their real derive and item-grammar invocations. No printed macro
output is reparsed.

Prepared procedural requests and exact worklist selection admission are connected
by [item macro requests](item-macro-requests.md). Automatic source resolution and dispatch, generated-source
reanalysis, full discovery/guest execution and MX018–MX020 remain pending. The
input assembler is component integration and does not establish release or Linux
acceptance.
