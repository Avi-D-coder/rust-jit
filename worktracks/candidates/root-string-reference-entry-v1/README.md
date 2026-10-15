# Shared string parameter entry and read

T041/T062 follow-on to `root-string-reference-arguments-v1`. The callee source-table installer and parameter descriptor preflight now admit shared `TyTerm::Str` references. Mutable string views and bare unsized values remain excluded. Actual source/cell authority is supplied by the closed-reference call contract; physical delivery and reads retain the existing checked reference machinery.

The existing all-edition component control now evaluates two real upstream NUL initializers into distinct argument cells, seals and delivers the resulting call packet, and executes the actual `ReadParameter` recipe from unchanged `match-borrowed-str.rs`. It checks that mismatched pointee admission fails before delivery, delivery cannot repeat, and the read preserves the genuine UTF-8 reference and source identity. This remains a component join of two original witnesses, not whole-main or end-to-end source-call acceptance.

Two leaves are formatted and exactly replayed, uncompiled and unexecuted. The coherent v2 packet includes this follow-on; do not apply both forms.
