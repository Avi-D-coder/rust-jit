# M012 component acceptance audit

M012 component acceptance is complete under E-57-001. MX025 and MX026 remain
pending for full LRR guest/discovery acceptance, and release_complete is false.
This does not replace those fixtures with native-only comparisons or change
Section 57 order. Batch C begins with T020; M013 follows T034 and H001–H003.

| Requirement | Implementation and exercised evidence |
| --- | --- |
| No automatic cross-epoch procedural reuse | Exact session keys contain epoch. MacroRun owns fresh matched sessions in each frozen request; a retained pure-output cache exposes no invocation-result lookup. The same tool binary observes an external-file edit from 11 to 29 in a new epoch. |
| Pure reuse after validation | Session fulfilment precedes output-cache validation. A later epoch executes again before identical pure token facts can share storage. Semantic identity excludes epoch alone while preserving unit, tool/export/kind, structural invocation/input, environment, parent context, editions, grammar, engine and output wire identity. |
| Exact once within an epoch | Shared coordinator operations and session records execute one invocation once across consumers/phases. Distinct counter invocations return 1 then 2; a fresh epoch starts at 1. |
| Independent consuming units | One actual Cargo root and its live dependency use independent matched unit sessions within one MacroRun. Both start at 1 and advance locally to 2. Compiler argv evidence rejects compilation of either live body during macro-only preparation. |
| Active query eligibility | ExpansionEpoch input tag23 and CurrentExpansionInput query tag21 track current readiness with ordinary dependency stamps. Missing/current/stale epoch, removal, no-memo suspension and stale adoption are exercised. Historical exact-record reads remain distinct and grant no current-run eligibility. |
| Recorded inputs | Explicit capability plans and existing bounded before/after captures verify recorded source/build/tool identities. Actual source mutation during native expansion rejects publication, closes the unit/coordinator and prevents retry. Pre-invocation mutation executes zero native calls. Undeclared reads remain non-hermetic. |
| Final verification | Private-field VerifiedMacroRunInputs is issued only after final verification and closure. Exact context/preparation/DB revision/epoch validation rejects other runs and republication. An actual source edit after expansion but before verification issues no certificate and prevents further native work. This is not a catalog seal. |
| Identity and resource refusal | The semantic matrix distinguishes every producer/input/context field, consuming unit and changed output provenance despite equal literal text. Exact epoch-only reuse retains semantic identity while freshness changes. Wrong epoch/environment and storage limits refuse without partial cache insertion. |
| Authority and side effects | Macro authority is checked before session execution even with retained pure facts. Query providers own no subprocesses, build tools or ambient filesystem reads. No selected live runtime unit is compiled; only explicit macro preparation builds the tool. |

All eight selected component checks are observed in the final macOS Gate00
host/doc output. Gate01 verifies source/query/edit/cache invariants. Clippy,
formatting, architecture and manifest audits pass; final artifacts retain source
hashes, actual macro outputs and compiler-call evidence. The rejected initial
multi-root fixture is preserved, and the single-root graph contract is unchanged.

Final integration remains required:

- T034 and M013–M015 must connect run ownership/current-epoch eligibility to guest
  execution and discovery consumers, with M016 rendering the final origin chain.
- M013 must require current typed input verification together with completed
  producer/membership facts before publishing actual catalog seals. The input
  certificate alone does not establish discovery completeness.
- M036 and M038 must revalidate portable/fresh-worker records and exact unit
  graphs rather than importing session/native state or inherited authority.
- M043 establishes complete full_lrr MX025/MX026 evidence; M045 completes the
  build/tool/membership mutation, aggregate limit and protocol stress matrices.

Remaining CLI/harness entry points join these tasks in their fixed order.
Component completion never claims their checks passed, whole-request containment,
Linux support or release acceptance. Linux remains a separately validated port.
