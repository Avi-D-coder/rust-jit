# T022 minimal LRIR and structural admission

The initial owned opcode inventory contains Literal, Unit, SetHint,
ResultTransfer, ScopeEnter, ScopeLeave and Unsupported. Explicit effect classes
separate primitive work, ownership transfer, scope activation, cleanup boundaries
and reached faults. No operation stores a parser object, guest handle, native
pointer or callback. Hint recipes currently cover absent and primitive contexts;
full symbolic type/member phases remain assigned to later tasks.

Recipe has the Section15 mandatory exit and boxed 1–8 operation/dependency lists.
RecipeDraft is a construction-only vector form with an exit list, allowing the
verifier to reject zero or multiple exits before producing VerifiedRecipe. The
admission wrapper has no public unchecked constructor or mutable accessor.
Future installation must require that wrapper. Exit records represent Next,
Branch, Call, Control, Effect and Fault; the non-Next records require explicit
dispatch boundaries. Packet sealing, broker publication and runtime lifecycle
contracts remain execution/call/effect tasks, not claimed here.

The verifier checks owned body/node/phase IDs, slot bounds/kinds, unique outputs,
non-aliasing result transfers, source spans, scope/control/site IDs, explicit
exits and metadata sizes. It does not type-check literal spellings, prove a bool
condition, resolve names, fetch successors or execute a guest operation. Original
body indices reject expanded origin ancestry until validated expanded-body
inputs arrive in M014/M016. No absent expansion becomes a successful origin.

Dependencies reuse the existing database's typed input/query stamps. New input,
namespace-bucket and stamp encodings use explicit domain/tag/field encodings;
existing query-key encodings are unchanged. Admission accounts dependencies,
sorts without allocation and rejects repeated keys. Reversed input order produces
an identical recipe fingerprint; namespace and observed fingerprint changes
remain distinct. The per-recipe 64 MiB guard is a local metadata admission bound,
not evidence that the whole installed LRIR cache budget is enforced.

Pure thunk helpers construct literal/unit/branch/unsupported recipes from body
structure and explicit symbolic inputs. Branch successors are PC data. There is
no child provider or recipe store in construction/verification. Zero-op branch
phases use one Unit operation, with an explicit non-condition scratch slot.

Component acceptance exercises slot kinds/bounds, duplicate writes/transfers,
operation counts 0/8/9, missing/duplicate exits, PCs/origins/scopes, dependency
identity/order, explicit call/effect site/resume/result fields, deferred fault
payloads and effect/quota unit controls. Structural acceptance of Unsupported
and its diagnostic payload is not proof of a VM fault. Actual opcode execution,
no-successor-request query traces, cleanup, guest handles and bound synthetic
exit observation must pass T024/T025/T034 and later recording/JIT integration.

See [macOS evidence](../artifacts/macos-first/t022-lrir-checkpoint.json). The
component uses E-57-001; full-LRR T022-DEFERRED-UNSUPPORTED, T022-LAZY-BRANCH and
MX014 remain pending. Call/effect preparation/publication, symbolic trait phases,
expanded origins, full opcode inventory, whole-worker metadata/cache/resource
checks and release/performance/security reporting remain mandatory. Linux is a
later separately exercised port.

The integration ledger now recognizes registered non-MX task fixtures as full
acceptance targets as well as normative MX IDs. Unknown/unregistered targets
remain rejected, MX IDs still require the specification, and pending or empty
passed evidence cannot permit release completion. The dedicated runtime-fixture
guard and the ledger test exercise this extension. The initial rejected ledger
run is retained separately; it is not a passed gate.
