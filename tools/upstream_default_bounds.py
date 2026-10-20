#!/usr/bin/env python3
"""Generate/audit pinned implicit-sizedness/default-trait selection.

The HIR enumeration is replaced by immutable source-bound rows selected using
actual Self/parameter identity (the common collector already combines direct
and same-parameter where bounds). All selection bodies are retained. Only
compiler-context/types/IDs/polarity/trait construction are adapted. No lowering,
static diagnostics, trait solver, or normalization is reproduced here.
"""
import argparse
import hashlib
import re
import subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_hir_analysis/src/hir_ty_lowering/bounds.rs'
TARGET = ROOT / 'crates/lrr/src/traits/upstream/source_default_bounds.rs'
PIN = 'd1a9036440b3611b71735a02dc9ec4cd1a572fb0715418cfeb79bfe4c5c1cbf9'
BEGIN = '// BEGIN PINNED DEFAULT BOUNDS POLICY\n'
END = '// END PINNED DEFAULT BOUNDS POLICY'

def expected():
    raw = SOURCE.read_bytes()
    if hashlib.sha256(raw).hexdigest() != PIN:
        raise ValueError('pinned default-bound source differs')
    text = raw.decode()
    start = text.index('#[derive(Debug, Default)]\nstruct CollectedBound')
    stop = text.index("fn search_bounds_for<'tcx>")
    structs = text[start:stop]
    # Enumeration is an adapter, not a second parser. Source rows have already
    # been resolved/filtered to the exact subject with authentic spans/polarity.
    search = '''fn search_bounds_for(
    hir_bounds: &[SourceBound],
    _context: ImpliedBoundsContext,
    mut f: impl FnMut(&SourceBound),
) {
    for bound in hir_bounds { f(bound); }
}
'''
    collect = text[text.index("fn collect_bounds<'a, 'tcx>"):text.index('/// Add a trait bound for `did`.')]
    collect = collect.replace("fn collect_bounds<'a, 'tcx>", 'fn collect_bounds')
    collect = collect.replace("fn collect_sizedness_bounds<'tcx>", 'fn collect_sizedness_bounds')
    collect = collect.replace("&'a [hir::GenericBound<'tcx>]", '&[SourceBound]')
    collect = collect.replace("&[hir::GenericBound<'_>]", '&[SourceBound]')
    collect = collect.replace("ImpliedBoundsContext<'tcx>", 'ImpliedBoundsContext')
    collect = collect.replace("TyCtxt<'tcx>", "PolicyCx<'_>").replace('DefId', 'Digest')
    collect = collect.replace('!matches!(ptr.trait_ref.path.res, Res::Def(DefKind::Trait, did) if did == target_did)', 'ptr.trait_id != target_did')
    collect = collect.replace('ptr.modifiers.polarity', 'ptr.polarity')
    collect = collect.replace('hir::BoundPolarity::Maybe(_)', 'BoundPolarity::Maybe')
    collect = collect.replace('hir::BoundPolarity::Negative(_)', 'BoundPolarity::Negative')
    collect = collect.replace('hir::BoundPolarity::Positive', 'BoundPolarity::Positive')
    add = text[text.index("fn add_trait_bound<'tcx>"):text.index("impl<'tcx> dyn HirTyLowerer")]
    add = add.replace("fn add_trait_bound<'tcx>", 'fn add_trait_bound')
    add = add.replace("TyCtxt<'tcx>", "PolicyCx<'_>").replace('DefId', 'Digest')
    add = add.replace("ty::Clause<'tcx>", 'TraitRef').replace("Ty<'tcx>", '&TyTerm')
    add = add.replace('let trait_ref = ty::TraitRef::new(tcx, did, [self_ty]);',
                      'let _ = tcx;\n    let trait_ref = TraitRef { trait_key: did, self_ty: self_ty.clone(), args: Vec::new() };')
    add = add.replace('trait_ref.upcast(tcx)', 'trait_ref')
    methods = text[text.index("impl<'tcx> dyn HirTyLowerer<'tcx> + '_ {"):text.index('    pub(crate) fn require_bound_to_relax_default_trait(')]
    methods = methods[:methods.rfind('\n')] + '\n}\n'
    methods = methods.replace("impl<'tcx> dyn HirTyLowerer<'tcx> + '_", "impl PolicyLowerer<'_>")
    methods = methods.replace("ty::Clause<'tcx>", 'TraitRef').replace("Ty<'tcx>", '&TyTerm')
    methods = methods.replace("&[hir::GenericBound<'_>]", '&[SourceBound]')
    methods = methods.replace("ImpliedBoundsContext<'tcx>", 'ImpliedBoundsContext').replace('DefId', 'Digest')
    methods = methods.replace('find_attr!(tcx, crate, RustcNoImplicitBounds)', 'tcx.no_implicit()')
    methods = methods.replace('find_attr!(self.tcx(), crate, RustcNoImplicitBounds)', 'self.tcx().no_implicit()')
    methods = methods.replace('let trait_did = trait_did.to_def_id();', 'let trait_did = trait_did;')
    methods = methods.replace('tcx.lang_items().get(trait_)', 'tcx.lang_item(trait_)')
    # Clause construction is the immutable source TraitRef bridge above;
    # inference/type-IR encoding is still the existing common consumer.
    context_path = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_middle/src/ty/context.rs'
    context_raw = context_path.read_bytes()
    if hashlib.sha256(context_raw).hexdigest() != '442e5ac01600b2274bb6456aa6319c4e85a0266afb3245cb23a614f7d72ae9f6':
        raise ValueError('pinned default-trait list source differs')
    context = context_raw.decode()
    default = context[context.index("    pub fn default_traits(self)"):context.index('    pub fn is_default_trait(self')]
    default = default.replace('pub fn default_traits', 'fn default_traits')
    default = default.replace('self.sess.opts.unstable_opts.experimental_default_bounds', 'self.experimental_default_bounds()')
    default = "impl PolicyCx<'_> {\n" + default + "}\n"
    result = structs + search + collect + add + methods + default
    return subprocess.run(['rustfmt', '+1.99.0', '--edition', '2024'], input=result.encode(),
                          check=True, capture_output=True).stdout.decode()

def audit_configuration_and_facade():
    options = ROOT / 'vendor/rustc-trait-solver/compiler/rustc_session/src/options.rs'
    raw = options.read_bytes()
    if hashlib.sha256(raw).hexdigest() != '36f697a2b27db7c037be556a8477f6f6e5c611384d390630faa5a3bbd9dc5c9e':
        raise ValueError('pinned compiler-option defaults differ')
    if 'experimental_default_bounds: bool = (false, parse_bool, [TRACKED],' not in raw.decode():
        raise ValueError('experimental-default-bounds compiler default differs')
    configuration = (ROOT / 'crates/lrr/src/traits/upstream/configuration.rs').read_text()
    if not re.search(r'fn experimental_default_bounds\(self\) -> bool\s*\{\s*false\s*\}', configuration):
        raise ValueError('authoritative no-override option adapter differs')
    marker = ROOT / 'vendor/rustc-trait-solver/library/core/src/marker.rs'
    original = marker.read_bytes()
    if hashlib.sha256(original).hexdigest() != '95ffa834dd6e2a8214c6e8a919cd69f4602982021b09571efc71fc2386440223':
        raise ValueError('pinned marker hierarchy differs')
    semantic_attribute = r'#\[(?:lang = "[^"]+"|fundamental|rustc_specialization_trait|rustc_deny_explicit_impl|rustc_dyn_incompatible_trait|rustc_coinductive)\]'
    def shapes(text):
        result = []
        for m in re.finditer(r'#\[lang = "(sized|meta_sized|pointee_sized)"\](.*?)pub trait ([^{]+)\{([^}]+|)\}', text, re.S):
            attrs = re.findall(semantic_attribute, m.group(0).split('pub trait')[0])
            body = re.sub(r'//[^\n]*', '', m.group(4)).strip()
            result.append((m.group(1), attrs, ' '.join(m.group(3).split()), body))
        return result
    facade = (ROOT / 'crates/lrr/facade/core/marker.rs').read_text()
    if shapes(original.decode()) != shapes(facade) or len(shapes(facade)) != 3:
        raise ValueError('source marker hierarchy/semantic attributes differ')

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    text = TARGET.read_text()
    prefix, rest = text.split(BEGIN, 1)
    _, suffix = rest.split(END, 1)
    audit_configuration_and_facade()
    policy = expected()
    if args.write:
        TARGET.write_text(prefix + BEGIN + policy + END + suffix)
    elif rest.split(END,1)[0] != policy:
        raise ValueError('default-bound complete policy port differs')
    print('Pinned implicit-sizedness/default-trait policy matches registered source-context adaptations.')
if __name__ == '__main__':
    main()
