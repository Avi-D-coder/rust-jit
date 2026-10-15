//! Read-only evidence helper; never parses, compiles or executes guest input.
use lrr::core::digest::Digest;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Component, Path},
    process::ExitCode,
};

#[derive(Deserialize)]
struct Manifest {
    files: Vec<Input>,
}
#[derive(Deserialize)]
struct Input {
    path: String,
    bytes: u64,
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("source digest input bound exceeded".into());
    }
    Ok(bytes)
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: lrr_source_digests LICENSED_SOURCE_DIRECTORY".into());
    }
    let root = Path::new(&args[0])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let manifest_bytes = read_bounded(&root.join("manifest.json"), 8 * 1024 * 1024)?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes).map_err(|e| e.to_string())?;
    if manifest.files.is_empty() || manifest.files.len() > 10_000 {
        return Err("source digest inventory bound exceeded".into());
    }
    let mut files = BTreeMap::new();
    let mut total = 0u64;
    for input in manifest.files {
        let path = Path::new(&input.path);
        if input.path.is_empty()
            || input.path.len() > 4096
            || !path
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err("nonrelative source digest path".into());
        }
        let absolute = root.join(path).canonicalize().map_err(|e| e.to_string())?;
        if !absolute.starts_with(&root) {
            return Err("source digest input escapes licensed directory".into());
        }
        let bytes = read_bounded(&absolute, 16 * 1024 * 1024)?;
        if input.bytes != bytes.len() as u64 {
            return Err("source digest input length differs from manifest".into());
        }
        total = total
            .checked_add(input.bytes)
            .filter(|total| *total <= 64 * 1024 * 1024)
            .ok_or("source digest total input bound exceeded")?;
        let value = json!({"bytes": input.bytes, "content": Digest::of_bytes(&bytes)});
        if files.insert(input.path, value).is_some() {
            return Err("duplicate source digest path".into());
        }
    }
    println!(
        "{}",
        json!({"schema": "lrr/v5/source-digest-index/v1", "files": files})
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
