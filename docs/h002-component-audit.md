# H002 component acceptance and mandatory integration

H002's current scope machinery is accepted provisionally under Section57 and
E-57-001. The full E2401–E2407/E2410 requirements remain pending; neither native
oracle results nor explicit kernel controls replace their full LRR assertions.

| Current implementation | Actual evidence |
|---|---|
| Failed conditions retire retained clauses before else; success retains storage through consequent | Source-backed if/while and kernel ownership controls |
| Let chains are ordered and later clauses stay cold after failure | Source-backed preparation/call counts and bindings controls |
| Clause cleanup reverses successful clauses and drains bindings before temporaries | Actual kernel retirement and interrupted-fuel ownership checks |
| Tail result escapes before tail and block retirement | Actual scalar guest execution and explicit kernel owning storage |
| Statement, ordinary boolean condition, lazy operand, and guard boundaries | Deterministic bounded index and executed source thunks |
| Let extension overrides default narrowed boundaries | Pure structural annotations and actual kernel owning storage |
| Constructor extension requires resolved constructor identity | Exact source-call guards remain unapplied while identity resolution is pending |
| Edition2024 native lifetime baselines | Recorded Rust1.99 macOS oracles, separately labeled native-only |

The current match-guard control uses source-backed scalar guard thunks inside an
explicit kernel prelude and join. It does not select an arm or execute Match Entry.
Scalar controls do not exercise source borrows, aggregate constructors, user Drop,
cleanup suspension, generated origin editions, or native tier replay.

Mandatory later integrations are T040–T043 for types, layouts, references and
owning values/annotations; T050/T051 for general pattern selection/commit and match;
T060–T062 for value/facade implementations and Drop; M014/M016 for generated origins
and diagnostics; M025/M028/M030 for resolved constructors, patterns and derived
behavior; T080/T084 and M035/M037 for actual JIT replay/cache equivalence; and M043
for the complete fixture matrix. Older dependency and mixed-edition token lifetime
cases must use their owning editions during generated-source integration.

Full acceptance requires E2401–E2407/E2410 executed under off/auto/force with
matching drop/effect transcripts and skipped preparation observations. The ledger
retains every fixture as pending and release_complete=false until that evidence
exists. No Linux support or complete revision4 release is claimed.
