# Mechanical assertion-constant lint follow-on

V110 strict compilation stopped at two Clippy diagnostics, actual terminal `831219/1`: a nested optional-source check and an owned SyntaxText conversion used only for comparison. This two-leaf follow-on uses the suggested let-chain and direct borrowed text comparison. No permission, return, source identity or execution behavior is relaxed; no test oracle changes.

Apply after the macro-constant packet `4c7e676d` and before the cast/local-match/driver successors. Exact hashes and zero-fuzz replay are recorded; formatting is checked. Fresh strict compilation, the four pending acceptance bodies, affected banks and a fresh full milestone remain required. The V110 strict failure is retained and no tests ran on that source. Canonical promotion and upstream pass counts remain unchanged.
