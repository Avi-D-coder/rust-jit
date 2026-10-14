# M011 frozen compile environment

`FrozenCompileEnvironment` owns an immutable exact-unit environment derived from
captured `CompileEnv` facts. Build-script rustc-env overrides Cargo settings, which
override declared values/absence. Names omitted from every captured map remain
`NeedCapture`; they cannot silently become absent. Required string operands diagnose
explicit absence, while optional operands return None. Present empty strings remain
present. No ambient worker environment or filesystem is consulted.

Observation identities encode unit, complete captured environment fingerprint,
name and the distinct value/absence/uncaptured state. A new value or a unit change
invalidates the observation; an existing frozen view retains its earlier facts.
Counts, string sizes and retained map metadata are checked before copying. OUT_DIR
must agree with the prepared Cargo setting and cannot come from rustc-env.

Three acceptance checks cover precedence, immutable snapshots, absence versus
capture, exact-unit separation, malformed names, reserved OUT_DIR and resource
bounds. A native Rust 1.99.0 macOS oracle compares required/optional values across
three units, including absence and empty values. A missing-required native control
must produce the exact compiler diagnostic.

This is compile-input operand infrastructure. Callers must first resolve a genuine
builtin identity. Token operand parsing, builtin dispatch, pure-query/coordinator
capture demands, concat/include path evaluation, source authority/capture and full
MX021–MX024 discovery/guest assertions remain pending. M011 and release acceptance
remain incomplete; Linux validation is deferred.

Prepared frozen environments now publish as exact-unit ordinary database inputs
(UnitCompileEnv). Input matching refuses a foreign unit, stable value encoding
uses explicit tag 22 and checked owned map capacities count toward metadata.
Debug formatting reports identity/count rather than values. The pure provider
reader returns the immutable environment or a typed CaptureCompileEnvironment
request (stable request tag 3), never ambient lookup or cached absence. Demand
admission binds that request to the current consumer unit. Actual preparation
publication remains outside queries and replaces the input universe; this request
does not authorize build tools or turn preparation into a provider side effect.

The eager worklist can enter through that provider reader, preserving source
revision and expansion environment-key checks. Regression evidence observes
value-to-explicit-absence memo invalidation, same-snapshot reuse, distinct-unit
values, removal suspension, unchanged held snapshots and foreign publication/
admission refusal. A real nested worklist suspends on a missing environment,
drives prepared input once available, and refuses its old expansion key after
the prepared environment changes. Exact standard Option facade output depends
on later catalogue/facade integration; NeedOptionFacade remains explicit. Full
MX023/guest/service acceptance and M011 completion remain pending.
