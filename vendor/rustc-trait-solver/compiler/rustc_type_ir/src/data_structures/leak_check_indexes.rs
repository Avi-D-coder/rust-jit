// Exact pinned compiler index declarations; original retained in infer/region_constraints/leak_check.rs.
rustc_index::newtype_index! {
    #[gate_rustc_only]
    #[orderable]
    #[debug_format = "LeakCheckNode({})"]
    pub struct LeakCheckNode {}
}

rustc_index::newtype_index! {
    #[gate_rustc_only]
    #[orderable]
    #[debug_format = "LeakCheckScc({})"]
    pub struct LeakCheckScc {}
}
