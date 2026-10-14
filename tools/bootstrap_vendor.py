#!/usr/bin/env python3
"""Install/audit the exact macro source snapshot; never execute upstream tooling."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import shutil
import sys
import tarfile
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
TREE = ROOT / 'vendor/rust-analyzer-macros'
INVENTORY = ROOT / 'vendor/rust-analyzer-macros.files.json'
ARCHIVE = ROOT / 'vendor/rust-analyzer-macros.tar.gz'
VCS = ROOT / 'vendor/pinned-syntax-vcs-info.json'
MAX_ARCHIVE = 64 * 1024 * 1024
MAX_TREE = 256 * 1024 * 1024
MAX_FILES = 10000


def sha(data):
    return hashlib.sha256(data).hexdigest()


def commit_from_vcs(path):
    record = json.loads(path.read_text())
    commit = record['git']['sha1']
    if not re.fullmatch('[0-9a-f]{40}', commit):
        raise ValueError('invalid source commit')
    if record['path_in_vcs'] != 'crates/syntax':
        raise ValueError('unexpected syntax package source path')
    return commit


def tree_inventory(tree):
    entries = []
    for p in sorted(tree.rglob('*')):
        if p.is_symlink():
            if not p.resolve().is_relative_to(tree.resolve()):
                raise ValueError('vendored symlink escapes source tree')
            entries.append({'path':p.relative_to(tree).as_posix(), 'symlink':p.readlink().as_posix()})
        elif p.is_file():
            entries.append({'path': p.relative_to(tree).as_posix(),
                            'bytes': p.stat().st_size, 'sha256': sha(p.read_bytes())})
    return entries


def apply_text_patch(original, patch, target):
    """Apply one exact unified diff in memory, rejecting any unexpected preimage/context."""
    lines = patch.decode('utf-8').splitlines(keepends=True)
    if lines[:2] != [f'--- a/{target}\n', f'+++ b/{target}\n']:
        raise ValueError('patch target header mismatch')
    source = original.decode('utf-8').splitlines(keepends=True)
    output, cursor, index = [], 0, 2
    while index < len(lines):
        header = re.fullmatch(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(?:[^\n]*)\n', lines[index])
        if not header:
            raise ValueError('invalid patch hunk header')
        old_start = int(header[1]) - 1
        old_count, new_count = int(header[2] or 1), int(header[4] or 1)
        if not cursor <= old_start <= len(source):
            raise ValueError('patch hunk range mismatch')
        output.extend(source[cursor:old_start]); cursor = old_start
        if len(output) != int(header[3]) - 1:
            raise ValueError('patch postimage hunk range mismatch')
        seen_old = seen_new = 0
        index += 1
        while index < len(lines) and not lines[index].startswith('@@ '):
            line = lines[index]; index += 1
            if not line or line[0] not in ' +-':
                raise ValueError('invalid patch hunk line')
            if line[0] in ' -':
                if cursor >= len(source) or source[cursor] != line[1:]:
                    raise ValueError('patch preimage/context mismatch')
                cursor += 1; seen_old += 1
            if line[0] in ' +':
                output.append(line[1:]); seen_new += 1
        if (seen_old, seen_new) != (old_count, new_count):
            raise ValueError('patch hunk count mismatch')
    output.extend(source[cursor:])
    return ''.join(output).encode('utf-8')


def patched_inventory(recorded, provenance, commit):
    # Preserve the original inventory and archive; independently derive each patched image.
    expected = [dict(entry) for entry in recorded['files']]
    by_path = {entry['path']: entry for entry in expected}
    receipts, seen = [], set()
    with tarfile.open(ARCHIVE, 'r:gz') as archive:
        for record in provenance.get('patches', []):
            target, patch_name = record['path'], record['patch']
            if target in seen or target not in by_path or PurePosixPath(target).is_absolute() or '..' in PurePosixPath(target).parts:
                raise ValueError('invalid/duplicate patch target')
            seen.add(target)
            if Path(patch_name).name != patch_name:
                raise ValueError('patch file must be directly inside vendor')
            patch = (ROOT / 'vendor' / patch_name).read_bytes()
            if len(patch) > 1024 * 1024 or sha(patch) != record['sha256']:
                raise ValueError('patch hash/size mismatch')
            member = archive.getmember(f'rust-analyzer-{commit}/{target}')
            if not member.isfile() or member.size > 8 * 1024 * 1024:
                raise ValueError('invalid patch preimage archive member')
            original = archive.extractfile(member).read()
            if sha(original) != record['preimage_sha256'] or sha(original) != by_path[target]['sha256'] or len(original) != by_path[target]['bytes']:
                raise ValueError('patch preimage differs from pinned inventory/archive')
            patched = apply_text_patch(original, patch, target)
            if sha(patched) != record['postimage_sha256']:
                raise ValueError('patch postimage hash mismatch')
            erratum = ROOT / record['erratum']
            if not erratum.is_relative_to(ROOT / 'docs' / 'errata') or not erratum.is_file():
                raise ValueError('patch erratum is missing')
            by_path[target].update(bytes=len(patched), sha256=sha(patched))
            receipts.append({'id': record['id'], 'target': target, 'patch_sha256': sha(patch),
                             'preimage_sha256': sha(original), 'postimage_sha256': sha(patched)})
    return expected, receipts


def audit():
    provenance = tomllib.loads((ROOT / 'vendor/UPSTREAM.toml').read_text())['rust_analyzer_macros']
    commit = commit_from_vcs(VCS)
    if provenance['commit'] != commit:
        raise ValueError('source commit differs from pinned package metadata')
    if provenance['source'] != f'https://codeload.github.com/rust-lang/rust-analyzer/tar.gz/{commit}':
        raise ValueError('archive URL does not name pinned commit')
    if ARCHIVE.stat().st_size > MAX_ARCHIVE or sha(ARCHIVE.read_bytes()) != provenance['sha256']:
        raise ValueError('upstream source archive hash mismatch')
    recorded = json.loads(INVENTORY.read_text())
    expected, receipts = patched_inventory(recorded, provenance, commit)
    if recorded['commit'] != commit or expected != tree_inventory(TREE):
        raise ValueError('vendored tree differs from recorded source inventory')
    if sha(INVENTORY.read_bytes()) != provenance['inventory_sha256']:
        raise ValueError('source inventory hash mismatch')
    for name in ['Cargo.toml', 'Cargo.lock', 'LICENSE-MIT', 'LICENSE-APACHE',
                 'crates/mbe/Cargo.toml', 'crates/proc-macro-api/Cargo.toml']:
        if not (TREE / name).is_file():
            raise ValueError(f'missing required upstream file {name}')
    return {'commit': commit, 'archive_sha256': provenance['sha256'],
            'files': len(recorded['files']), 'patches': receipts, 'status': 'passed'}


def install(syntax_dir, archive):
    manifest = tomllib.loads((syntax_dir / 'Cargo.toml').read_text())
    if manifest['package']['name'] != 'ra_ap_syntax' or manifest['package']['version'] != '0.0.356':
        raise ValueError('expected pinned ra_ap_syntax 0.0.356 package')
    vcs = syntax_dir / '.cargo_vcs_info.json'
    commit = commit_from_vcs(vcs)
    if TREE.exists() or INVENTORY.exists() or ARCHIVE.exists() or VCS.exists():
        raise ValueError('existing vendor installation: audit it; do not overwrite')
    if archive.stat().st_size > MAX_ARCHIVE:
        raise ValueError('archive exceeds size limit')
    prefix = f'rust-analyzer-{commit}'
    with tempfile.TemporaryDirectory(prefix='lrr-macro-vendor-', dir=ROOT / 'vendor') as temporary:
        staged = Path(temporary)
        with tarfile.open(archive, 'r:gz') as tar:
            members = tar.getmembers()
            if len(members) > MAX_FILES or sum(m.size for m in members) > MAX_TREE:
                raise ValueError('source archive exceeds extraction limits')
            seen = set()
            links = []
            for member in members:
                parts = PurePosixPath(member.name).parts
                if not parts or parts[0] != prefix or any(p in ('..', '.') for p in parts):
                    raise ValueError('unsafe archive member path')
                if member.name in seen:
                    raise ValueError('duplicate archive member')
                seen.add(member.name)
                target = staged.joinpath(*parts[1:])
                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                elif member.isfile():
                    target.parent.mkdir(parents=True, exist_ok=True)
                    source = tar.extractfile(member)
                    if source is None:
                        raise ValueError('archive member has no bytes')
                    with source, target.open('xb') as output:
                        shutil.copyfileobj(source, output)
                    target.chmod(member.mode & 0o777)
                elif member.issym():
                    links.append((target,member.linkname))
                else:
                    raise ValueError('archive hard links/devices are forbidden')
            for target,link in links:
                destination=(target.parent / link).resolve()
                if not destination.is_relative_to(staged.resolve()) or not destination.is_file():
                    raise ValueError('archive symlink escapes or has no file target')
                target.parent.mkdir(parents=True,exist_ok=True)
                target.symlink_to(link)
        entries = tree_inventory(staged)
        # Installation follows complete bounded validation. Upstream files stay exact.
        staged.rename(TREE)
    shutil.copyfile(archive, ARCHIVE)
    shutil.copyfile(vcs, VCS)
    INVENTORY.write_text(json.dumps({'schema': 1, 'commit': commit, 'files': entries}, indent=2) + '\n')
    with (ROOT / 'vendor/UPSTREAM.toml').open('a') as output:
        output.write(f'\n[rust_analyzer_macros]\ncommit = "{commit}"\n'
                     f'source = "https://codeload.github.com/rust-lang/rust-analyzer/tar.gz/{commit}"\n'
                     f'archive = "rust-analyzer-macros.tar.gz"\nsha256 = "{sha(ARCHIVE.read_bytes())}"\n'
                     f'inventory_sha256 = "{sha(INVENTORY.read_bytes())}"\n'
                     'syntax_package = "ra_ap_syntax 0.0.356"\n'
                     'syntax_package_dirty = true\nlicenses = ["MIT", "Apache-2.0"]\n')
    return audit()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--install', action='store_true')
    parser.add_argument('--syntax-crate-dir', type=Path)
    parser.add_argument('--archive', type=Path)
    args = parser.parse_args()
    if args.install:
        if args.syntax_crate_dir is None or args.archive is None:
            parser.error('--install requires --syntax-crate-dir and --archive')
        result = install(args.syntax_crate_dir, args.archive)
    else:
        result = audit()
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        print(f'vendor bootstrap failed: {error}', file=sys.stderr)
        sys.exit(1)
