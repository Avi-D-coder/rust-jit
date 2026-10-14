# E-M005-001: pinned raw token converter doc-comment panic

The adapter source pin remains
`bcf0514497034e1156f3cc2bb72b99cc9634a949`. On Rust 1.99.0 macOS ARM64,
`syntax_bridge::parse_to_token_tree` panics on the minimal input `/// docs`.
Its `RawConverter::convert_doc_comment` calls
`syntax::ast::make::tokens::doc_comment`, which parses that comment alone as a
complete source file and unwraps the parser's `expected an item` error. Adding a
following item to the original input does not fix this: the helper still reparses
the individual comment token alone.

The adapter correction uses the supported
`syntax_bridge::syntax_node_to_token_tree` API with the actual captured-source
syntax token and an absolute-range `SpanMapper`. This retains the pinned lexer,
parser, doc-comment desugaring, token storage and frontend. It introduces no new
macro expander, dependency version, upstream patch or type inference. Parsing
here constructs tokens; it does not compile or type-check live source.

Regression:
`token_wire::pinned::tests::pinned_token_remapping_is_independent_of_upstream_file_slots_and_preserves_spans`
includes a doc comment, lifetimes, raw identifiers, nested groups, literal
spelling and joint operators, and round-trips through the pinned storage with
changed ephemeral file slots. Failures are not caught or treated as success.

The original failed run and stack trace are preserved in
`artifacts/macos-first/m005-pinned-token-tests.log` and
`artifacts/macos-first/m005-pinned-token-panic-backtrace.log`; the corrected run is
`artifacts/macos-first/m005-pinned-token-tests-fixed.log`. This is an adapter API
erratum for the pinned upstream source; revision 4's architecture is unchanged.
