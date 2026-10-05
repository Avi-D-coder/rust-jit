#!/usr/bin/env python3
"""H004: evidence-producing Cargo artifact reuse experiment, no LRR execution."""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import subprocess
import sys
import time


def digest(path):
    return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-native-build', action='store_true')
    parser.add_argument('--work-dir', required=True, type=pathlib.Path)
    parser.add_argument('--trace-fingerprints', action='store_true')
    args = parser.parse_args()
    if not args.allow_native_build:
        parser.error('--allow-native-build is required for Cargo and generated native execution')
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        parser.error('the controlled experiment requires Linux x86-64')
    work = args.work_dir.resolve()
    if work.exists() and (not work.is_dir() or any(work.iterdir())):
        parser.error('work directory must be new or empty; existing evidence is never replaced')
    work.mkdir(parents=True, exist_ok=True)
    evidence = work / 'evidence'
    evidence.mkdir()
    report = {'schema': 1, 'passed': False, 'commands': [], 'stages': [],
              'errors': [], 'scope': 'Cargo artifact reuse only; no live execution or production ABI'}

    def write(path, text):
        path = work / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def run(label, command, role='native_preparation'):
        started = time.monotonic()
        overrides = {'CARGO_LOG': 'cargo::core::compiler::fingerprint=trace'} if args.trace_fingerprints and command[:2] == ['cargo', 'build'] else {}
        result = subprocess.run(command, cwd=work, capture_output=True, timeout=180, env={**os.environ, **overrides})
        ordinal = len(report['commands'])
        stdout = evidence / f'{ordinal:02d}-{label}.stdout'
        stderr = evidence / f'{ordinal:02d}-{label}.stderr'
        stdout.write_bytes(result.stdout)
        stderr.write_bytes(result.stderr)
        record = {'label': label, 'role': role, 'arguments': command, 'cwd': str(work),
                  'environment_overrides': overrides, 'exit_code': result.returncode, 'elapsed_seconds': time.monotonic() - started,
                  'stdout': stdout.name, 'stderr': stderr.name}
        report['commands'].append(record)
        if result.returncode != 0:
            raise RuntimeError(f'{label} exited {result.returncode}: {result.stderr.decode(errors="replace")}')
        return result.stdout.decode()

    def build(label, package):
        stdout = run(label, ['cargo', 'build', '--package', package, '--lib', '--release',
                             '--locked', '--offline', '--message-format=json'] + (['-vv'] if args.trace_fingerprints else []))
        artifacts = []
        for line in stdout.splitlines():
            try:
                event = json.loads(line)
            except json.JSONDecodeError:
                continue
            if event.get('reason') == 'compiler-artifact':
                assert event['target']['name'] != 'live_app', 'live application was compiled'
                event['content_hashes'] = {p: digest(p) for p in event['filenames']}
                artifacts.append(event)
        assert any(json.loads(line).get('reason') == 'build-finished' and json.loads(line).get('success')
                   for line in stdout.splitlines() if line.startswith('{')), 'missing successful build-finished'
        report['stages'].append({'name': label, 'artifacts': artifacts})
        return artifacts

    def artifact(events, name, suffix):
        matches = [e for e in events if e['target']['name'] == name]
        assert len(matches) == 1, f'exactly one artifact required for {name}'
        event = matches[0]
        files = [p for p in event['filenames'] if p.endswith(suffix)]
        assert len(files) == 1, f'exactly one reported {suffix} required for {name}'
        return {'path': files[0], 'sha256': digest(files[0]), 'fresh': event['fresh'],
                'package_id': event['package_id'], 'profile': event['profile'], 'features': event['features']}

    def load(label, library, generic=False, expected=18):
        child = '''import ctypes,json,sys
library=ctypes.CDLL(sys.argv[1])
scale=library.lrr_probe_scale
scale.argtypes=[ctypes.c_int64]
scale.restype=ctypes.c_int64
result={'scale':scale(6)}
if sys.argv[2]=='yes':
    twice=library.lrr_probe_twice
    twice.argtypes=[ctypes.c_int64]
    twice.restype=ctypes.c_int64
    result['twice']=twice(7)
print(json.dumps(result))
'''
        output = json.loads(run(label, [sys.executable, '-c', child, library['path'],
                                       'yes' if generic else 'no'], role='unsandboxed_native_probe'))
        assert output == ({'scale': expected, 'twice': 14} if generic else {'scale': expected}), output
        report['stages'].append({'name': label, 'native_output': output, 'bridge': library})

    try:
        report['toolchain'] = {'cargo': run('cargo-version', ['cargo', '--version']),
                               'rustc': run('rustc-version', ['rustc', '-vV'])}
        manifest = '[workspace]\nmembers = ["native_math", "live_app"]\nresolver = "3"\n'
        write('Cargo.toml', manifest)
        write('native_math/Cargo.toml', '[package]\nname = "native_math"\nversion = "0.1.0"\nedition = "2024"\n')
        native_source = 'pub fn scale(value: i64) -> i64 { value * 3 }\npub fn twice<T: Copy + std::ops::Add<Output=T>>(value: T) -> T { value + value }\n'
        write('native_math/src/lib.rs', native_source)
        write('live_app/Cargo.toml', '[package]\nname = "live_app"\nversion = "0.1.0"\nedition = "2024"\n[dependencies]\nnative_math = { path = "../native_math" }\n')
        write('live_app/src/lib.rs', 'pub fn live(value: i64) -> i64 { native_math::scale(value) }\nfn cold() { let _: u8 = "deliberately ill-typed live code"; }\n')
        run('initial-lock', ['cargo', 'generate-lockfile', '--offline'])
        baseline = artifact(build('baseline', 'native_math'), 'native_math', '.rlib')
        assert not baseline['fresh'], 'new baseline was unexpectedly fresh'
        report['baseline'] = baseline

        write('Cargo.toml', manifest.replace('"live_app"]', '"live_app", "lrr_probe_bridge"]') +
              '\n[profile.release.package.lrr_probe_bridge]\nopt-level = 3\nincremental = true\ncodegen-units = 16\n')
        write('lrr_probe_bridge/Cargo.toml', '[package]\nname = "lrr_probe_bridge"\nversion = "0.1.0"\nedition = "2024"\n[lib]\ncrate-type = ["cdylib"]\n[dependencies]\nnative_math = { path = "../native_math" }\n')
        bridge_source = '#[unsafe(no_mangle)]\npub extern "C" fn lrr_probe_scale(value: i64) -> i64 { native_math::scale(value) }\n'
        write('lrr_probe_bridge/src/lib.rs', bridge_source)
        run('bridge-lock', ['cargo', 'generate-lockfile', '--offline'])
        events = build('first-bridge', 'lrr_probe_bridge')
        dependency = artifact(events, 'native_math', '.rlib')
        bridge = artifact(events, 'lrr_probe_bridge', '.so')
        assert dependency['fresh'] and dependency['sha256'] == baseline['sha256'], 'first bridge failed baseline dependency reuse'
        assert not bridge['fresh'], 'new bridge was not compiled'
        load('first-load', bridge)

        events = build('unchanged-bridge', 'lrr_probe_bridge')
        dependency = artifact(events, 'native_math', '.rlib')
        unchanged = artifact(events, 'lrr_probe_bridge', '.so')
        assert dependency['fresh'] and unchanged['fresh'], 'unchanged units rebuilt'
        assert unchanged['sha256'] == bridge['sha256'], 'unchanged bridge bytes changed'
        cargo_before = sum(r['arguments'][0] == 'cargo' for r in report['commands'])
        write('live_app/src/lib.rs', 'pub fn live(value: i64) -> i64 { native_math::scale(value + 1) }\nfn cold() { absent_after_live_edit(); }\n')
        load('live-edit-load-without-cargo', bridge)
        assert sum(r['arguments'][0] == 'cargo' for r in report['commands']) == cargo_before
        report['stages'].append({'name': 'live-edit', 'cargo_invocations': 0,
                                 'live_code_executed': False})

        write('lrr_probe_bridge/src/lib.rs', bridge_source + '#[unsafe(no_mangle)]\npub extern "C" fn lrr_probe_twice(value: i64) -> i64 { native_math::twice::<i64>(value) }\n')
        events = build('generic-wrapper-edit', 'lrr_probe_bridge')
        dependency = artifact(events, 'native_math', '.rlib')
        bridge = artifact(events, 'lrr_probe_bridge', '.so')
        assert dependency['fresh'] and dependency['sha256'] == baseline['sha256'], 'wrapper edit rebuilt dependency'
        assert not bridge['fresh'], 'edited bridge was unexpectedly fresh'
        load('generic-load', bridge, generic=True)

        write('native_math/src/lib.rs', native_source.replace('value * 3', 'value * 4'))
        events = build('native-edit', 'lrr_probe_bridge')
        dependency = artifact(events, 'native_math', '.rlib')
        bridge = artifact(events, 'lrr_probe_bridge', '.so')
        assert not dependency['fresh'] and not bridge['fresh'], 'native edit failed to rebuild closure'
        assert dependency['sha256'] != baseline['sha256'], 'native edit did not change dependency bytes'
        load('native-edit-load', bridge, generic=True, expected=24)
        report['passed'] = True
    except Exception as error:
        report['errors'].append(f'{type(error).__name__}: {error}')
    finally:
        # Preserve exact input sources alongside command outputs, even on failure.
        report['inputs'] = {}
        for path in sorted(work.rglob('*')):
            if path.is_file() and not any(p in ('target', 'evidence') for p in path.relative_to(work).parts):
                report['inputs'][str(path.relative_to(work))] = path.read_text()
        (evidence / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'passed': report['passed'], 'report': str(evidence / 'report.json'),
                      'errors': report['errors']}))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
