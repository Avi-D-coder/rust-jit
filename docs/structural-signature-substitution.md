# T032 structural signature substitution — work in progress

The signature recipe now retains known structural terms alongside original type
syntax: rigid definition parameters, tuples/unit, arrays, slices, named-lifetime
references and never. Parameter identity includes its definition and ordinal.
Lifetime names are canonicalized while original syntax remains intact. Other
paths, including primitive-looking names, stay deferred until source identity is
resolved; a local `struct i64` cannot be guessed into a primitive contract.
Projection/function/dyn/opaque syntax also remains retained for later lowering.

The initial owned term vocabulary separates type, const and region arguments,
nominal definition identity, definition parameters and transaction-local inference
variables. Explicit stable tags encode every represented distinction. Term byte,
node and nesting accounting is checked before unifier cloning; unification has a
finite work budget and bounds the total retained binding nodes. Higher-ranked
bound-region inference is guarded until binder/universe integration is available.
The guard does not establish M017's full shifting/leak-safe binder machinery.

The local unifier binds only Infer, applies an occurs check through existing
bindings, keeps Params rigid and compares nominal identities rather than layout.
Concrete widths never widen, and unknown variables never default to unit/i32.
Each constraint is staged; a mismatch publishes no partial binding prefix.
Foreign inference transactions are rejected. SignatureSubstitution retains the
original rigid argument/result templates and instantiates only type parameters
into call-local inference variables. Unused parameters and bounds stay unforced.
Region/const parameters remain rigid here; full substitution is still required.

Actual tests exercise direct/indirect cycles, nominal/parameter/width/namespace
identity, rejected bound-region capture, atomic rollback, foreign variables,
nesting exhaustion and an actual demand-captured generic signature. Its argument
and result constraints agree on i64, a u64 disagreement preserves the earlier
binding, and the source template remains T. Query traces contain no body/thunk/
name/constant demand. These are source contract constraints, not an executed call.

T032 remains in progress. Source type resolution, primitive/alias identity,
complete binders/projections/consts, separate concrete RuntimeSubst and source
views, ordered arguments, sealed CallPackets, parameter/return boundaries and
actual guest-call tests remain mandatory. General tabled trait goals remain the
later M tasks. macOS is primary; no release or Linux acceptance is claimed.

The first full gate failed in the pre-existing owned-macro-descendant fixture
before its native invocation: the server stream closed during the 500 ms startup/
inventory budget. The same fixture passed in isolation. Its fixed test budget is
now two seconds, still below the unchanged five-second completion bound; exact
timeout/no-retry/helper-reap/real descendant-PID assertions remain. The descendant
report records that budget. Production deadline policy is unchanged. Retained
attempt1 and isolated reproduction logs are required evidence; neither is a
successful full gate. This is fixture deadline sizing, not a backend erratum.
