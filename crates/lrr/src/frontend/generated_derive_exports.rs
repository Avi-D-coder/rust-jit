//! Ordered pure derive-export choices. No helper metadata or execution grant.
use crate::{
    core::{
        diagnostic::Diagnostic,
        digest::{Digest, StableEncode, StableEncoder},
    },
    expand::{
        generated_derives::{GeneratedDerivePath, GeneratedDerivePlan},
        generated_items::GeneratedItems,
        identity::{MacroIdentity, ProceduralKind},
        scopes::MacroDefinition,
    },
    frontend::{
        macro_names::{MacroNamespaces, MacroPath, PathLookup},
        procedural_exports::PreparedProcCatalog,
    },
    syntax::{
        imports::ImportSegment,
        outline::ScopeKey,
        token_wire::{Edition, Key, SpanRecord, TokenTree},
    },
};
use std::collections::BTreeMap;
fn fault(code: &'static str, message: &str) -> Diagnostic {
    Diagnostic::new(code, message.into())
}
#[derive(Debug)]
pub struct SelectedDeriveExport {
    definition: MacroDefinition,
    identity: MacroIdentity,
}
impl SelectedDeriveExport {
    pub fn definition(&self) -> &MacroDefinition {
        &self.definition
    }
    pub fn identity(&self) -> &MacroIdentity {
        &self.identity
    }
}
pub struct GeneratedDeriveExportRow {
    occurrence: Digest,
    candidate: PathLookup,
    selected: Option<SelectedDeriveExport>,
    helper_demand: Option<Digest>,
}
impl GeneratedDeriveExportRow {
    pub fn occurrence(&self) -> Digest {
        self.occurrence
    }
    pub fn candidate(&self) -> &PathLookup {
        &self.candidate
    }
    pub fn selected(&self) -> Option<&SelectedDeriveExport> {
        self.selected.as_ref()
    }
    pub fn helper_demand(&self) -> Option<Digest> {
        self.helper_demand
    }
}
pub struct GeneratedDeriveExports {
    context: Digest,
    catalog: Digest,
    origin: crate::expand::record::ExpansionDescriptor,
    rows: Vec<GeneratedDeriveExportRow>,
}
impl GeneratedDeriveExports {
    pub(crate) fn resolve(
        plan: &GeneratedDerivePlan,
        items: &GeneratedItems,
        scope: ScopeKey,
        names: &MacroNamespaces,
        catalog: &PreparedProcCatalog,
    ) -> Result<Self, Diagnostic> {
        if scope.crate_key != catalog.consumer()
            || items.epoch() != catalog.epoch()
            || items.record().key().consumer_compile_env != catalog.environment()
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated derive catalog consumer/environment/epoch differs",
            ));
        }
        let catalog_id = catalog.fingerprint();
        let mut e = StableEncoder::new("lrr/v2/generated-derive-export-context");
        e.record(&scope);
        e.record(items.record().key());
        e.digest(Digest::of_bytes(items.record().wire()));
        e.u64(items.epoch());
        e.digest(plan.facts().item.cfg);
        e.digest(plan.item_identity());
        e.digest(catalog_id);
        let context = e.finish();
        let count = plan
            .annotations()
            .iter()
            .try_fold(0usize, |n, a| n.checked_add(a.paths().len()))
            .filter(|n| *n <= 4096)
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated derive export count limit",
                )
            })?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count).map_err(|_| {
            fault(
                "LRR-E-DISCOVERY-LIMIT",
                "generated derive export allocation",
            )
        })?;
        let mut bytes = rows
            .capacity()
            .checked_mul(std::mem::size_of::<GeneratedDeriveExportRow>())
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "generated derive export size overflow",
                )
            })?;
        for annotation in plan.annotations() {
            for path in annotation.paths() {
                let candidate = lookup(path, items, &scope, names)?;
                let extra = match &candidate {
                    PathLookup::Ambiguous(defs) => defs
                        .capacity()
                        .checked_mul(std::mem::size_of::<MacroDefinition>()),
                    PathLookup::Inaccessible(paths) => paths.iter().try_fold(0usize, |n, path| {
                        path.components().iter().try_fold(n, |n, name| {
                            n.checked_add(name.as_str().len())
                                .and_then(|n| n.checked_add(128))
                        })
                    }),
                    _ => Some(128),
                };
                bytes = extra
                    .and_then(|n| bytes.checked_add(n))
                    .filter(|n| *n <= 64 * 1024 * 1024)
                    .ok_or_else(|| {
                        fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "generated derive export metadata limit",
                        )
                    })?;
                let selected = match &candidate {
                    PathLookup::Defined(definition) => catalog
                        .select(definition, ProceduralKind::Derive)?
                        .map(|export| SelectedDeriveExport {
                            definition: definition.clone(),
                            identity: export.identity().clone(),
                        }),
                    _ => None,
                };
                let helper_demand = if let Some(export) = &selected {
                    if let MacroIdentity::Procedural { export: name, .. } = &export.identity {
                        bytes = bytes
                            .checked_add(name.capacity())
                            .filter(|n| *n <= 64 * 1024 * 1024)
                            .ok_or_else(|| {
                                fault(
                                    "LRR-E-DISCOVERY-LIMIT",
                                    "generated derive export name retention limit",
                                )
                            })?;
                    }
                    let mut e =
                        StableEncoder::new("lrr/v2/generated-derive-helper-declaration-demand");
                    e.digest(path.identity);
                    e.digest(catalog_id);
                    e.digest(export.definition.identity);
                    e.digest(export.definition.defining_unit);
                    e.u16(export.definition.definition_edition);
                    Some(e.finish())
                } else {
                    None
                };
                rows.push(GeneratedDeriveExportRow {
                    occurrence: path.identity,
                    candidate,
                    selected,
                    helper_demand,
                });
            }
        }
        Ok(Self {
            context,
            catalog: catalog_id,
            origin: items.record().descriptor(),
            rows,
        })
    }
    /// An immutable demand; the worker still requires the actual parent receipt.
    pub fn declaration_request(
        &self,
        index: usize,
        catalog: &PreparedProcCatalog,
    ) -> Result<DeriveDeclarationRequest, Diagnostic> {
        if self.catalog != catalog.fingerprint() {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "derive declaration catalog changed",
            ));
        }
        let row = self.rows.get(index).ok_or_else(|| {
            fault(
                "LRR-E-EXPANSION-INPUT",
                "derive declaration occurrence absent",
            )
        })?;
        let selected = row
            .selected
            .as_ref()
            .ok_or_else(|| fault("LRR-E-CATALOG-INCOMPLETE", "derive export is unresolved"))?;
        let demand = row.helper_demand.ok_or_else(|| {
            fault(
                "LRR-E-CATALOG-INCOMPLETE",
                "derive declaration demand absent",
            )
        })?;
        let bytes = self
            .origin
            .estimated_bytes()
            .checked_add(match &selected.identity {
                MacroIdentity::Procedural { export, .. } => export.capacity(),
                _ => 0,
            })
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "derive declaration request metadata limit",
                )
            })?;
        let _ = bytes;
        Ok(DeriveDeclarationRequest {
            demand,
            occurrence: row.occurrence,
            context: self.context,
            catalog: self.catalog,
            origin: crate::expand::derive_origin::DeriveDeclarationOrigin::Generated(
                self.origin.clone(),
            ),
            definition: selected.definition.clone(),
            identity: selected.identity.clone(),
            consumer: catalog.consumer(),
            graph: catalog.graph(),
            epoch: catalog.epoch(),
            environment: catalog.environment(),
            preparation: catalog.preparation(),
        })
    }
    pub fn rows(&self) -> &[GeneratedDeriveExportRow] {
        &self.rows
    }
    /// A ready export still requires declarations. Absence remains unresolved data.
    pub fn first_demand(&self) -> Result<Option<Digest>, Diagnostic> {
        self.row_demand(0)
    }
    pub(crate) fn row_demand(&self, index: usize) -> Result<Option<Digest>, Diagnostic> {
        let Some(row) = self.rows.get(index) else {
            return Ok(None);
        };
        match &row.candidate {
            PathLookup::Ambiguous(_) => Err(fault(
                "LRR-E-MACRO-AMBIGUOUS",
                "generated derive path is ambiguous",
            )),
            PathLookup::Inaccessible(_) => Err(fault(
                "LRR-E-MACRO-INACCESSIBLE",
                "generated derive path is inaccessible",
            )),
            PathLookup::NeedInput(input) => Ok(Some(*input)),
            _ => Ok(Some(row.helper_demand.unwrap_or(row.occurrence))),
        }
    }
    pub fn validate_replay(&self, current: &Self) -> Result<(), Diagnostic> {
        if self.context != current.context
            || self.rows.len() != current.rows.len()
            || self
                .rows
                .iter()
                .zip(&current.rows)
                .any(|(old, new)| old.occurrence != new.occurrence)
        {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "generated derive export context/membership differs",
            ));
        }
        for (old, new) in self.rows.iter().zip(&current.rows) {
            let Some(selected) = &old.selected else {
                continue;
            };
            match &new.candidate {
                PathLookup::Ambiguous(_) => {
                    return Err(fault(
                        "LRR-E-MACRO-AMBIGUOUS",
                        "selected generated derive became ambiguous",
                    ));
                }
                PathLookup::Inaccessible(_) => {
                    return Err(fault(
                        "LRR-E-MACRO-INACCESSIBLE",
                        "selected generated derive became inaccessible",
                    ));
                }
                PathLookup::Absent | PathLookup::NeedInput(_) => {
                    return Err(fault(
                        "LRR-E-CATALOG-INCOMPLETE",
                        "selected generated derive is pending or absent",
                    ));
                }
                PathLookup::Defined(definition)
                    if definition == &selected.definition
                        && new
                            .selected
                            .as_ref()
                            .is_some_and(|s| s.identity == selected.identity)
                        && old.helper_demand == new.helper_demand => {}
                _ => {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "selected generated derive identity changed",
                    ));
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
pub struct DeriveDeclarationRequest {
    pub(crate) demand: Digest,
    pub(crate) occurrence: Digest,
    context: Digest,
    pub(crate) catalog: Digest,
    pub(crate) origin: crate::expand::derive_origin::DeriveDeclarationOrigin,
    pub(crate) definition: MacroDefinition,
    pub(crate) identity: MacroIdentity,
    pub(crate) consumer: Digest,
    pub(crate) graph: Digest,
    pub(crate) epoch: u64,
    pub(crate) environment: Digest,
    pub(crate) preparation: Digest,
}
impl DeriveDeclarationRequest {
    pub(crate) fn from_source(
        source: &crate::source::changes::PreparedSourceView,
        item: &crate::syntax::outline::ItemOutline,
        producer: Digest,
        occurrence: Digest,
        definition: MacroDefinition,
        catalog: &PreparedProcCatalog,
    ) -> Result<Option<Self>, Diagnostic> {
        if catalog.consumer() != source.outline().root_scope.crate_key {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "source derive catalog consumer differs",
            ));
        }
        let Some(export) = catalog.select(&definition, ProceduralKind::Derive)? else {
            return Ok(None);
        };
        let export_bytes = match export.identity() {
            MacroIdentity::Procedural { export, .. } => export.capacity(),
            _ => 0,
        };
        source
            .selection()
            .metadata_bytes()?
            .checked_add(export_bytes)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() + 128))
            .filter(|n| *n <= 32 * 1024 * 1024)
            .ok_or_else(|| {
                fault(
                    "LRR-E-DISCOVERY-LIMIT",
                    "source derive request allocation limit",
                )
            })?;
        let origin = crate::expand::derive_origin::DeriveDeclarationOrigin::Source {
            selection: source.selection().clone(),
            producer,
            item: item.def_key.fingerprint(),
            header: item.header.fingerprint(),
        };
        let mut e = StableEncoder::new("lrr/v2/source-derive-export-context");
        e.record(&origin);
        e.digest(catalog.fingerprint());
        e.digest(occurrence);
        let context = e.finish();
        let mut e = StableEncoder::new("lrr/v2/source-derive-helper-declaration-demand");
        e.digest(context);
        e.digest(occurrence);
        e.digest(definition.identity);
        let MacroIdentity::Procedural {
            artifact,
            export: name,
            kind: ProceduralKind::Derive,
        } = export.identity()
        else {
            return Err(fault(
                "LRR-E-EXPANSION-INPUT",
                "source derive export identity kind differs",
            ));
        };
        e.digest(*artifact);
        e.string(name);
        e.u8(2);
        let request = Self {
            demand: e.finish(),
            occurrence,
            context,
            catalog: catalog.fingerprint(),
            origin,
            definition,
            identity: export.identity().clone(),
            consumer: catalog.consumer(),
            graph: catalog.graph(),
            epoch: catalog.epoch(),
            environment: catalog.environment(),
            preparation: catalog.preparation(),
        };
        if request.metadata_bytes()? > 32 * 1024 * 1024 {
            return Err(fault(
                "LRR-E-DISCOVERY-LIMIT",
                "source derive request retention limit",
            ));
        }
        Ok(Some(request))
    }
    pub(crate) fn metadata_bytes(&self) -> Result<usize, Diagnostic> {
        let export = match &self.identity {
            MacroIdentity::Procedural { export, .. } => export.capacity(),
            _ => 0,
        };
        std::mem::size_of::<Self>()
            .checked_add(export)
            .and_then(|n| n.checked_add(self.origin.metadata_bytes().ok()?))
            .ok_or_else(|| fault("LRR-E-METADATA-LIMIT", "derive request metadata overflow"))
    }

    pub fn demand(&self) -> Digest {
        self.demand
    }
    pub fn occurrence(&self) -> Digest {
        self.occurrence
    }
}
impl StableEncode for DeriveDeclarationRequest {
    const DOMAIN: &'static str = "lrr/v2/derive-declaration-request/source-or-generated-v1";
    fn encode_fields(&self, e: &mut StableEncoder) {
        e.digest(self.demand);
        e.digest(self.occurrence);
        e.digest(self.context);
        e.digest(self.catalog);
        e.record(&self.origin);
        e.digest(self.definition.identity);
        e.digest(self.definition.defining_unit);
        e.u16(self.definition.definition_edition);
        e.digest(self.consumer);
        e.digest(self.graph);
        e.u64(self.epoch);
        e.digest(self.environment);
        e.digest(self.preparation);
        if let MacroIdentity::Procedural {
            artifact,
            export,
            kind,
        } = &self.identity
        {
            e.u8(2);
            e.digest(*artifact);
            e.string(export);
            e.u8(match kind {
                ProceduralKind::Function => 0,
                ProceduralKind::Attribute => 1,
                ProceduralKind::Derive => 2,
            });
        } else {
            e.u8(0);
        }
    }
}
fn lookup(
    path: &GeneratedDerivePath,
    items: &GeneratedItems,
    scope: &ScopeKey,
    names: &MacroNamespaces,
) -> Result<PathLookup, Diagnostic> {
    let spans = path
        .tokens()
        .spans()
        .iter()
        .map(|s| (s.key(), s))
        .collect::<BTreeMap<_, _>>();
    let mut keys = Vec::new();
    fn visit(tokens: &[TokenTree], keys: &mut Vec<Key>, depth: usize) -> Result<(), Diagnostic> {
        if depth > 256 {
            return Err(fault(
                "LRR-E-DISCOVERY-LIMIT",
                "derive path token depth limit",
            ));
        }
        for token in tokens {
            match token {
                TokenTree::Group { children, .. } => visit(children, keys, depth + 1)?,
                TokenTree::Ident { span, .. } | TokenTree::Punct { span, .. } => {
                    if keys.len() >= 2_000_000 {
                        return Err(fault(
                            "LRR-E-DISCOVERY-LIMIT",
                            "derive path token count limit",
                        ));
                    }
                    keys.try_reserve(1).map_err(|_| {
                        fault("LRR-E-DISCOVERY-LIMIT", "derive path token allocation")
                    })?;
                    keys.push(*span);
                }
                _ => {
                    return Err(fault(
                        "LRR-E-EXPANSION-INPUT",
                        "derive path has a non-path token",
                    ));
                }
            }
        }
        Ok(())
    }
    visit(path.tokens().roots(), &mut keys, 0)?;
    let first = spans
        .get(&path.first_span)
        .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "derive path first span absent"))?;
    let hygiene = items.hygiene_facts();
    let class = hygiene.name_item(Key(first.context.0))?;
    for key in keys {
        let span: &&SpanRecord = spans
            .get(&key)
            .ok_or_else(|| fault("LRR-E-EXPANSION-INPUT", "derive path span absent"))?;
        if hygiene.name_item(Key(span.context.0))? != class || span.edition != first.edition {
            return Ok(PathLookup::NeedInput(path.identity));
        }
    }
    let edition = match first.edition {
        Edition::E2015 => 2015,
        Edition::E2018 => 2018,
        Edition::E2021 => 2021,
        Edition::E2024 => 2024,
    };
    let segments = path
        .segments
        .iter()
        .map(|name| match name.as_str() {
            "crate" => ImportSegment::Crate,
            "self" => ImportSegment::SelfModule,
            "super" => ImportSegment::Super,
            _ => ImportSegment::Name(name.clone()),
        })
        .collect::<Vec<_>>();
    if path.segments.first().is_some_and(|name| name == "$crate") {
        let unit = Digest(hygiene.dollar_crate(Key(first.context.0))?.0);
        let components = path.segments[1..]
            .iter()
            .map(|name| crate::expand::scopes::MacroName::parse(name, edition))
            .collect::<Result<Vec<_>, _>>()?;
        names.lookup_from(&MacroPath::new(unit, components)?, scope)
    } else {
        names.lookup_invocation_segments(&segments, path.absolute, scope, edition)
    }
}
