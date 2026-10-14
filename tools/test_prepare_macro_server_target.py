"""LRR preparation-path regressions; no compiler or macro is executed."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tools'))
spec = importlib.util.spec_from_file_location('prepared_tool_target_test_module', ROOT / 'tools/prepare_macro_server.py')
prepared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepared)


class PreparedToolTargetTests(unittest.TestCase):
    def test_inherited_target_cannot_redirect_explicitly_prepared_server(self):
        with tempfile.TemporaryDirectory(dir=ROOT / 'artifacts/macos-first') as temp:
            work = Path(temp)
            package = work / 'package'
            for name in ['Cargo.toml', 'Cargo.lock', 'build.rs', 'src/main.rs', 'stdio-v1.patch']:
                destination = package / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(prepared.PACKAGE / name, destination)
            evidence = work / 'evidence'
            evidence.mkdir()
            sysroot = work / 'sysroot'
            component = sysroot / 'lib/rustlib/manifest-rustc-dev-aarch64-apple-darwin'
            component.parent.mkdir(parents=True)
            component.write_text('mock compiler component; never executed\n')
            redirected = work / 'inherited-target'
            binary = package / 'target/debug/rust-analyzer-proc-macro-srv'
            calls = []
            commit = 'mock-compiler-commit'
            vendor = {'commit': 'mock-vendor-commit'}
            version = f'commit-hash: {commit}\nhost: aarch64-apple-darwin\nrelease: 1.99.0\n'

            def run(argv, **kwargs):
                calls.append(list(argv))
                if argv[0] == 'python3':
                    return mock.Mock(stdout=json.dumps(vendor))
                self.assertEqual(argv[:4], ['cargo', '+1.99.0', 'build', '--locked'])
                # Model Cargo's explicit target taking precedence over the
                # inherited environment; the launch path is tested afterward.
                target = (Path(argv[argv.index('--target-dir') + 1])
                          if '--target-dir' in argv else Path(kwargs['env']['CARGO_TARGET_DIR']))
                output = target / 'debug/rust-analyzer-proc-macro-srv'
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(b'mocked output, not an executable')
                return mock.Mock(returncode=0)

            def output(argv, **kwargs):
                if argv == ['rustc', '+1.99.0', '-vV']:
                    return version
                if argv == ['rustc', '+1.99.0', '--print', 'sysroot']:
                    return str(sysroot) + '\n'
                self.assertEqual(argv, [str(binary), '--version'])
                self.assertTrue(binary.is_file(), 'server launch path differs from actual Cargo target')
                self.assertEqual(kwargs['env'], {'RUST_ANALYZER_INTERNALS_DO_NOT_USE': 'explicit LRR tool preparation'})
                return f'rust-analyzer-proc-macro-srv 1.99.0 {commit} {vendor["commit"]} lrr-stdio-v1\n'

            with mock.patch.object(prepared, 'PACKAGE', package), \
                 mock.patch.object(prepared.subprocess, 'run', side_effect=run), \
                 mock.patch.object(prepared.subprocess, 'check_output', side_effect=output), \
                 mock.patch.dict(os.environ, {'CARGO_TARGET_DIR': str(redirected)}), \
                 mock.patch.object(sys, 'argv', ['prepare', '--allow-tool-preparation', '--receipt-directory', str(evidence)]), \
                 contextlib.redirect_stdout(io.StringIO()):
                prepared.main()
            receipt = json.loads((evidence / 'server-preparation.json').read_text())
            self.assertEqual(receipt['server'], str(binary))
            self.assertFalse(receipt['live_runtime_unit_compiled'])
            self.assertEqual(len(calls), 2)
            self.assertEqual(Path(calls[1][calls[1].index('--target-dir') + 1]), package / 'target')
            self.assertFalse(redirected.exists())

    def test_missing_authority_refuses_before_any_preparation_process(self):
        with mock.patch.object(sys, 'argv', ['prepare']), \
             mock.patch.object(prepared.subprocess, 'run') as run, \
             mock.patch.object(prepared.subprocess, 'check_output') as output, \
             contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as error:
                prepared.main()
            self.assertEqual(error.exception.code, 2)
            run.assert_not_called()
            output.assert_not_called()


if __name__ == '__main__':
    unittest.main()
