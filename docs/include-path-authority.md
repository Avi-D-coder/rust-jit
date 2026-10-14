# M011 pure include path authority

`IncludeAuthorities` owns explicit source/build-output root facts for one consuming
unit and source generation. It routes a selected include/include_str/include_bytes
descriptor to an immutable `GeneratedSourceRequest`; it performs no filesystem or
ambient environment access and grants no runtime read permission.

Relative paths use the containing original/virtual file's directory. Included
locations retain that new base for nested requests and do not invent module scope.
Absolute paths must fall under an explicitly granted root. The most specific root
wins, so a prepared OUT_DIR nested inside a source root keeps its build-output
producer identity. Duplicate authorities, foreign units, absent caller roots,
invalid paths and metadata/depth/count excesses refuse. Denial diagnostics preserve
caller/include origins with bounded path excerpts.

Requests bind authority fingerprint, unit/generation, builtin identity, source file,
origin keys, original requested spelling and routed root/path. Original path spelling
is retained for coordinator capability canonicalization. This lexical plan is not
proof against symlink escapes: physical canonicalization and race-aware capture are
still required before adopting bytes, and are pending M011 work.

Acceptance checks nested/parent-relative and build-output bases, exact request reuse,
generation/origin changes, separate include kinds, denial and bounded errors. Native
Rust 1.99.0 compares nested relative includes plus an OUT_DIR include; the LRR check
compares pure routes only. Even a missing file yields a capture request without a
read. No LRR nested builtin execution, capture, discovery or guest assertion is
claimed by that native comparison.

Source capture/adoption, symlink and race checks, immutable build-output reads,
full include operand/eager expansion integration and MX021–MX024 remain pending.
M011, project release and Linux acceptance remain incomplete.
