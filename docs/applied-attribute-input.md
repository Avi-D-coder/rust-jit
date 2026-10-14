# M010 applied attribute input

Pinned source facts distinguish an original outer attribute range from a meta introduced by cfg_attr. item_without_applied_attribute finds one original item in its captured token forest and removes exactly the selected hash plus bracket group. It preserves later attribute groups, item/body tokens, original spans and owned context ancestry. Token text is never printed or reconstructed.

Items with cfg/cfg_attr syntax return AttributeItemInput::NeedCfgMaterialization. A cfg_attr meta shares a source wrapper with sibling effective attributes, so deleting that wrapper would drop unrelated attributes. Ordinary transforming attributes on cfg-controlled items also need preprocessing before native input is ready. This explicit incomplete outcome cannot be mistaken for a ready native input. The remaining general materializer/worklist is still required.

The actual check_applied_absent host fixture walks native TokenTrees, checks that its own attribute is absent and inline remains, then returns the item unchanged. Both the matched external server and native Rust1.99 accept that input; the returned owned forest and native function result agree. The source-body tokens are preserved without executable lowering. Acceptance also checks original-record retention and cfg/cfg_attr deferral.

This implements ordinary applied-attribute removal and verifies its native ABI boundary. Sequential replacement/derive append, cfg materialization, automatic helper metadata and full generated/guest MX018–MX020 remain pending. No task/release/Linux completion is claimed.
