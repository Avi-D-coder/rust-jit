# Common indexed binding declaration attachment

Coordinator `/root`, LOCAL-024, T024/T030/T050 and M014; baseline
`53ab9d3f97eb4e963b65f24d39c0df1ea9e0669e`. Source-only work, uncompiled and
unexecuted. Root owns syntax/node_lookup.rs and node_lookup_tests.rs; the pattern
worker owns the two lowering call sites in literal_match.rs and condition_ranges.rs.

Invariant: an already resolved indexed binding identifies one actual pattern
occurrence. Finding its enclosing let statement must use that indexed range,
never search every statement in the body again. The existing ordinary route is
hygienic local_binding -> BodyIndex Pattern NodeId -> primitive source annotation
or initializer constraints -> verified ordinary scalar operations. Unchanged
rustc macro-nested_stmt_macros.rs, return-nil.rs and regions-infer-call.rs drive
range correspondence; no imported originals change. Generated/retained syntax
joins through the same admitted parser and index, without granting name/type or
runtime authority from a structural range.

Extend ParsedFile's existing syntax_at service with binding_let_statement.
Require matching body/index/profile, preserve entered_body_root's actual entry
and content checks, require the indexed Pattern role, and accept only an exact
pattern whose immediate parent is its owning LetStmt. Match/condition bindings
must return None, never a nearby let. Preserve byte/range overflow and body-boundary
checks. The two consumers remove their old full-body declaration rediscovery;
source hints, Type proof requirements and continuations keep existing owners.

Cost: bounded direct tree descent plus the existing body-content validation,
not a claim that all phase preparation is cached or constant time. No namespace
map, BodyIndex clone, parser, evaluator, source publication, macro authority,
new opcode or installed runtime work. Other lexical scans, plan reconstruction,
body hashing and global namespace costs remain held. Source review covers all
eight architecture modes: no semantic alternate routes (1-5), no new repeated
plan (6), no namespace change (7), removes two source lookup scans without
weakening body validation (8). Sole validator must check the frozen combined
pattern candidate and existing numeric/pattern routes; no pass or closure is
claimed by this record. All eight macro audit findings remain open.
