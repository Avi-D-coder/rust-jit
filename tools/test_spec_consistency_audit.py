"""Mutation controls for the witnessed specification contradictions."""
import copy
import json
from pathlib import Path
import unittest
from spec_consistency_audit import OBSOLETE, TIER_REQUIRED, FINAL_REQUIRED, audit

ROOT = Path(__file__).resolve().parents[1]


class SpecConsistencyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.spec = (ROOT / 'lrr_rust2024_macros_traits_implementation_spec.md').read_text()
        cls.ledger = json.loads((ROOT / 'docs/tasks.json').read_text())

    def test_current_document(self):
        self.assertEqual(audit(self.spec, self.ledger), [])

    def test_duplicate_task_rejected(self):
        ledger = copy.deepcopy(self.ledger)
        ledger['cross_ledger_order'][0]['tasks_in_order'].append('T000')
        self.assertIn('task_order', audit(self.spec, ledger))

    def test_stale_service_example_rejected(self):
        spec = self.spec.replace('"schema_version":3', '"schema_version":2', 1)
        self.assertNotEqual(spec, self.spec)
        self.assertIn('service_example_version', audit(spec, self.ledger))

    def test_throughput_with_numeric_fuel_rejected(self):
        spec = self.spec.replace('"execution_contract":"precise"', '"execution_contract":"throughput"', 1)
        self.assertNotEqual(spec, self.spec)
        self.assertIn('fuel_contract', audit(spec, self.ledger))


    def test_binary_header_overlap_rejected(self):
        spec = self.spec.replace('| 24 | sequence, u64 little-endian |', '| 20 | sequence, u64 little-endian |', 1)
        self.assertNotEqual(spec, self.spec)
        self.assertIn('binary_header_offsets', audit(spec, self.ledger))

    def test_internal_and_public_versions_are_not_conflated(self):
        ledger = copy.deepcopy(self.ledger)
        ledger['revision_transition']['encoding_amendment']['public_service_schema'] = 4
        self.assertIn('encoding_amendment_ledger', audit(self.spec, ledger))

    def test_new_transfer_fixture_must_have_normative_row(self):
        spec = self.spec.replace('| CA029 |', '| REMOVED |', 1)
        self.assertIn('tier_transfer_amendment_ledger', audit(spec, self.ledger))

    def test_new_acceptance_cannot_inherit_component_pass(self):
        ledger = copy.deepcopy(self.ledger)
        task = next(t for t in ledger['tasks'] if t['id'] == 'T024')
        task['tier_transfer_amendment_acceptance']['status'] = 'passed'
        self.assertIn('tier_transfer_pending_acceptance', audit(self.spec, ledger))

    def test_permanent_optimization_exclusion_is_rejected(self):
        spec = self.spec + '\nprevents further automatic exploration of the same unit/backend/dependency key'
        self.assertIn('permanent_optimization_exclusion', audit(spec, self.ledger))

    def test_final_contract_fixture_cannot_be_omitted(self):
        spec = self.spec.replace('| CA037 |', '| REMOVED |', 1)
        self.assertIn('final_contract_amendment_ledger', audit(spec, self.ledger))

    def test_final_contract_cannot_inherit_component_pass(self):
        ledger = copy.deepcopy(self.ledger)
        task = next(t for t in ledger['tasks'] if t['id'] == 'T023')
        task['final_contract_amendment_acceptance']['status'] = 'passed'
        self.assertIn('final_contract_pending_acceptance', audit(self.spec, ledger))

    def test_materialized_backing_cannot_be_freed_on_native_exit(self):
        spec = self.spec + '\nPhysical reservations count against worker memory, are released on exit'
        self.assertIn('premature_backing_release', audit(spec, self.ledger))

    def test_static_checker_cannot_be_reintroduced_as_runtime_policy(self):
        ledger = copy.deepcopy(self.ledger)
        ledger['revision_transition']['runtime_acceptance_policy']['borrow_checker'] = True
        self.assertIn('runtime_acceptance_policy', audit(self.spec, ledger))

    def test_diagnostics_cannot_block_runtime_gate(self):
        ledger = copy.deepcopy(self.ledger)
        ledger['revision_transition']['runtime_acceptance_policy']['jit_blocking_contracts'].append('diagnostic')
        self.assertIn('runtime_acceptance_policy', audit(self.spec, ledger))

    def test_old_all_category_gate_rejected(self):
        spec = self.spec + '\nuntil every applicable configuration is full_pass'
        self.assertIn('obsolete_all_category_gate', audit(spec, self.ledger))

    def test_mixed_runtime_pass_cannot_mean_full_fixture_pass(self):
        spec = self.spec.replace('a runtime-contract pass may coexist with outstanding diagnostic/tooling assertions', 'REMOVED')
        self.assertIn('mixed_case_accounting', audit(spec, self.ledger))


for name, clause in FINAL_REQUIRED.items():
    def test(self, name=name, clause=clause):
        self.assertIn(name, audit(self.spec.replace(clause, 'REMOVED'), self.ledger))
    setattr(SpecConsistencyTests, f'test_final_required_{name}', test)


for name, clause in TIER_REQUIRED.items():
    def test(self, name=name, clause=clause):
        self.assertIn(name, audit(self.spec.replace(clause, 'REMOVED'), self.ledger))
    setattr(SpecConsistencyTests, f'test_required_{name}', test)


for name, clause in OBSOLETE.items():
    def test(self, name=name, clause=clause):
        self.assertIn(name, audit(self.spec + '\n' + clause + '\n', self.ledger))
    setattr(SpecConsistencyTests, f'test_regression_{name}', test)


if __name__ == '__main__':
    unittest.main()
