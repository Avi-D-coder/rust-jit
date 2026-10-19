"""Project actual pinned compiler constraint representation/mutators into the
existing compatible region store. No custom relation, visibility or leak rule.
"""
import hashlib
from pathlib import Path
from sync_upstream_binders import method
from upstream_solver_regions import project

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_infer/src/infer/region_constraints/mod.rs'
SHA = '01e53909ea1a62b3415889611c7593e3d226c3a1eadf19f6dee4525b368fd5cb'


def selected(source, name):
    # Visibility only: method() selects complete bodies, including generics.
    return method(source.replace('    pub(super) fn ', '    pub fn ')
                        .replace('    fn ', '    pub fn '), name)


def adapt(text):
    text = project(text).replace("Region<'tcx>", "Region<'db>")
    text = text.replace('ty::VisibleForLeakCheck', 'ty::solve::VisibleForLeakCheck')
    text = text.replace("SubregionOrigin<'db>", 'Span')
    text = text.replace('RegionVariableOrigin::Misc(origin.span())', 'origin')
    text = text.replace('RegionVariableValue::Known { value: reg }', 'RegionVariableValue::Known { value: reg, span: None }')
    text = text.replace('RegionVariableValue::Known { value }', 'RegionVariableValue::Known { value, .. }')
    text = text.replace('RegionVariableValue::Unknown { universe }', 'RegionVariableValue::Unknown { universe, .. }')
    for op, left, right in [('==', 'a', 'b'), ('<=', 'sub', 'sup')]:
        text = text.replace('span_bug!(origin.span(), "cannot relate bound region: {:?} ' + op + ' {:?}", ' + left + ', ' + right + ')',
                            'panic!("cannot relate bound region at {:?}: {:?} ' + op + ' {:?}", origin, ' + left + ', ' + right + ')')
    return text


def replace_method(body, name, replacement):
    old = selected(body, name)
    # Restore the original source visibility in the lookup, then replace once.
    actual = old
    if actual not in body:
        actual = actual.replace('    pub fn ', '    pub(super) fn ', 1)
    if actual not in body:
        actual = old.replace('    pub fn ', '    fn ', 1)
    if body.count(actual) != 1:
        raise ValueError('region collector method anchor differs: ' + name)
    return body.replace(actual, replacement, 1)


def extend_collector(body, rustfmt):
    raw = SOURCE.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SHA:
        raise ValueError('compiler region collector original differs')
    compiler = raw.decode()
    start = '/// Represents a constraint that influences the inference process.'
    stop = '#[derive(Debug, Clone)]\npub struct Verify'
    current = compiler[compiler.index(start):compiler.index(stop)]
    body = body[:body.index(start)] + adapt(current) + body[body.index(stop):]
    body = body.replace('pub constraints: Vec<Constraint<\'db>>,', "pub constraints: Vec<(Constraint<'db>, Span)>,", 1)
    body = body.replace('use rustc_type_ir::{RegionKind, RegionVid, UniverseIndex};',
        'use rustc_type_ir::{self as ty, RegionKind, RegionVid, UniverseIndex};\nuse rustc_type_ir::RegionKind::{ReBound, ReVar, ReStatic};', 1)
    body = body.replace('use std::ops::Range;', 'use std::ops::Range;\nuse std::iter;', 1)
    for name in ('add_constraint', 'make_eqregion', 'make_subregion', 'combine_vars'):
        replacement = adapt(selected(compiler, name))
        replacement = replacement.replace('    pub fn ', '    pub(super) fn ' if name.startswith('make_') else '    fn ', 1)
        body = replace_method(body, name, replacement)
    for old, new in [('opportunistic_resolve_var', 'shallow_resolve_region_var'), ('probe_value', 'try_resolve_region_var')]:
        body = replace_method(body, old, adapt(selected(compiler, new)))
    body = body.replace('match self.probe_value(vid)', 'match self.try_resolve_region_var(vid)', 1)
    # Same compiler Copy records: all fields are copyable, no ownership algorithm.
    body = body.replace('#[derive(Debug, Clone, PartialEq, Eq, Hash)]\npub(crate) struct TwoRegions',
                        '#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]\npub(crate) struct TwoRegions', 1)
    body = body.replace('#[derive(Clone, PartialEq)]\npub(crate) enum CombineMapType',
                        '#[derive(Copy, Clone, PartialEq)]\npub(crate) enum CombineMapType', 1)
    return rustfmt(body)
