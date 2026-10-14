//! Frozen compile-input facts. No ambient environment, filesystem or VM access.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    project::tool_manifest::CompileEnv,
};
use std::collections::BTreeMap;
const MAX_BYTES: usize = 32 * 1024 * 1024;
fn fault(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-COMPILE-INPUT", message.into())
}
fn text(value: &str) -> Result<(), Diagnostic> {
    if value.len() > 64 * 1024 || value.contains('\0') {
        return Err(fault("invalid compile-input string"));
    }
    Ok(())
}
fn name(value: &str) -> Result<(), Diagnostic> {
    text(value)?;
    if value.is_empty() || value.contains('=') {
        return Err(fault("invalid compile-input environment name"));
    }
    Ok(())
}
fn charge(bytes: &mut usize, amount: usize) -> Result<(), Diagnostic> {
    *bytes = bytes
        .checked_add(amount)
        .filter(|n| *n <= MAX_BYTES)
        .ok_or_else(|| fault("compile-input environment metadata limit"))?;
    Ok(())
}
#[derive(Clone)]
pub struct FrozenCompileEnvironment {
    unit: Digest,
    fingerprint: Digest,
    variables: BTreeMap<String, Option<String>>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum CapturedEnvironmentValue<'a> {
    Value(&'a str),
    Absent,
    NeedCapture,
}
#[derive(Debug)]
pub enum CompileStringPoll<'a> {
    Ready(Result<Option<&'a str>, Diagnostic>),
    NeedCapture {
        unit: Digest,
        name: &'a str,
        environment: Digest,
    },
}
impl FrozenCompileEnvironment {
    pub(crate) fn allocated_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .variables
            .len()
            .checked_mul(128)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(|| fault("compile environment size overflow"))?;
        for (name, value) in &self.variables {
            charge(&mut bytes, name.capacity())?;
            if let Some(value) = value {
                charge(&mut bytes, value.capacity())?;
            }
        }
        if bytes > MAX_BYTES {
            return Err(fault("compile environment metadata limit"));
        }
        Ok(bytes)
    }

    /// Operand dispatch after namespace selection of an exact compiler descriptor.
    pub fn environment_operand<'a>(
        &'a self,
        descriptor: &super::compile_input_identity::CompileInputDescriptor,
        key: &'a str,
    ) -> Result<CompileStringPoll<'a>, Diagnostic> {
        use super::compile_input_identity::CompileInputKind;
        match descriptor.kind() {
            CompileInputKind::Env => self.string_input(key, true),
            CompileInputKind::OptionEnv => self.string_input(key, false),
            _ => Err(fault("selected builtin is not an environment operand")),
        }
    }
    pub fn new(unit: Digest, input: &CompileEnv) -> Result<Self, Diagnostic> {
        if unit != input.unit {
            return Err(fault("compile-input environment belongs to another unit"));
        }
        if input.cargo.get("OUT_DIR") != input.out_dir.as_ref()
            || input.rustc_env.contains_key("OUT_DIR")
        {
            return Err(fault("compile-input OUT_DIR differs from prepared output"));
        }
        let count = input
            .declared
            .len()
            .checked_add(input.cargo.len())
            .and_then(|n| n.checked_add(input.rustc_env.len()))
            .filter(|n| *n <= 100_000)
            .ok_or_else(|| fault("compile-input environment entry limit"))?;
        let mut bytes = input.cfg.metadata_bytes()?;
        charge(
            &mut bytes,
            count
                .checked_mul(128)
                .ok_or_else(|| fault("environment size overflow"))?,
        )?;
        for value in [&input.host, &input.target] {
            text(value)?;
            charge(&mut bytes, value.capacity())?;
        }
        if let Some(value) = &input.out_dir {
            text(value)?;
            charge(&mut bytes, value.capacity())?;
        }
        for (key, value) in input.declared.iter().map(|(k, v)| (k, v.as_deref())).chain(
            input
                .cargo
                .iter()
                .chain(&input.rustc_env)
                .map(|(k, v)| (k, Some(v.as_str()))),
        ) {
            name(key)?;
            charge(&mut bytes, key.capacity())?;
            if let Some(value) = value {
                text(value)?;
                charge(&mut bytes, value.len())?;
            }
        }
        let mut variables = input.declared.clone();
        for map in [&input.cargo, &input.rustc_env] {
            for (key, value) in map {
                variables.insert(key.clone(), Some(value.clone()));
            }
        }
        Ok(Self {
            unit,
            fingerprint: input.fingerprint(),
            variables,
        })
    }
    pub fn fingerprint(&self) -> Digest {
        self.fingerprint
    }
    pub fn unit(&self) -> Digest {
        self.unit
    }
    pub fn lookup(&self, key: &str) -> Result<CapturedEnvironmentValue<'_>, Diagnostic> {
        name(key)?;
        Ok(match self.variables.get(key) {
            Some(Some(value)) => CapturedEnvironmentValue::Value(value),
            Some(None) => CapturedEnvironmentValue::Absent,
            None => CapturedEnvironmentValue::NeedCapture,
        })
    }
    /// Every observation, including absence and an unresolved capture, is unit-bound.
    pub fn dependency(&self, key: &str) -> Result<Digest, Diagnostic> {
        let value = self.lookup(key)?;
        let mut e = StableEncoder::new("lrr/v2/compile-environment-observation");
        e.digest(self.unit);
        e.digest(self.fingerprint);
        e.string(key);
        match value {
            CapturedEnvironmentValue::Value(value) => {
                e.u8(0);
                e.string(value);
            }
            CapturedEnvironmentValue::Absent => e.u8(1),
            CapturedEnvironmentValue::NeedCapture => e.u8(2),
        }
        Ok(e.finish())
    }
    /// Operand evaluation only: callers must resolve the genuine builtin first.
    pub fn string_input<'a>(
        &'a self,
        key: &'a str,
        required: bool,
    ) -> Result<CompileStringPoll<'a>, Diagnostic> {
        Ok(match self.lookup(key)? {
            CapturedEnvironmentValue::Value(value) => CompileStringPoll::Ready(Ok(Some(value))),
            CapturedEnvironmentValue::Absent if required => CompileStringPoll::Ready(Err(fault(
                "required compile-time environment value is absent",
            ))),
            CapturedEnvironmentValue::Absent => CompileStringPoll::Ready(Ok(None)),
            CapturedEnvironmentValue::NeedCapture => CompileStringPoll::NeedCapture {
                unit: self.unit,
                name: key,
                environment: self.fingerprint,
            },
        })
    }
}

impl std::fmt::Debug for FrozenCompileEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrozenCompileEnvironment")
            .field("unit", &self.unit)
            .field("fingerprint", &self.fingerprint)
            .field("entries", &self.variables.len())
            .finish()
    }
}
