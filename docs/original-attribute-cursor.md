# M010 lexical original attribute cursor

`OriginalItemAttributeCursor` retains an indexed original item, parse identity,
cfg, helper owner and source scope. The bounded source index captures effective
meta ranges in lexical order and refuses overlapping or reversed attachments.
Cursor polling resolves only the current meta through `OriginalAttributeResolver`;
later metas cannot produce premature resolution errors.

The cursor can advance only a non-transforming helper candidate or builtin derive
annotation. It refuses procedural transformations, builtin test handling, unresolved
inputs, completion, stale parses, changed owner/cfg/index membership and stale
ordinals. A procedural replacement needs reanalysis and a new cursor. Candidate
inspection does not seal namespaces, authorize execution or publish query results.

The native alias acceptance fixture exercises derive advancement, blocked test and
procedural advancement, repeated polling, stale parses and ordinals. An additional
native case places an unknown attribute after `make_tests`: Rust 1.99 replaces the
item with two discovered tests without resolving that later attribute. LRR polling
stops at the original procedural candidate with no macro execution or later error.

Automatic worklist/query integration, generated reanalysis, complete inert/builtin
attribute handling, namespace sealing and full MX018–MX020 remain pending. M010
and release acceptance remain incomplete; Linux acceptance is deferred.
