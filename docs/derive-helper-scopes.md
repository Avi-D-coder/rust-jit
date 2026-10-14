# M010 exact derive helper scopes

DeriveHelperBinding binds helper declarations to the full item-derive ExpansionKey and a declaration-content identity. It accepts only procedural derive identities in item grammar. Names use the pinned source identifier validation and raw/NFC normalization; native Rust permits duplicate helper spellings, so canonical duplicates form a set. Count/name/metadata bounds are checked.

ItemHelperScope binds these declarations to the owning item and explicit cfg identity. Fields and variants use their owning ADT identity; unrelated items cannot borrow its scope. Only exact active expansion identities grant helpers, including artifact/export/kind, invocation, cfg/environment, context/editions, engine and epoch. Binding order is preserved for deterministic owner reporting and stable encoding.

An active derive without a matching declaration yields HelperLookup::NeedDeclarations. It is not an empty ready answer and cannot establish a cached absence. The convenience helper_owners interface rejects such incomplete lookup. Empty active derives legitimately grant no helpers. Lookup performs no macro invocation, file read or preparation.

The matched API6 export listing exposes names and kinds, not the helper declaration list. Metadata is therefore explicit here and never inferred from export spelling. The native acceptance uses the controlled prepared EmitItems declaration and compares active, absent, sibling and cfg-inactive cases with actual Rust1.99 metadata compilation. A separate synthetic proc-host metadata probe demonstrates canonical duplicate declarations are valid. Production automatic metadata acquisition, derive-path extraction, replacing/appending transformations and full MX018–MX020 remain pending.
