//! Pure package metadata and exact-unit compile environment construction.
use super::{BuildOutput, CheckedJsonShape, CompileEnv, absolute, env_name, fault, text};
use crate::{
    core::{diagnostic::Diagnostic, digest::StableEncode},
    project::cargo_units::{ArtifactProfiles, NormalizedUnit, TargetKind, UnitRole},
    source::snapshot::CfgSet,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
#[derive(Debug, Clone)]
pub struct PackageEnvironments {
    packages: BTreeMap<String, BTreeMap<String, String>>,
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Diagnostic> {
    let s = value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| fault(format!("Cargo metadata {key} absent")))?;
    text(s)?;
    Ok(s)
}
fn optional(value: &Value, key: &str) -> Result<String, Diagnostic> {
    match value.get(key) {
        Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => {
            text(s)?;
            Ok(s.clone())
        }
        _ => Err(fault(format!("Cargo metadata {key} malformed"))),
    }
}
fn version_parts(version: &str) -> Result<[String; 4], Diagnostic> {
    let without_build = version.split_once('+').map_or(version, |(main, _)| main);
    let (core, pre) = without_build.split_once('-').unwrap_or((without_build, ""));
    let parts = core.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || !p.bytes().all(|c| c.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
                || p.parse::<u64>().is_err()
        })
    {
        return Err(fault("invalid Cargo package version"));
    }
    let valid_identifiers = |input: &str, numeric_zero: bool| {
        !input.is_empty()
            && input.split('.').all(|part| {
                !part.is_empty()
                    && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    && (!numeric_zero
                        || !part.bytes().all(|b| b.is_ascii_digit())
                        || part.len() == 1
                        || !part.starts_with('0'))
            })
    };
    if (!pre.is_empty() && !valid_identifiers(pre, true))
        || without_build.ends_with('-')
        || version
            .split_once('+')
            .is_some_and(|(_, build)| !valid_identifiers(build, false))
    {
        return Err(fault("invalid Cargo package version identifiers"));
    }
    Ok([
        parts[0].into(),
        parts[1].into(),
        parts[2].into(),
        pre.into(),
    ])
}
impl PackageEnvironments {
    /// Cargo metadata schema1, with bounded duplicate-free JSON. Unused metadata is opaque.
    pub fn decode(bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(fault("Cargo metadata exceeds 16 MiB"));
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        CheckedJsonShape::<false>::deserialize(&mut decoder).map_err(|e| fault(e.to_string()))?;
        decoder.end().map_err(|e| fault(e.to_string()))?;
        let value: Value = serde_json::from_slice(bytes).map_err(|e| fault(e.to_string()))?;
        if value["version"] != 1 {
            return Err(fault("requires Cargo metadata schema1"));
        }
        let raw = value["packages"]
            .as_array()
            .ok_or_else(|| fault("Cargo metadata packages absent"))?;
        if raw.len() > 100_000 {
            return Err(fault("Cargo package count limit"));
        }
        let mut packages = BTreeMap::new();
        for package in raw {
            let id = string(package, "id")?;
            let name = string(package, "name")?;
            let version = string(package, "version")?;
            if id.is_empty() || name.is_empty() {
                return Err(fault("Cargo package identity absent"));
            }
            let manifest = string(package, "manifest_path")?;
            absolute(manifest)?;
            let directory = Path::new(manifest)
                .parent()
                .and_then(Path::to_str)
                .ok_or_else(|| fault("package manifest directory absent"))?;
            let parts = version_parts(version)?;
            let authors = package["authors"]
                .as_array()
                .ok_or_else(|| fault("Cargo package authors malformed"))?;
            let authors = authors
                .iter()
                .map(|v| {
                    let s = v.as_str().ok_or_else(|| fault("Cargo author malformed"))?;
                    text(s)?;
                    Ok(s)
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .join(":");
            text(&authors)?;
            let mut variables = BTreeMap::from([
                ("CARGO_MANIFEST_DIR".into(), directory.into()),
                ("CARGO_MANIFEST_PATH".into(), manifest.into()),
                ("CARGO_PKG_NAME".into(), name.into()),
                ("CARGO_PKG_VERSION".into(), version.into()),
                ("CARGO_PKG_AUTHORS".into(), authors),
            ]);
            for (key, part) in ["MAJOR", "MINOR", "PATCH", "PRE"].into_iter().zip(parts) {
                variables.insert(format!("CARGO_PKG_VERSION_{key}"), part);
            }
            for (key, field) in [
                ("DESCRIPTION", "description"),
                ("HOMEPAGE", "homepage"),
                ("REPOSITORY", "repository"),
                ("LICENSE", "license"),
                ("LICENSE_FILE", "license_file"),
                ("RUST_VERSION", "rust_version"),
                ("README", "readme"),
            ] {
                variables.insert(format!("CARGO_PKG_{key}"), optional(package, field)?);
            }
            if packages.insert(id.into(), variables).is_some() {
                return Err(fault("duplicate Cargo package identity"));
            }
        }
        Ok(Self { packages })
    }
    pub fn unit_variables(
        &self,
        unit: &NormalizedUnit,
        cargo_path: &str,
        primary_packages: &BTreeSet<String>,
    ) -> Result<BTreeMap<String, String>, Diagnostic> {
        absolute(cargo_path)?;
        let mut variables = self
            .packages
            .get(&unit.key.package)
            .ok_or_else(|| fault("exact unit package metadata absent"))?
            .clone();
        variables.insert("CARGO".into(), cargo_path.into());
        variables.insert(
            "CARGO_CRATE_NAME".into(),
            unit.key.target_name.replace('-', "_"),
        );
        if unit.key.target_kind == TargetKind::Binary {
            variables.insert("CARGO_BIN_NAME".into(), unit.key.target_name.clone());
        }
        if primary_packages.contains(&unit.key.package) {
            variables.insert("CARGO_PRIMARY_PACKAGE".into(), "1".into());
        }
        Ok(variables)
    }
}
pub struct CompileEnvInputs<'a> {
    pub host: &'a str,
    pub base_cfg: &'a CfgSet,
    pub profiles: &'a ArtifactProfiles,
    pub cargo: BTreeMap<String, String>,
    pub declared: BTreeMap<String, Option<String>>,
    pub output: Option<&'a BuildOutput>,
}
impl CompileEnv {
    /// Exact-unit assembly from captured facts; no ambient environment or tool execution.
    pub fn assemble(
        unit: &NormalizedUnit,
        inputs: CompileEnvInputs<'_>,
    ) -> Result<Self, Diagnostic> {
        let CompileEnvInputs {
            host,
            base_cfg,
            profiles,
            mut cargo,
            declared,
            output,
        } = inputs;
        let id = unit.key.fingerprint();
        if matches!(unit.key.role, UnitRole::BuildHost | UnitRole::ProcMacroHost)
            && unit.key.compile_target != host
        {
            return Err(fault("compile environment unit/world mismatch"));
        }
        let mut cfg = base_cfg.clone();
        cfg.atoms.remove("test");
        cfg.atoms.remove("debug_assertions");
        cfg.values.remove("feature");
        if unit.key.test_intent {
            cfg.atoms.insert("test".into());
        }
        if profiles.debug_assertions(id, unit)? {
            cfg.atoms.insert("debug_assertions".into());
        }
        if !unit.key.features.is_empty() {
            cfg.values
                .insert("feature".into(), unit.key.features.clone());
        }
        if cargo.contains_key("OUT_DIR") {
            return Err(fault("OUT_DIR must come from captured exact-unit output"));
        }
        let (out_dir, rustc_env) = match output {
            Some(output) => {
                if !output.consumers.contains(&id)
                    || !unit
                        .dependencies
                        .iter()
                        .any(|e| e.target == output.producer)
                {
                    return Err(fault("build output does not belong to compile unit"));
                }
                output.tree.validate()?;
                if output.tree.producer() != output.producer
                    || output.rustc_env.contains_key("OUT_DIR")
                {
                    return Err(fault(
                        "build output identity or reserved environment mismatch",
                    ));
                }
                for value in &output.cfgs {
                    crate::syntax::cfg_input::add(&mut cfg, value)?;
                }
                let path = output.tree.out_dir().to_owned();
                cargo.insert("OUT_DIR".into(), path.clone());
                (Some(path), output.rustc_env.clone())
            }
            None => (None, BTreeMap::new()),
        };
        for (key, value) in cargo.iter().chain(&rustc_env) {
            env_name(key)?;
            text(value)?;
        }
        for (key, value) in &declared {
            env_name(key)?;
            if let Some(value) = value {
                text(value)?;
            }
        }
        Ok(Self {
            unit: id,
            host: host.into(),
            target: unit.key.compile_target.clone(),
            cfg,
            cargo,
            rustc_env,
            declared,
            out_dir,
        })
    }
    /// Compile-time precedence; absent declared values never fall back to the host process.
    pub fn lookup(&self, key: &str) -> Option<&str> {
        self.rustc_env
            .get(key)
            .or_else(|| self.cargo.get(key))
            .map(String::as_str)
            .or_else(|| self.declared.get(key).and_then(Option::as_deref))
    }
}
