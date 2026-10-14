#!/usr/bin/env python3
"""Controlled M002 probe: forward real compiler calls, record evidence, never fake artifacts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'fixtures/macros/tool-workspace'
WRAPPER = '''#!{python}
import fcntl,json,os,subprocess,sys
compiler,*args=sys.argv[1:]
result=subprocess.run([compiler,*args],check=False,close_fds=False)
record=json.dumps({{"compiler":compiler,"argv":args,"exit_code":result.returncode}})+"\\n"
with open(os.environ['LRR_PROBE_COMPILER_LOG'],'a') as log:
    fcntl.flock(log,fcntl.LOCK_EX)
    log.write(record)
    log.flush()
sys.exit(result.returncode)
'''


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(command, environment, output, label):
    result = subprocess.run(command, cwd=FIXTURE, env=environment, capture_output=True,
                            timeout=120, check=False)
    stdout = output / f'{label}.stdout'
    stderr = output / f'{label}.stderr'
    stdout.write_bytes(result.stdout)
    stderr.write_bytes(result.stderr)
    return {'argv': [str(x) for x in command], 'exit_code': result.returncode,
            'stdout': str(stdout), 'stderr': str(stderr)}


def records(command):
    return [json.loads(line) for line in Path(command['stdout']).read_text().splitlines() if line]


def compiler_records(path):
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def is_information(args):
    return any(a.startswith('--print') or a in ('-vV', '--version') for a in args)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-build-tools', action='store_true')
    parser.add_argument('--planner', type=Path, required=True)
    parser.add_argument('--rustc', type=Path, required=True)
    parser.add_argument('--evidence-dir', type=Path, required=True)
    parser.add_argument('--target-dir', type=Path, required=True)
    parser.add_argument('--target', required=True, choices=['aarch64-apple-darwin', 'x86_64-apple-darwin', 'x86_64-unknown-linux-gnu'])
    args = parser.parse_args()
    if not args.allow_build_tools:
        parser.error('controlled fixture build tools require --allow-build-tools')
    if not args.planner.is_absolute() or not args.rustc.is_absolute():
        parser.error('tool paths must be absolute')
    for name in ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC_BOOTSTRAP'):
        if os.environ.get(name):
            parser.error(f'conflicting ambient {name}')
    output = args.evidence_dir.resolve()
    output.mkdir(parents=True, exist_ok=False)
    wrapper = output / 'forward-compiler.py'
    wrapper.write_text(WRAPPER.format(python=sys.executable))
    wrapper.chmod(0o755)
    # The controlled probe declares its forwarding wrapper explicitly.
    env = {k: os.environ[k] for k in ('PATH', 'HOME', 'TMPDIR', 'SDKROOT',
           'DEVELOPER_DIR', 'RUSTUP_HOME', 'CARGO_HOME') if k in os.environ}
    env['RUSTC'] = str(args.rustc)
    env['RUSTC_WRAPPER'] = str(wrapper)
    env['LRR_PROBE_COMPILER_LOG'] = str(output / 'information-compiler.jsonl')
    commands = []
    checks = []
    report = {'schema': 1, 'evidence_scope': 'selected-platform capability; Linux port validation deferred',
              'target': args.target, 'build_tools_authorized': True,
              'planner': {'path': str(args.planner), 'sha256': digest(args.planner)},
              'compiler': {'path': str(args.rustc), 'sha256': digest(args.rustc)},
              'wrapper': {'path': str(wrapper), 'sha256': digest(wrapper), 'behavior': 'always forward'},
              'commands': commands, 'checks': checks, 'passed': False}

    def checked(name, condition, detail):
        checks.append({'name': name, 'passed': bool(condition), 'detail': detail})

    try:
        planner = invoke([args.planner, '-Vv'], env, output, 'planner-version')
        compiler = invoke([args.rustc, '-vV'], env, output, 'compiler-version')
        commands.extend([planner, compiler])
        text = Path(planner['stdout']).read_text()
        checked('pinned_planner', 'commit-date: 2026-09-29' in text and '1.101.0-nightly' in text, planner)
        checked('pinned_project_compiler', 'release: 1.99.0\n' in Path(compiler['stdout']).read_text(), compiler)
        if not all(c['passed'] for c in checks):
            raise ValueError('tool capability pin mismatch')
        for variant, selector in [('lib-tests', ['--lib']), ('integration', ['--test', 'integration'])]:
            base = [args.planner, 'check', '-Z', 'unstable-options', '--manifest-path', FIXTURE / 'Cargo.toml',
                    '--package', 'lrr_tool_probe_app', *selector, '--profile', 'test',
                    '--target', args.target, '--target-dir', args.target_dir.resolve() / variant, '--locked', '--offline']
            env['LRR_PROBE_COMPILER_LOG'] = str(output / f'{variant}-graph-compiler.jsonl')
            graph = invoke([*base, '--unit-graph'], env, output, f'{variant}-unit-graph')
            commands.append(graph)
            checked(f'{variant}:graph_exit', graph['exit_code'] == 0, graph)
            if graph['exit_code'] != 0:
                continue
            units = json.loads(Path(graph['stdout']).read_text())
            checked(f'{variant}:graph_schema', units.get('version') == 1, units.get('version'))
            macros = [u for u in units['units'] if u['target']['kind'] == ['proc-macro']]
            checked(f'{variant}:test_feature_context', len(macros) == 1 and macros[0]['features'] == ['test-intent-witness'], macros)
            counter = None
            for attempt in ('first', 'warm'):
                log = output / f'{variant}-{attempt}-compiler.jsonl'
                env['LRR_PROBE_COMPILER_LOG'] = str(log)
                prepare = invoke([*base, '--compile-time-deps', '--message-format=json-render-diagnostics'], env, output, f'{variant}-{attempt}')
                commands.append(prepare)
                checked(f'{variant}:{attempt}:prepare', prepare['exit_code'] == 0, prepare)
                invocations = compiler_records(log)
                sources = {str((FIXTURE / 'app/src/lib.rs').resolve()), str((FIXTURE / 'app/tests/integration.rs').resolve())}
                forbidden = [r for r in invocations if not is_information(r['argv']) and any(str((FIXTURE / a).resolve()) in sources for a in r['argv'] if a.endswith('.rs'))]
                checked(f'{variant}:{attempt}:no_live_compilation', not forbidden, invocations)
                messages = records(prepare)
                artifacts = [m for m in messages if m.get('reason') == 'compiler-artifact' and m['target']['kind'] == ['proc-macro']]
                checked(f'{variant}:{attempt}:macro_artifact', len(artifacts) == 1 and all(Path(p).is_file() for m in artifacts for p in m['filenames']), artifacts)
                build = [m for m in messages if m.get('reason') == 'build-script-executed']
                checked(f'{variant}:{attempt}:build_output', len(build) == 1 and build[0]['cfgs'] == ['lrr_tool_prepared'] and build[0]['env'] == [['LRR_TOOL_PROBE', 'captured']], build)
                if len(build) == 1:
                    directory = Path(build[0]['out_dir'])
                    generated = directory / 'generated.rs'
                    current = (directory / 'execution-count.txt').read_text()
                    checked(f'{variant}:{attempt}:generated_source', generated.read_text() == '42_u32\n', {'path': str(generated), 'sha256': digest(generated)})
                    if attempt == 'first':
                        counter = current
                    else:
                        checked(f'{variant}:no_script_reexecution', current == counter, {'first': counter, 'warm': current})
                        checked(f'{variant}:cargo_fresh_macro', bool(artifacts) and all(m['fresh'] for m in artifacts), artifacts)
                if attempt == 'warm':
                    checked(f'{variant}:no_warm_compilation', all(is_information(r['argv']) for r in invocations), invocations)
            env['LRR_PROBE_COMPILER_LOG'] = str(output / f'{variant}-oracle-compiler.jsonl')
            oracle = invoke([x for x in base if x not in ('-Z', 'unstable-options')] + ['--message-format=json-render-diagnostics'], env, output, f'{variant}-ordinary-oracle')
            commands.append(oracle)
            diagnostic = [m for m in records(oracle) if m.get('reason') == 'compiler-message' and (m['message'].get('code') or {}).get('code') == 'E0425']
            oracle_text = Path(oracle['stderr']).read_text()
            oracle_calls = compiler_records(Path(env['LRR_PROBE_COMPILER_LOG']))
            failed_live = any(r['exit_code'] != 0 and any(a.endswith('app/src/lib.rs') for a in r['argv']) for r in oracle_calls)
            missing = any('lrr_deliberately_missing_function' in m['message']['message'] for m in diagnostic) or ('error[E0425]' in oracle_text and 'cannot find function `lrr_deliberately_missing_function`' in oracle_text)
            checked(f'{variant}:ordinary_check_rejects_live_body', oracle['exit_code'] != 0 and failed_live and missing, oracle)
            if variant == 'lib-tests':
                calls = compiler_records(Path(env['LRR_PROBE_COMPILER_LOG']))
                live = [r for r in calls if any(a.endswith('app/src/lib.rs') for a in r['argv']) and not is_information(r['argv'])]
                checked('lib_tests_have_test_harness_intent', bool(live) and all('--test' in r['argv'] for r in live), live)
        report['passed'] = bool(checks) and all(c['passed'] for c in checks)
    except (ValueError, OSError, KeyError, subprocess.TimeoutExpired) as error:
        report['error'] = str(error)
    finally:
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps({'passed': report['passed'], 'report': str(output / 'report.json')}))
    return 0 if report['passed'] else 1


if __name__ == '__main__':
    sys.exit(main())
