//! Impl members of nominal types: the run's inherent and trait impls joined to
//! the nominal declaration their self type names, built once from every
//! captured source's impl items through the ordinary Type namespace.
//!
//! This serves associated-item paths (`Type::item`), method probing and the
//! `Drop` impl inventory. An impl whose self type does not resolve to a
//! nominal declaration (`impl<T> Trait for T`, `impl Trait for &P`, ...) is
//! retained as unresolved, so callers can refuse instead of guessing.
use super::names::{NameLookup, NameNamespaces};
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode},
    },
    source::{
        changes::PreparedSourceView,
        manifest::{ItemKind, Namespace},
    },
    syntax::{imports::ImportSegment, outline::ScopeKey},
};
use ra_ap_syntax::ast::{self, HasModuleItem};
use std::collections::{BTreeMap, BTreeSet};

/// A builtin self type of an inherent or trait impl (`impl i32`, `impl str`,
/// `impl<T> [T]`, `impl<T, const N: usize> [T; N]`), as the facade core declares.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum BuiltinSelf {
    Primitive(crate::ir::op::PrimitiveType),
    Str,
    Slice,
    Array,
    /// `impl .. for (A, B, ..)` by arity (`()` is arity 0).
    Tuple(u32),
    /// `impl .. for &A` (false) or `&mut A` (true).
    Reference(bool),
}
impl BuiltinSelf {
    pub(crate) fn named(name: &str) -> Option<Self> {
        use crate::ir::op::PrimitiveType as P;
        Some(Self::Primitive(match name {
            "bool" => P::Bool,
            "char" => P::Char,
            "i8" => P::I8,
            "i16" => P::I16,
            "i32" => P::I32,
            "i64" => P::I64,
            "i128" => P::I128,
            "isize" => P::Isize,
            "u8" => P::U8,
            "u16" => P::U16,
            "u32" => P::U32,
            "u64" => P::U64,
            "u128" => P::U128,
            "usize" => P::Usize,
            "f32" => P::F32,
            "f64" => P::F64,
            "str" => return Some(Self::Str),
            _ => return None,
        }))
    }
}
/// `MemberIndex::primitive_candidates`.
#[derive(Debug, Clone)]
pub(crate) struct PrimitiveCandidates {
    pub(crate) direct: Vec<crate::ir::op::PrimitiveType>,
    pub(crate) blankets: Vec<Vec<Option<Digest>>>,
    pub(crate) unresolved: bool,
}
/// `MemberIndex::method_alternatives`.
#[derive(Debug, Clone)]
pub(crate) struct MethodAlternatives {
    /// The distinct functions of the winning probe tier.
    pub(crate) functions: Vec<Digest>,
    /// The trait every alternative implements, when they are impls of one
    /// trait that differ by the trait's arguments.
    pub(crate) same_trait: Option<Digest>,
    /// The alternatives are inherent functions of impls for different
    /// instantiations of one type (`impl Foo<u8> { fn bar }` beside
    /// `impl Foo<u16> { fn bar }`): the receiver's type arguments select.
    pub(crate) inherent: bool,
    /// The probe took the array-to-slice unsizing step.
    pub(crate) unsize: bool,
}
/// A resolved block-local container of an entered body: an impl block with
/// its associated functions, types and constants, or a trait with its
/// declarations. The owning body resolved the headers through its lexical
/// Type lookup; the index only records the result.
#[derive(Debug, Clone)]
pub(crate) enum LocalContainer {
    Impl {
        members: ImplMembers,
        aliases: Vec<(Digest, AssociatedTypeAlias)>,
    },
    Trait {
        definition: Digest,
        /// Every function the trait declares, required or provided.
        declarations: BTreeMap<String, Digest>,
        /// The provided (bodied) functions.
        defaults: BTreeMap<String, Digest>,
        consts: BTreeMap<String, Digest>,
    },
}
impl LocalContainer {
    fn identity(&self) -> Digest {
        match self {
            Self::Impl { members, .. } => members.identity,
            Self::Trait { definition, .. } => *definition,
        }
    }
}
/// One impl block.
#[derive(Debug, Clone)]
pub struct ImplMembers {
    pub identity: Digest,
    /// The nominal declaration the self type names, when it names one.
    pub self_nominal: Option<Digest>,
    /// The builtin self type, when the self type is one.
    pub self_builtin: Option<BuiltinSelf>,
    /// For a reference self type (`impl .. for &A`, `&mut A`): what its
    /// referent names, when that is a declaration or a builtin type (a
    /// generic referent `&'a A` names none and matches any referent).
    pub self_pointee: Option<Result<Digest, BuiltinSelf>>,
    /// The implemented trait's path as written, when this is a trait impl.
    pub trait_path: Option<Vec<ImportSegment>>,
    /// The trait's resolved definition, when it resolves to a source trait.
    pub trait_definition: Option<Digest>,
    /// Associated functions by name (definition identities).
    pub functions: BTreeMap<String, Digest>,
    /// Associated types (`type Name = Target;`) by name (alias identities).
    pub types: BTreeMap<String, Digest>,
    /// Associated constants (`const NAME: T = ..;`) by name.
    pub consts: BTreeMap<String, Digest>,
    /// A blanket impl (`impl<T: Bound> Trait for T`): the resolved trait
    /// definitions bounding its self parameter (unresolved bounds are `None`).
    pub blanket: Option<Vec<Option<Digest>>>,
}
/// An associated type alias of a trait impl: its written target in its
/// impl-owned scope, and the impl's generic parameter count.
#[derive(Debug, Clone)]
pub struct AssociatedTypeAlias {
    pub header: crate::syntax::outline::HeaderRecipe,
    pub scope: ScopeKey,
    pub impl_generic_count: usize,
}

#[derive(Debug, Default)]
pub struct MemberIndex {
    /// The next layer of block-local impls and traits that an entered body
    /// published after this index was built (`publish_local`). Layers are
    /// append-only and shared through every handle to the world index, so
    /// holders of the world see later publications without a refresh and
    /// nothing of the world is copied; lookups read this layer, then `next`.
    next: std::sync::OnceLock<std::sync::Arc<MemberIndex>>,
    /// Identities of the block-local containers already published (kept on
    /// the world index only), so a body entered again publishes once.
    local_published: std::sync::Mutex<BTreeSet<Digest>>,
    impls: Vec<ImplMembers>,
    by_nominal: BTreeMap<Digest, Vec<usize>>,
    by_builtin: BTreeMap<BuiltinSelf, Vec<usize>>,
    /// Impls whose self type is not a nominal path (or did not resolve).
    unresolved: Vec<usize>,
    /// Owning impl of each associated function.
    function_owner: BTreeMap<Digest, usize>,
    /// Provided (default) bodies of each trait, by name.
    trait_defaults: BTreeMap<Digest, BTreeMap<String, Digest>>,
    /// Every associated function each trait declares (required or
    /// provided), by name: the declaration a trait-path call names.
    trait_declarations: BTreeMap<Digest, BTreeMap<String, Digest>>,
    /// The trait definitions of the run's world.
    traits: BTreeSet<Digest>,
    /// Associated type aliases of trait impls, by alias identity.
    type_aliases: BTreeMap<Digest, AssociatedTypeAlias>,
    /// Shorthand associated-type name keys (`T::Name`), to names.
    type_names: BTreeMap<Digest, String>,
    /// Provided (default) associated constants of each trait, by name.
    trait_consts: BTreeMap<Digest, BTreeMap<String, Digest>>,
    /// The library's `core::marker::Copy` and `core::ops::Drop` definitions,
    /// when the run's world includes the core facade.
    lang_copy: Option<Digest>,
    lang_drop: Option<Digest>,
    /// The alloc facade's `boxed::Box` declaration (rustc's `owned_box`
    /// lang item), when the run's world includes it.
    lang_box: Option<Digest>,
    lang_unsafe_cell: Option<Digest>,
    /// The alloc facade's `vec::Vec` declaration (the growable-array lang
    /// item), when the run's world includes it.
    lang_vec: Option<Digest>,
}
/// Stable key of an associated type name in a shorthand projection
/// (`T::Name` whose trait is selected from the bound type's impls).
pub(crate) fn associated_name_key(name: &str) -> Digest {
    let mut e = crate::core::digest::StableEncoder::new("lrr/v1/associated-type-name");
    e.string(name);
    e.finish()
}

fn parser_edition(edition: u16) -> Option<ra_ap_syntax::Edition> {
    Some(match edition {
        2015 => ra_ap_syntax::Edition::Edition2015,
        2018 => ra_ap_syntax::Edition::Edition2018,
        2021 => ra_ap_syntax::Edition::Edition2021,
        2024 => ra_ap_syntax::Edition::Edition2024,
        _ => return None,
    })
}

/// Stands for the n-th `$crate` token of a generated impl header.
const DOLLAR_CRATE_PLACEHOLDER: &str = "__lrr_dollar_crate_";
/// Impl header text for the source parser: a generated `$crate` token is not
/// source syntax, so each becomes a placeholder name (resolved separately
/// where its crate matters).
pub(crate) fn impl_header_text(tokens: &[crate::syntax::ast_adapter::TokenRecipe]) -> String {
    let mut dollar = 0usize;
    tokens
        .iter()
        .map(|token| {
            if token.text == "$crate" {
                dollar += 1;
                format!("{DOLLAR_CRATE_PLACEHOLDER}{}", dollar - 1)
            } else {
                token.text.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
pub(crate) fn path_segments(path: ast::Path) -> Option<(Vec<ImportSegment>, bool)> {
    let mut segments = Vec::new();
    let mut cursor = Some(path);
    let mut absolute = false;
    while let Some(path) = cursor {
        let segment = path.segment()?;
        segments.push(match segment.kind()? {
            ast::PathSegmentKind::Name(name) => ImportSegment::Name(name.text().to_string()),
            ast::PathSegmentKind::CrateKw => ImportSegment::Crate,
            ast::PathSegmentKind::SelfKw => ImportSegment::SelfModule,
            ast::PathSegmentKind::SuperKw => ImportSegment::Super,
            _ => return None,
        });
        if path.qualifier().is_none() {
            absolute = segment.coloncolon_token().is_some();
        }
        cursor = path.qualifier();
    }
    segments.reverse();
    Some((segments, absolute))
}

impl MemberIndex {
    pub fn build(graph: &NameNamespaces, sources: &[&PreparedSourceView]) -> Result<Self, Diagnostic> {
        let mut index = Self::default();
        for source in sources {
            index.add_outline(graph, source.outline(), &|identity| identity, None)?;
        }
        Ok(index)
    }
    /// Publish block-local impls and traits of an entered body into this
    /// (world) index as one new layer. Containers already published are
    /// skipped, so re-entering the body adds nothing. Callers hold the world
    /// handle; later lookups through any clone of it see the layer.
    pub(crate) fn publish_local(&self, rows: Vec<LocalContainer>) {
        let mut published = self
            .local_published
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let rows: Vec<LocalContainer> = rows
            .into_iter()
            .filter(|row| published.insert(row.identity()))
            .collect();
        if rows.is_empty() {
            return;
        }
        let mut layer = Self::default();
        for row in rows {
            match row {
                LocalContainer::Trait {
                    definition,
                    declarations,
                    defaults,
                    consts,
                } => {
                    layer.traits.insert(definition);
                    layer.trait_declarations.insert(definition, declarations);
                    layer.trait_defaults.insert(definition, defaults);
                    layer.trait_consts.insert(definition, consts);
                }
                LocalContainer::Impl { members, aliases } => {
                    let ordinal = layer.impls.len();
                    for function in members.functions.values() {
                        layer.function_owner.insert(*function, ordinal);
                    }
                    for (identity, alias) in aliases {
                        layer.type_aliases.insert(identity, alias);
                    }
                    for name in members.types.keys() {
                        layer
                            .type_names
                            .insert(associated_name_key(name), name.clone());
                    }
                    match (members.self_nominal, members.self_builtin) {
                        (Some(nominal), _) => {
                            layer.by_nominal.entry(nominal).or_default().push(ordinal)
                        }
                        (None, Some(builtin)) => {
                            layer.by_builtin.entry(builtin).or_default().push(ordinal)
                        }
                        (None, None) => layer.unresolved.push(ordinal),
                    }
                    layer.impls.push(members);
                }
            }
        }
        let layer = std::sync::Arc::new(layer);
        let mut tail = self;
        loop {
            match tail.next.get() {
                Some(next) => tail = next,
                None => match tail.next.set(layer.clone()) {
                    Ok(()) => return,
                    Err(_) => continue,
                },
            }
        }
    }
    /// Index the impls/traits of a macro-generated item unit. Its published
    /// identities are the generated header keys, not outline fingerprints.
    pub(crate) fn add_generated(
        &mut self,
        graph: &NameNamespaces,
        input: &crate::expand::generated_syntax::GeneratedSyntaxInput,
    ) -> Result<(), Diagnostic> {
        let outline = input.outline()?;
        let published: BTreeMap<Digest, Digest> = input
            .definitions
            .iter()
            .map(|(id, def_key)| (def_key.fingerprint(), *id))
            .collect();
        // Generated impl headers may name `$crate` (macro_rules output, builtin
        // derives): each token names the crate its definition-site hygiene
        // selects, read from the unit's own token contexts.
        let dollar = outline
            .items
            .iter()
            .any(|item| {
                item.def_key.item_kind == ItemKind::Impl
                    && item.header.tokens.iter().any(|token| token.text == "$crate")
            })
            .then(|| crate::syntax::parse::ParsedFile::from_generated_items(&input.items))
            .transpose()?;
        let dollar_units = |item: &crate::syntax::outline::ItemOutline| -> Result<Vec<Digest>, Diagnostic> {
            let Some(parsed) = &dollar else {
                return Ok(Vec::new());
            };
            let text = input.items.text();
            let mut units = Vec::new();
            for origin in input.items.origins() {
                if origin.start < item.declaration_span.start
                    || origin.end > item.declaration_span.end
                    || text.get(origin.start as usize..origin.end as usize) != Some("$crate")
                {
                    continue;
                }
                let range = crate::syntax::spans::SourceRange::checked(origin.start, origin.end)?;
                let (unit, _) = parsed.dollar_crate_root(range)?.ok_or_else(|| {
                    Diagnostic::new(
                        "LRR-E-MACRO-OUTPUT",
                        "generated `$crate` lacks its token context".into(),
                    )
                })?;
                units.push(unit);
            }
            Ok(units)
        };
        self.add_outline(
            graph,
            &outline,
            &|identity| published.get(&identity).copied().unwrap_or(identity),
            Some(&dollar_units),
        )
    }
    /// Index a macro-generated associated function written into an original
    /// impl (as that impl's member) or trait (as its provided default).
    pub(crate) fn add_associated(
        &mut self,
        owner: Digest,
        owner_trait: bool,
        constant: bool,
        name: &str,
        identity: Digest,
        provided: bool,
    ) {
        if owner_trait {
            if constant {
                if provided {
                    self.trait_consts
                        .entry(owner)
                        .or_default()
                        .insert(name.to_owned(), identity);
                }
                return;
            }
            self.trait_declarations
                .entry(owner)
                .or_default()
                .insert(name.to_owned(), identity);
            if provided {
                self.trait_defaults
                    .entry(owner)
                    .or_default()
                    .insert(name.to_owned(), identity);
            }
            return;
        }
        if let Some(ordinal) = self.impls.iter().position(|members| members.identity == owner) {
            if constant {
                self.impls[ordinal].consts.insert(name.to_owned(), identity);
                return;
            }
            self.impls[ordinal]
                .functions
                .insert(name.to_owned(), identity);
            self.function_owner.insert(identity, ordinal);
        }
    }
    fn add_outline(
        &mut self,
        graph: &NameNamespaces,
        outline: &crate::syntax::outline::Outline,
        published: &dyn Fn(Digest) -> Digest,
        dollar_units: Option<
            &dyn Fn(&crate::syntax::outline::ItemOutline) -> Result<Vec<Digest>, Diagnostic>,
        >,
    ) -> Result<(), Diagnostic> {
        let index = self;
        {
            let edition = outline.profile.edition;
            let Some(parser) = parser_edition(edition) else {
                return Ok(());
            };
            // Trait definitions of this outline; the core facade's
            // `marker::Copy` and `ops::Drop` are the run's lang traits.
            let core = crate::facade::unit(&outline.profile, "core");
            // Module-level type aliases of this outline, by identity.
            let module_aliases: BTreeMap<Digest, &crate::syntax::outline::ItemOutline> = outline
                .items
                .iter()
                .filter(|item| {
                    item.def_key.item_kind == ItemKind::TypeAlias
                        && item.def_key.enclosing_impl.is_none()
                        && item.scope.owner.is_none()
                })
                .map(|item| (published(item.def_key.fingerprint()), item))
                .collect();
            let owned_box = crate::facade::owned_box(&outline.profile);
            let owned_vec = crate::facade::owned_vec(&outline.profile);
            let unsafe_cell = crate::facade::unsafe_cell(&outline.profile);
            let mut traits = BTreeSet::new();
            for item in &outline.items {
                if item.def_key.fingerprint() == owned_box {
                    index.lang_box = Some(published(owned_box));
                }
                if item.def_key.fingerprint() == owned_vec {
                    index.lang_vec = Some(published(owned_vec));
                }
                if item.def_key.fingerprint() == unsafe_cell {
                    index.lang_unsafe_cell = Some(published(unsafe_cell));
                }
                if item.def_key.item_kind != ItemKind::Trait {
                    continue;
                }
                let fingerprint = item.def_key.fingerprint();
                traits.insert(fingerprint);
                index.traits.insert(published(fingerprint));
                if item.def_key.crate_key == core && item.def_key.enclosing_impl.is_none() {
                    match (item.def_key.module_path.as_slice(), item.def_key.declared_name.as_str()) {
                        ([module], "Copy") if module == "marker" => {
                            index.lang_copy = Some(published(fingerprint));
                        }
                        ([module], "Drop") if module == "ops" => {
                            index.lang_drop = Some(published(fingerprint));
                        }
                        _ => {}
                    }
                }
            }
            // Trait method declarations and provided bodies, by owning
            // trait definition.
            for item in &outline.items {
                if item.def_key.item_kind == ItemKind::Function
                    && let Some(owner) = item.scope.owner
                    && traits.contains(&owner)
                {
                    index
                        .trait_declarations
                        .entry(published(owner))
                        .or_default()
                        .insert(item.def_key.declared_name.clone(), published(item.def_key.fingerprint()));
                }
            }
            for item in &outline.items {
                if item.def_key.item_kind == ItemKind::Function
                    && item.body.is_some()
                    && let Some(owner) = item.scope.owner
                    && traits.contains(&owner)
                {
                    index
                        .trait_defaults
                        .entry(published(owner))
                        .or_default()
                        .insert(item.def_key.declared_name.clone(), published(item.def_key.fingerprint()));
                }
            }
            // Associated types by owning impl.
            let mut types: BTreeMap<Digest, BTreeMap<String, Digest>> = BTreeMap::new();
            for item in &outline.items {
                if item.def_key.item_kind == ItemKind::TypeAlias
                    && let Some(owner) = item.scope.owner
                    && item.def_key.enclosing_impl == Some(owner)
                {
                    let name = item.def_key.declared_name.clone();
                    index
                        .type_names
                        .insert(associated_name_key(&name), name.clone());
                    types
                        .entry(owner)
                        .or_default()
                        .insert(name, published(item.def_key.fingerprint()));
                    index.type_aliases.insert(
                        published(item.def_key.fingerprint()),
                        AssociatedTypeAlias {
                            header: item.header.clone(),
                            // Impl-owned: names resolve in the enclosing module.
                            scope: item.scope.clone(),
                            impl_generic_count: 0,
                        },
                    );
                }
            }
            // Associated constants by owning impl, and trait-provided defaults.
            let mut consts: BTreeMap<Digest, BTreeMap<String, Digest>> = BTreeMap::new();
            for item in &outline.items {
                if item.def_key.item_kind != ItemKind::Constant {
                    continue;
                }
                let Some(owner) = item.scope.owner else {
                    continue;
                };
                let name = item.def_key.declared_name.clone();
                // A trait's own member (whose owner is the trait definition)
                // is a provided default when it has a body, never an impl's.
                let trait_owned = outline.items.iter().any(|candidate| {
                    candidate.def_key.item_kind == ItemKind::Trait
                        && candidate.def_key.fingerprint() == owner
                });
                if trait_owned {
                    if item.body.is_some() {
                        index
                            .trait_consts
                            .entry(published(owner))
                            .or_default()
                            .insert(name, published(item.def_key.fingerprint()));
                    }
                } else if item.def_key.enclosing_impl == Some(owner) {
                    consts
                        .entry(owner)
                        .or_default()
                        .insert(name, published(item.def_key.fingerprint()));
                }
            }
            // Associated functions by owning impl.
            let mut functions: BTreeMap<Digest, BTreeMap<String, Digest>> = BTreeMap::new();
            for item in &outline.items {
                if item.def_key.item_kind == ItemKind::Function
                    && let Some(owner) = item.scope.owner
                    && item.def_key.enclosing_impl == Some(owner)
                {
                    functions
                        .entry(owner)
                        .or_default()
                        .insert(item.def_key.declared_name.clone(), published(item.def_key.fingerprint()));
                }
            }
            for item in &outline.items {
                if item.def_key.item_kind != ItemKind::Impl {
                    continue;
                }
                let Some(identity) = item.def_key.enclosing_impl else {
                    continue;
                };
                // Header `$crate` tokens (in order) become placeholder names
                // resolved directly in the crate each one names.
                let dollar = match dollar_units {
                    Some(units)
                        if item.header.tokens.iter().any(|token| token.text == "$crate") =>
                    {
                        units(item)?
                    }
                    _ => Vec::new(),
                };
                let mut dollar_index = 0usize;
                let header = item
                    .header
                    .tokens
                    .iter()
                    .map(|token| {
                        if token.text == "$crate" && dollar_index < dollar.len() {
                            dollar_index += 1;
                            format!("{DOLLAR_CRATE_PLACEHOLDER}{}", dollar_index - 1)
                        } else {
                            token.text.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let parsed = ast::SourceFile::parse(&format!("{header} {{}}"), parser);
                let Some(ast::Item::Impl(implementation)) = parsed.tree().items().next() else {
                    continue;
                };
                let scope = ScopeKey {
                    crate_key: item.scope.crate_key,
                    module_path: item.scope.module_path.clone(),
                    owner: None,
                };
                let resolve = |path: Option<ast::Path>| -> Result<Option<Digest>, Diagnostic> {
                    let Some((segments, absolute)) = path.and_then(path_segments) else {
                        return Ok(None);
                    };
                    if let Some(ImportSegment::Name(first)) = segments.first()
                        && let Some(unit) = first
                            .strip_prefix(DOLLAR_CRATE_PLACEHOLDER)
                            .and_then(|index| index.parse::<usize>().ok())
                            .and_then(|index| dollar.get(index))
                    {
                        let mut tail = Vec::new();
                        for segment in &segments[1..] {
                            let ImportSegment::Name(name) = segment else {
                                return Ok(None);
                            };
                            tail.push(name.as_str());
                        }
                        let path = NameNamespaces::path(*unit, &tail, edition)?;
                        return Ok(match graph.lookup(Namespace::Type, &path, &scope)? {
                            NameLookup::Defined(definition) => Some(definition.identity),
                            _ => None,
                        });
                    }
                    Ok(
                        match graph.lookup_source_path(
                            Namespace::Type,
                            &segments,
                            absolute,
                            &scope,
                            edition,
                        )? {
                            NameLookup::Defined(definition) => Some(definition.identity),
                            _ => None,
                        },
                    )
                };
                // A `$crate` root keeps the spelling of the crate it names.
                let spell_dollar = |mut segments: Vec<ImportSegment>| {
                    if let Some(ImportSegment::Name(first)) = segments.first_mut()
                        && let Some(unit) = first
                            .strip_prefix(DOLLAR_CRATE_PLACEHOLDER)
                            .and_then(|index| index.parse::<usize>().ok())
                            .and_then(|index| dollar.get(index))
                    {
                        *first = ["core", "alloc", "std"]
                            .into_iter()
                            .find(|name| crate::facade::unit(&outline.profile, name) == *unit)
                            .map(str::to_owned)
                            .unwrap_or_else(|| super::callables::dollar_crate_segment(*unit));
                    }
                    segments
                };
                let (trait_path, trait_definition) = match implementation.trait_() {
                    Some(ast::Type::PathType(path)) => (
                        path.path()
                            .and_then(path_segments)
                            .map(|(s, _)| spell_dollar(s)),
                        resolve(path.path())?,
                    ),
                    Some(_) => (Some(Vec::new()), None),
                    None => (None, None),
                };
                let mut self_ty = implementation.self_ty();
                let mut self_nominal = match &self_ty {
                    Some(ast::Type::PathType(path)) => resolve(path.path())?,
                    // E-R5-008: raw and fn pointer self types are the
                    // reserved identities of `traits::pointer_terms`.
                    Some(ast::Type::PtrType(pointer)) => Some(if pointer.mut_token().is_some() {
                        crate::traits::pointer_terms::mut_pointer_identity()
                    } else {
                        crate::traits::pointer_terms::const_pointer_identity()
                    }),
                    Some(ast::Type::FnPtrType(_)) => {
                        Some(crate::traits::pointer_terms::fn_pointer_identity())
                    }
                    _ => None,
                };
                // `type Foo = W<u32>; impl Foo {..}`: an impl of a type alias
                // is an impl of the aliased type (module aliases of this
                // outline; the alias's own target resolves in its own scope).
                let mut followed = BTreeSet::new();
                while let Some(alias) = self_nominal
                    .and_then(|identity| module_aliases.get(&identity).map(|item| (identity, item)))
                {
                    if !followed.insert(alias.0) {
                        break;
                    }
                    let item = alias.1;
                    let header = item
                        .header
                        .tokens
                        .iter()
                        .map(|token| token.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let parsed = ast::SourceFile::parse(&format!("{header};"), parser);
                    let Some(ast::Item::TypeAlias(declared)) = parsed.tree().items().next() else {
                        break;
                    };
                    self_ty = declared.ty();
                    let alias_scope = ScopeKey {
                        crate_key: item.scope.crate_key,
                        module_path: item.scope.module_path.clone(),
                        owner: None,
                    };
                    self_nominal = match &self_ty {
                        Some(ast::Type::PathType(path)) => match path.path().and_then(path_segments) {
                            Some((segments, absolute)) => match graph.lookup_source_path(
                                Namespace::Type,
                                &segments,
                                absolute,
                                &alias_scope,
                                edition,
                            )? {
                                NameLookup::Defined(definition) => Some(definition.identity),
                                _ => None,
                            },
                            None => None,
                        },
                        _ => None,
                    };
                }
                let self_builtin = match (self_nominal, self_ty) {
                    (None, Some(ast::Type::PathType(path))) => path
                        .path()
                        .filter(|path| path.qualifier().is_none())
                        .and_then(|path| path.segment())
                        .and_then(|segment| segment.name_ref())
                        .and_then(|name| BuiltinSelf::named(&name.text())),
                    (None, Some(ast::Type::SliceType(_))) => Some(BuiltinSelf::Slice),
                    (None, Some(ast::Type::ArrayType(_))) => Some(BuiltinSelf::Array),
                    (None, Some(ast::Type::TupleType(tuple))) => {
                        u32::try_from(tuple.fields().count()).ok().map(BuiltinSelf::Tuple)
                    }
                    (None, Some(ast::Type::RefType(reference))) => {
                        Some(BuiltinSelf::Reference(reference.mut_token().is_some()))
                    }
                    _ => None,
                };
                let self_pointee = match &implementation.self_ty() {
                    Some(ast::Type::RefType(reference)) if self_nominal.is_none() => {
                        match reference.ty() {
                            Some(ast::Type::PathType(path)) => match resolve(path.path())? {
                                Some(nominal) => Some(Ok(nominal)),
                                None => path
                                    .path()
                                    .filter(|path| path.qualifier().is_none())
                                    .and_then(|path| path.segment())
                                    .and_then(|segment| segment.name_ref())
                                    .and_then(|name| BuiltinSelf::named(&name.text()))
                                    .map(Err),
                            },
                            Some(ast::Type::SliceType(_)) => Some(Err(BuiltinSelf::Slice)),
                            Some(ast::Type::ArrayType(_)) => Some(Err(BuiltinSelf::Array)),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let blanket = blanket_bounds(&implementation, &resolve)?;
                let ordinal = index.impls.len();
                let functions = functions.remove(&identity).unwrap_or_default();
                let types = types.remove(&identity).unwrap_or_default();
                let consts = consts.remove(&identity).unwrap_or_default();
                let impl_generic_count = ast::HasGenericParams::generic_param_list(&implementation)
                    .map_or(0, |list| list.generic_params().count());
                for alias in types.values() {
                    if let Some(row) = index.type_aliases.get_mut(alias) {
                        row.impl_generic_count = impl_generic_count;
                    }
                }
                for function in functions.values() {
                    index.function_owner.insert(*function, ordinal);
                }
                match (self_nominal, self_builtin) {
                    (Some(nominal), _) => index.by_nominal.entry(nominal).or_default().push(ordinal),
                    (None, Some(builtin)) => {
                        index.by_builtin.entry(builtin).or_default().push(ordinal)
                    }
                    (None, None) => index.unresolved.push(ordinal),
                }
                index.impls.push(ImplMembers {
                    identity: published(identity),
                    self_nominal,
                    self_builtin,
                    self_pointee,
                    trait_path,
                    trait_definition,
                    functions,
                    types,
                    consts,
                    blanket,
                });
            }
        }
        Ok(())
    }
    /// Inherent associated functions named `name` of nominal `ty`.
    pub fn inherent(&self, ty: Digest, name: &str) -> Vec<Digest> {
        self.impls_of(ty)
            .filter(|members| members.trait_path.is_none())
            .filter_map(|members| members.functions.get(name).copied())
            .collect()
    }
    /// Trait-impl associated functions named `name` of nominal `ty`.
    /// Trait-impl associated functions named `name` of nominal `ty`: the
    /// impl's own definition, else its trait's provided body.
    pub fn trait_functions(&self, ty: Digest, name: &str) -> Vec<(&ImplMembers, Digest)> {
        self.impls_of(ty)
            .filter(|members| members.trait_path.is_some())
            .filter_map(|members| {
                members
                    .functions
                    .get(name)
                    .or_else(|| {
                        members
                            .trait_definition
                            .and_then(|definition| self.trait_defaults_of(definition))
                            .and_then(|defaults| defaults.get(name))
                    })
                    .map(|f| (members, *f))
            })
            .collect()
    }
    /// Inherent associated functions named `name` of a builtin self type.
    pub fn builtin_inherent(&self, ty: BuiltinSelf, name: &str) -> Vec<Digest> {
        self.builtin_inherent_of(ty, None, name)
    }
    fn builtin_inherent_of(
        &self,
        ty: BuiltinSelf,
        pointee: Option<Result<Digest, BuiltinSelf>>,
        name: &str,
    ) -> Vec<Digest> {
        for tier in self.pointee_tiers(ty, pointee) {
            let found: Vec<Digest> = tier
                .into_iter()
                .filter(|members| members.trait_path.is_none())
                .filter_map(|members| members.functions.get(name).copied())
                .collect();
            if !found.is_empty() {
                return found;
            }
        }
        Vec::new()
    }
    /// Trait-impl functions named `name` of a builtin self type (own
    /// definition, else the trait's provided body).
    pub fn builtin_trait_functions(&self, ty: BuiltinSelf, name: &str) -> Vec<Digest> {
        self.builtin_trait_impl_functions(ty, name)
            .into_iter()
            .map(|(_, function)| function)
            .collect()
    }
    /// `builtin_trait_functions` with the implemented trait of each impl.
    fn builtin_trait_impl_functions(
        &self,
        ty: BuiltinSelf,
        name: &str,
    ) -> Vec<(Option<Digest>, Digest)> {
        self.builtin_trait_impl_functions_of(ty, None, name)
    }
    /// The impls of a reference self type, in the tiers a referent selects
    /// among: those naming `pointee`, then those generic in it (one tier of
    /// every impl when no referent is given).
    fn pointee_tiers<'a>(
        &'a self,
        ty: BuiltinSelf,
        pointee: Option<Result<Digest, BuiltinSelf>>,
    ) -> Vec<Vec<&'a ImplMembers>> {
        let impls: Vec<&ImplMembers> = self.builtin_impls(ty).collect();
        let Some(pointee) = pointee else {
            return vec![impls];
        };
        let named: Vec<&ImplMembers> = impls
            .iter()
            .copied()
            .filter(|members| members.self_pointee == Some(pointee))
            .collect();
        let generic: Vec<&ImplMembers> = impls
            .into_iter()
            .filter(|members| members.self_pointee.is_none())
            .collect();
        vec![named, generic]
    }
    fn builtin_trait_impl_functions_of(
        &self,
        ty: BuiltinSelf,
        pointee: Option<Result<Digest, BuiltinSelf>>,
        name: &str,
    ) -> Vec<(Option<Digest>, Digest)> {
        for tier in self.pointee_tiers(ty, pointee) {
            let found: Vec<(Option<Digest>, Digest)> = tier
                .into_iter()
                .filter(|members| members.trait_path.is_some())
                .filter_map(|members| {
                    members
                        .functions
                        .get(name)
                        .or_else(|| {
                            members
                                .trait_definition
                                .and_then(|definition| self.trait_defaults_of(definition))
                                .and_then(|defaults| defaults.get(name))
                        })
                        .map(|function| (members.trait_definition, *function))
                })
                .collect();
            if !found.is_empty() {
                return found;
            }
        }
        Vec::new()
    }
    /// `<ty as Trait>::name` for the trait whose definition is `trait_key`:
    /// each impl of that trait for `ty` (its own definition, else the trait's
    /// provided body), then, when none applies directly, each blanket impl
    /// of that trait whose resolved bounds `ty` meets. Operator expressions
    /// select their lang-item trait method through this (rustc's operator
    /// lookup), never by method name alone.
    pub fn trait_method(
        &self,
        ty: Result<Digest, BuiltinSelf>,
        trait_key: Digest,
        name: &str,
    ) -> Vec<(&ImplMembers, Digest)> {
        let provided = |members: &ImplMembers| {
            members
                .functions
                .get(name)
                .or_else(|| {
                    self.trait_defaults_of(trait_key)
                        .and_then(|defaults| defaults.get(name))
                })
                .copied()
        };
        let direct: Vec<_> = match ty {
            Ok(nominal) => self.impls_of(nominal).collect::<Vec<_>>(),
            Err(builtin) => self.builtin_impls(builtin).collect(),
        }
        .into_iter()
        .filter(|members| members.trait_definition == Some(trait_key))
        .filter_map(|members| provided(members).map(|function| (members, function)))
        .collect();
        if !direct.is_empty() {
            return direct;
        }
        self.unresolved()
            .filter(|members| {
                members.trait_definition == Some(trait_key)
                    && members.blanket.as_ref().is_some_and(|bounds| {
                        bounds
                            .iter()
                            .all(|bound| bound.is_none_or(|bound| self.implements(ty, bound)))
                    })
            })
            .filter_map(|members| provided(members).map(|function| (members, function)))
            .collect()
    }
    /// Whether `key` is a trait definition of the run's world.
    pub fn is_trait(&self, key: Digest) -> bool {
        self.layers().any(|layer| layer.traits.contains(&key))
    }
    /// The associated function `name` trait `trait_key` declares (required
    /// or provided): the signature a trait-path call (`Trait::name(..)`) is
    /// written against before its self type selects the impl.
    pub fn trait_declaration(&self, trait_key: Digest, name: &str) -> Option<Digest> {
        self.layers()
            .filter_map(|layer| layer.trait_declarations.get(&trait_key))
            .find_map(|declarations| declarations.get(name))
            .copied()
    }
    /// Trait-provided functions `name` for self type `ty`, as a path through
    /// a generic parameter (`T::name`) selects them: the trait impls of the
    /// type itself (own definition, else the trait's provided body), else
    /// applicable blanket impls. Inherent functions of the reached type are
    /// not visible through the parameter.
    pub fn trait_candidates(&self, ty: Result<Digest, BuiltinSelf>, name: &str) -> Vec<Digest> {
        let direct = match ty {
            Ok(nominal) => self
                .trait_functions(nominal, name)
                .into_iter()
                .map(|(_, function)| function)
                .collect::<Vec<_>>(),
            Err(builtin) => self.builtin_trait_functions(builtin, name),
        };
        if direct.is_empty() {
            self.blanket_functions(ty, name)
        } else {
            direct
        }
    }
    /// The blanket impls' functions applicable to `ty` (`impl<T> From<T> for
    /// T`), alongside whatever direct impls it has.
    pub fn blanket_candidates(&self, ty: Result<Digest, BuiltinSelf>, name: &str) -> Vec<Digest> {
        self.blanket_functions(ty, name)
    }
    /// `trait_candidates` where the self type is a reference: the impls for
    /// `&U` are indexed together, so the referent `pointee` selects among
    /// them (an impl naming it, else one generic in it).
    pub fn trait_candidates_of(
        &self,
        ty: Result<Digest, BuiltinSelf>,
        pointee: Option<Result<Digest, BuiltinSelf>>,
        name: &str,
    ) -> Vec<Digest> {
        match (ty, pointee) {
            (Err(builtin @ BuiltinSelf::Reference(_)), Some(_)) => {
                let direct: Vec<Digest> = self
                    .builtin_trait_impl_functions_of(builtin, pointee, name)
                    .into_iter()
                    .map(|(_, function)| function)
                    .collect();
                if direct.is_empty() {
                    self.blanket_functions(ty, name)
                } else {
                    direct
                }
            }
            _ => self.trait_candidates(ty, name),
        }
    }
    /// The trait every one of `functions` implements, when they are impls of
    /// one trait (differing by the trait's arguments).
    pub fn same_trait_of(&self, functions: &[Digest]) -> Option<Digest> {
        let trait_of = |function: &Digest| {
            self.layers()
                .flat_map(|layer| layer.impls.iter())
                .find(|members| members.functions.values().any(|f| f == function))
                .and_then(|members| members.trait_definition)
        };
        let first = trait_of(functions.first()?)?;
        functions
            .iter()
            .all(|function| trait_of(function) == Some(first))
            .then_some(first)
    }
    pub fn impls_of(&self, ty: Digest) -> impl Iterator<Item = &ImplMembers> {
        self.layers().flat_map(move |layer| {
            layer
                .by_nominal
                .get(&ty)
                .into_iter()
                .flatten()
                .map(move |ordinal| &layer.impls[*ordinal])
        })
    }
    /// The world's index, then the local layer of entered bodies' impls.
    fn layers(&self) -> impl Iterator<Item = &MemberIndex> {
        std::iter::successors(Some(self), |layer| layer.next.get().map(|next| &**next))
    }
    /// The impls of a builtin self type, across layers.
    fn builtin_impls(&self, ty: BuiltinSelf) -> impl Iterator<Item = &ImplMembers> {
        self.layers().flat_map(move |layer| {
            layer
                .by_builtin
                .get(&ty)
                .into_iter()
                .flatten()
                .map(move |ordinal| &layer.impls[*ordinal])
        })
    }
    /// The provided (default) functions of a trait, by name.
    fn trait_defaults_of(&self, definition: Digest) -> Option<&BTreeMap<String, Digest>> {
        self.layers()
            .find_map(|layer| layer.trait_defaults.get(&definition))
    }
    /// Whether `ty` implements trait `trait_key`: a direct trait impl, or a
    /// blanket impl whose bounds it meets (rustc requires a unique candidate,
    /// so any applicable impl suffices here).
    pub fn implements(&self, ty: Result<Digest, BuiltinSelf>, trait_key: Digest) -> bool {
        self.implements_depth(ty, trait_key, 0)
    }
    fn implements_depth(&self, ty: Result<Digest, BuiltinSelf>, trait_key: Digest, depth: u32) -> bool {
        // rustc's recursion limit bounds trait-solving depth.
        if depth > 128 {
            return false;
        }
        let direct = match ty {
            Ok(nominal) => self
                .impls_of(nominal)
                .any(|members| members.trait_definition == Some(trait_key)),
            Err(builtin) => self
                .builtin_impls(builtin)
                .any(|members| members.trait_definition == Some(trait_key)),
        };
        direct
            || self.unresolved().any(|members| {
                members.trait_definition == Some(trait_key)
                    && members.blanket.as_ref().is_some_and(|bounds| {
                        bounds.iter().all(|bound| {
                            bound.is_none_or(|bound| self.implements_depth(ty, bound, depth + 1))
                        })
                    })
            })
    }
    /// Functions named `name` of blanket impls (`impl<T: B> Trait for T`)
    /// whose resolved bounds `ty` meets: the impl's own definition, else its
    /// trait's provided body.
    pub fn blanket_functions(&self, ty: Result<Digest, BuiltinSelf>, name: &str) -> Vec<Digest> {
        self.unresolved()
            .filter(|members| {
                members.trait_path.is_some()
                    && members.blanket.as_ref().is_some_and(|bounds| {
                        bounds
                            .iter()
                            .all(|bound| bound.is_none_or(|bound| self.implements(ty, bound)))
                    })
            })
            .filter_map(|members| {
                members.functions.get(name).or_else(|| {
                    members
                        .trait_definition
                        .and_then(|definition| self.trait_defaults_of(definition))
                        .and_then(|defaults| defaults.get(name))
                })
            })
            .copied()
            .collect()
    }
    /// Method `method` for an autoderef'd receiver self type, as rustc's
    /// probe selects it: the impls of the self type itself (inherent impls
    /// first, then trait impls), else applicable blanket impls; an array
    /// with no method of its own takes the unsizing step to its slice
    /// (`true`). Exactly one candidate must remain. The dispatcher and the
    /// static source-constraint service share this rule.
    pub(crate) fn select_method(
        &self,
        self_type: Result<Digest, BuiltinSelf>,
        method: &str,
    ) -> Result<(Digest, bool), Diagnostic> {
        let alternatives = self.method_alternatives(self_type, method)?;
        match alternatives.functions.as_slice() {
            [function] => Ok((*function, alternatives.unsize)),
            _ if alternatives.inherent => Err(Diagnostic::new(
                "LRR-E-NAME-AMBIGUOUS",
                format!("method `{method}` has several inherent definitions"),
            )),
            _ => Err(Diagnostic::new(
                "LRR-E-NAME-AMBIGUOUS",
                format!("method `{method}` is provided by several trait impls"),
            )),
        }
    }
    /// The probe of `select_method`, keeping every distinct trait-impl
    /// function of the winning tier. Several impls of one trait that differ
    /// only by the trait's arguments (`PartialEq<i32>` and `PartialEq<M>`)
    /// are one method rustc selects by the call's argument types: they are
    /// `same_trait` alternatives. Impls sharing the trait's provided body
    /// are one function.
    pub(crate) fn method_alternatives(
        &self,
        self_type: Result<Digest, BuiltinSelf>,
        method: &str,
    ) -> Result<MethodAlternatives, Diagnostic> {
        self.method_alternatives_of(self_type, None, method)
    }
    /// `method_alternatives` for a reference self type, selecting among the
    /// reference impls by the referent they name.
    pub(crate) fn method_alternatives_of(
        &self,
        self_type: Result<Digest, BuiltinSelf>,
        pointee: Option<Result<Digest, BuiltinSelf>>,
        method: &str,
    ) -> Result<MethodAlternatives, Diagnostic> {
        let candidates = |self_type| {
            let (inherent, traits) = match self_type {
                Ok(nominal) => (
                    self.inherent(nominal, method),
                    self.trait_functions(nominal, method)
                        .into_iter()
                        .map(|(members, function)| (members.trait_definition, function))
                        .collect::<Vec<_>>(),
                ),
                Err(builtin) => (
                    self.builtin_inherent_of(builtin, pointee, method),
                    self.builtin_trait_impl_functions_of(builtin, pointee, method),
                ),
            };
            // Blanket impls (`impl<T: Bound> Trait for T`) apply when no impl
            // of the receiver's own type provides the method.
            if inherent.is_empty() && traits.is_empty() {
                (
                    inherent,
                    self.blanket_functions(self_type, method)
                        .into_iter()
                        .map(|function| (None, function))
                        .collect(),
                )
            } else {
                (inherent, traits)
            }
        };
        let (mut inherent, mut traits) = candidates(self_type);
        // rustc's probe ends with the unsizing step `[T; N]` -> `[T]`: an
        // array receiver with no method of its own selects the slice method.
        let mut unsize = false;
        if inherent.is_empty() && traits.is_empty() && self_type == Err(BuiltinSelf::Array) {
            (inherent, traits) = candidates(Err(BuiltinSelf::Slice));
            unsize = true;
        }
        match inherent.as_slice() {
            [function] => Ok(MethodAlternatives {
                functions: vec![*function],
                same_trait: None,
                inherent: false,
                unsize,
            }),
            [] => {
                if traits.is_empty() {
                    return Err(Diagnostic::new(
                        "LRR-E-NAME-MISSING",
                        format!("no method `{method}` for the receiver's type"),
                    ));
                }
                let mut functions = Vec::new();
                for (_, function) in &traits {
                    if !functions.contains(function) {
                        functions.push(*function);
                    }
                }
                let same_trait = traits
                    .first()
                    .and_then(|(trait_key, _)| *trait_key)
                    .filter(|first| traits.iter().all(|(key, _)| *key == Some(*first)));
                Ok(MethodAlternatives {
                    functions,
                    same_trait,
                    inherent: false,
                    unsize,
                })
            }
            // Several inherent definitions are the impls of different
            // instantiations of the type (rustc rejects duplicates of one
            // instantiation): the receiver's type arguments select.
            _ => Ok(MethodAlternatives {
                functions: inherent,
                same_trait: None,
                inherent: true,
                unsize,
            }),
        }
    }
    /// The associated type alias named `name` that a trait impl for nominal
    /// `ty` (or builtin self `builtin`) declares; `trait_key` selects the trait
    /// when known. Exactly one impl must provide it.
    pub fn associated_type(
        &self,
        ty: Result<Digest, BuiltinSelf>,
        trait_key: Option<Digest>,
        name: &str,
    ) -> Result<Option<(Digest, &AssociatedTypeAlias)>, Diagnostic> {
        self.associated_type_of(ty, None, trait_key, name)
    }
    /// `associated_type` for a reference self type, whose `pointee` (the
    /// referent a `&A` names) selects among the reference impls the index
    /// buckets together; an impl naming the referent wins over one generic
    /// in it.
    pub fn associated_type_of(
        &self,
        ty: Result<Digest, BuiltinSelf>,
        pointee: Option<Result<Digest, BuiltinSelf>>,
        trait_key: Option<Digest>,
        name: &str,
    ) -> Result<Option<(Digest, &AssociatedTypeAlias)>, Diagnostic> {
        let mut candidates = self.associated_type_candidates(ty, pointee, trait_key, name);
        if candidates.len() > 1 {
            return Err(Diagnostic::new(
                "LRR-E-NAME-AMBIGUOUS",
                format!("associated type `{name}` is provided by several impls"),
            ));
        }
        Ok(candidates.pop())
    }
    /// Every associated type alias named `name` that a trait impl of the
    /// self type provides: the impls naming the type itself, else the blanket
    /// impls whose bounds it meets. Impls of one trait that differ by its
    /// arguments (`TryFrom<u8>`, `TryFrom<u16>`) are all candidates; the
    /// projection's trait arguments select among them.
    pub fn associated_type_candidates(
        &self,
        ty: Result<Digest, BuiltinSelf>,
        pointee: Option<Result<Digest, BuiltinSelf>>,
        trait_key: Option<Digest>,
        name: &str,
    ) -> Vec<(Digest, &AssociatedTypeAlias)> {
        let mut impls: Vec<&ImplMembers> = match ty {
            Ok(nominal) => self.impls_of(nominal).collect(),
            Err(builtin) => self.builtin_impls(builtin).collect(),
        };
        if let Some(pointee) = pointee {
            let named = impls
                .iter()
                .any(|members| members.self_pointee == Some(pointee));
            impls.retain(|members| {
                if named {
                    members.self_pointee == Some(pointee)
                } else {
                    members.self_pointee.is_none()
                }
            });
        }
        let mut selected: Vec<Digest> = impls
            .into_iter()
            .filter(|members| {
                members.trait_path.is_some()
                    && trait_key.is_none_or(|key| members.trait_definition == Some(key))
            })
            .filter_map(|members| members.types.get(name).copied())
            .collect();
        // A blanket impl (`impl<T: Get> Grab for T`) whose bounds the type
        // meets provides the name when no impl of the type itself does.
        if selected.is_empty() {
            selected = self
                .unresolved()
                .filter(|members| {
                    members.trait_path.is_some()
                        && trait_key.is_none_or(|key| members.trait_definition == Some(key))
                        && members.blanket.as_ref().is_some_and(|bounds| {
                            bounds
                                .iter()
                                .all(|bound| bound.is_none_or(|bound| self.implements(ty, bound)))
                        })
                })
                .filter_map(|members| members.types.get(name).copied())
                .collect();
        }
        selected
            .into_iter()
            .filter_map(|alias| self.layers().find_map(|layer| layer.type_aliases.get(&alias)).map(|row| (alias, row)))
            .collect()
    }
    /// The associated constant `name` of a type (`Type::NAME`, `Self::NAME`):
    /// an inherent impl's, else a trait impl's own, else the implemented
    /// trait's provided default; `trait_key` restricts to one trait
    /// (`<Type as Trait>::NAME`). Blanket impls apply as for methods.
    pub fn associated_const(
        &self,
        ty: Result<Digest, BuiltinSelf>,
        trait_key: Option<Digest>,
        name: &str,
    ) -> Result<Option<Digest>, Diagnostic> {
        let impls: Vec<&ImplMembers> = match ty {
            Ok(nominal) => self.impls_of(nominal).collect(),
            Err(builtin) => self.builtin_impls(builtin).collect(),
        };
        if trait_key.is_none()
            && let Some(found) = impls
                .iter()
                .filter(|members| members.trait_path.is_none())
                .find_map(|members| members.consts.get(name))
        {
            return Ok(Some(*found));
        }
        let applicable = |members: &&ImplMembers| {
            members.trait_path.is_some()
                && trait_key.is_none_or(|key| members.trait_definition == Some(key))
        };
        let blanket = self.unresolved().filter(|members| {
            members.blanket.as_ref().is_some_and(|bounds| {
                bounds
                    .iter()
                    .all(|bound| bound.is_none_or(|bound| self.implements(ty, bound)))
            })
        });
        let mut found = Vec::new();
        for members in impls.iter().copied().chain(blanket).filter(applicable) {
            if let Some(constant) = members.consts.get(name).or_else(|| {
                members
                    .trait_definition
                    .and_then(|definition| {
                        self.layers().find_map(|layer| layer.trait_consts.get(&definition))
                    })
                    .and_then(|defaults| defaults.get(name))
            }) {
                found.push(*constant);
            }
        }
        match found.as_slice() {
            [] => Ok(None),
            [one] => Ok(Some(*one)),
            _ => Err(Diagnostic::new(
                "LRR-E-NAME-AMBIGUOUS",
                format!("associated constant `{name}` is provided by several trait impls"),
            )),
        }
    }
    /// The name of a shorthand associated-type key.
    pub fn associated_type_name(&self, key: Digest) -> Option<&str> {
        self.layers()
            .find_map(|layer| layer.type_names.get(&key))
            .map(String::as_str)
    }
    /// Whether some impl's self type did not resolve to a nominal declaration.
    /// rustc's impl candidates for an integer or float inference variable
    /// `?v: Trait` (`family`: the variable's primitives): the primitives with
    /// their own impl of the trait, the bounds of each applicable blanket
    /// impl (`impl<T: B> Trait for T`), and whether some impl of the trait
    /// has a self type this index does not resolve.
    pub(crate) fn primitive_candidates(
        &self,
        trait_key: Digest,
        family: &[crate::ir::op::PrimitiveType],
    ) -> PrimitiveCandidates {
        let direct = family
            .iter()
            .copied()
            .filter(|primitive| {
                self.builtin_impls(BuiltinSelf::Primitive(*primitive))
                    .any(|members| members.trait_definition == Some(trait_key))
            })
            .collect();
        let mut blankets = Vec::new();
        let mut unresolved = false;
        for members in self
            .unresolved()
            .filter(|members| members.trait_definition == Some(trait_key))
        {
            match &members.blanket {
                Some(bounds) => blankets.push(bounds.clone()),
                None => unresolved = true,
            }
        }
        PrimitiveCandidates {
            direct,
            blankets,
            unresolved,
        }
    }
    /// rustc's method candidates for an integer or float inference variable
    /// receiver (`4.f()`): whether a primitive of `family` has an inherent
    /// `name` (rustc requires the receiver's type to be known then), and the
    /// traits whose primitive or blanket impls provide `name`.
    pub(crate) fn primitive_method_traits(
        &self,
        family: &[crate::ir::op::PrimitiveType],
        name: &str,
    ) -> (bool, Vec<Digest>) {
        let provides = |members: &ImplMembers| {
            members.functions.contains_key(name)
                || members
                    .trait_definition
                    .and_then(|definition| self.trait_defaults_of(definition))
                    .is_some_and(|defaults| defaults.contains_key(name))
        };
        let mut inherent = false;
        let mut traits = Vec::new();
        let primitive_impls = family
            .iter()
            .flat_map(|primitive| self.builtin_impls(BuiltinSelf::Primitive(*primitive)));
        let blanket_impls = self.unresolved().filter(|members| members.blanket.is_some());
        for members in primitive_impls.chain(blanket_impls) {
            match (&members.trait_path, members.trait_definition) {
                (None, _) if members.functions.contains_key(name) => inherent = true,
                (Some(_), Some(definition)) if provides(members) => {
                    if !traits.contains(&definition) {
                        traits.push(definition);
                    }
                }
                _ => {}
            }
        }
        (inherent, traits)
    }
    pub fn has_unresolved(&self) -> bool {
        self.layers().any(|layer| !layer.unresolved.is_empty())
    }
    pub fn unresolved(&self) -> impl Iterator<Item = &ImplMembers> {
        self.layers().flat_map(|layer| {
            layer
                .unresolved
                .iter()
                .map(move |ordinal| &layer.impls[*ordinal])
        })
    }
    /// The impl that owns associated function `function`, if indexed.
    pub fn owner_of(&self, function: Digest) -> Option<&ImplMembers> {
        self.layers().find_map(|layer| {
            layer
                .function_owner
                .get(&function)
                .map(|ordinal| &layer.impls[*ordinal])
        })
    }
    /// Whether a trait impl implements `Copy`: the core facade's
    /// `marker::Copy` by identity; a trait path that resolves to no source
    /// trait (a world without the facade) by its written spelling.
    fn implements_copy(&self, members: &ImplMembers) -> bool {
        match members.trait_definition {
            Some(definition) => Some(definition) == self.lang_item(|layer| layer.lang_copy),
            None => spelled(members, &[&["Copy"], &["marker", "Copy"]], "marker", "Copy"),
        }
    }
    /// Whether a trait impl implements `Drop` (as `implements_copy`).
    fn implements_drop(&self, members: &ImplMembers) -> bool {
        match members.trait_definition {
            Some(definition) => Some(definition) == self.lang_item(|layer| layer.lang_drop),
            None => spelled(members, &[&["Drop"], &["ops", "Drop"]], "ops", "Drop"),
        }
    }
    fn lang_item(&self, read: impl Fn(&MemberIndex) -> Option<Digest>) -> Option<Digest> {
        self.layers().find_map(read)
    }
    /// The `owned_box` lang item: the alloc facade's `Box` declaration.
    pub fn owned_box(&self) -> Option<Digest> {
        self.lang_item(|layer| layer.lang_box)
    }
    /// The `unsafe_cell` lang item: the core facade's `UnsafeCell`.
    pub fn unsafe_cell(&self) -> Option<Digest> {
        self.lang_item(|layer| layer.lang_unsafe_cell)
    }
    /// The growable-array lang item: the alloc facade's `Vec` declaration.
    pub fn owned_vec(&self) -> Option<Digest> {
        self.lang_item(|layer| layer.lang_vec)
    }
    /// Nominal declarations with a `Copy` impl. `None` when some trait impl's
    /// self type is unresolved and could be such an impl. The impl's own
    /// bounds are the value's fields being Copy (rustc requires every field of
    /// a `Copy` type to be `Copy`); the store checks those per value.
    pub fn copy_types(&self) -> Option<BTreeSet<Digest>> {
        if self.unresolved().any(|members| self.implements_copy(members)) {
            return None;
        }
        Some(
            self.layers()
                .flat_map(|layer| layer.impls.iter())
                .filter(|members| self.implements_copy(members))
                .filter_map(|members| members.self_nominal)
                .collect(),
        )
    }
    /// Nominal declarations with a `Drop` impl. `None` when some trait impl's
    /// self type is unresolved and could be such an impl.
    pub fn drop_types(&self) -> Option<BTreeSet<Digest>> {
        if self.unresolved().any(|members| self.implements_drop(members)) {
            return None;
        }
        Some(
            self.layers()
                .flat_map(|layer| layer.impls.iter())
                .filter(|members| self.implements_drop(members))
                .filter_map(|members| members.self_nominal)
                .collect(),
        )
    }
    /// `<ty as Drop>::drop`: the `drop` function of nominal `ty`'s `Drop`
    /// impl(s), by the same written-path inventory as `drop_types`.
    pub fn drop_functions(&self, ty: Digest) -> Vec<Digest> {
        self.impls_of(ty)
            .filter(|members| self.implements_drop(members))
            .filter_map(|members| members.functions.get("drop").copied())
            .collect()
    }
}
/// `impl<T: A + B> Trait for T where T: C`: the resolved bound traits of the
/// self parameter, when the self type is one of the impl's own type
/// parameters. `?Sized` and lifetime bounds impose nothing here.
fn blanket_bounds(
    implementation: &ast::Impl,
    resolve: &dyn Fn(Option<ast::Path>) -> Result<Option<Digest>, Diagnostic>,
) -> Result<Option<Vec<Option<Digest>>>, Diagnostic> {
    use ast::{HasGenericParams, HasName, HasTypeBounds};
    let Some(ast::Type::PathType(self_ty)) = implementation.self_ty() else {
        return Ok(None);
    };
    let Some(path) = self_ty.path() else {
        return Ok(None);
    };
    if path.qualifier().is_some() {
        return Ok(None);
    }
    let Some(name) = path
        .segment()
        .and_then(|segment| segment.name_ref())
        .map(|name| name.text().to_string())
    else {
        return Ok(None);
    };
    let Some(params) = implementation.generic_param_list() else {
        return Ok(None);
    };
    let Some(param) = params.type_or_const_params().find_map(|param| match param {
        ast::TypeOrConstParam::Type(param)
            if param.name().is_some_and(|n| n.text() == name.as_str()) =>
        {
            Some(param)
        }
        _ => None,
    }) else {
        return Ok(None);
    };
    let mut bounds = Vec::new();
    let mut add = |list: Option<ast::TypeBoundList>| -> Result<(), Diagnostic> {
        for bound in list.into_iter().flat_map(|list| list.bounds()) {
            if bound.question_mark_token().is_some() || bound.lifetime().is_some() {
                continue;
            }
            let Some(ast::Type::PathType(ty)) = bound.ty() else {
                continue;
            };
            bounds.push(resolve(ty.path())?);
        }
        Ok(())
    };
    add(param.type_bound_list())?;
    if let Some(clause) = implementation.where_clause() {
        for predicate in clause.predicates() {
            let names_param = matches!(predicate.ty(), Some(ast::Type::PathType(ty))
                if ty.path().is_some_and(|path| path.qualifier().is_none()
                    && path.segment().and_then(|segment| segment.name_ref())
                        .is_some_and(|n| n.text() == name.as_str())));
            if names_param {
                add(predicate.type_bound_list())?;
            }
        }
    }
    Ok(Some(bounds))
}
/// A trait impl's written path naming `module::name` as `name`,
/// `module::name`, `std::module::name` or `core::module::name`.
fn spelled(members: &ImplMembers, short: &[&[&str]], module: &str, name: &str) -> bool {
    members.trait_path.as_ref().is_some_and(|path| {
        let names: Vec<_> = path
            .iter()
            .map(|segment| match segment {
                ImportSegment::Name(name) => name.as_str(),
                _ => "",
            })
            .collect();
        short.contains(&names.as_slice())
            || matches!(names.as_slice(), [krate, m, n]
                if (*krate == "std" || *krate == "core") && *m == module && *n == name)
    })
}
