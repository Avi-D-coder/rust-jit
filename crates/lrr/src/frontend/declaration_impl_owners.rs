//! Canonical cold local impl rows from the same lexical declaration producer
//! used at entry. This attaches source metadata, not Type/member applicability.
use super::*;
use crate::syntax::declaration_scope_plan::DeclarationScopePlan;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
pub(super) struct LocalDeclarationLocation {
    plan: u32,
    row: u32,
    member: bool,
}
struct LocalPlan {
    parent: u32,
    plan: Arc<DeclarationScopePlan>,
    items: BTreeMap<(u32, u32), u32>,
    members: BTreeMap<(Digest, (u32, u32)), u32>,
}
pub(super) struct LocalImplOwners {
    plans: Vec<LocalPlan>,
    owners: BTreeMap<(u32, u32), LocalDeclarationLocation>,
}
impl LocalImplOwners {
    pub(super) fn prepare(
        source: &PreparedSourceView,
        scan: &DeclarationScan,
    ) -> Result<Self, Diagnostic> {
        let parsed = source.retained_declaration_parser()?;
        let cfg = source.captured_macro_cfg()?;
        if parsed.key() != *scan.parse() || cfg.fingerprint() != scan.cfg() {
            return Err(fault("cold impl owner source/cfg differs"));
        }
        let outline = source.outline();
        let functions: BTreeMap<_, _> = outline
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.def_key.item_kind == ItemKind::Function && item.body.is_some())
            .map(|(index, item)| (key(item.declaration_span), index))
            .collect();
        let mut prepared = Self {
            plans: Vec::new(),
            owners: BTreeMap::new(),
        };
        let mut by_function = BTreeMap::new();
        for raw in scan.impls() {
            let Some(parent) = raw
                .containers
                .iter()
                .rev()
                .find_map(|range| functions.get(&key(*range)))
                .copied()
            else {
                continue;
            };
            let function = &outline.items[parent];
            let body = function
                .body
                .as_ref()
                .ok_or_else(|| fault("cold impl parent body absent"))?;
            if body.entry_blocked {
                continue;
            }
            let plan_index = if let Some(index) = by_function.get(&parent) {
                *index
            } else {
                publication(source, function)?;
                let plan = parsed.declaration_scope_plan(body, cfg, &function.scope)?;
                if plan.parse() != scan.parse()
                    || plan.body() != body
                    || plan.scope() != &function.scope
                    || plan.cfg() != scan.cfg()
                {
                    return Err(fault("cold impl canonical parent differs"));
                }
                let mut items = BTreeMap::new();
                let mut members = BTreeMap::new();
                for (index, row) in plan.declarations().items.iter().enumerate() {
                    if items
                        .insert(
                            key(row.item.declaration_span),
                            u32::try_from(index).map_err(|_| resource())?,
                        )
                        .is_some()
                    {
                        return Err(fault("cold declaration range has multiple owners"));
                    }
                }
                for (index, row) in plan.declarations().members.iter().enumerate() {
                    let owner = row
                        .item
                        .scope
                        .owner
                        .ok_or_else(|| fault("cold associated member owner absent"))?;
                    if members
                        .insert(
                            (owner, key(row.item.declaration_span)),
                            u32::try_from(index).map_err(|_| resource())?,
                        )
                        .is_some()
                    {
                        return Err(fault("cold associated range has multiple owners"));
                    }
                }
                let index = u32::try_from(prepared.plans.len()).map_err(|_| resource())?;
                prepared.plans.try_reserve(1).map_err(|_| resource())?;
                prepared.plans.push(LocalPlan {
                    parent: u32::try_from(parent).map_err(|_| resource())?,
                    plan,
                    items,
                    members,
                });
                by_function.insert(parent, index);
                index
            };
            let plan = &prepared.plans[plan_index as usize];
            let Some(index) = plan.items.get(&key(raw.range)).copied() else {
                // A nested function/method has its own canonical owner. The
                // outer body's plan is not a permit to invent that identity.
                continue;
            };
            let row = &plan.plan.declarations().items[index as usize];
            if row.attribute_input.is_some() {
                continue;
            }
            let item = &row.item;
            if item.def_key.item_kind != ItemKind::Impl
                || item.def_key.namespace != Namespace::Type
                || item.header.tokens != raw.header
                || item.header.profile != scan.parse().profile
                || item.scope.crate_key != raw.scope.crate_key
                || item.scope.module_path != raw.scope.module_path
                || item.def_key.enclosing_impl.is_none()
                || !body.range.contains(raw.range)
            {
                return Err(fault("cold impl actual canonical attachment differs"));
            }
            if prepared
                .owners
                .insert(
                    key(raw.range),
                    LocalDeclarationLocation {
                        plan: plan_index,
                        row: index,
                        member: false,
                    },
                )
                .is_some()
            {
                return Err(fault("cold impl canonical owner is duplicated"));
            }
        }
        Ok(prepared)
    }
    pub(super) fn scopes(&self) -> impl Iterator<Item = (u32, &Arc<DeclarationScopePlan>)> {
        self.plans.iter().map(|scope| (scope.parent, &scope.plan))
    }
    pub(super) fn owner(&self, range: SourceRange) -> Option<LocalDeclarationLocation> {
        self.owners.get(&key(range)).copied()
    }
    pub(super) fn scope(
        &self,
        location: LocalDeclarationLocation,
    ) -> (u32, &Arc<DeclarationScopePlan>) {
        let scope = &self.plans[location.plan as usize];
        (scope.parent, &scope.plan)
    }
    pub(super) fn item(&self, location: LocalDeclarationLocation) -> &ItemOutline {
        let facts = self.plans[location.plan as usize].plan.declarations();
        let rows = if location.member {
            &facts.members
        } else {
            &facts.items
        };
        &rows[location.row as usize].item
    }
    pub(super) fn member(
        &self,
        owner: LocalDeclarationLocation,
        range: SourceRange,
    ) -> Option<LocalDeclarationLocation> {
        let child_owner = self.item(owner).def_key.enclosing_impl?;
        let row = *self.plans[owner.plan as usize]
            .members
            .get(&(child_owner, key(range)))?;
        let member = &self.plans[owner.plan as usize].plan.declarations().members[row as usize];
        member
            .attribute_input
            .is_none()
            .then_some(LocalDeclarationLocation {
                plan: owner.plan,
                row,
                member: true,
            })
    }
}
