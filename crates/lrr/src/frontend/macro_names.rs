//! Canonical unit/module macro routes, supplied by the source/import index.
//! This index does not parse imports or execute expansions. Definition identity
//! survives reexports; `$crate` starts at the defining compilation unit.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::scopes::{MacroDefinition, MacroName},
    syntax::{outline::ScopeKey, visibility::VisibilityScope},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct MacroPath {
    pub unit: Digest,
    components: Vec<MacroName>,
}
impl MacroPath {
    pub fn new(unit: Digest, components: Vec<MacroName>) -> Result<Self, Diagnostic> {
        if components.is_empty() || components.len() > 256 {
            return Err(error("empty or oversized macro path"));
        }
        components
            .iter()
            .try_fold(0usize, |bytes, name| {
                bytes
                    .checked_add(name.as_str().len())
                    .filter(|n| *n <= 32 * 1024 * 1024)
            })
            .ok_or_else(|| error("macro path text limit exceeded"))?;
        Ok(Self { unit, components })
    }
    pub fn components(&self) -> &[MacroName] {
        &self.components
    }
}
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PathLookup {
    Defined(MacroDefinition),
    Ambiguous(Vec<MacroDefinition>),
    Absent,
    Inaccessible(Vec<MacroPath>),
    NeedInput(Digest),
}
/// Disclosed source module membership; this does not establish a namespace seal.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ModuleLookup {
    Ambiguous(Vec<(Digest, Vec<MacroName>)>),
    Defined {
        unit: Digest,
        components: Vec<MacroName>,
        visibility: VisibilityScope,
    },
    Absent,
    Inaccessible {
        unit: Digest,
        components: Vec<MacroName>,
    },
}
/// Disclosed provenance, never inferred from module registry membership.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum DeclarationModuleOrigin {
    Original,
    Generated {
        producer: Digest,
        generation: Digest,
        context: crate::syntax::token_wire::Key,
    },
    Unproven {
        input: Digest,
    },
}
impl StableEncode for DeclarationModuleOrigin {
    const DOMAIN: &'static str = "lrr/v2/declaration-module-origin";
    fn encode_fields(&self, e: &mut StableEncoder) {
        match self {
            Self::Original => e.u32(0),
            Self::Generated {
                producer,
                generation,
                context,
            } => {
                e.u32(1);
                e.digest(*producer);
                e.digest(*generation);
                e.digest(Digest(context.0));
            }
            Self::Unproven { input } => {
                e.u32(2);
                e.digest(*input);
            }
        }
    }
}
/// Exact source/expansion attachment for one module binding. Its route is an
/// existing authoritative registry route, not a second module resolution graph.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DeclarationModuleBinding {
    pub identity: Digest,
    pub name: MacroName,
    pub scope: ScopeKey,
    pub lexical_scope: crate::expand::scopes::ScopeKey,
    pub scope_kind: crate::syntax::declaration_scan::DeclarationScopeKind,
    /// The retained lexical ancestry proves attachment to a crate/module route.
    /// An immediate Module kind beneath a block is insufficient.
    pub canonical_attachment: bool,
    pub route_unit: Digest,
    pub route_components: Vec<MacroName>,
    pub visibility: VisibilityScope,
    pub origin: DeclarationModuleOrigin,
    pub pending: Option<Digest>,
}
impl StableEncode for DeclarationModuleBinding {
    const DOMAIN: &'static str = "lrr/v2/declaration-module-binding-with-attachment";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.identity);
        e.string(self.name.as_str());
        e.record(&self.scope);
        e.record(&self.lexical_scope);
        e.record(&self.scope_kind);
        e.bool(self.canonical_attachment);
        e.digest(self.route_unit);
        e.sequence(&self.route_components, |e, name| e.string(name.as_str()));
        e.record(&self.visibility);
        e.record(&self.origin);
        e.option(&self.pending, |e, input| e.digest(*input));
    }
}
impl DeclarationModuleBinding {
    /// Pure conversion from a retained module row. A generated origin must
    /// name this exact capture generation; caller validates its context/authority.
    /// Use `from_generated_scan` for matched generated insertion attachment.
    pub fn from_scan(
        scan: &crate::syntax::declaration_scan::DeclarationScan,
        index: usize,
        origin: DeclarationModuleOrigin,
    ) -> Result<Self, Diagnostic> {
        let row = scan
            .modules()
            .get(index)
            .ok_or_else(|| error("scoped module capture row absent"))?;
        match &origin {
            DeclarationModuleOrigin::Original if scan.generated_origin().is_some() => {
                return Err(error("generated module capture lacks original provenance"));
            }
            DeclarationModuleOrigin::Generated { generation, .. }
                if scan.generated_origin() != Some(*generation) =>
            {
                return Err(error("scoped module generation differs"));
            }
            _ => {}
        }
        let scope_kind = scan
            .lexical_scopes()
            .iter()
            .find(|scope| scope.identity == row.lexical_scope)
            .ok_or_else(|| error("scoped module capture attachment absent"))?
            .kind;
        let name = MacroName::parse(&row.name, row.name_edition)?;
        let mut route_components = super::source_macro_routes::module_names(
            &row.scope.module_path,
            scan.parse().profile.edition,
        )?;
        route_components
            .try_reserve_exact(1)
            .map_err(|_| error("scoped module captured route allocation"))?;
        route_components.push(name.clone());
        let result = Self {
            identity: row.identity,
            name,
            scope: row.scope.clone(),
            lexical_scope: row.lexical_scope,
            scope_kind,
            canonical_attachment: origin == DeclarationModuleOrigin::Original
                && Self::canonical_module_attachment(scan, row.lexical_scope)?,
            route_unit: row.scope.crate_key,
            route_components,
            visibility: row.visibility.clone(),
            origin,
            pending: row.attribute_inputs.first().copied(),
        };
        if result.metadata_bytes()? > 32 * 1024 * 1024 {
            return Err(error("scoped module captured metadata limit"));
        }
        Ok(result)
    }
    /// Pure matched-capture attachment. Only a generated root row is projected
    /// onto its retained producer lexical key; nested capture keys stay exact.
    /// The coordinator must separately admit the completed generation/grammar,
    /// epoch and pinned name context authority before installing this fact.
    pub fn from_generated_scan(
        parent: &crate::syntax::declaration_scan::DeclarationScan,
        producer: Digest,
        generated: &crate::syntax::declaration_scan::DeclarationScan,
        index: usize,
        context: crate::syntax::token_wire::Key,
    ) -> Result<Self, Diagnostic> {
        Self::from_generated_scan_in_slot(parent, producer, generated, index, context, None)
    }
    /// Exact-slot companion for a matched owned expansion record. Unlike pure
    /// Items-shaped argument capture, non-Items roots retain their actual kind.
    pub(crate) fn from_generated_scan_in_grammar(
        parent: &crate::syntax::declaration_scan::DeclarationScan,
        producer: Digest,
        generated: &crate::syntax::declaration_scan::DeclarationScan,
        index: usize,
        context: crate::syntax::token_wire::Key,
        grammar: crate::expand::identity::GrammarSlot,
    ) -> Result<Self, Diagnostic> {
        Self::from_generated_scan_in_slot(
            parent,
            producer,
            generated,
            index,
            context,
            Some(grammar),
        )
    }
    fn from_generated_scan_in_slot(
        parent: &crate::syntax::declaration_scan::DeclarationScan,
        producer: Digest,
        generated: &crate::syntax::declaration_scan::DeclarationScan,
        index: usize,
        context: crate::syntax::token_wire::Key,
        grammar: Option<crate::expand::identity::GrammarSlot>,
    ) -> Result<Self, Diagnostic> {
        let producer = parent
            .producers()
            .iter()
            .find(|row| row.identity == producer)
            .ok_or_else(|| error("scoped module parent producer absent"))?;
        let insertion = parent
            .lexical_scopes()
            .iter()
            .find(|row| row.identity == producer.lexical_scope)
            .ok_or_else(|| error("scoped module producer attachment absent"))?;
        let root = generated
            .lexical_scopes()
            .first()
            .ok_or_else(|| error("scoped module generated root absent"))?;
        // Only Items inherit the producer's insertion kind. Other grammar
        // slots retain the scanner's explicit physical root; nested keys stay
        // owned by that generated tree and cannot acquire a global route.
        if grammar.is_some_and(|actual| actual != producer.grammar) {
            return Err(error("scoped module generated grammar differs"));
        }
        let expected_root = match grammar {
            None | Some(crate::expand::identity::GrammarSlot::Items) => insertion.kind,
            Some(crate::expand::identity::GrammarSlot::Statements) => {
                crate::syntax::declaration_scan::DeclarationScopeKind::Block
            }
            Some(
                crate::expand::identity::GrammarSlot::TraitItems
                | crate::expand::identity::GrammarSlot::ImplItems,
            ) => crate::syntax::declaration_scan::DeclarationScopeKind::AssociatedItems,
            Some(
                crate::expand::identity::GrammarSlot::Expression
                | crate::expand::identity::GrammarSlot::Type
                | crate::expand::identity::GrammarSlot::Pattern,
            ) => crate::syntax::declaration_scan::DeclarationScopeKind::Unattached,
        };
        if generated.scope() != &producer.scope
            || generated.cfg() != parent.cfg()
            || root.kind != expected_root
        {
            return Err(error("scoped module generated insertion differs"));
        }
        let generation = generated
            .generated_origin()
            .ok_or_else(|| error("scoped module generated capture absent"))?;
        let mut result = Self::from_scan(
            generated,
            index,
            DeclarationModuleOrigin::Generated {
                producer: producer.identity,
                generation,
                context,
            },
        )?;
        // An immediate generated parent capture does not retain its own outer
        // insertion chain. Refuse promotion until such proof is represented.
        result.canonical_attachment = parent.generated_origin().is_none()
            && Self::canonical_module_attachment(parent, producer.lexical_scope)?
            && Self::canonical_module_attachment(generated, result.lexical_scope)?;
        if result.lexical_scope == root.identity {
            let mut namespace_scope = producer.scope.clone();
            if insertion.kind == crate::syntax::declaration_scan::DeclarationScopeKind::Module {
                namespace_scope.owner = None;
            }
            if result.scope != namespace_scope {
                return Err(error("scoped module generated root ownership differs"));
            }
            result.lexical_scope = producer.lexical_scope;
        }
        Ok(result)
    }
    /// Pure bounded ancestry proof over retained rows, with no graph mutation.
    fn canonical_module_attachment(
        scan: &crate::syntax::declaration_scan::DeclarationScan,
        mut lexical: crate::expand::scopes::ScopeKey,
    ) -> Result<bool, Diagnostic> {
        let mut work = 0usize;
        for _ in 0..=256 {
            let mut found = None;
            for row in scan.lexical_scopes() {
                work = work
                    .checked_add(128)
                    .filter(|n| *n <= 32 * 1024 * 1024)
                    .ok_or_else(|| error("scoped module ancestry work limit"))?;
                if row.identity == lexical {
                    found = Some(row);
                    break;
                }
            }
            let row = found.ok_or_else(|| error("scoped module lexical ancestry absent"))?;
            if row.kind != crate::syntax::declaration_scan::DeclarationScopeKind::Module {
                return Ok(false);
            }
            let Some(parent) = row.parent else {
                return Ok(true);
            };
            lexical = parent;
        }
        Err(error("scoped module ancestry depth limit"))
    }
    pub(crate) fn demand(&self) -> Digest {
        self.pending.unwrap_or(match self.origin {
            DeclarationModuleOrigin::Original => self.identity,
            DeclarationModuleOrigin::Generated { producer, .. } => producer,
            DeclarationModuleOrigin::Unproven { input } => input,
        })
    }
    /// Pure checked admission/read accounting, including owned index key copies.
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let mut bytes = self
            .name
            .as_str()
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(512))
            .ok_or_else(|| error("scoped module name size overflow"))?;
        for name in self.scope.module_path.iter().chain(match &self.visibility {
            VisibilityScope::Public => [].as_slice(),
            VisibilityScope::Within { module_path, .. } => module_path.as_slice(),
        }) {
            bytes = name
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(128))
                .and_then(|n| n.checked_add(bytes))
                .ok_or_else(|| error("scoped module ownership size overflow"))?;
        }
        for name in &self.route_components {
            bytes = name
                .as_str()
                .len()
                .checked_add(64)
                .and_then(|n| n.checked_add(bytes))
                .ok_or_else(|| error("scoped module route size overflow"))?;
        }
        Ok(bytes)
    }
}
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum OriginBinding {
    Definition(MacroDefinition),
    Named(MacroPath),
    Glob(MacroPath),
}
enum OriginBindingRef<'a> {
    Definition(&'a MacroDefinition),
    Named(&'a MacroPath),
    Glob(&'a MacroPath),
}
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
struct BindingOrigin {
    binding: OriginBinding,
    visibility: VisibilityScope,
    origin: Option<crate::syntax::token_wire::Key>,
}
#[derive(Default, Clone)]
pub(super) struct Bindings {
    pub(super) definitions: BTreeSet<MacroDefinition>,
    pub(super) reexports: BTreeSet<MacroPath>,
    pub(super) pending: BTreeSet<Digest>,
    pub(super) definition_access: BTreeMap<MacroDefinition, BTreeSet<VisibilityScope>>,
    pub(super) reexport_access: BTreeMap<MacroPath, BTreeSet<VisibilityScope>>,
}
#[derive(Clone)]
struct MacroUseBinding {
    provider: Digest,
    selectors: Option<BTreeSet<MacroName>>,
    generated: bool,
}
#[derive(Clone, Default)]
pub struct MacroNamespaces {
    module_binding_rows: BTreeMap<
        (crate::expand::scopes::ScopeKey, MacroName),
        BTreeMap<Digest, DeclarationModuleBinding>,
    >,
    module_binding_identities: BTreeMap<Digest, (crate::expand::scopes::ScopeKey, MacroName)>,
    module_binding_bytes: usize,
    binding_origins: BTreeMap<MacroPath, BTreeSet<BindingOrigin>>,
    origin_bytes: usize,
    pub(super) bindings: BTreeMap<MacroPath, Bindings>,
    pub(super) glob_reexports: BTreeMap<MacroPath, BTreeMap<MacroPath, BTreeSet<VisibilityScope>>>,
    pub(crate) module_paths: super::module_paths::ModulePaths,
    dependencies: BTreeMap<(Digest, MacroName), Digest>,
    compiler_imports: BTreeMap<(Digest, MacroName), Digest>,
    compiler_import_bytes: usize,
    prelude_bytes: usize,
    preludes: BTreeMap<ScopeKey, Digest>,
    compiler_globals: BTreeMap<(Digest, MacroName), MacroDefinition>,
    compiler_global_bytes: usize,
    macro_uses: BTreeMap<Digest, BTreeMap<(Vec<u32>, Digest), MacroUseBinding>>,
    macro_use_count: usize,
    export_pending: BTreeMap<Digest, BTreeSet<Digest>>,
    body_owner_pending: BTreeMap<Digest, Digest>,
    implicit_disabled: BTreeSet<ScopeKey>,
    headers: usize,
    edges: usize,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::new("LRR-E-MACRO-SCOPE", message.into())
}
fn declaration_scope(path: &MacroPath) -> ScopeKey {
    ScopeKey {
        crate_key: path.unit,
        module_path: path.components[..path.components.len() - 1]
            .iter()
            .map(|name| name.as_str().into())
            .collect(),
        owner: None,
    }
}
fn admit(current: usize, ceiling: usize) -> Result<usize, Diagnostic> {
    current
        .checked_add(1)
        .filter(|n| *n <= ceiling)
        .ok_or_else(|| error("macro namespace index limit exceeded"))
}
impl MacroNamespaces {
    /// Pure owned fact admission. The coordinator supplies matched source,
    /// generation/context and insertion-scope facts; this never parses/expands.
    /// Repeated facts are idempotent; changed ownership/origin is a conflict.
    pub fn declare_scoped_module_binding(
        &mut self,
        binding: DeclarationModuleBinding,
    ) -> Result<(), Diagnostic> {
        let amount = binding.metadata_bytes()?;
        if amount > 32 * 1024 * 1024 {
            return Err(error("scoped module binding metadata limit"));
        }
        if binding.scope.module_path.len() > 256 {
            return Err(error("scoped module declaration depth limit"));
        }
        super::module_paths::ModulePath::validate_components(&binding.route_components)?;
        binding
            .visibility
            .validate_for_declaration(&binding.scope)?;
        if crate::syntax::ast_adapter::semantic_module_path(&binding.scope.module_path).as_ref()
            != Some(&binding.scope.module_path)
            || (binding.scope_kind == crate::syntax::declaration_scan::DeclarationScopeKind::Module
                && binding.scope.owner.is_some())
        {
            return Err(error("scoped module declaration ownership differs"));
        }
        if binding.canonical_attachment
            && (binding.scope_kind != crate::syntax::declaration_scan::DeclarationScopeKind::Module
                || binding.scope.owner.is_some()
                || matches!(binding.origin, DeclarationModuleOrigin::Unproven { .. }))
        {
            return Err(error("scoped module canonical attachment differs"));
        }
        let mut expected =
            super::source_macro_routes::module_names(&binding.scope.module_path, 2024)?;
        expected
            .try_reserve_exact(1)
            .map_err(|_| error("scoped module declaration route allocation"))?;
        expected.push(binding.name.clone());
        if binding.route_unit != binding.scope.crate_key || binding.route_components != expected {
            return Err(error("scoped module declaration route differs"));
        }
        let key = (binding.lexical_scope, binding.name.clone());
        if let Some(old_key) = self.module_binding_identities.get(&binding.identity) {
            if old_key == &key
                && self
                    .module_binding_rows
                    .get(&key)
                    .and_then(|rows| rows.get(&binding.identity))
                    == Some(&binding)
            {
                return Ok(());
            }
            return Err(error("scoped module source identity/ownership conflict"));
        }
        if self.module_binding_identities.len() >= 100_000 {
            return Err(error("scoped module binding count limit"));
        }
        let next = self
            .module_binding_bytes
            .checked_add(amount)
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("scoped module binding metadata limit"))?;
        self.module_binding_identities
            .insert(binding.identity, key.clone());
        self.module_binding_rows
            .entry(key)
            .or_default()
            .insert(binding.identity, binding);
        self.module_binding_bytes = next;
        Ok(())
    }
    /// Pure borrowed ownership lookup with caller-supplied checked work budget.
    pub(crate) fn declaration_module_bindings<'a>(
        &'a self,
        lexical: crate::expand::scopes::ScopeKey,
        name: &MacroName,
        reference: &ScopeKey,
        owner: Option<Digest>,
        mut charge: impl FnMut(usize) -> Result<(), Diagnostic>,
    ) -> Result<Vec<&'a DeclarationModuleBinding>, Diagnostic> {
        charge(
            name.as_str()
                .len()
                .checked_add(128)
                .ok_or_else(|| error("scoped module name work overflow"))?,
        )?;
        let Some(rows) = self.module_binding_rows.get(&(lexical, name.clone())) else {
            return Ok(vec![]);
        };
        let mut result = Vec::new();
        for row in rows.values() {
            charge(row.metadata_bytes()?)?;
            if row.scope.crate_key != reference.crate_key
                || row.scope.module_path != reference.module_path
                || row.scope.owner != owner
                || !row.visibility.allows(reference)?
            {
                continue;
            }
            charge(std::mem::size_of::<&DeclarationModuleBinding>())?;
            result
                .try_reserve_exact(1)
                .map_err(|_| error("scoped module binding reference allocation"))?;
            result.push(row);
        }
        Ok(result)
    }
    /// Pure provenance uniqueness check. Two declaration identities may not
    /// silently share one flattened module route when member routing is selected.
    pub(crate) fn declaration_module_route_unique(
        &self,
        binding: &DeclarationModuleBinding,
        mut charge: impl FnMut(usize) -> Result<(), Diagnostic>,
    ) -> Result<bool, Diagnostic> {
        for rows in self.module_binding_rows.values() {
            for row in rows.values() {
                charge(row.metadata_bytes()?)?;
                if row.identity != binding.identity
                    && row.route_unit == binding.route_unit
                    && row.route_components == binding.route_components
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    /// The current attribute cannot introduce a binding before its own lookup.
    /// Preserve every other producer frontier when resolving that attribute.
    pub(crate) fn before_attribute(&self, producer: Digest, header: Digest) -> Self {
        let mut view = self.clone();
        for bindings in view.bindings.values_mut() {
            bindings.pending.remove(&producer);
            bindings.pending.remove(&header);
        }
        for pending in view.export_pending.values_mut() {
            pending.remove(&producer);
            pending.remove(&header);
        }
        view
    }
    fn record_binding_origin(
        &mut self,
        path: &MacroPath,
        binding: OriginBindingRef<'_>,
        visibility: &VisibilityScope,
        origin: Option<crate::syntax::token_wire::Key>,
    ) -> Result<(), Diagnostic> {
        if self.binding_origins.get(path).is_some_and(|rows| {
            rows.iter().any(|row| {
                row.origin == origin
                    && &row.visibility == visibility
                    && match (&row.binding, &binding) {
                        (OriginBinding::Definition(a), OriginBindingRef::Definition(b)) => a == *b,
                        (OriginBinding::Named(a), OriginBindingRef::Named(b))
                        | (OriginBinding::Glob(a), OriginBindingRef::Glob(b)) => a == *b,
                        _ => false,
                    }
            })
        }) {
            return Ok(());
        }
        let path_bytes = |p: &MacroPath| -> Option<usize> {
            p.components().iter().try_fold(128usize, |n, name| {
                name.as_str()
                    .len()
                    .checked_add(std::mem::size_of::<MacroName>())
                    .and_then(|m| n.checked_add(m))
            })
        };
        let target_bytes = match &binding {
            OriginBindingRef::Definition(_) => Some(128),
            OriginBindingRef::Named(p) | OriginBindingRef::Glob(p) => path_bytes(p),
        };
        let visibility_bytes = match visibility {
            VisibilityScope::Public => Some(32),
            VisibilityScope::Within { module_path, .. } => {
                module_path.iter().try_fold(128usize, |n, p| {
                    p.len()
                        .checked_add(std::mem::size_of::<String>())
                        .and_then(|m| n.checked_add(m))
                })
            }
        };
        let bytes = path_bytes(path)
            .and_then(|n| target_bytes.and_then(|m| n.checked_add(m)))
            .and_then(|n| visibility_bytes.and_then(|m| n.checked_add(m)))
            .and_then(|n| self.origin_bytes.checked_add(n))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("macro binding origin metadata limit"))?;
        let binding = match binding {
            OriginBindingRef::Definition(d) => OriginBinding::Definition(d.clone()),
            OriginBindingRef::Named(p) => OriginBinding::Named(p.clone()),
            OriginBindingRef::Glob(p) => OriginBinding::Glob(p.clone()),
        };
        self.binding_origins
            .entry(path.clone())
            .or_default()
            .insert(BindingOrigin {
                binding,
                visibility: visibility.clone(),
                origin,
            });
        self.origin_bytes = bytes;
        Ok(())
    }
    pub(crate) fn binding_origins_from(
        &self,
        path: &MacroPath,
        definition: &MacroDefinition,
        caller: &ScopeKey,
    ) -> Result<Vec<Option<crate::syntax::token_wire::Key>>, Diagnostic> {
        let Some(rows) = self.binding_origins.get(path) else {
            return Err(error("macro binding origin facts absent"));
        };
        let mut strong = BTreeSet::new();
        let mut weak = BTreeSet::new();
        for row in rows {
            let active = match &row.binding {
                OriginBinding::Definition(d) => self
                    .bindings
                    .get(path)
                    .and_then(|b| b.definition_access.get(d))
                    .is_some_and(|ranges| ranges.contains(&row.visibility)),
                OriginBinding::Named(target) => self
                    .bindings
                    .get(path)
                    .and_then(|b| b.reexport_access.get(target))
                    .is_some_and(|ranges| ranges.contains(&row.visibility)),
                OriginBinding::Glob(target) => self
                    .glob_reexports
                    .get(path)
                    .and_then(|targets| targets.get(target))
                    .is_some_and(|ranges| ranges.contains(&row.visibility)),
            };
            if !active {
                continue;
            }
            if !row.visibility.allows(caller)? {
                continue;
            }
            let matches = match &row.binding {
                OriginBinding::Definition(d) => d == definition,
                OriginBinding::Named(target) | OriginBinding::Glob(target) => {
                    matches!(self.lookup_from(target,&declaration_scope(path))?,PathLookup::Defined(ref found) if found==definition)
                }
            };
            if matches {
                if matches!(row.binding, OriginBinding::Glob(_)) {
                    weak.insert(row.origin);
                } else {
                    strong.insert(row.origin);
                }
            }
        }
        let selected = if strong.is_empty() { weak } else { strong };
        if selected.is_empty() {
            return Err(error("selected macro binding origin facts absent"));
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(selected.len())
            .map_err(|_| error("macro binding origin allocation"))?;
        result.extend(selected);
        Ok(result)
    }
    /// Pure borrowed provenance proof. Each inspected row is charged before use;
    /// it never resolves a target, executes expansion, or mutates namespace state.
    pub(crate) fn has_original_strong_binding(
        &self,
        path: &MacroPath,
        caller: &ScopeKey,
        mut charge: impl FnMut(usize) -> Result<(), Diagnostic>,
    ) -> Result<bool, Diagnostic> {
        let Some(rows) = self.binding_origins.get(path) else {
            return Ok(false);
        };
        for row in rows {
            let mut bytes = 160usize;
            if let VisibilityScope::Within { module_path, .. } = &row.visibility {
                bytes = module_path
                    .iter()
                    .try_fold(bytes, |n, name| {
                        n.checked_add(name.len()).and_then(|n| n.checked_add(64))
                    })
                    .ok_or_else(|| error("macro binding provenance work overflow"))?;
            }
            if let OriginBinding::Named(target) | OriginBinding::Glob(target) = &row.binding {
                bytes = target
                    .components()
                    .iter()
                    .try_fold(bytes, |n, name| {
                        n.checked_add(name.as_str().len())
                            .and_then(|n| n.checked_add(64))
                    })
                    .ok_or_else(|| error("macro binding provenance work overflow"))?;
            }
            charge(bytes)?;
            if row.origin.is_some() || !row.visibility.allows(caller)? {
                continue;
            }
            let active = match &row.binding {
                OriginBinding::Definition(definition) => self
                    .bindings
                    .get(path)
                    .and_then(|binding| binding.definition_access.get(definition))
                    .is_some_and(|ranges| ranges.contains(&row.visibility)),
                OriginBinding::Named(target) => self
                    .bindings
                    .get(path)
                    .and_then(|binding| binding.reexport_access.get(target))
                    .is_some_and(|ranges| ranges.contains(&row.visibility)),
                OriginBinding::Glob(_) => false,
            };
            if active {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) fn macro_use_import(
        &mut self,
        row: &super::source_macro_routes::ExternAlias,
        edition: u16,
        generated: bool,
    ) -> Result<(), Diagnostic> {
        let Some(recipe) = &row.macro_use else {
            return Ok(());
        };
        if row.order.is_empty()
            || row.order.len() > 256
            || !row.scope.module_path.is_empty()
            || row.scope.owner.is_some()
        {
            return Err(error("macro-use import scope/order is invalid"));
        }
        let count = admit(self.macro_use_count, 100_000)?;
        let bytes = self
            .prelude_bytes
            .checked_add(recipe.bytes()?)
            .and_then(|n| n.checked_add(row.order.len().checked_mul(4)?))
            .and_then(|n| n.checked_add(256))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("macro-use prelude metadata budget"))?;
        let selectors = match recipe {
            crate::syntax::imports::MacroUseRecipe::All => None,
            crate::syntax::imports::MacroUseRecipe::Only(names) => Some(
                names
                    .iter()
                    .map(|name| MacroName::parse(name, edition))
                    .collect::<Result<BTreeSet<_>, _>>()?,
            ),
        };
        let imports = self.macro_uses.entry(row.scope.crate_key).or_default();
        let key = (row.order.clone(), row.declaration);
        if let Some(old) = imports.get(&key) {
            return if old.provider == row.provider
                && old.selectors == selectors
                && old.generated == generated
            {
                Ok(())
            } else {
                Err(error("macro-use import identity changed"))
            };
        }
        imports.insert(
            key,
            MacroUseBinding {
                provider: row.provider,
                selectors,
                generated,
            },
        );
        self.macro_use_count = count;
        self.prelude_bytes = bytes;
        Ok(())
    }
    pub(crate) fn pending_exports(
        &mut self,
        unit: Digest,
        producers: &BTreeSet<Digest>,
    ) -> Result<(), Diagnostic> {
        let bytes = producers
            .len()
            .checked_mul(32)
            .and_then(|n| n.checked_add(64))
            .and_then(|n| n.checked_add(self.prelude_bytes))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("macro-use export frontier budget"))?;
        self.export_pending
            .entry(unit)
            .or_default()
            .extend(producers.iter().copied());
        self.prelude_bytes = bytes;
        Ok(())
    }
    fn check_generated_macro_use_shadow(
        &self,
        consumer: Digest,
        order: &[u32],
        import: &MacroUseBinding,
    ) -> Result<(), Diagnostic> {
        if !import.generated {
            return Ok(());
        }
        let caller = ScopeKey {
            crate_key: consumer,
            module_path: vec![],
            owner: None,
        };
        let mut names = BTreeSet::new();
        let mut bytes = 0usize;
        for path in self.bindings.keys() {
            let Some(name) = path.components.last() else {
                continue;
            };
            if names.contains(name) {
                continue;
            }
            bytes = bytes
                .checked_add(name.as_str().len())
                .and_then(|n| n.checked_add(128))
                .filter(|n| *n <= 32 * 1024 * 1024)
                .ok_or_else(|| error("macro-use shadow name universe budget"))?;
            names.insert(name.clone());
        }
        for name in names {
            if import
                .selectors
                .as_ref()
                .is_some_and(|set| !set.contains(&name))
            {
                continue;
            }
            let exported = self.lookup_from(
                &MacroPath::new(import.provider, vec![name.clone()])?,
                &caller,
            )?;
            if !matches!(exported, PathLookup::Defined(_)) {
                continue;
            }
            if let Some(imports) = self.macro_uses.get(&consumer) {
                for ((old_order, _), old) in imports {
                    if old_order.as_slice() >= order
                        || old
                            .selectors
                            .as_ref()
                            .is_some_and(|names| !names.contains(&name))
                    {
                        continue;
                    }
                    if matches!(
                        self.lookup_from(
                            &MacroPath::new(old.provider, vec![name.clone()])?,
                            &caller
                        )?,
                        PathLookup::Defined(_)
                    ) {
                        return Err(error(
                            "generated macro-use import may not shadow an existing macro-use import",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn validate_macro_use_selectors(&self) -> Result<(), Diagnostic> {
        for (consumer, imports) in &self.macro_uses {
            let scope = ScopeKey {
                crate_key: *consumer,
                module_path: vec![],
                owner: None,
            };
            for ((order, _), import) in imports {
                self.check_generated_macro_use_shadow(*consumer, order, import)?;
                if let Some(selectors) = &import.selectors {
                    for name in selectors {
                        let found = self.lookup_from(
                            &MacroPath::new(import.provider, vec![name.clone()])?,
                            &scope,
                        )?;
                        if found == PathLookup::Absent
                            && self
                                .export_pending
                                .get(&import.provider)
                                .is_none_or(BTreeSet::is_empty)
                        {
                            return Err(error("selective macro-use names an unexported macro"));
                        }
                        if matches!(
                            found,
                            PathLookup::Inaccessible(_) | PathLookup::Ambiguous(_)
                        ) {
                            return Err(error(
                                "selective macro-use export is inaccessible or ambiguous",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn lookup_macro_use(
        &self,
        name: &MacroName,
        scope: &ScopeKey,
    ) -> Result<PathLookup, Diagnostic> {
        let Some(imports) = self.macro_uses.get(&scope.crate_key) else {
            return Ok(PathLookup::Absent);
        };
        for import in imports.values().rev() {
            if import
                .selectors
                .as_ref()
                .is_some_and(|names| !names.contains(name))
            {
                continue;
            }
            let found =
                self.lookup_from(&MacroPath::new(import.provider, vec![name.clone()])?, scope)?;
            if found != PathLookup::Absent {
                return Ok(found);
            }
            if let Some(producer) = self
                .export_pending
                .get(&import.provider)
                .and_then(|producers| producers.first())
            {
                return Ok(PathLookup::NeedInput(*producer));
            }
        }
        Ok(PathLookup::Absent)
    }
    pub(crate) fn lookup_invocation_segments(
        &self,
        segments: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        scope: &ScopeKey,
        edition: u16,
    ) -> Result<PathLookup, Diagnostic> {
        let found = match self.source_segments_path(segments, absolute, scope, edition)? {
            Some(path) => self.lookup_from(&path, scope)?,
            None => PathLookup::Absent,
        };
        if found != PathLookup::Absent || absolute || segments.len() != 1 {
            return Ok(found);
        }
        let Some(crate::syntax::imports::ImportSegment::Name(name)) = segments.first() else {
            return Ok(found);
        };
        let name = MacroName::parse(name, edition)?;
        if self.implicit_enabled(scope) {
            let imported = self.lookup_macro_use(&name, scope)?;
            if imported != PathLookup::Absent {
                return Ok(imported);
            }
            if let Some(unit) = self.preludes.get(scope) {
                let prelude =
                    self.lookup_from(&MacroPath::new(*unit, vec![name.clone()])?, scope)?;
                if prelude != PathLookup::Absent {
                    return Ok(prelude);
                }
            }
        }
        Ok(self
            .compiler_globals
            .get(&(scope.crate_key, name))
            .map(|definition| PathLookup::Defined(definition.clone()))
            .unwrap_or(PathLookup::Absent))
    }
    pub(crate) fn compiler_global(
        &mut self,
        consumer: Digest,
        name: MacroName,
        definition: MacroDefinition,
    ) -> Result<(), Diagnostic> {
        definition.validate()?;
        let key = (consumer, name);
        if let Some(previous) = self.compiler_globals.get(&key) {
            return if previous == &definition {
                Ok(())
            } else {
                Err(error("conflicting compiler global definition"))
            };
        }
        if self.compiler_globals.len() >= 100_000 {
            return Err(error("compiler global count limit"));
        }
        let bytes = self
            .compiler_global_bytes
            .checked_add(key.1.as_str().len())
            .and_then(|n| n.checked_add(192))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("compiler global metadata limit"))?;
        self.compiler_globals.insert(key, definition);
        self.compiler_global_bytes = bytes;
        Ok(())
    }
    pub(crate) fn defer_body_owner(
        &mut self,
        definition: Digest,
        producer: Digest,
    ) -> Result<(), Diagnostic> {
        if self.body_owner_pending.len() >= 100_000
            && !self.body_owner_pending.contains_key(&definition)
        {
            return Err(error("macro body owner count limit"));
        }
        if self
            .body_owner_pending
            .get(&definition)
            .is_some_and(|old| *old != producer)
        {
            return Err(error("macro body owner identity conflict"));
        }
        self.body_owner_pending.insert(definition, producer);
        Ok(())
    }
    pub(crate) fn pending_body_owner(&self, definition: Digest) -> Option<Digest> {
        self.body_owner_pending.get(&definition).copied()
    }
    pub(crate) fn set_prelude(&mut self, scope: ScopeKey, unit: Digest) -> Result<(), Diagnostic> {
        if scope.owner.is_some()
            || self.preludes.len() >= 100_000
            || scope.module_path.len() > 256
            || scope.heap_bytes() > 65_536
        {
            return Err(error("macro prelude scope budget"));
        }
        let bytes = self
            .prelude_bytes
            .checked_add(scope.heap_bytes())
            .and_then(|n| n.checked_add(64))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("macro prelude aggregate metadata budget"))?;
        if self.preludes.contains_key(&scope) {
            return Err(error("duplicate macro prelude scope"));
        }
        self.preludes.insert(scope, unit);
        self.prelude_bytes = bytes;
        Ok(())
    }
    pub(crate) fn disable_implicit(&mut self, scope: ScopeKey) -> Result<(), Diagnostic> {
        if scope.owner.is_some()
            || self.implicit_disabled.len() >= 100_000
            || scope.module_path.len() > 256
            || scope.heap_bytes() > 65_536
        {
            return Err(error("disabled macro prelude scope budget"));
        }
        if self.implicit_disabled.contains(&scope) {
            return Ok(());
        }
        let bytes = self
            .prelude_bytes
            .checked_add(scope.heap_bytes())
            .and_then(|n| n.checked_add(64))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("disabled prelude aggregate metadata budget"))?;
        self.prelude_bytes = bytes;
        self.implicit_disabled.insert(scope);
        Ok(())
    }
    fn implicit_enabled(&self, scope: &ScopeKey) -> bool {
        !self.implicit_disabled.iter().any(|disabled| {
            disabled.crate_key == scope.crate_key
                && scope.module_path.starts_with(&disabled.module_path)
        })
    }
    pub(crate) fn lookup_invocation_path(
        &self,
        call: &crate::syntax::macro_calls::SourceMacroCall,
        scope: &ScopeKey,
    ) -> Result<PathLookup, Diagnostic> {
        let found = match self.source_call_path(call, scope)? {
            Some(path) => self.lookup_from(&path, scope)?,
            None => PathLookup::Absent,
        };
        if found != PathLookup::Absent || call.absolute() || call.segments().len() != 1 {
            return Ok(found);
        }
        self.lookup_invocation_segments(
            call.segments(),
            call.absolute(),
            scope,
            call.key().profile.edition,
        )
    }
    pub(crate) fn dependency_bindings(
        &self,
    ) -> impl Iterator<Item = (&(Digest, MacroName), &Digest)> {
        self.dependencies.iter()
    }

    pub fn lookup_module_from(
        &self,
        unit: Digest,
        components: &[MacroName],
        caller: &ScopeKey,
    ) -> Result<ModuleLookup, Diagnostic> {
        use super::module_paths::{ModulePath, ModuleResolution};
        ModulePath::validate_components(components)?;
        let path = ModulePath {
            unit,
            names: components.to_vec(),
        };
        Ok(
            match self.module_paths.resolve_module(&path, Some(caller))? {
                ModuleResolution::Known { path, visibility } => ModuleLookup::Defined {
                    unit: path.unit,
                    components: path.names,
                    visibility,
                },
                ModuleResolution::Ambiguous(paths) => {
                    ModuleLookup::Ambiguous(paths.into_iter().map(|p| (p.unit, p.names)).collect())
                }
                ModuleResolution::Absent => ModuleLookup::Absent,
                ModuleResolution::Denied(path) => ModuleLookup::Inaccessible {
                    unit: path.unit,
                    components: path.names,
                },
            },
        )
    }
    pub fn define(
        &mut self,
        path: MacroPath,
        definition: MacroDefinition,
    ) -> Result<(), Diagnostic> {
        self.define_visible(path, definition, VisibilityScope::Public)
    }
    pub fn define_visible(
        &mut self,
        path: MacroPath,
        definition: MacroDefinition,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.define_visible_origin(path, definition, visibility, None)
    }
    pub(crate) fn define_visible_origin(
        &mut self,
        path: MacroPath,
        definition: MacroDefinition,
        visibility: VisibilityScope,
        origin: Option<crate::syntax::token_wire::Key>,
    ) -> Result<(), Diagnostic> {
        definition.validate()?;
        visibility.validate_for_declaration(&declaration_scope(&path))?;
        let exists = self
            .bindings
            .get(&path)
            .and_then(|b| b.definition_access.get(&definition))
            .is_some_and(|v| v.contains(&visibility));
        let count = if exists {
            self.headers
        } else {
            admit(self.headers, 1_000_000)?
        };
        self.record_binding_origin(
            &path,
            OriginBindingRef::Definition(&definition),
            &visibility,
            origin,
        )?;
        if exists {
            return Ok(());
        }
        let binding = self.bindings.entry(path).or_default();
        binding.definitions.insert(definition.clone());
        binding
            .definition_access
            .entry(definition)
            .or_default()
            .insert(visibility);
        self.headers = count;
        Ok(())
    }
    pub fn reexport(&mut self, path: MacroPath, target: MacroPath) -> Result<(), Diagnostic> {
        self.reexport_visible(path, target, VisibilityScope::Public)
    }
    pub fn reexport_visible(
        &mut self,
        path: MacroPath,
        target: MacroPath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.reexport_visible_origin(path, target, visibility, None)
    }
    pub(crate) fn reexport_visible_origin(
        &mut self,
        path: MacroPath,
        target: MacroPath,
        visibility: VisibilityScope,
        origin: Option<crate::syntax::token_wire::Key>,
    ) -> Result<(), Diagnostic> {
        visibility.validate_for_declaration(&declaration_scope(&path))?;
        let exists = self
            .bindings
            .get(&path)
            .and_then(|b| b.reexport_access.get(&target))
            .is_some_and(|v| v.contains(&visibility));
        let count = if exists {
            self.edges
        } else {
            admit(self.edges, 100_000)?
        };
        self.record_binding_origin(&path, OriginBindingRef::Named(&target), &visibility, origin)?;
        if exists {
            return Ok(());
        }
        let binding = self.bindings.entry(path).or_default();
        binding.reexports.insert(target.clone());
        binding
            .reexport_access
            .entry(target)
            .or_default()
            .insert(visibility);
        self.edges = count;
        Ok(())
    }
    pub(crate) fn glob_member(
        &mut self,
        path: MacroPath,
        target: MacroPath,
        visibility: VisibilityScope,
    ) -> Result<(), Diagnostic> {
        self.glob_member_origin(path, target, visibility, None)
    }
    pub(crate) fn glob_member_origin(
        &mut self,
        path: MacroPath,
        target: MacroPath,
        visibility: VisibilityScope,
        origin: Option<crate::syntax::token_wire::Key>,
    ) -> Result<(), Diagnostic> {
        visibility.validate_for_declaration(&declaration_scope(&path))?;
        let exists = self
            .glob_reexports
            .get(&path)
            .and_then(|targets| targets.get(&target))
            .is_some_and(|ranges| ranges.contains(&visibility));
        let count = if exists {
            self.headers
        } else {
            admit(self.headers, 1_000_000)?
        };
        self.record_binding_origin(&path, OriginBindingRef::Glob(&target), &visibility, origin)?;
        if exists {
            return Ok(());
        }
        self.glob_reexports
            .entry(path)
            .or_default()
            .entry(target)
            .or_default()
            .insert(visibility);
        self.headers = count;
        Ok(())
    }
    pub(crate) fn remove_glob_member(
        &mut self,
        path: &MacroPath,
        target: &MacroPath,
        visibility: &VisibilityScope,
    ) -> Result<(), Diagnostic> {
        if let Some(targets) = self.glob_reexports.get_mut(path) {
            if let Some(ranges) = targets.get_mut(target) {
                if ranges.contains(visibility) {
                    let headers = self
                        .headers
                        .checked_sub(1)
                        .ok_or_else(|| error("glob header quota accounting mismatch"))?;
                    ranges.remove(visibility);
                    self.headers = headers;
                }
                if ranges.is_empty() {
                    targets.remove(target);
                }
            }
            if targets.is_empty() {
                self.glob_reexports.remove(path);
            }
        }
        Ok(())
    }
    pub(crate) fn has_import_edge(&self, path: &MacroPath, target: &MacroPath) -> bool {
        self.bindings
            .get(path)
            .is_some_and(|b| b.reexports.contains(target))
            || self
                .glob_reexports
                .get(path)
                .is_some_and(|targets| targets.contains_key(target))
    }
    pub(crate) fn member_names(&self) -> BTreeSet<MacroName> {
        self.bindings
            .keys()
            .filter_map(|path| path.components.last().cloned())
            .collect()
    }
    /// A finite candidate subset only. Every result still needs ordinary
    /// lookup/visibility/frontier checks. None requests the unchanged global
    /// universe, including any original diagnostic on an unusual scope.
    pub(super) fn scoped_member_names(
        &self,
        scope: &ScopeKey,
        edition: u16,
    ) -> Option<BTreeSet<MacroName>> {
        if scope.owner.is_some() || !matches!(edition, 2015 | 2018 | 2021 | 2024) {
            return None;
        }
        let prefix = super::source_macro_routes::module_names(&scope.module_path, edition).ok()?;
        if prefix.len() >= 256 {
            return None;
        }
        // A rewritten, ambiguous, denied or missing module may produce
        // non-Absent outcomes for spellings with no local destination row.
        match self
            .lookup_module_from(scope.crate_key, &prefix, scope)
            .ok()?
        {
            ModuleLookup::Defined {
                unit, components, ..
            } if unit == scope.crate_key && components == prefix => {}
            _ => return None,
        }
        // This transient map lower bound is not published or used as a lookup
        // path. Only here may its component prefix be empty for a crate root.
        let start = MacroPath {
            unit: scope.crate_key,
            components: prefix.clone(),
        };
        let inside = |path: &&MacroPath| {
            path.unit == scope.crate_key && path.components.starts_with(&prefix)
        };
        let mut names = BTreeSet::new();
        for path in self
            .bindings
            .range(start.clone()..)
            .map(|(path, _)| path)
            .take_while(inside)
        {
            if path.components.len() == prefix.len() + 1 {
                names.insert(path.components.last()?.clone());
            }
        }
        for (path, targets) in self.glob_reexports.range(start..).take_while(|(path, _)| {
            path.unit == scope.crate_key && path.components.starts_with(&prefix)
        }) {
            if path.components.len() != prefix.len() + 1 {
                continue;
            }
            let name = path.components.last()?;
            if !names.contains(name) {
                // The old universe contains binding spellings, not arbitrary
                // weak destinations. A real same-spelling target row proves
                // inclusion without an all-global scan. Complex/cyclic/renamed
                // manual weak edges conservatively keep old enumeration.
                if !targets.keys().any(|target| {
                    target.components.last() == Some(name) && self.bindings.contains_key(target)
                }) {
                    return None;
                }
                names.insert(name.clone());
            }
        }
        Some(names)
    }
    pub fn pending(&mut self, path: MacroPath, invocation: Digest) -> Result<(), Diagnostic> {
        if self
            .bindings
            .get(&path)
            .is_some_and(|b| b.pending.contains(&invocation))
        {
            return Ok(());
        }
        let count = admit(self.headers, 1_000_000)?;
        self.bindings
            .entry(path)
            .or_default()
            .pending
            .insert(invocation);
        self.headers = count;
        Ok(())
    }
    pub fn dependency(
        &mut self,
        consumer: Digest,
        alias: MacroName,
        provider: Digest,
    ) -> Result<(), Diagnostic> {
        let key = (consumer, alias);
        if let Some(existing) = self.dependencies.get(&key) {
            return if *existing == provider {
                Ok(())
            } else {
                Err(error("conflicting dependency rename"))
            };
        }
        let count = admit(self.edges, 100_000)?;
        self.dependencies.insert(key, provider);
        self.edges = count;
        Ok(())
    }
    /// Admitted only by the private compiler catalog after checking the actual
    /// selected source policy. This is not a Cargo dependency/prelude grant.
    pub(super) fn compiler_injected_import(
        &mut self,
        consumer: Digest,
        alias: MacroName,
        provider: Digest,
    ) -> Result<(), Diagnostic> {
        if !matches!(alias.as_str(), "core" | "std") {
            return Err(error("compiler-injected import is not a fixed facade"));
        }
        let key = (consumer, alias);
        // A differently owned captured dependency cannot borrow compiler status.
        if self.dependencies.get(&key) != Some(&provider) {
            return Ok(());
        }
        if let Some(existing) = self.compiler_imports.get(&key) {
            return if *existing == provider {
                Ok(())
            } else {
                Err(error("conflicting compiler-injected import owner"))
            };
        }
        if self.compiler_imports.len() >= 100_000 {
            return Err(error("compiler-injected import count limit"));
        }
        let bytes = self
            .compiler_import_bytes
            .checked_add(128)
            .and_then(|n| n.checked_add(key.1.as_str().len()))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| error("compiler-injected import metadata limit"))?;
        self.compiler_imports.insert(key, provider);
        self.compiler_import_bytes = bytes;
        Ok(())
    }
    pub(super) fn encode_compiler_imports(&self, e: &mut StableEncoder) {
        let mut state = StableEncoder::new("lrr/v1/compiler-injected-imports");
        state.map(&self.compiler_imports, |e, (consumer, name), provider| {
            e.digest(*consumer);
            e.string(name.as_str());
            e.digest(*provider);
        });
        e.digest(state.finish());
    }
    /// Commit one producer's complete result only after validating every
    /// definition. An empty result completes its demand; it is not an error.
    pub fn publish(
        &mut self,
        path: MacroPath,
        invocation: Digest,
        definitions: Vec<MacroDefinition>,
    ) -> Result<(), Diagnostic> {
        if !self
            .bindings
            .get(&path)
            .is_some_and(|b| b.pending.contains(&invocation))
        {
            return Err(error("stale or missing macro namespace producer"));
        }
        let count = self
            .headers
            .checked_add(definitions.len())
            .filter(|n| *n <= 1_000_000)
            .ok_or_else(|| error("macro namespace index limit exceeded"))?;
        for definition in &definitions {
            definition.validate()?;
        }
        let bindings = self
            .bindings
            .get_mut(&path)
            .expect("validated producer path");
        for definition in definitions {
            bindings.definitions.insert(definition.clone());
            bindings
                .definition_access
                .entry(definition)
                .or_default()
                .insert(VisibilityScope::Public);
        }
        bindings.pending.remove(&invocation);
        self.headers = count;
        Ok(())
    }
    pub(crate) fn source_call_path(
        &self,
        call: &crate::syntax::macro_calls::SourceMacroCall,
        scope: &ScopeKey,
    ) -> Result<Option<MacroPath>, Diagnostic> {
        self.source_segments_path(
            call.segments(),
            call.absolute(),
            scope,
            call.key().profile.edition,
        )
    }
    pub(crate) fn source_segments_path(
        &self,
        parts: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        scope: &ScopeKey,
        edition: u16,
    ) -> Result<Option<MacroPath>, Diagnostic> {
        let Some((unit, names)) =
            self.source_segments_route_optional(parts, absolute, scope, edition, false, false)?
        else {
            return Ok(None);
        };
        Ok(Some(MacroPath::new(unit, names)?))
    }
    fn import_reference_scope(
        parts: &[crate::syntax::imports::ImportSegment],
        scope: &ScopeKey,
        edition: u16,
    ) -> ScopeKey {
        let mut reference = scope.clone();
        if edition == 2015
            && matches!(
                parts.first(),
                Some(crate::syntax::imports::ImportSegment::Name(_))
            )
        {
            reference.module_path.clear();
        }
        reference
    }
    pub(crate) fn import_segments_path(
        &self,
        parts: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        scope: &ScopeKey,
        edition: u16,
    ) -> Result<Option<MacroPath>, Diagnostic> {
        let reference = Self::import_reference_scope(parts, scope, edition);
        let Some((unit, names)) =
            self.source_segments_route_optional(parts, absolute, &reference, edition, false, true)?
        else {
            return Ok(None);
        };
        Ok(Some(MacroPath::new(unit, names)?))
    }
    pub(crate) fn import_module_route(
        &self,
        parts: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        scope: &ScopeKey,
        edition: u16,
    ) -> Result<Option<super::module_paths::ModulePath>, Diagnostic> {
        let reference = Self::import_reference_scope(parts, scope, edition);
        let Some((unit, names)) =
            self.source_segments_route_optional(parts, absolute, &reference, edition, true, true)?
        else {
            return Ok(None);
        };
        super::module_paths::ModulePath::validate_components(&names)?;
        Ok(Some(super::module_paths::ModulePath { unit, names }))
    }
    pub(crate) fn source_module_route(
        &self,
        parts: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        scope: &ScopeKey,
        edition: u16,
    ) -> Result<Option<super::module_paths::ModulePath>, Diagnostic> {
        let Some((unit, names)) =
            self.source_segments_route_optional(parts, absolute, scope, edition, true, false)?
        else {
            return Ok(None);
        };
        super::module_paths::ModulePath::validate_components(&names)?;
        Ok(Some(super::module_paths::ModulePath { unit, names }))
    }
    fn source_segments_route_optional(
        &self,
        parts: &[crate::syntax::imports::ImportSegment],
        absolute: bool,
        scope: &ScopeKey,
        edition: u16,
        module: bool,
        import: bool,
    ) -> Result<Option<(Digest, Vec<MacroName>)>, Diagnostic> {
        use crate::syntax::imports::ImportSegment;
        let mut unit = scope.crate_key;
        let mut names = super::source_macro_routes::module_names(&scope.module_path, edition)?;
        let mut segments = parts.iter().peekable();
        if absolute && edition >= 2018 {
            let Some(ImportSegment::Name(first)) = segments.next() else {
                return Err(error("absolute macro path requires a dependency alias"));
            };
            let alias = MacroName::parse(first, edition)?;
            if !self.implicit_enabled(scope) {
                return Ok(None);
            }
            let Some(provider) = self.dependencies.get(&(unit, alias)) else {
                return Ok(None);
            };
            unit = *provider;
            names.clear();
        } else {
            if absolute {
                names.clear();
            }
            match segments.peek() {
                Some(ImportSegment::Crate) => {
                    names.clear();
                    segments.next();
                }
                Some(ImportSegment::SelfModule) => {
                    segments.next();
                }
                Some(ImportSegment::Name(first)) if parts.len() > 1 || module => {
                    let first = MacroName::parse(first, edition)?;
                    let mut local = names.clone();
                    local.push(first.clone());
                    if matches!(
                        self.lookup_module_from(unit, &local, scope)?,
                        ModuleLookup::Absent
                    ) && ((!import && !absolute)
                        || edition >= 2018
                        || self.compiler_imports.contains_key(&(unit, first.clone())))
                        && self.implicit_enabled(scope)
                        && let Some(provider) = self.dependencies.get(&(unit, first))
                    {
                        unit = *provider;
                        names.clear();
                        segments.next();
                    }
                }
                _ => {}
            }
        }
        let mut named = false;
        for segment in segments {
            match segment {
                ImportSegment::Super if !named => {
                    names
                        .pop()
                        .ok_or_else(|| error("macro path escapes crate root"))?;
                }
                ImportSegment::Name(name) => {
                    named = true;
                    if names.len() >= 256 {
                        return Err(error("macro invocation resolved path depth exceeded"));
                    }
                    names
                        .try_reserve(1)
                        .map_err(|_| error("macro invocation route allocation failed"))?;
                    names.push(MacroName::parse(name, edition)?);
                }
                _ => return Err(error("invalid macro invocation path qualifier")),
            }
        }
        Ok(Some((unit, names)))
    }
    pub fn dependency_path(
        &self,
        consumer: Digest,
        alias: &MacroName,
        tail: Vec<MacroName>,
    ) -> Result<MacroPath, Diagnostic> {
        let unit = self
            .dependencies
            .get(&(consumer, alias.clone()))
            .ok_or_else(|| error("unknown dependency alias"))?;
        MacroPath::new(*unit, tail)
    }
    /// Also usable as the unit route for ordinary `$crate::helper` item lookup;
    /// the later source resolver determines the helper's item namespace.
    pub fn dollar_crate(
        definition: &MacroDefinition,
        tail: Vec<MacroName>,
    ) -> Result<MacroPath, Diagnostic> {
        definition.validate()?;
        MacroPath::new(definition.defining_unit, tail)
    }
    /// Check binding visibility at every edge from that import's declaration
    /// scope. Reexports cannot widen a private imported binding.
    /// Module-prefix visibility and namespace completeness are separate facts.
    pub fn lookup_from(
        &self,
        path: &MacroPath,
        caller: &ScopeKey,
    ) -> Result<PathLookup, Diagnostic> {
        if !self.glob_reexports.is_empty() {
            return super::glob_lookup::lookup(self, path, Some(caller));
        }
        let caller = ScopeKey {
            crate_key: caller.crate_key,
            module_path: crate::syntax::ast_adapter::semantic_module_path(&caller.module_path)
                .ok_or_else(|| error("invalid macro access caller"))?,
            owner: None,
        };
        let mut stack = vec![(path.clone(), caller)];
        let mut visited = BTreeSet::new();
        let mut definitions = BTreeSet::new();
        let mut denied = BTreeSet::new();
        let mut pending = BTreeSet::new();
        while let Some((path, caller)) = stack.pop() {
            let path = match self.module_paths.resolve(&path, Some(&caller))? {
                super::module_paths::Resolution::Known(path) => path,
                super::module_paths::Resolution::Absent => continue,
                super::module_paths::Resolution::Denied(path) => {
                    denied.insert(path);
                    continue;
                }
            };
            if !visited.insert((path.clone(), caller.clone())) {
                continue;
            }
            if visited.len() > 1_000_000 {
                return Err(error("macro access walk limit exceeded"));
            }
            let Some(bindings) = self.bindings.get(&path) else {
                continue;
            };
            pending.extend(bindings.pending.iter().copied());
            for definition in &bindings.definitions {
                let access = bindings
                    .definition_access
                    .get(definition)
                    .ok_or_else(|| error("definition visibility facts missing"))?;
                let mut allowed = false;
                for range in access {
                    allowed |= range.allows(&caller)?;
                }
                if allowed {
                    definitions.insert(definition.clone());
                } else {
                    denied.insert(path.clone());
                }
            }
            for target in &bindings.reexports {
                let access = bindings
                    .reexport_access
                    .get(target)
                    .ok_or_else(|| error("import visibility facts missing"))?;
                let mut allowed = false;
                for range in access {
                    allowed |= range.allows(&caller)?;
                }
                if !allowed {
                    denied.insert(path.clone());
                    continue;
                }
                let origin = declaration_scope(&path);
                let checked_target = match self.module_paths.resolve(target, Some(&origin))? {
                    super::module_paths::Resolution::Known(target) => target,
                    super::module_paths::Resolution::Absent => continue,
                    super::module_paths::Resolution::Denied(witness) => {
                        denied.insert(witness);
                        continue;
                    }
                };
                if let Some(target_binding) = self.bindings.get(&checked_target) {
                    pending.extend(target_binding.pending.iter().copied());
                    let target_ranges = || {
                        target_binding
                            .definition_access
                            .values()
                            .chain(target_binding.reexport_access.values())
                            .flat_map(|ranges| ranges.iter())
                    };
                    let mut valid_width = target_ranges().next().is_none();
                    for export_range in access {
                        if export_range.allows(&caller)?
                            && target_ranges()
                                .any(|target_range| target_range.permits_reexport_as(export_range))
                        {
                            valid_width = true;
                        }
                    }
                    if !valid_width {
                        denied.insert(checked_target);
                        continue;
                    }
                }
                let origin = ScopeKey {
                    crate_key: path.unit,
                    module_path: path.components[..path.components.len() - 1]
                        .iter()
                        .map(|n| n.as_str().to_owned())
                        .collect(),
                    owner: None,
                };
                stack
                    .try_reserve(1)
                    .map_err(|_| error("macro access worklist allocation failed"))?;
                stack.push((target.clone(), origin));
            }
        }
        if let Some(input) = pending.first() {
            return Ok(PathLookup::NeedInput(*input));
        }
        if !denied.is_empty() {
            return Ok(PathLookup::Inaccessible(denied.into_iter().collect()));
        }
        Ok(match definitions.len() {
            0 => PathLookup::Absent,
            1 => PathLookup::Defined(definitions.pop_first().expect("one definition")),
            _ => PathLookup::Ambiguous(definitions.into_iter().collect()),
        })
    }
    /// Pure source facts. The source index must register every unresolved
    /// producer frontier before lookup; M010/M014 seals enforce that boundary.
    /// This walk does not memoize absence across membership changes.
    pub fn lookup(&self, path: &MacroPath) -> Result<PathLookup, Diagnostic> {
        if !self.glob_reexports.is_empty() {
            return super::glob_lookup::lookup(self, path, None);
        }
        // Iterative graph walk avoids host recursion on long import chains.
        let mut stack = vec![(path.clone(), false)];
        let mut active = BTreeSet::new();
        let mut visited = BTreeSet::new();
        let mut definitions = BTreeSet::new();
        let mut pending = BTreeSet::new();
        while let Some((path, exiting)) = stack.pop() {
            let path = match self.module_paths.resolve(&path, None)? {
                super::module_paths::Resolution::Known(path) => path,
                super::module_paths::Resolution::Absent => continue,
                super::module_paths::Resolution::Denied(_) => {
                    return Err(error(
                        "candidate module resolution unexpectedly checked privacy",
                    ));
                }
            };
            if exiting {
                active.remove(&path);
                visited.insert(path);
                continue;
            }
            if visited.contains(&path) {
                continue;
            }
            if !active.insert(path.clone()) {
                continue;
            }
            stack.push((path.clone(), true));
            let Some(bindings) = self.bindings.get(&path) else {
                continue;
            };
            definitions.extend(bindings.definitions.iter().cloned());
            pending.extend(bindings.pending.iter().copied());
            stack
                .try_reserve(bindings.reexports.len())
                .map_err(|_| error("macro lookup worklist allocation failed"))?;
            for target in bindings.reexports.iter().rev() {
                stack.push((target.clone(), false));
            }
        }
        if let Some(invocation) = pending.first() {
            return Ok(PathLookup::NeedInput(*invocation));
        }
        // A producer-complete cycle without an origin is empty (revision4
        // Section50.2), not a resolution error. Pending producers above prevent
        // premature absence; publication causes the next pure walk to retry.
        Ok(match definitions.len() {
            0 => PathLookup::Absent,
            1 => PathLookup::Defined(definitions.pop_first().expect("one definition")),
            _ => PathLookup::Ambiguous(definitions.into_iter().collect()),
        })
    }
}

#[cfg(test)]
mod import_accounting_tests {
    use super::*;
    #[test]
    fn binding_origin_priority_and_failed_admission_preserve_visible_edges() {
        let unit = Digest([87; 32]);
        let path =
            |name| MacroPath::new(unit, vec![MacroName::parse(name, 2024).unwrap()]).unwrap();
        let target = path("original");
        let alias = path("alias");
        let weak = path("weak");
        let definition = MacroDefinition {
            identity: Digest([89; 32]),
            defining_unit: unit,
            definition_edition: 2024,
        };
        let caller = ScopeKey {
            crate_key: unit,
            module_path: vec![],
            owner: None,
        };
        let first = Some(crate::syntax::token_wire::Key([1; 32]));
        let second = Some(crate::syntax::token_wire::Key([2; 32]));
        let mut names = MacroNamespaces::default();
        names.define(target.clone(), definition.clone()).unwrap();
        names
            .reexport_visible_origin(
                alias.clone(),
                target.clone(),
                VisibilityScope::Public,
                first,
            )
            .unwrap();
        names
            .glob_member_origin(
                alias.clone(),
                target.clone(),
                VisibilityScope::Public,
                second,
            )
            .unwrap();
        assert_eq!(
            names.lookup_from(&alias, &caller).unwrap(),
            PathLookup::Defined(definition.clone())
        );
        assert_eq!(
            names
                .binding_origins_from(&alias, &definition, &caller)
                .unwrap(),
            vec![first]
        );
        names
            .glob_member_origin(
                weak.clone(),
                target.clone(),
                VisibilityScope::Public,
                second,
            )
            .unwrap();
        assert_eq!(
            names
                .binding_origins_from(&weak, &definition, &caller)
                .unwrap(),
            vec![second]
        );
        names
            .remove_glob_member(&weak, &target, &VisibilityScope::Public)
            .unwrap();
        assert_eq!(
            names.lookup_from(&weak, &caller).unwrap(),
            PathLookup::Absent
        );
        assert_eq!(
            names
                .binding_origins_from(&weak, &definition, &caller)
                .unwrap_err()
                .code,
            "LRR-E-MACRO-SCOPE"
        );
        names.origin_bytes = 32 * 1024 * 1024 - 1;
        let edges = names.edges;
        let rows = names.binding_origins.len();
        let failed = path("failed");
        assert_eq!(
            names
                .reexport_visible_origin(failed.clone(), target, VisibilityScope::Public, second)
                .unwrap_err()
                .code,
            "LRR-E-MACRO-SCOPE"
        );
        assert_eq!(
            names.lookup_from(&failed, &caller).unwrap(),
            PathLookup::Absent
        );
        assert_eq!(names.edges, edges);
        assert_eq!(names.binding_origins.len(), rows);
        assert_eq!(
            names
                .binding_origins_from(&alias, &definition, &caller)
                .unwrap(),
            vec![first]
        );
    }
    #[test]
    fn removing_a_glob_restores_its_counter_and_retains_named_edges() {
        let unit = Digest([87; 32]);
        let path = MacroPath::new(unit, vec![MacroName::parse("alias", 2024).unwrap()]).unwrap();
        let target =
            MacroPath::new(unit, vec![MacroName::parse("original", 2024).unwrap()]).unwrap();
        let mut names = MacroNamespaces::default();
        names
            .reexport_visible(path.clone(), target.clone(), VisibilityScope::Public)
            .unwrap();
        let headers = names.headers;
        let edges = names.edges;
        names
            .glob_member(path.clone(), target.clone(), VisibilityScope::Public)
            .unwrap();
        assert_eq!(names.headers, headers + 1);
        for _ in 0..2 {
            names
                .remove_glob_member(&path, &target, &VisibilityScope::Public)
                .unwrap();
            assert_eq!(names.headers, headers);
            assert_eq!(names.edges, edges);
            assert!(names.has_import_edge(&path, &target));
            assert!(names.glob_reexports.is_empty());
        }
    }
}

#[cfg(test)]
mod original_strong_binding_tests {
    use super::*;
    fn setup() -> (MacroNamespaces, MacroPath, ScopeKey, MacroDefinition) {
        let unit = Digest([221; 32]);
        let path = MacroPath::new(unit, vec![MacroName::parse("m", 2024).unwrap()]).unwrap();
        let caller = ScopeKey {
            crate_key: unit,
            module_path: vec![],
            owner: None,
        };
        let definition = MacroDefinition {
            identity: Digest([222; 32]),
            defining_unit: unit,
            definition_edition: 2024,
        };
        (MacroNamespaces::default(), path, caller, definition)
    }
    #[test]
    fn generated_and_weak_origins_do_not_prove_an_original_strong_binding() {
        let (mut names, path, caller, definition) = setup();
        names
            .define_visible_origin(
                path.clone(),
                definition.clone(),
                VisibilityScope::Public,
                Some(crate::syntax::token_wire::Key([223; 32])),
            )
            .unwrap();
        assert!(
            !names
                .has_original_strong_binding(&path, &caller, |_| Ok(()))
                .unwrap()
        );
        let weak =
            MacroPath::new(path.unit, vec![MacroName::parse("weak", 2024).unwrap()]).unwrap();
        names
            .glob_member_origin(weak.clone(), path.clone(), VisibilityScope::Public, None)
            .unwrap();
        assert!(
            !names
                .has_original_strong_binding(&weak, &caller, |_| Ok(()))
                .unwrap()
        );
        names
            .define_visible_origin(path.clone(), definition, VisibilityScope::Public, None)
            .unwrap();
        assert!(
            names
                .has_original_strong_binding(&path, &caller, |_| Ok(()))
                .unwrap()
        );
    }
    #[test]
    fn original_named_origin_must_be_active_and_visible() {
        let (mut names, path, caller, definition) = setup();
        let target =
            MacroPath::new(path.unit, vec![MacroName::parse("target", 2024).unwrap()]).unwrap();
        names.define(target.clone(), definition).unwrap();
        names.reexport(path.clone(), target.clone()).unwrap();
        assert!(
            names
                .has_original_strong_binding(&path, &caller, |_| Ok(()))
                .unwrap()
        );
        names
            .bindings
            .get_mut(&path)
            .unwrap()
            .reexport_access
            .remove(&target);
        assert!(
            !names
                .has_original_strong_binding(&path, &caller, |_| Ok(()))
                .unwrap()
        );
        let inaccessible = ScopeKey {
            crate_key: Digest([224; 32]),
            module_path: vec![],
            owner: None,
        };
        names
            .reexport_visible(
                path.clone(),
                target,
                VisibilityScope::Within {
                    unit: path.unit,
                    module_path: vec![],
                },
            )
            .unwrap();
        assert!(
            !names
                .has_original_strong_binding(&path, &inaccessible, |_| Ok(()))
                .unwrap()
        );
    }
    #[test]
    fn failed_provenance_charge_is_typed_and_does_not_mutate_facts() {
        let (mut names, path, caller, definition) = setup();
        names.define(path.clone(), definition).unwrap();
        let before = names.binding_origins.get(&path).unwrap().clone();
        let diagnostic = error("controlled provenance work limit");
        let failure = names
            .has_original_strong_binding(&path, &caller, |_| Err(diagnostic.clone()))
            .unwrap_err();
        assert_eq!(failure.code, diagnostic.code);
        assert_eq!(failure.message, diagnostic.message);
        assert!(names.binding_origins.get(&path).unwrap() == &before);
        assert!(
            names
                .has_original_strong_binding(&path, &caller, |_| Ok(()))
                .unwrap()
        );
    }
}

#[cfg(test)]
mod scoped_module_binding_tests {
    use super::*;
    use crate::{
        core::profile::ProfileIdentity,
        source::snapshot::{CfgSet, SourceBlob, SourcePath},
        syntax::{
            declaration_scan::{
                DeclarationScanLimits, DeclarationScopeKind, generated_declaration_fixture,
            },
            parse::ParsedFile,
        },
    };
    fn binding() -> DeclarationModuleBinding {
        let unit = Digest([240; 32]);
        DeclarationModuleBinding {
            identity: Digest([241; 32]),
            name: MacroName::parse("module", 2024).unwrap(),
            scope: ScopeKey {
                crate_key: unit,
                module_path: vec![],
                owner: None,
            },
            lexical_scope: crate::expand::scopes::ScopeKey::root(unit),
            scope_kind: DeclarationScopeKind::Module,
            canonical_attachment: true,
            route_unit: unit,
            route_components: vec![MacroName::parse("module", 2024).unwrap()],
            visibility: VisibilityScope::Public,
            origin: DeclarationModuleOrigin::Original,
            pending: None,
        }
    }
    #[test]
    fn scoped_binding_admission_is_idempotent_and_conflicts_are_atomic() {
        let row = binding();
        let mut names = MacroNamespaces::default();
        names.declare_scoped_module_binding(row.clone()).unwrap();
        let bytes = names.module_binding_bytes;
        names.declare_scoped_module_binding(row.clone()).unwrap();
        assert_eq!(names.module_binding_bytes, bytes);
        let mut changed = row.clone();
        changed.origin = DeclarationModuleOrigin::Generated {
            producer: Digest([242; 32]),
            generation: Digest([243; 32]),
            context: crate::syntax::token_wire::Key([244; 32]),
        };
        assert_ne!(row.fingerprint(), changed.fingerprint());
        assert!(names.declare_scoped_module_binding(changed).is_err());
        let mut changed = row.clone();
        changed.scope_kind = DeclarationScopeKind::Block;
        changed.scope.owner = Some(Digest([245; 32]));
        changed.lexical_scope = crate::expand::scopes::ScopeKey::root(Digest([246; 32]));
        assert!(names.declare_scoped_module_binding(changed).is_err());
        assert_eq!(names.module_binding_bytes, bytes);
        assert_eq!(names.module_binding_identities.len(), 1);
        assert_eq!(
            names
                .module_binding_rows
                .values()
                .next()
                .unwrap()
                .get(&row.identity),
            Some(&row)
        );
        let mut invalid = row.clone();
        invalid.identity = Digest([247; 32]);
        invalid.route_components = vec![MacroName::parse("other", 2024).unwrap()];
        assert!(names.declare_scoped_module_binding(invalid).is_err());
        assert_eq!(names.module_binding_identities.len(), 1);
        // Duplicate admission must still succeed at an exhausted aggregate quota.
        names.module_binding_bytes = 32 * 1024 * 1024;
        names.declare_scoped_module_binding(row.clone()).unwrap();
        let mut extra = row;
        extra.identity = Digest([248; 32]);
        assert!(names.declare_scoped_module_binding(extra).is_err());
        assert_eq!(names.module_binding_bytes, 32 * 1024 * 1024);
        assert_eq!(names.module_binding_identities.len(), 1);
    }
    #[test]
    fn scoped_binding_read_budget_and_route_collision_do_not_mutate_facts() {
        let row = binding();
        let mut names = MacroNamespaces::default();
        names.declare_scoped_module_binding(row.clone()).unwrap();
        let bytes = names.module_binding_bytes;
        let result = names.declaration_module_bindings(
            row.lexical_scope,
            &row.name,
            &row.scope,
            None,
            |_| Err(error("controlled scoped module read quota")),
        );
        assert_eq!(result.unwrap_err().code, "LRR-E-MACRO-SCOPE");
        assert_eq!(names.module_binding_bytes, bytes);
        assert!(
            names
                .declaration_module_bindings(
                    row.lexical_scope,
                    &row.name,
                    &row.scope,
                    Some(Digest([249; 32])),
                    |_| Ok(())
                )
                .unwrap()
                .is_empty()
        );
        assert!(
            names
                .declaration_module_route_unique(&row, |_| Ok(()))
                .unwrap()
        );
        let mut other = row.clone();
        other.identity = Digest([250; 32]);
        other.scope.owner = Some(Digest([251; 32]));
        other.scope_kind = DeclarationScopeKind::Block;
        other.canonical_attachment = false;
        other.lexical_scope = crate::expand::scopes::ScopeKey::root(Digest([252; 32]));
        other.origin = DeclarationModuleOrigin::Unproven {
            input: Digest([253; 32]),
        };
        names.declare_scoped_module_binding(other.clone()).unwrap();
        assert_eq!(other.demand(), Digest([253; 32]));
        assert!(
            !names
                .declaration_module_route_unique(&row, |_| Ok(()))
                .unwrap()
        );
        let mut pending = other.clone();
        pending.pending = Some(Digest([254; 32]));
        assert_eq!(pending.demand(), Digest([254; 32]));
        assert_ne!(pending.fingerprint(), other.fingerprint());
        assert_ne!(
            DeclarationModuleOrigin::Original.fingerprint(),
            DeclarationModuleOrigin::Unproven {
                input: Digest([253; 32])
            }
            .fingerprint()
        );
    }
    #[test]
    fn generated_module_attachment_projects_only_the_matched_root() {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(b"fn cold(){ outer!{} }".to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let parent = parsed
            .scan_declarations(
                SourcePath::new("attachment.rs").unwrap(),
                binding().scope,
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        let producer = &parent.producers()[0];
        let parsed = generated_declaration_fixture(
            "#[pending] mod local { pub mod nested {} }",
            crate::expand::identity::GrammarSlot::Items,
        );
        let capture = |scope, kind, cfg: &CfgSet| {
            parsed
                .scan_declarations_at_root(
                    parent.path().clone(),
                    scope,
                    cfg,
                    DeclarationScanLimits::default(),
                    kind,
                )
                .unwrap()
        };
        let generated = capture(
            producer.scope.clone(),
            DeclarationScopeKind::Block,
            &CfgSet::default(),
        );
        let context = crate::syntax::token_wire::Key([237; 32]);
        let root = DeclarationModuleBinding::from_generated_scan(
            &parent,
            producer.identity,
            &generated,
            0,
            context,
        )
        .unwrap();
        assert_eq!(root.lexical_scope, producer.lexical_scope);
        assert_eq!(root.scope.owner, producer.scope.owner);
        assert_eq!(root.scope_kind, DeclarationScopeKind::Block);
        assert!(!root.canonical_attachment);
        assert_eq!(
            root.pending,
            generated.modules()[0].attribute_inputs.first().copied()
        );
        assert_eq!(root.demand(), root.pending.unwrap());
        assert_eq!(
            root.origin,
            DeclarationModuleOrigin::Generated {
                producer: producer.identity,
                generation: generated.generated_origin().unwrap(),
                context,
            }
        );
        let nested = DeclarationModuleBinding::from_generated_scan(
            &parent,
            producer.identity,
            &generated,
            1,
            context,
        )
        .unwrap();
        assert_eq!(nested.lexical_scope, generated.modules()[1].lexical_scope);
        assert_ne!(nested.lexical_scope, producer.lexical_scope);
        assert_eq!(nested.scope_kind, DeclarationScopeKind::Module);
        assert!(!nested.canonical_attachment);
        assert_eq!(nested.scope.owner, None);
        assert_eq!(nested.scope.module_path, ["local"]);
        assert!(
            DeclarationModuleBinding::from_generated_scan(
                &parent,
                Digest([238; 32]),
                &generated,
                0,
                context,
            )
            .is_err()
        );
        assert!(
            DeclarationModuleBinding::from_generated_scan(
                &parent,
                producer.identity,
                &generated,
                2,
                context,
            )
            .is_err()
        );
        let wrong_kind = capture(
            producer.scope.clone(),
            DeclarationScopeKind::Module,
            &CfgSet::default(),
        );
        assert!(
            DeclarationModuleBinding::from_generated_scan(
                &parent,
                producer.identity,
                &wrong_kind,
                0,
                context,
            )
            .is_err()
        );
        let mut scope = producer.scope.clone();
        scope.owner = None;
        let wrong_owner = capture(scope, DeclarationScopeKind::Block, &CfgSet::default());
        assert!(
            DeclarationModuleBinding::from_generated_scan(
                &parent,
                producer.identity,
                &wrong_owner,
                0,
                context,
            )
            .is_err()
        );
        let mut cfg = CfgSet::default();
        cfg.atoms.insert("different".into());
        let wrong_cfg = capture(producer.scope.clone(), DeclarationScopeKind::Block, &cfg);
        assert!(
            DeclarationModuleBinding::from_generated_scan(
                &parent,
                producer.identity,
                &wrong_cfg,
                0,
                context,
            )
            .is_err()
        );
    }
    #[test]
    fn module_capture_conversion_requires_exact_original_or_generated_provenance() {
        let parsed = ParsedFile::parse(
            &SourceBlob::new(b"mod local {}".to_vec()).unwrap(),
            ProfileIdentity::current(),
        )
        .unwrap();
        let original = parsed
            .scan_declarations(
                SourcePath::new("origin.rs").unwrap(),
                binding().scope,
                &CfgSet::default(),
                DeclarationScanLimits::default(),
            )
            .unwrap();
        assert!(
            DeclarationModuleBinding::from_scan(&original, 0, DeclarationModuleOrigin::Original)
                .unwrap()
                .canonical_attachment
        );
        let generated = generated_declaration_fixture(
            "mod local {}",
            crate::expand::identity::GrammarSlot::Items,
        )
        .scan_declarations_at_root(
            original.path().clone(),
            original.scope().clone(),
            &CfgSet::default(),
            DeclarationScanLimits::default(),
            DeclarationScopeKind::Module,
        )
        .unwrap();
        assert!(
            DeclarationModuleBinding::from_scan(&generated, 0, DeclarationModuleOrigin::Original,)
                .is_err()
        );
        let mismatch = DeclarationModuleOrigin::Generated {
            producer: Digest([239; 32]),
            generation: Digest([236; 32]),
            context: crate::syntax::token_wire::Key([235; 32]),
        };
        assert!(DeclarationModuleBinding::from_scan(&original, 0, mismatch.clone()).is_err());
        assert!(DeclarationModuleBinding::from_scan(&generated, 0, mismatch).is_err());
        let unproven = DeclarationModuleBinding::from_scan(
            &generated,
            0,
            DeclarationModuleOrigin::Unproven {
                input: Digest([234; 32]),
            },
        )
        .unwrap();
        assert_eq!(unproven.demand(), Digest([234; 32]));
        assert!(!unproven.canonical_attachment);
    }
    #[test]
    fn generated_module_attachment_keeps_outer_block_and_generated_parent_unproven() {
        for (source, outer_kind, is_generated, canonical) in [
            ("outer!{}", DeclarationScopeKind::Module, false, true),
            (
                "fn cold(){mod local{outer!{}}}",
                DeclarationScopeKind::Module,
                false,
                false,
            ),
            ("outer!{}", DeclarationScopeKind::Module, true, false),
        ] {
            let parsed = if is_generated {
                generated_declaration_fixture(source, crate::expand::identity::GrammarSlot::Items)
            } else {
                ParsedFile::parse(
                    &SourceBlob::new(source.as_bytes().to_vec()).unwrap(),
                    ProfileIdentity::current(),
                )
                .unwrap()
            };
            let parent = parsed
                .scan_declarations_at_root(
                    SourcePath::new("outer.rs").unwrap(),
                    binding().scope,
                    &CfgSet::default(),
                    DeclarationScanLimits::default(),
                    outer_kind,
                )
                .unwrap();
            let producer = &parent.producers()[0];
            let generated = generated_declaration_fixture(
                "mod child{}",
                crate::expand::identity::GrammarSlot::Items,
            )
            .scan_declarations_at_root(
                parent.path().clone(),
                producer.scope.clone(),
                &CfgSet::default(),
                DeclarationScanLimits::default(),
                DeclarationScopeKind::Module,
            )
            .unwrap();
            let row = DeclarationModuleBinding::from_generated_scan(
                &parent,
                producer.identity,
                &generated,
                0,
                crate::syntax::token_wire::Key([230; 32]),
            )
            .unwrap();
            assert_eq!(row.lexical_scope, producer.lexical_scope);
            assert_eq!(row.scope.owner, None);
            assert_eq!(row.canonical_attachment, canonical);
        }
    }
}
