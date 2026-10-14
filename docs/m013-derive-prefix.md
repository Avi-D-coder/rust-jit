# M013 ordered derive-output visibility

The minimal implementation witness is a generated ADT with two derives. The
first emits `macro_rules! prefix` whose body declares `FIRST_PREFIX`. The second
emits `prefix!()`, then a same-named macro declaring `SECOND_PREFIX`, then another
`prefix!()`. Discovery previously remained incomplete at the first call in the
second output. Both derives had executed, but the ancestor lookup boundary
excluded all bindings at the original ADT's source position.

`m013-derive-prefix-before-production.log` records that failure. The earlier
`before` and `before-final` logs are harness failures while extending the former
empty-output control; they are not implementation witnesses. This is an
implementation gap, not a specification contradiction or a design change.

Each appended derive child now retains an immutable textual prefix view before
that child is admitted. The view keeps preceding output bindings, excludes its
own and later outputs, and retires only its own producer blocker. Other
frontiers remain pending. Ancestor lookup and final selection replay use the
same view and a checked exclusive boundary immediately after the original ADT
position. The production parent keeps its producer open until every derive
output completes. View retention has a checked 32 MiB bound and contributes to
catalogue/discovery metadata accounting.

The native LRR control performs two attributes and two derives, then two
declarative expansions. It requires both generated constants, original-item
retention, discarded-item absence, exact selection replay, seal publication and
the owning run's existing authority/adoption checks. The separate scope control
checks that views cannot admit a wrong producer, remain unchanged after a later
shadow, and preserve another pending frontier.

The official rustc case `tests/ui/proc-macro/gen-macro-rules.rs` and its auxiliary
are copied unchanged at commit `ae0fe511b15ab760d7e963b13389d1bc2ab42b18`, with
their original MIT OR Apache-2.0 licenses. An additional component test explicitly
prepares only the unchanged auxiliary as a macro-host crate. It derives the
payload from the original `S` declaration, executes `repro` through the matched
external server, then expands the emitted macro using the original `m!()` token
group. The runtime source is never compiled. This proves native macro execution
and original invocation expansion, not a complete rustc compiletest or LRR run.

Full copied-fixture acceptance, cross-output import/namespace matrices, deeper
nested producers, replacing attributes after derive, legacy helpers and the
remaining M013 contracts are pending. M013 remains in progress; release and
Linux acceptance remain pending.
