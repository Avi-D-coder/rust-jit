//! Named and glob import routes in one retained entered body. No expansion effects.
use super::macro_names::{MacroNamespaces, PathLookup};
use crate::{
    core::diagnostic::Diagnostic,
    expand::scopes::MacroName,
    syntax::{
        entered_macros::EnteredMacroFacts,
        imports::{ImportKind, ImportSegment},
        macro_calls::SourceMacroCall,
        outline::ScopeKey,
    },
};
use std::collections::BTreeSet;
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
pub(super) fn name(
    row: &crate::syntax::entered_macros::BodyImport,
    edition: u16,
) -> Result<Option<MacroName>, Diagnostic> {
    match &row.source.kind {
        ImportKind::Named { alias: Some(alias) } => Ok(Some(MacroName::parse(alias, edition)?)),
        ImportKind::Named { alias: None } => row
            .source
            .path
            .iter()
            .rev()
            .find_map(|s| match s {
                ImportSegment::Name(n) => Some(MacroName::parse(n, edition).map(Some)),
                _ => None,
            })
            .unwrap_or(Ok(None)),
        _ => Ok(None),
    }
}
pub(super) fn lookup(
    facts: &EnteredMacroFacts,
    mut scope: u32,
    call: &SourceMacroCall,
    graph: &MacroNamespaces,
    caller: &ScopeKey,
    mut textual: impl FnMut(
        &crate::syntax::entered_macros::BodyImport,
        &str,
    ) -> Result<super::macro_scopes::HygienicLookup, Diagnostic>,
) -> Result<Option<PathLookup>, Diagnostic> {
    if call.absolute() || !matches!(call.segments().first(), Some(ImportSegment::Name(_))) {
        return Ok(None);
    }
    let edition = call.key().profile.edition;
    let mut parts = call.segments().to_vec();
    let mut absolute = false;
    let mut rewritten = false;
    let mut seen = BTreeSet::new();
    let mut bytes = 0usize;
    let mut work = 0usize;
    let mut weak = Candidates::default();
    let modules = super::body_modules::Modules::build(
        facts, graph, caller, edition, scope, &mut work, &mut bytes,
    )?;
    if parts.len() > 1 {
        let prefix = modules.resolve(
            scope,
            &parts[..parts.len() - 1],
            absolute,
            &mut work,
            &mut bytes,
        )?;
        if let Some(input) = prefix.pending.first() {
            return Ok(Some(PathLookup::NeedInput(*input)));
        }
        if !prefix.denied.is_empty() {
            return Err(error("inaccessible body module prefix"));
        }
        if prefix.ambiguous() {
            return Err(error("ambiguous body module prefix"));
        }
        if !prefix.candidates.is_empty() {
            let ImportSegment::Name(last) = parts.last().expect("nonempty call") else {
                return Err(error("invalid body macro member"));
            };
            let name = MacroName::parse(last, edition)?;
            let mut result = Candidates::default();
            for candidate in prefix.candidates {
                bytes = candidate
                    .route
                    .names
                    .iter()
                    .try_fold(bytes, |n, s| {
                        n.checked_add(std::mem::size_of::<MacroName>())
                            .and_then(|n| n.checked_add(s.as_str().len()))
                    })
                    .and_then(|n| n.checked_add(std::mem::size_of::<MacroName>()))
                    .and_then(|n| n.checked_add(name.as_str().len()))
                    .ok_or_else(|| error("body qualified member metadata budget exceeded"))?;
                let mut member = candidate.route.names;
                member
                    .try_reserve(1)
                    .map_err(|_| error("body module macro member allocation failed"))?;
                member.push(name.clone());
                result.add(graph.lookup_from(
                    &super::macro_names::MacroPath::new(candidate.route.unit, member)?,
                    caller,
                )?)?;
            }
            return Ok(Some(result.finish(PathLookup::Absent)?));
        }
    }
    loop {
        work = work
            .checked_add(1)
            .ok_or_else(|| error("body import work limit exceeded"))?;
        let Some(ImportSegment::Name(first)) = parts.first() else {
            break;
        };
        if absolute {
            break;
        }
        let first = MacroName::parse(first, edition)?;
        let mut current = Some(scope);
        let mut selected = None;
        while let Some(ordinal) = current {
            let mut matches = Vec::new();
            for row in &facts.imports {
                work = work
                    .checked_add(1)
                    .ok_or_else(|| error("body import work limit exceeded"))?;
                if row.scope != ordinal {
                    continue;
                }
                if let Some(input) = row.attribute_input {
                    return Ok(Some(PathLookup::NeedInput(input)));
                }
                if name(row, edition)?.as_ref() == Some(&first) {
                    matches
                        .try_reserve(1)
                        .map_err(|_| error("body import candidate allocation failed"))?;
                    matches.push(row);
                }
            }
            if matches.len() > 1 {
                matches.sort_by_key(|row| row.source.identity);
                let origins = matches
                    .iter()
                    .map(|row| row.source.identity.hex())
                    .collect::<Vec<_>>()
                    .join(",");
                return Err(error(&format!(
                    "ambiguous body named import binding: {origins}"
                )));
            }
            if let Some(row) = matches.pop() {
                selected = Some(row);
                break;
            }
            for row in facts
                .imports
                .iter()
                .filter(|row| row.scope == ordinal && row.source.kind == ImportKind::Glob)
            {
                let target = modules.resolve(
                    row.scope,
                    &row.source.path,
                    row.source.absolute,
                    &mut work,
                    &mut bytes,
                )?;
                if let Some(input) = target.pending.first() {
                    return Ok(Some(PathLookup::NeedInput(*input)));
                }
                if !target.denied.is_empty() {
                    return Err(error("inaccessible body glob target module"));
                }
                if target.ambiguous() {
                    return Err(error("ambiguous body glob target module"));
                }
                for candidate in target.candidates {
                    let unit = candidate.route.unit;
                    let components = candidate.route.names;
                    bytes = components
                        .iter()
                        .try_fold(bytes, |n, name| {
                            n.checked_add(std::mem::size_of::<MacroName>())
                                .and_then(|n| n.checked_add(name.as_str().len()))
                        })
                        .ok_or_else(|| error("body glob member metadata budget exceeded"))?;
                    let mut member = components;
                    for part in &parts {
                        let ImportSegment::Name(name) = part else {
                            return Err(error("invalid body glob member qualifier"));
                        };
                        bytes = bytes
                            .checked_add(std::mem::size_of::<MacroName>())
                            .and_then(|n| n.checked_add(name.len()))
                            .ok_or_else(|| error("body glob member metadata budget exceeded"))?;
                        member
                            .try_reserve(1)
                            .map_err(|_| error("body glob member allocation failed"))?;
                        member.push(MacroName::parse(name, edition)?);
                    }
                    // Private members do not contribute through a glob. The target
                    // module's visibility was checked independently above.
                    let result = graph
                        .lookup_from(&super::macro_names::MacroPath::new(unit, member)?, caller)?;
                    if !matches!(result, PathLookup::Inaccessible(_)) {
                        weak.add(result)?;
                    }
                }
            }
            current = facts
                .scopes
                .iter()
                .find(|s| s.ordinal == ordinal)
                .ok_or_else(|| error("body import lexical scope missing"))?
                .parent;
        }
        let Some(row) = selected else {
            break;
        };
        let path_bytes = parts
            .iter()
            .try_fold(
                std::mem::size_of::<(u32, bool, Vec<ImportSegment>)>(),
                |n, s| {
                    n.checked_add(std::mem::size_of::<ImportSegment>())
                        .and_then(|n| {
                            n.checked_add(match s {
                                ImportSegment::Name(n) => n.len(),
                                _ => 0,
                            })
                        })
                },
            )
            .ok_or_else(|| error("body import metadata overflow"))?;
        bytes = bytes
            .checked_add(path_bytes)
            .ok_or_else(|| error("body import metadata budget exceeded"))?;
        if !seen.insert((scope, absolute, parts.clone())) {
            return Ok(Some(weak.finish(PathLookup::Absent)?));
        }
        let mut target = row.source.path.as_slice();
        if target.len() > 1 && target.last() == Some(&ImportSegment::SelfModule) {
            target = &target[..target.len() - 1];
        }
        if !row.source.absolute
            && parts.len() == 1
            && let [ImportSegment::Name(name)] = target
        {
            use super::{macro_origins::MacroSelection, macro_scopes::HygienicLookup};
            match textual(row, name)? {
                HygienicLookup::NeedInput(input) => {
                    return Ok(Some(PathLookup::NeedInput(input)));
                }
                HygienicLookup::Selection(MacroSelection::Defined(def)) => {
                    return Ok(Some(weak.finish(PathLookup::Defined(def))?));
                }
                HygienicLookup::Selection(MacroSelection::Ambiguous(defs)) => {
                    return Ok(Some(weak.finish(PathLookup::Ambiguous(defs))?));
                }
                HygienicLookup::Selection(MacroSelection::Absent) => {}
            }
        }
        target
            .len()
            .checked_add(parts.len() - 1)            .ok_or_else(|| error("body import route depth exceeded"))?;
        let next_bytes = target
            .iter()
            .chain(parts[1..].iter())
            .try_fold(0usize, |n, part| {
                n.checked_add(std::mem::size_of::<ImportSegment>())
                    .and_then(|n| {
                        n.checked_add(match part {
                            ImportSegment::Name(name) => name.len(),
                            _ => 0,
                        })
                    })
            })
            .ok_or_else(|| error("body import route metadata overflow"))?;
        bytes = bytes
            .checked_add(next_bytes)
            .ok_or_else(|| error("body import route metadata budget exceeded"))?;
        let mut next = Vec::new();
        next.try_reserve(target.len() + parts.len() - 1)
            .map_err(|_| error("body import route allocation failed"))?;
        next.extend_from_slice(target);
        next.extend_from_slice(&parts[1..]);
        parts = next;
        absolute = row.source.absolute;
        scope = row.scope;
        rewritten = true;
    }
    if !rewritten && weak.is_empty() {
        return Ok(None);
    }
    match graph.source_segments_path(&parts, absolute, caller, edition)? {
        Some(path) => Ok(Some(weak.finish(graph.lookup_from(&path, caller)?)?)),
        None => Ok(Some(weak.finish(PathLookup::Absent)?)),
    }
}

#[derive(Default)]
pub(super) struct Candidates {
    definitions: BTreeSet<crate::expand::scopes::MacroDefinition>,
    pending: BTreeSet<crate::core::digest::Digest>,
}
impl Candidates {
    fn is_empty(&self) -> bool {
        self.definitions.is_empty() && self.pending.is_empty()
    }
    pub(super) fn add(&mut self, result: PathLookup) -> Result<(), Diagnostic> {
        match result {
            PathLookup::Defined(def) => self.definition(def)?,
            PathLookup::Ambiguous(defs) => {
                for def in defs {
                    self.definition(def)?;
                }
            }
            PathLookup::NeedInput(input) => {
                if !self.pending.contains(&input) {
                    self.admit(0, 1)?;
                    self.pending.insert(input);
                }
            }
            PathLookup::Absent => {}
            PathLookup::Inaccessible(_) => {
                return Err(error("inaccessible strong body import target"));
            }
        }
        Ok(())
    }
    fn admit(&self, definitions: usize, pending: usize) -> Result<(), Diagnostic> {
        self.definitions
            .len()
            .checked_add(definitions)
            .and_then(|n| {
                n.checked_mul(std::mem::size_of::<crate::expand::scopes::MacroDefinition>())
            })
            .and_then(|n| {
                self.pending
                    .len()
                    .checked_add(pending)
                    .and_then(|p| p.checked_mul(32))
                    .and_then(|p| n.checked_add(p))
            })
            .ok_or_else(|| error("body glob candidate metadata budget exceeded"))?;
        Ok(())
    }
    fn definition(
        &mut self,
        definition: crate::expand::scopes::MacroDefinition,
    ) -> Result<(), Diagnostic> {
        if !self.definitions.contains(&definition) {
            self.admit(1, 0)?;
            self.definitions.insert(definition);
        }
        Ok(())
    }
    pub(super) fn finish(mut self, strong: PathLookup) -> Result<PathLookup, Diagnostic> {
        if self.is_empty() {
            return Ok(strong);
        }
        if !self.pending.is_empty() && matches!(strong, PathLookup::Inaccessible(_)) {
            return Ok(PathLookup::NeedInput(
                *self.pending.first().expect("pending input"),
            ));
        }
        self.add(strong)?;
        if let Some(input) = self.pending.first() {
            return Ok(PathLookup::NeedInput(*input));
        }
        Ok(match self.definitions.len() {
            0 => PathLookup::Absent,
            1 => PathLookup::Defined(self.definitions.pop_first().expect("one definition")),
            _ => PathLookup::Ambiguous(self.definitions.into_iter().collect()),
        })
    }
}
