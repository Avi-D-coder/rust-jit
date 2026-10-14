# E-M006-006: stable ordinary macro definitions

The pinned backend accepts `${count($v)}` in an ordinary user macro, emitting a
literal successfully. Native Rust 1.99.0 rejects the same standalone source with
E0658: meta-variable expressions are unstable. Revision 4 excludes nightly
language extensions. The native probe receipt is
`artifacts/macos-first/m006-stable-profile-probe/native.json`; the failing pinned
regression is `artifacts/macos-first/m006-stable-profile-pinned-witness.log`.

A stable-profile definition entry point preserves the pinned rule parser and
adds a check over its original structured token input. It rejects nightly rule
headers, `$$` and `${...}` forms on definition demand. It does not inspect literal
contents, does not confuse adjacent tokens across delimiter boundaries, and does
not reject ordinary nested unbound variables used to generate a macro definition.
Ordinary unsafe body syntax remains syntax at this stage; guest lowering/capability
rules still decide reached operations. This check grants no unsafe/native authority.

The error is attached to and retained with the demanded immutable definition.
Repeated invocation attempts return the same attributed definition error with
zero preflight calls and zero output materializations. Actual native/profile
comparison and positive literal/nested-variable/body-syntax regressions exercise
that distinction. No compiler is invoked by the adapter or a query provider.

The existing library/API patch is extended against its original archived
preimage, with the same audit/tampering checks and unchanged dependency pins.
The ordinary-source stable entry point is LRR's default. Certified compiler-owned
builtin identities/recipes and any explicitly permitted internal builtin syntax
remain M011; this does not grant arbitrary user macros compiler privileges.
