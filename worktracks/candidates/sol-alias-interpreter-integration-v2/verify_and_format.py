#!/usr/bin/env python3
"""Only enumerated LRR leaves may be formatted; immutable originals are Git-checked twice."""
from pathlib import Path
import argparse
import hashlib
import json
import subprocess

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
PIN = 'ae0fe511b15ab760d7e963b13389d1bc2ab42b18'
FMT = '/Users/avi/.rustup/toolchains/1.99.0-aarch64-apple-darwin/bin/rustfmt'

def verify_originals():
    base = ROOT / 'candidate/fixtures/upstream/rustc-checkout'
    manifest = json.loads((base / 'manifest.json').read_text())
    assert manifest['commit'] == PIN
    assert len(manifest['files']) == 52
    assert len({row['path'] for row in manifest['files']}) == 52
    for row in manifest['files']:
        rel = Path(row['path'])
        assert not rel.is_absolute() and '..' not in rel.parts
        oid = subprocess.run(['git', '-C', str(REPO / 'rustc/rust'), 'rev-parse', PIN + ':' + str(rel)], check=True, capture_output=True).stdout.decode().strip()
        assert oid == row['git_blob'], rel
        raw = subprocess.run(['git', '-C', str(REPO / 'rustc/rust'), 'cat-file', 'blob', oid], check=True, capture_output=True).stdout
        p = base / rel
        assert not p.is_symlink() and p.resolve().is_relative_to(base.resolve())
        assert p.read_bytes() == raw, rel
        assert len(raw) == row['bytes'] and hashlib.sha256(raw).hexdigest() == row['sha256'], rel
    return 52

def safe_formatter_inputs():
    rows = json.loads((ROOT / 'before-after-manifest-v2.json').read_text())['paths']
    assert len(rows) == 38
    files = []
    for row in rows:
        rel = Path(row['path'])
        assert not rel.is_absolute() and '..' not in rel.parts
        assert rel.parts[0] in ('crates', 'xtask') and rel.suffix == '.rs'
        assert 'fixtures' not in rel.parts and 'originals' not in rel.parts
        p = ROOT / 'candidate' / rel
        assert not p.is_symlink() and p.resolve().is_relative_to((ROOT / 'candidate').resolve())
        assert hashlib.sha256(p.read_bytes()).hexdigest() == row['after_sha256'], rel
        files.append(str(p))
    assert len(set(files)) == 38
    return files

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--write', action='store_true', help='Format only the explicitly enumerated LRR leaves; regenerate their manifest before freezing.')
    args = parser.parse_args()
    count = verify_originals()
    files = safe_formatter_inputs()
    subprocess.run([FMT, '--edition', '2024', '--config', 'skip_children=true'] + ([] if args.write else ['--check']) + files, check=True)
    assert verify_originals() == count
    print('checked 38 safe formatter inputs and all52 pinned Git originals before/after formatting; no fixture path submitted')
