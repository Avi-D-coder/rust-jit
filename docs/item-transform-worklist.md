# M010 deterministic item transformation worklist

`expand::worklist` owns one annotated item and its resolver-supplied lexical
attribute and derive slots. Unresolved slots yield typed resolution demands;
they never become inert through lookup failure. Explicit Inert decisions require
the caller to finish builtin/helper admission. Attributes run before derives.
Execution remains outside the worklist and outside pure query providers.

An attribute result replaces the item and suspends for reanalysis. Its ticket
binds owner, cfg, checked revision, exact expansion key and output digest.
Reanalysis must supply exactly that output's token forest under the same owner/cfg.
The old plan is discarded; newly introduced attributes and derives are scheduled
from new resolver facts. Empty replacement is supported. A derive result appends
an immutable expansion record while preserving the original item; sibling derives
share the retained annotated item. The queue accepts builtin derive identities and
exact procedural attribute/derive identities. Invocation reuse, wrong kind,
grammar, compile environment or epoch, stale reanalysis and excessive storage/counts
refuse. Result records preserve origins and token forests rather than printed text.

`ExpansionCoordinator::admit_transform` connects the worklist to reply adoption.
It requires the exact successful output already published through `adopt` before
allowing advancement. Changed database source revision, stale context, early advancement and repeated advancement
are rejected. Starting a ticket grants no execution authority. The existing macro
session checks permission and the matched external server independently.

The native acceptance explicitly prepares only a controlled proc-macro crate.
Two real attribute invocations replace an undefined cold function with a struct
and generated derives. Reanalysis drops old actions. Two real derives share their
input, append a constant and an empty result, and preserve the struct. Each output
passes ticket start, reply acceptance, database adoption and transform admission.
The observed generated constant agrees with standalone native Rust 1.99.0 macOS
execution. Unit acceptance also exercises unresolved lexical slots, sibling append
ordering, builtin identity admission, duplicate/consumed invocation refusal,
slot bounds, stale tickets and empty replacements.

Scope remains component integration. Resolver facts and item boundaries are
supplied by the fixture; automatic generated-token item scanning and namespace
reanalysis await M013–M016. Whole-file/multiple-item publication, field cfg,
automatic exact-export helper metadata, guest execution and full MX018–MX020
remain pending. No full task or Linux acceptance is claimed.
