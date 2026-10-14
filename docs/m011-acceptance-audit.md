# M011 component acceptance audit

M011 component acceptance is complete under E-57-001. This records the owned
include/compile-input substrate, not final guest acceptance. MX021–MX024 remain
pending and release_complete remains false. Fixed Section 57 order is preserved;
M012 may implement epoch revalidation next. No final fixture is skipped or marked
passed by this audit.

| Requirement | Current implementation and exercised evidence |
| --- | --- |
| Exact compiler identity | Six pinned descriptors and source-bound compiler facets distinguish genuine aliases from user shadowing. Original token, edition, context and consumer environment bind expansion keys; forged identities refuse. |
| Compile environment | Frozen exact-unit values distinguish explicit absence from uncaptured names. Required env diagnostics and optional values agree with native Rust. Published DB inputs invalidate value/absence/removal memos without ambient lookup. |
| Eager path construction | Ordered owned env/concat/include operands resolve through existing source/hygiene facts, suspend for missing input and resume only after exact coordinator adoption. Nested producers are re-observed before capture/emission. |
| Coordinator authority | Pure routing selects explicit source/build-output roots. Only SourceCapture opens files. Regular files are read with checked limits and before/after identity verification; root/file replacement and FIFO swaps refuse. Runtime read grants do not authorize source capture. |
| Immutable output | Prepared build outputs remain unchanged after OUT_DIR deletion/recreation, including access through source aliases. Uncaptured membership remains absent. UTF8 errors differ from exact binary capture. |
| Source bases and scope | Captures retain physical authority, logical syntax address and raw filename. Included views retain leases and dependency proofs. File/directory symlink and dot-parent witnesses agree with native nested bases; non-root caller context survives without a fresh mark. |
| Revalidation | Source sets stage and revalidate before publication. Changed bytes, removal, membership/generation changes and alias retarget invalidate/refuse; retained old captures stay immutable. Ordinary DB stamps invalidate dependent include outputs. |
| Resource limits | Checked input/plan/worklist metadata and depth limits refuse. Actual 8 MiB capture succeeds and byte+1 refuses. Actual 8 MiB byte literal agrees byte-for-byte with native include_bytes; exact wire budget, escaped text and aggregate span-path limits refuse before large output construction. |
| Concat semantics | Pinned syntax-only calculation has audited API exposure and numeric normalization repair. Native radix/separator/sign/suffix/float/u128 and invalid-literal controls pass. Original output spans and adopted parent keys are preserved. |
| Side-effect separation | Queries read owned records and return typed requests. Produced/admitted output cannot advance a parent until adopted. No query executes a procedural macro, build tool or native runtime unit. Gate architecture checks enforce the selected boundaries. |

All 52 selected component tests are observed as passing in the current macOS
Gate00 host/doc output. Gate01 supplies source/query/edit/cache evidence. Clippy,
formatting and architecture checks pass. Source hashes and native observations
are retained in the final component audit artifact; historical failures and
errata E-M011-001/002/003 remain available.

Final acceptance dependencies remain explicit:

- MX021 requires included generated definitions/impls to enter discovery and
  execute in the selected guest test (T034, M013–M015, M038, M043).
- MX022 requires final lexical integration and guest execution; current native
  base/context comparisons and owned source views do not prove that execution
  (T034, M013, M016, M043).
- MX023 requires automatic compiler/prelude facets and exact Option facade output,
  not guessed Some/None identifiers. NeedOptionFacade and missing-environment
  preparation remain typed incomplete outcomes (T020, T051, M013, M016, M028,
  M038, M043).
- MX024 requires epoch/service preparation transactions and final full-run
  invalidation. Explicit immutable source-set/DB invalidation tests are component
  evidence only (M012, M038, M043).

User/declarative/procedural continuation scheduling, namespace seals, generated
DefKeys, automatic preparation publication, complete reached grammar/error and
aggregate stress matrices must be exercised by these later integrations and the
final fixture gate. No native oracle or component report replaces full_lrr
fixture evidence. Linux remains a separate later port.
