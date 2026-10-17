use clap::{Parser, Subcommand};
#[derive(Parser)]
#[command(
    name = "lrr",
    version,
    about = "Rust 2015/2018/2021/2024: LRR revision-4 development runtime"
)]
pub struct Args {
    /// Explicit coordinator directory for bounded local internal-failure artifacts.
    #[arg(long, global = true)]
    pub diagnostic_dir: Option<std::path::PathBuf>,
    /// Owning edition for standalone run/test source; defaults to 2024.
    #[arg(long, global = true, default_value_t = crate::core::profile::LIVE_EDITION)]
    pub edition: u16,
    /// Source dependency crate, rustc-style `[noprelude:]NAME=PATH` (repeatable).
    /// Without `noprelude:` NAME joins the root crate's extern prelude; every
    /// captured crate also reaches it through `extern crate NAME;`.
    #[arg(long = "extern", global = true, value_parser = super::crate_graph::parse_extern)]
    pub externs: Vec<super::crate_graph::ExternArg>,
    /// Owning edition of an `--extern` crate: `NAME=YEAR`; defaults to `--edition`.
    #[arg(long = "extern-edition", global = true, value_parser = super::crate_graph::parse_extern_edition)]
    pub extern_editions: Vec<(String, u16)>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    /// Explain a diagnostic using the checked-in local manifest.
    Explain { code: String },
    /// Execute a selected standalone source with the initial LRIR interpreter.
    Run {
        source: std::path::PathBuf,
        /// Observe the actual interpreter terminal and closed guest effects.
        #[arg(long)]
        runtime_capture: bool,
        /// Caller correlation only; never configuration or execution authority.
        #[arg(long, requires = "runtime_capture", value_parser = parse_capture_context)]
        capture_context: Option<crate::core::digest::Digest>,
    },
    /// Observe actual original-source/main structure before guest execution.
    SourceWorld {
        source: std::path::PathBuf,
        #[arg(long, value_parser = parse_world_context)]
        context: crate::core::digest::Digest,
    },
    /// Discover or execute selected original-source tests.
    Test {
        source: std::path::PathBuf,
        filter: Option<String>,
        #[arg(long)]
        exact: bool,
        #[arg(long)]
        list: bool,
        #[arg(long, conflicts_with = "include_ignored")]
        ignored: bool,
        #[arg(long, conflicts_with = "ignored")]
        include_ignored: bool,
    },
}

pub(super) fn parse_capture_context(value: &str) -> Result<crate::core::digest::Digest, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("capture context must be exactly 64 lowercase hexadecimal characters".into());
    }
    serde_json::from_str(&format!("\"{value}\"")).map_err(|e| e.to_string())
}

fn parse_world_context(value: &str) -> Result<crate::core::digest::Digest, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("context must be exactly 64 lowercase hexadecimal characters".into());
    }
    let mut bytes = [0u8; 32];
    for (i, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let nibble = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        bytes[i] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    Ok(crate::core::digest::Digest(bytes))
}
