# T040 structural constructor dispatcher draft

This component is now integrated after segmented scoped validation:32 selected cases passed on v3 and the added stale caller metadata control passed separately on production-identical v4. Both outer drivers and independent collectors exited0; this is not a single combined validation run. MAIN retains104 official rustc cases/191 unchanged files and55 T040 registrations. The fresh integrated macOS milestone is pending; the last completed parent milestone passed2160 project tests/45 controls. Full T040, stock fixtures, guest Drop/JIT, release and Linux remain incomplete. [Integration record](../artifacts/macos-first/t040-constructor-continuation-components-integration.json). The draft description below records its prior scoped stage.

This private successor is unvalidated and not integrated. It is based on the scoped accepted initializer-source v4 snapshot, independently of the initializer-return draft. Full stock rustc fixture passes remain zero. Source type inference, composite symbolic views, ordinary aggregate bindings/projections, constant evaluation, resumable Copy/Drop, guest JIT and release acceptance remain pending.

Task invariant: a reached tuple or written-element array uses its exact source/body/cfg constructor plan and an already-bound worker descriptor before opcode fuel or guest guard allocation. A source spelling, runtime field layout or missing context never supplies an invented element type. Empty tuples have intrinsic unit identity; empty arrays still require element context. Record update and repeat retain their existing refusal paths.

The dispatcher shares its source-owner check across nominal and structural constructors. Structural preparation additionally replays the plan against the immutable source lease, verifies the namespace/source pairing and current source/body/header/declaration/cfg inputs, and then passes the checked result hint to the existing frame preparation method. A retained rigid/composite source requirement remains refused; primitive source identities must match the worker descriptor. Worker TypeIds remain outside pure LRIR/query results. This does not resolve tuple/array annotations or infer a context from field values.

The source lease helper proves only exact constructor syntax, body and captured cfg. It neither issues a namespace seal nor authorizes macro execution. Structural preparation does not perform named type lookup; generated source remains subject to its existing immutable publication and token-edition plan identity.

Seven unchanged-stock controls are staged outside the licensed original sources:

- tuple/one-tuple.rs: the original ('d',) construction receives an explicit checked worker tuple-of-char context, enters its actual thunks and transfers the original char into one aggregate field. The earlier match and later tuple destructuring/assertions are unexecuted.
- array-slice-vec/vec-fixed-length.rs: the original [1,2,3,4] construction receives an explicit worker array-of-isize context and enters all written fields in order. Source annotation binding, indexing, size_of, the huge-array declaration and the original main oracle are unexecuted.
- cast/issue-10991.rs: the original empty tuple constructs unit without an invented element context. The original invalid unit-to-usize cast and expected compiler stderr are unexecuted.
- consts/bool-comparison-in-const-array-len.rs: the original empty array refuses absent element context before fuel/guard effects, then commits a zero-length array under explicit bound worker unit-element context. It remains an array object rather than unit; no field can be projected. Source alias comparison/cast/const-length evaluation and the complete original main remain unexecuted.
- tuple/index-float.rs: the original (((),),) constructor receives an explicit worker nested-tuple context. Each reached child constructor prepares its propagated descriptor before guard/fuel effects; three guards finish with eight operations and retain distinct outer/inner tuple identities and unit. Harness drains inspect the stored nested unit. The original commented/whitespace tuple projections, unit patterns and full check-pass oracle remain unexecuted.
- one-tuple source refusal: absent context, a wrong descriptor shape, a forged rigid result source requirement and duplicated/missing source owners fail before opcode fuel, guest handles or result-cell allocation. The refused rigid term is preserved rather than erased by construction.

A seventh independent control retains the original one-tuple body and worker context while diverging its caller header, declaration scope or range between database runs. It checks refusal before fuel, handles, result cells or PC changes, then restores the exact immutable inputs and checks that constructor preparation is admitted again. This regression is unexecuted; the frozen nine-control validation candidate does not contain it.

The harness explicitly enters the captured scope chain and selects a constructor prefix. It checks that dispatcher preparation does not allocate guest handles or consume opcode fuel, then fetches only reached original thunks. Scalar field drains are harness cleanup, not ordinary guest Drop. These are component controls, not full stock fixture passes.

Effect classification:

| Path | Effects |
| --- | --- |
| PreparedSourceView::require_constructor_plan | Pure read/replay of pinned AST and captured cfg in the existing immutable lease; checked constructor metadata allocation, no new filesystem read, query, guest allocation or authority |
| RunMachine::prepare_constructor structural branch | ConstructorPlan query and current immutable source inputs; checked source/namespace pairing and retained frame hint; installs dispatcher-owned worker facts, without opcode fuel or guest construction allocation |
| Existing Frame::prepare_structural_constructor | Checked worker descriptor/shape and child hint reservation; no guest guard allocation; unchanged |
| Existing constructor thunks/opcodes | Pure source recipes select left-to-right continuations; actual guard allocation, transfers and commit occur only in VM opcodes; unchanged |

All four newly copied stock cases retain original directives, expected stderr where supplied, the pinned upstream commit and hashes, and both license texts. Inventory is104 cases/191 retained files. No staged test, strict Clippy, architecture or milestone has run for this successor. Acceptance and MAIN integration remain pending.
