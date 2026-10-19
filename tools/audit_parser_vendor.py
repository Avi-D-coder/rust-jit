#!/usr/bin/env python3
"""Reconstruct the pinned ordinary parser package and its one recorded grammar patch."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import tarfile
import tomllib

from bootstrap_vendor import apply_text_patch

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def audit():
    record = tomllib.loads((ROOT / 'vendor/UPSTREAM.toml').read_text())['ra_ap_parser']
    archive = ROOT / 'vendor' / record['archive']
    assert sha(archive.read_bytes()) == record['sha256'], 'parser package checksum mismatch'
    assert record['source'] == f'https://static.crates.io/crates/ra_ap_parser/ra_ap_parser-{record["version"]}.crate'
    expected = {}
    with tarfile.open(archive, 'r:gz') as bundle:
        for member in bundle.getmembers():
            if not member.isfile():
                continue
            name = str(PurePosixPath(member.name).relative_to('ra_ap_parser-' + record['version']))
            # Cargo's standalone package lock is not used by the workspace.
            if name != 'Cargo.lock':
                expected[name] = bundle.extractfile(member).read()
    vcs = json.loads(expected['.cargo_vcs_info.json'])
    assert vcs['git']['sha1'] == record['commit'] and vcs['path_in_vcs'] == 'crates/parser'
    patch = (ROOT / 'vendor' / record['patch']).read_bytes()
    original = expected[record['path']]
    assert sha(patch) == record['patch_sha256'], 'parser patch checksum mismatch'
    assert sha(original) == record['preimage_sha256'], 'parser patch preimage mismatch'
    expected[record['path']] = apply_text_patch(original, patch, record['path'])
    assert sha(expected[record['path']]) == record['postimage_sha256'], 'parser patch postimage mismatch'
    for name in ['LICENSE-MIT', 'LICENSE-APACHE']:
        expected[name] = (ROOT / 'vendor/rust-analyzer-macros' / name).read_bytes()
    tree = ROOT / 'vendor/ra-ap-parser'
    actual = {p.relative_to(tree).as_posix(): p.read_bytes() for p in tree.rglob('*') if p.is_file()}
    assert actual == expected, 'parser vendor tree differs beyond the recorded patch and licenses'
    assert expected[record['path']] == (ROOT / 'vendor/rust-analyzer-macros/crates/parser' / record['path']).read_bytes(), 'ordinary/adapter parser grammar differs'
    return {'status': 'passed', 'package': 'ra_ap_parser', 'version': record['version'],
            'files': len(expected), 'archive_sha256': record['sha256'],
            'postimage_sha256': record['postimage_sha256']}


if __name__ == '__main__':
    print(json.dumps(audit(), sort_keys=True))
