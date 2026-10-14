# H001 current identity audit and mandatory integration

This audit covers existing keys and admission paths. It does not claim caches or
checkpoint restoration assigned to later tasks already exist or pass acceptance.

| Existing family | Edition/profile binding |
|---|---|
| CrateKey | guest_edition and semantics_profile are explicit stable fields |
| DefKey, BodyKey and phase/PcKey | declaration contains the crate identity; body contains declaration identity; phase contains body identity |
| ParseKey, OutlineKey and constants | explicit ProfileIdentity; outline contains its parse key |
| Scope/bucket and signature identities | source scope or declaration identity inherits the crate identity |
| BodyIndex including structural patterns and reserved temporary slots | explicit profile in stable encoding, plus body identity |
| Verified recipes and installed PC lookup | source body/phase identity; installed tables retain source publication revision |
| ExpansionKey and output reuse | separate definition/call editions, grammar, engine and context/input identities; no live-root edition rewrite |
| Compilation-unit identity | recorded target edition remains part of normalized unit identity; selected live root must be 2024 |
| Prepared tool manifest | schema4 header contains semantics_profile and rejects mismatched runtime semantics |
| Current CLI results | live_edition=2024 and semantics_profile=lrr-2024-v2 in completion, panic, execution and listing records |

Actual tests distinguish identical source bytes under the normative CrateKey
edition/profile changes in parse, outline, declaration, body, index, phase and
recipe identities. The publication witness executes the valid guest twice around
rejected older-edition and older-profile refreshes/selections. Rejection cannot
publish source, query an incompatible selection, mutate the source revision or
replace the warm memo value. The surviving valid run still completes.

Older dependency and macro-host units retain their recorded editions. The immutable
captured planner graph is only a pure decoding witness; actual matched-server
older macro source-facet controls are separately included in the fresh gate.
No selected ordinary live unit is compiled to establish these assertions.

H001 component completion is provisional under Section57. E2409's actual forced
native tier replay and E2411's dedicated pattern/lifetime caches, generated-origin
cases and executable checkpoint restoration remain pending through H002/H003,
M014/M016, T051, T080/T084, M035/M037 and M043. The ledger retains those requirements
against the raw-identifier and identity-family fixture controls. Their host checks
are component evidence, not full-LRR fixture evidence. Final release admission
requires new full observations; release_complete remains false.
