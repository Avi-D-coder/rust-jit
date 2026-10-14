# M007 invocation selection in entered bodies

Entered macro facts now retain enabled invocation ranges, lexical scope ordinals
and own attribute producer identities. Expression macro attributes use the pinned
MacroExpr node, while the selected invocation range belongs to its inner
MacroCall. The existing attribute/cfg classifier is shared with item discovery.
Disabled macro expressions do not disclose a call or outward producer. Statement
macro producers remain separate from initializer expression calls.

The entered-frame API validates the exact SourceSelection and BodyKey, requires
full range containment in the entered body and rejects opaque nested syntax
before reading a call from the retained immutable source view. Exact AST selection
rejects partial ranges. Enabled unqualified calls use local/captured textual
precedence; qualified or namespace-fallback paths use the containing module and
supplied macro namespace graph. Prior and own attribute producer demand remains
NeedInput. The invocation inventory has checked count/allocation admission.

Two source acceptance checks cover lexical shadowing, nested function/closure/
cold-body rejection, wrong body keys, partial ranges, cfg absence, own attribute
suspension and a qualified original-source route. The existing actual native
macOS body-shadowing oracle now uses this invocation API before expanding the
selected original rules with the pinned adapter and comparing native execution.
The failing expression attribute/range witness is retained before correction.

Linked-module outer-chain invocation routing, body-local imports, local-binding
ribs, generated epochs and attribute execution/publication, membership seals,
guest VM and M012 aggregate accounting remain pending. These checks do not claim
full body namespace completion or finished runtime acceptance.
