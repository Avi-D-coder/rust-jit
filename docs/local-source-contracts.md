# T032 scalar local source contracts — work in progress

A reached local declared as a generic parameter now checks the initializer's
rigid source parameter identity before transfer. Assignment performs the same
check against the local's retained source view after the ordinary storage
admission checks and before moving the RHS or replacing the old value. Distinct
parameters reject with LRR-E-GENERIC-BINDING even if both have the same concrete
runtime descriptor. This preserves RHS-first evaluation and existing concrete
storage, mutability, and literal-width rules.

An inferred local preserves its initializer's source view. An uninitialized
inferred local adopts the source view of its first initialization and retains
that contract across replacements. Written primitive and generic annotations
establish the written source view for subsequent reads, rather than accidentally
retaining a generic initializer's view under a primitive annotation. A reached
primitive annotation still checks the actual runtime contract. It does not infer
a caller's argument type from a later callee local declaration.

Source controls cover initialized and deferred locals, inferred replacement,
written primitive views, ordinary replacement, shadowing, and generic literal
initialization. The unit regression checks that rejected initialization and
replacement leave values, source views, target allocation and fuel unchanged.
The pre-fix failure is retained as evidence.

This is the scalar primitive/rigid-parameter component. General structural
coercions, composite source views, projections, full SourceViewId/ParamEnvId tables,
trait dictionaries and generated-source integration remain pending. T032 and
release acceptance remain in progress; Linux acceptance remains pending.
