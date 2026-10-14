# M010 effective attribute materialization

Source facts retain every original outer attribute range and each effective meta's enclosing wrapper, including nested cfg_attr. The materializer selects one original item, removes original attribute wrappers, and builds the remaining active attributes in lexical order from original meta token subtrees. It removes cfg and inactive branches and omits the currently applied attribute. Body and remaining tokens are copied without executable lowering or printed-text reconstruction.

An active cfg_attr child shares its source hash/bracket wrapper with other children. The new bracket group keeps the original wrapper's span keys and the child's original tokens; full context ancestry is retained. Span/context rows are pruned to referenced records. Missing/ambiguous wrappers, foreign/stale source and invalid active/index states fail with typed diagnostics.

Acceptance verifies nested conditional attributes produce inline/cold/must_use in order while omitting the applied first attribute and unavailable branch. The actual matched macro inspects native TokenTrees: cfg/cfg_attr and its own applied attribute are absent, and inline remains. Its unchanged returned item and native Rust1.99 execution agree. This supplies the previously explicit NeedCfgMaterialization boundary for original attribute inputs.

General sequential replacement/derive append scheduling, automatic helper metadata, generated discovery/catalog and full MX018–MX020 remain pending. M010 and the project remain in progress; no release/Linux completion is claimed.
