# Revision 4 migration

Revision 4 is the sole active specification. The revision-3 file was deleted at
the user's request. Repository instructions, dependency-pin audit, and task-ledger
validation read the replacement file.

The workspace release is 0.2.0 and the live profile is `lrr-2024-v2`. Snapshot
semantics version is 2. Crate identity includes a profile fingerprint; parse,
header, outline, body, and PC identities inherit it directly or through their
definition identity. A request carrying `lrr-2024-v1` fails profile validation.
No persistent live cache, preparation manifest, or public service protocol has
been implemented yet, so there is no existing on-disk cache migration to perform.
Their future implementations must enforce Sections 56 and 59's schema rules.

All 140 task cards (T/H/M) are tracked. Prior completed cards retain their commit
and artifact references and are marked in progress with historical validation
scope until revision-4 integrations pass. New release gates fail explicitly until
implemented. The former bootstrap reports retain their original profile and
version; newly generated reports identify revision 4 and provisional scope.

The source outline and query database currently cover macro-free structural work.
Their incomplete-scope markers must be integrated with required expansion epochs
and declaration demands; they are not a macro or trait implementation. Section
57 puts M001–M003 ahead of the remaining source/query batch.

Migration validation: all 58 local unit/integration/doc tests pass. Linux revision-4
validation remains failed: host disk exhaustion caused Docker overlay/containerd
metadata input/output errors. Incremental build caches were removed, but Docker
still could not start the retry. See `artifacts/revision4-migration.json` and the
retained raw logs. This is not a passing Linux gate.
