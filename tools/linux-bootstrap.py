#!/usr/bin/env python3
"""T003/H004/M001–M003: isolated pinned Linux bootstrap validation.

Explicit tool/native/macro preparation only. Never restarts Docker or modifies
the input checkout. The evidence directory is new for every invocation.
"""
import argparse
import json
import pathlib
import subprocess
import sys
import time

IMAGE = 'rust@sha256:59037199c44290f2befcdd58dcc540164763fc296950255aaefeef096a1866b0'
CONTAINER_SCRIPT = r'''
set -euo pipefail
mkdir -p /work
tar -C /input --exclude='./target' --exclude='./.lrr/tool-target' \
    --exclude='./artifacts/linux-bootstrap' -cf - . | tar -C /work -xf -
cd /work
mkdir -p artifacts/linux-validation
trap 'cp -a /work/artifacts /evidence/container-artifacts' EXIT
test "$(uname -s)" = Linux
test "$(uname -m)" = x86_64
uname -a > artifacts/linux-validation/host.txt
rustc -vV > artifacts/linux-validation/compiler.txt
apt-get update
apt-get install -y --no-install-recommends python3
rustup toolchain install nightly-2026-10-01 --profile minimal
rustup toolchain install 1.99.0 --profile minimal
rustup component add --toolchain 1.99.0 clippy rustfmt
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_INCREMENTAL=0
python3 tools/bootstrap_vendor.py > artifacts/linux-validation/vendor.json
python3 tools/cargo-reuse-probe/probe.py --allow-native-build \
    --work-dir /work/artifacts/linux-validation/reuse \
    > artifacts/linux-validation/reuse-command.stdout \
    2> artifacts/linux-validation/reuse-command.stderr
cargo +1.99.0 run --locked -p xtask -- gate 00 \
    > artifacts/linux-validation/gate00.stdout \
    2> artifacts/linux-validation/gate00.stderr
cargo +1.99.0 run --locked -p xtask -- compile-time-tools-gate \
    > artifacts/linux-validation/compile-time-tools.stdout \
    2> artifacts/linux-validation/compile-time-tools.stderr
cargo +1.99.0 clippy --workspace --all-targets --locked -- -D warnings \
    > artifacts/linux-validation/clippy.stdout \
    2> artifacts/linux-validation/clippy.stderr
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-build-tools', action='store_true',
                        help='authorize isolated Cargo builds, native reuse fixture, and macro probes')
    parser.add_argument('--evidence-dir', required=True, type=pathlib.Path)
    args = parser.parse_args()
    if not args.allow_build_tools:
        parser.error('--allow-build-tools is required before Docker or compiler invocation')
    evidence = args.evidence_dir.resolve()
    if evidence.exists():
        parser.error('evidence directory must be new; prior evidence is never replaced')
    root = pathlib.Path(__file__).resolve().parent.parent
    if ',' in str(root) or ',' in str(evidence):
        parser.error('Docker mount paths cannot contain commas')
    evidence.mkdir(parents=True)
    report = {'schema': 1, 'passed': False, 'spec_revision': '4.0',
              'scope': 'Linux x86-64 bootstrap/tool capability; full release remains incomplete',
              'image': IMAGE, 'platform': 'linux/amd64', 'commands': [], 'errors': []}

    def run(label, command, timeout=None):
        started = time.monotonic()
        record = {'label': label, 'arguments': command, 'cwd': str(root),
                  'stdout': f'{label}.stdout', 'stderr': f'{label}.stderr'}
        report['commands'].append(record)
        with (evidence / record['stdout']).open('wb') as stdout, \
                (evidence / record['stderr']).open('wb') as stderr:
            try:
                completed = subprocess.run(command, cwd=root, stdout=stdout, stderr=stderr,
                                           timeout=timeout, check=False)
                record['exit_code'] = completed.returncode
            except subprocess.TimeoutExpired:
                record['timed_out_seconds'] = timeout
                raise RuntimeError(f'{label} did not respond within {timeout} seconds')
            finally:
                record['elapsed_seconds'] = time.monotonic() - started
        if completed.returncode != 0:
            raise RuntimeError(f'{label} exited {completed.returncode}; inspect raw stderr')

    try:
        run('docker-readiness', ['docker', 'info', '--format', '{{.ServerVersion}}'], timeout=20)
        (evidence / 'container-command.sh').write_text(CONTAINER_SCRIPT)
        run('linux-container', ['docker', 'run', '--rm', '--platform', 'linux/amd64',
            '--mount', f'type=bind,src={root},dst=/input,readonly',
            '--mount', f'type=bind,src={evidence},dst=/evidence',
            IMAGE, 'bash', '-c', CONTAINER_SCRIPT])
        proofs = {}
        for label, relative in [
            ('cargo_reuse', 'linux-validation/reuse/evidence/report.json'),
            ('gate_00', 'gates/00.json'),
            ('compile_time_tools', 'gates/compile-time-tools.json'),
        ]:
            path = evidence / 'container-artifacts' / relative
            proof = json.loads(path.read_text())
            proofs[label] = {'artifact': str(path), 'passed': proof['passed']}
            if proof['passed'] is not True:
                raise RuntimeError(f'{label} did not pass')
        report['proofs'] = proofs
        report['passed'] = True
    except (OSError, RuntimeError, KeyError, ValueError) as error:
        report['errors'].append(f'{type(error).__name__}: {error}')
    finally:
        (evidence / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'passed': report['passed'], 'report': str(evidence / 'report.json'),
                      'errors': report['errors']}))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
