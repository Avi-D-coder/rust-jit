# Body locator dependency identity correction

LOCAL-024, T014/T024/T032/M014; October15. Actual V5 frozen bank32e1c6/1
reconciles6pass1fail. The unchanged regions-addr-of-ret.rs header control
rejects changed source/header/context but changed DefinitionBody.range.start
reuses a cached ReferenceHeaderPlan at the old range39..58. This is a real
currentness defect, not a passing negative test or an expected fallback.

InputValue::DefinitionBody fingerprint currently delegates solely to BodyHandle
semantic identity. BodyHandle intentionally encodes body key and entry refusal
without source offsets, so a changed locator produces the same dependency
stamp. Source-dependent FunctionSignature and ReferenceHeader consumers observe
actual handle ranges; their input stamp must also observe them. Fix that shared
input boundary, keeping stable semantic BodyKey/BodyHandle identities intact.

Use an explicit new domain for the DefinitionBody input variant and encode both
start/end little-endian u32 values after its existing handle record. Preserve all
other input variant fingerprints; no new provider effect or alternate resolver.
Extend the existing external regression to cover each locator endpoint and
require exact CALL-SIGNATURE refusal rather than accepting arbitrary errors.
No stock original is changed. Source/header/body dependency replay remains the
existing pure memo framework; immutable source planning remains demanded once.

All8 review: no new evaluator, context-specific inference or generated-source
rule; no pattern/runtime semantics or cold executable preparation; no repeated
plan construction, global namespace census or immutable-copy path. The one-time
input digest gains two fixed fields, while installed recipes use existing
dependency validation. The unchanged source71-plus-correction must be frozen
and validated by the sole asynchronous validator. Current actual failures and
old semantic identities are preserved. No task/architecture/runtime/fullfixture
or JIT acceptance from this source inspection.
