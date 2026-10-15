# Coherent string literal, equality and call candidate

This 29-leaf T022/T024/T041/T062/M017 packet supersedes the five packets listed in `SOURCE-AUDIT.json`. Apply this combined form once, after the byte milestone; do not also apply its constituent patches. Every base leaf was checked against frozen V87 source `2e19361d`. The independently validated immutable-root ancestry repair is carried forward exactly, including its cycle test in the prerequisite baseline.

The union adds decoded UTF-8 literals, distinct unsized source identity, bounded immutable storage and literal execution, current-source macro operand admission, source-selected string equality, and shared string argument contracts through parameter delivery/read. Existing byte behavior and source/ownership/epoch/quota checks remain required regressions.

Pending whole-main witnesses are unchanged `slowparse-string.rs` and `underscores-in-unicode-escapes.rs`. Other Unicode/NUL/CRLF checks exercise literal or operand components. The borrowed-string call control explicitly joins upstream header/literal witnesses and executes the original parameter read; the match fixture still needs string pattern execution. Methods, owned strings, aliases, mutable/general borrowed string views and reference return contracts remain outstanding.

The union's source hashes, ordered joins, guard carryforward and exact replay were checked. Rust compilation and all string runtime controls remain pending. No stock runtime/full-fixture pass or broad task completion is claimed. Merge upstream manifests additively: the frozen byte inventory does not contain all newer canonical string originals and must not replace them.
