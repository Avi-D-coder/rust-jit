# Cold initializer macro contracts

V78 exposed a test contradiction with the current specification: an unresolved expression macro can introduce implementations and therefore block declaration discovery even in a cold initializer. Sections 8.5 and 50 require the unknown frontier to remain incomplete; ordinary cold type/runtime errors remain unevaluated.

The header-only control now uses the genuinely prepared panic intrinsic alongside division by zero and the unrepresentable integer. It still requires source header selection with no Constant, BodyStructure or Thunk demand. A separate negative control retains the original unresolved macro and requires CATALOG-INCOMPLETE, one blocked producer and macro origin, without publishing a seal or evaluating any body. No production behavior, upstream original or runtime acceptance changes.

Apply exact hashes and validate both controls, then resume the held affected scoped queue. This source-only correction does not turn failed discovery into a runtime pass.
