# M007 original receiver bindings

The pinned SelfParam in an explicitly entered method header supplies an owned
receiver recipe. Receiver=5 is an explicit stable binding origin tag; prior
binding tags are preserved. Identity encodes the declared method, root scope,
receiver tag, semantic self name and deterministic duplicate ordinal. Original
self-token and whole-receiver ranges remain attached. Receiver names are explicit
bindings, visible from body entry, and cannot resolve as constant-pattern demands.
Header cfg filters disabled receiver candidates. Body edits preserve identity.

The local name index admits the exact receiver keyword self while retaining
pinned identifier validation for other spellings. Macro source-name validation
still rejects self and local lookup rejects r#self. Owned, mutable, borrowed,
lifetime-qualified and explicitly typed receiver forms share the original-name
contract. Nested blocks inherit it; separately entered named functions do not.
All recipe/index metadata admission remains checked and bounded.

Three source checks cover six receiver spellings, retained header ranges, body
and nested-block visibility, function isolation, cfg filtering and identity
stability. Four standalone macOS Rust 1.99 programs compare original receiver
identity selections and fixture instance literal fields with actual native
execution. Each case retains its own source, compiler output and report. This
checks receiver binding scope, not guest method dispatch, ownership checking,
field evaluation or typing of arbitrary explicit receivers.

Generated publication, other binding forms, closure capture, pattern equivalence
and query/service/VM integration remain pending. M007 and release remain
incomplete. Evidence: artifacts/macos-first/m007-receiver-bindings-checkpoint.json.
