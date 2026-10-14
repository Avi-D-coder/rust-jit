use std::{env, process::Command};
fn main() {
    let rustc = env::var("RUSTC").expect("explicit tool preparation requires RUSTC");
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("compiler version capture");
    assert!(output.status.success());
    let version = String::from_utf8(output.stdout).expect("compiler version UTF-8");
    assert!(version.lines().any(|line| line == "release: 1.99.0"));
    assert!(
        version
            .lines()
            .any(|line| line == "host: aarch64-apple-darwin")
    );
    let commit = version
        .lines()
        .find_map(|line| line.strip_prefix("commit-hash: "))
        .expect("compiler commit");
    let compiler = env::var("RUSTC").expect("compiler path");
    let sysroot = Command::new(compiler)
        .args(["--print", "sysroot"])
        .output()
        .expect("compiler sysroot");
    assert!(sysroot.status.success());
    let sysroot = String::from_utf8(sysroot.stdout).expect("sysroot UTF-8");
    let library = std::path::Path::new(sysroot.trim()).join("lib");
    assert!(library.join("libLLVM.dylib").is_file());
    println!("cargo:rustc-link-search=native={}", library.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", library.display());
    println!("cargo:rustc-env=LRR_SERVER_COMPILER_COMMIT={commit}");
    println!("cargo:rustc-env=LRR_SERVER_SOURCE_COMMIT=bcf0514497034e1156f3cc2bb72b99cc9634a949");
}
