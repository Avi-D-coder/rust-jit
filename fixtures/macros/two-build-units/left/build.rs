use std::{env, fs, path::PathBuf};
fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    fs::write(out.join("generated.rs"), "31_u32\n").unwrap();
    println!("cargo::rustc-env=OUTPUT_OWNER=left");
    println!("cargo::rustc-cfg=lrr_generated_left");
    println!("cargo::rerun-if-changed=build.rs");
}
