# T020 component acceptance audit

Normative task: revision 4 Section 27, T020; cross-ledger position: Section 57,
batch C before T021. This audit covers the original-source namespace/import
component. The component is complete after its current Gate01, strict Clippy, formatting
and architecture audits passed. Component acceptance follows [E-57-001](errata/e57-component-integration-order.md)
and never means release completion or generated-source acceptance.

| Required component behavior | Authoritative implementation and acceptance |
| --- | --- |
| Separate local/module/prelude/type/macro buckets | `frontend/names.rs` owns namespace-specific indexes; `frontend/source_locals.rs` pairs local scopes with exact source/body/context and retains textual macro lookup. `namespace_tables_preserve_equal_names_and_named_over_glob_precedence`, `ordinary_module_precedence_and_explicit_prelude_keep_namespaces_separate`, and `ordinary_body_macro_view_preserves_textual_definition_order` exercise the distinct buckets. |
| Lexical shadowing | `ordinary_local_lookup_preserves_producer_demands_and_exact_source_validation`, `body_items_shadow_outer_locals_and_module_names_in_exact_scopes`, and `nested_functions_capture_enclosing_items_without_enclosing_local_bindings` cover source order, hoisted items and enclosing item scopes. Native comparisons select the same original declarations. |
| Import fixed point, aliases and explicit versus glob selection | Symbolic named targets preserve original identity. Deterministic global rounds replace source-derived weak edges; iterative body continuations resolve chained glob bases. `global_glob_chains_converge_and_retract_superseded_member_origins`, `global_glob_rebuild_preserves_manual_edges_and_repeated_resolution`, and the body chained-glob controls exercise convergence, late explicit precedence and ownership. |
| Missing, private and ambiguous imports | `source_named_import_reports_missing_private_and_blocked_targets_without_false_completion`, `qualified_body_member_preserves_original_private_visibility`, global late-ambiguity controls, and anonymous-facet controls distinguish typed outcomes. A pending scope can preempt ordinary lookup while its import report retains the specific failure. Final coordinator diagnostics are later integration. |
| Cyclic reexports | `source_globs_preserve_precedence_ambiguity_cycles_and_open_producers` and its native comparison cover originating and empty cycles; body unresolved-cycle controls retain demands. Cycles cannot manufacture a definition. |
| cfg membership | `source_namespace_views_use_cfg_membership_constructors_and_cold_headers`, source/native cfg controls, and `inactive_anonymous_imports_do_not_create_bindings_records_or_demands` exercise selected membership and absence of inactive demands. |
| Macro-blocked namespace cannot be complete | Source/body producer controls keep both disclosed and missing members pending; `global_glob_convergence_retains_late_ambiguity_and_open_producer_demands` and anonymous controls cannot discharge open producers. No discovery seal is issued by these indexes. |
| Anonymous imports | Anonymous source/body records retain original import/definition IDs, namespace facets and original scope without creating a name binding. Captured scopes, aliases, privacy, missing/ambiguous targets, cfg and stale-source rejection are exercised. Native anonymous trait visibility agrees; LRR trait method dispatch is not claimed. |
| Determinism, sizes and effect boundaries | Stable explicit encodings cover definitions/imports/anonymous records and convergence state. Existing index guards, bounded work/depth/metadata and two quota-restoration controls are exercised. Helpers read immutable selected facts or mutate caller-owned indexes during construction; they do not read files, execute macros, invoke build tools, load symbols or publish seals. Aggregate release resource boundaries remain mandatory. |

The full selected inventory is the T020 entry in `docs/tasks.json` and
`compat/fixtures.toml`; its current acceptance evidence is
[the macOS checkpoint](../artifacts/macos-first/t020-anonymous-imports-checkpoint.json).
Gate01 includes Gate00 workspace tests and the existing source/query/cache gate.
No omitted or future fixture is counted as passed.

Full integration remains mandatory: T030 supplies the supported standard facade;
T034 supplies guest execution acceptance; M013–M016 supply generated discovery,
body/name publication and transformed catalogs; M017 consumes trait-aware source
contexts; M036/M038 cover edition and live/native unit worlds; M043 supplies full
regression/cache matrices. MX013, MX016, MX021 and MX027–MX029 are explicitly
pending in the ledger, with no full-LRR evidence attached. Project preparation,
generated seals, linked/native integration, final diagnostics, aggregate quotas,
performance/security reporting and release gates must still pass. Linux and
additional macOS architectures require their own actual platform acceptance.

Verified macOS checkpoint: 564 workspace host/doc tests, 65 namespace checks
and the two quota-restoration helper controls passed. The task ledger records
component completion and `release_complete=false`; its six full-LRR integration
fixtures remain pending.
