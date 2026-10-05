#!/usr/bin/env python3
"""Verify every vendor byte against the retained archive and sole allowed patch."""
import hashlib
import pathlib
import tarfile
import tomllib

root = pathlib.Path(__file__).resolve().parents[1]
record = tomllib.loads((root / 'vendor/UPSTREAM.toml').read_text())
archive = root / 'vendor' / record['archive']
assert hashlib.sha256(archive.read_bytes()).hexdigest() == record['sha256'], 'archive checksum mismatch'
addition = b'''    /// Disable all fallback symbol lookup; explicit symbols remain available.
    pub fn clear_symbol_lookup_functions(&mut self) -> &mut Self {
        self.lookup_symbols.clear();
        self
    }

'''
expected = {}
with tarfile.open(archive) as bundle:
    for member in bundle.getmembers():
        if member.isfile():
            name = str(pathlib.PurePosixPath(member.name).relative_to('cranelift-jit-0.136.2'))
            data = bundle.extractfile(member).read()
            if name == 'src/backend.rs':
                assert data.count(b'impl JITBuilder {\n') == 1
                data = data.replace(b'impl JITBuilder {\n', b'impl JITBuilder {\n' + addition, 1)
            expected[name] = data
actual = {str(p.relative_to(root / 'vendor/cranelift-jit')): p.read_bytes()
          for p in (root / 'vendor/cranelift-jit').rglob('*') if p.is_file()}
assert actual == expected, 'vendor tree differs beyond the specified method'
print('vendor_patch_is_exact: passed')
