fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let count_path = out.join("execution-count.txt");
    let count = std::fs::read_to_string(&count_path)
        .ok()
        .map(|text| text.parse::<u64>().unwrap())
        .unwrap_or(0);
    std::fs::write(count_path, (count + 1).to_string()).unwrap();
    std::fs::write(out.join("generated.rs"), format!("{}_u32\n", lrr_tool_probe_build_support::generated_value())).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(lrr_tool_prepared)");
    println!("cargo:rustc-cfg=lrr_tool_prepared");
    println!("cargo:rustc-env=LRR_TOOL_PROBE=captured");
}
