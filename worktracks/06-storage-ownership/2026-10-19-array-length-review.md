# T040/M018 — separate array-length lexical source review

Baseline9541aedd162a852ebfb4c9a4d251971f131ec96b. Reviewed candidate V457 source
c9ae85543ecf2c180847607d2d6c5e7ffab1295a7bc20cf75ecf15184ff406a9;
CLI c124026e5d8737bb31d3222c66073a33c791c604b91ce3be4c2e7d2ec4f15c1b.
This records inspected source and actual scoped evidence, not feature completion.
MA002/MA003/MA006 are separate audit IDs; this does not close those repairs.

1. No assertion/expression evaluator. syntax::literal_decode::array_length
projects an existing DecodedLiteral::Integer produced by integer() from the
actual RA LiteralKind/IntNumber token. Common thunks/pure-constant/pattern
integer decoding remains unchanged. No second lexer/parser or display reparse.
2. No generated scalar/break executor, special verifier namespace or opcode.
All source keeps ordinary declaration Type/schema, constraint, thunk and VM
routes. There is no generated-source special case in this patch.
3. No context-selected numeric inference/method rule. The helper reads lexical
magnitude with checked u64 conversion; it does not bind a guest number or guess
an inferred Type. No new static suffix/type check. Unsuffixed/suffixed/radix
spellings use the same existing integer decoder; required Type arguments and
nonliteral constant expressions remain their existing separate demand.
4. Original/generated source identity, scopes, editions, schema/header occurrence
and namespace guards remain unchanged in SchemaNames::actual_field_type and
HeaderShape::capture. All these paths use the same lexical projection; no
original-only exclusion or receipt fabricated to admit a generated field.
5. No pattern or executable-arm preparation changes. signatures::lower_type and
source_types::resolve_actual_type read only their required declared Type AST;
HeaderShape captures the actual required declaration header. Literal array Type
metadata does not lower an initializer or an unselected body/branch.
6. Decimal display-text parsing is removed at source_types array lengths;
signatures' parallel decimal parse and HeaderShape's separate token extraction /
unvalidated u128 truncation are removed. The helper is used by memoized pure
source preparation, not installed operations. Ordinary VM repeat executes the
reached initializer through its existing construction/copy continuation.
7. No namespace map/census, new invalidation scope or runtime dispatch work. A
literal token's existing parser value is decoded on the cold query path;
checked u64 conversion reflects the term representation, not a resource cap.
8. No parser/BodyIndex deep clones or whole-file searches. Captured AST locations
and existing source plans are retained. Cold lexical work is O(token spelling),
with no new LRR resource accounting/limit, sandbox, static checker, unsafe,
frontend/query/backend or custom trait solver. Performance is unmeasured.

V457 both macOS Rust1.99.0 builds0; three exact controls0: actual generic/const/
region parameter header identity, unchanged static integer spelling in all
four editions, and existing ordinary repr/nested/generic layout execution.
Initial dry-run selector resolution rejected two guessed module paths before
running tests; selectors were corrected from the actual binary --list. This
rejection is not a passed control. Owning2015 unchanged align-struct completes
54932matched VM dispatch/outcomes,174source calls,2initializers, unit/empty
managed streams/cleanup. This is one distinct upstream runtime gain against
V456's51559dispatch constructor failure, not four fixtures or full-suite parity.
Closing source/HEAD/test/CLI guard0. Other editions are supplemental explicit
root runs and remain pending until their terminal/guards are collected.

Unmet scope: full nonliteral array Type const demand, const generics, complete
local/generated/wrapper/standard-macro metadata route matrix and all broader
source/member/upstream-solver integration. Legacy binder/signature consumers
and nested union view destruction remain held. No full architecture, task,
milestone, interpreter, JIT, diagnostic/tooling, Linux or performance closure.

Final: supplemental explicit root editions2018/2021/2024 also pass this exact
unchanged original under the existing observe/classify capture checks, process
terminal0. These supplement owning2015; original directives remain2015. All
four runs complete unit/empty managed streams/cleanup. The post-edition complete
source/HEAD/test/CLI comparison passes; no source changed during validation.
No generated/wrapper Type-metadata matrix is inferred from these edition rows.
