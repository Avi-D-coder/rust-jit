#!/usr/bin/env python3
"""Document-only regression audit; not runtime, ABI or language acceptance."""
import argparse
import json
import re
import tomllib
from pathlib import Path

# Minimal contradictory clauses retained for E-R5-001 regression mutation tests.
OBSOLETE = {
    'old_portable_payload': 'format = "lrr-portable-v1"',
    'hex_bulk_transport': 'Each byte chunk contains at most 1 MiB raw data encoded as lowercase hexadecimal',
    'json_only_internal_frame': 'Use a 4-byte little-endian length prefix followed by UTF-8 JSON',
    'combined_protocol_version': '| Public/internal LRR service envelope | 3 |',
    'deferred_hex_removal': 'The remaining high-volume hex effect transport is deliberate in this revision',

    'index_failure_class': 'Acceptance: out-of-bounds/index-type faults;',
    'producer_contract': 'which may differ because only the defined C bridge ABI crosses the boundary',
    'service_version': 'Use public protocol version2',
    'call_convention': "default calling convention to construct all signatures",
    'scalar_scope': 'All multiplication, division, remainder, shifts, floating-point operations, casts, aggregate operations, and reference operations use step in this release.',
    'run_field': '`run` names snapshot, selected tests/main, mode, and policy.',
    'old_native_store': 'value: std::cell::RefCell<Option<NativeValue>>',
    'resident_identity': '`NativeResident` is a new managed descriptor',
    'oracle_editions': 'Build only valid supported fixtures using pinned rustc, edition2024, opt0',
    'edition_conformance': 'The conformance suite pins 2024 and records native 1.99 outputs',
    'generation_identity': '`GenerationId` binds the linked bytes and the entire producer/schema/input manifest',
    'compiler_owner': 'Thus the compiler/VM/query owner remains single-threaded.',
    'mandatory_future_import': '7. Expand admitted native IR classes only when',
}


TIER_REQUIRED = {
    'literal_decode_once': 'Decode integer sign, radix, magnitude and written suffix once',
    'entry_maps': 'Every optimized entry has a verified entry map',
    'transfer_status': '| 9 | `TierTransfer` |',
    'transfer_helper': '`lrr_tier_transfer_v2(ctx, map_id, values_ptr)`',
    'guard_cooldown': 'After eight consecutive guard failures',
    'logical_quota_parity': 'Logical managed resource accounting follows the canonical LRIR execution in both contracts',
    'reserved_reconstruction': 'After an effect, reconstruct from the reserved backing without a fallible allocation',
    'native_feedback': 'Publish checked per-unit/per-tier completed traversal/backedge deltas',
    'native_job_submission': 'a newly eligible candidate with available queue capacity may request TierTransfer',
    'completion_poll': 'At those same safepoints, check compiler-completion readiness without blocking',
    'bounded_reconsideration': 'allow at most three automatic retries after the initial optimization attempt',
}


FINAL_REQUIRED = {
    'complete_upstream_inventory': "official rustc/rust checkout's complete `tests/` tree",
    'unsupported_stays_blocker': 'Runtime unsupported/unexecuted/failed/contract-blocked cases carry required backlog entries',
    'separate_virtual_identity': 'a logical ordinal is never decoded as an arena index',
    'adopted_backing_lifetime': 'Release only unused reservation capacity on exit',
    'all_job_draining': 'drains results for every submitted job',
    'compiler_lifecycle': 'submitted, receiving, received, validated, published, selected',
    'policy_version': 'Freeze their initial values as `SchedulingPolicyV1`',
}


def json_examples(spec):
    for block in re.findall(r'```json\n(.*?)\n```', spec, re.S):
        try:
            yield json.loads(block)
        except json.JSONDecodeError:
            for line in block.splitlines():
                yield json.loads(line)


def audit(spec, ledger):
    errors = []
    for name, clause in OBSOLETE.items():
        if clause in spec:
            errors.append(name)
    anchors = re.findall(r'<a id="(section-\d+)"></a>', spec)
    if len(anchors) != len(set(anchors)) or any(
        a not in anchors for a in re.findall(r'\]\(#(section-\d+)\)', spec)
    ):
        errors.append('section_anchors')
    cards = re.findall(r'^\*\*([THM]\d{3}) — ', spec, re.M)
    tasks = [t['id'] for t in ledger['tasks']]
    ordered = []
    for batch in ledger['cross_ledger_order']:
        for item in batch['tasks_in_order']:
            if '–' in item:
                start, end = item.split('–')
                ordered.extend(f'{start[0]}{n:03}' for n in range(int(start[1:]), int(end[1:]) + 1))
            else:
                ordered.append(item)
    if any(len(x) != len(set(x)) for x in (cards, tasks, ordered)) or not set(cards) == set(tasks) == set(ordered):
        errors.append('task_order')
    revision = re.search(r'\*\*Document revision:\*\* (\S+)', spec)
    if not revision or revision[1] != ledger['spec_revision']:
        errors.append('revision')
    try:
        for block in re.findall(r'```toml\n(.*?)\n```', spec, re.S):
            tomllib.loads(block)
        for obj in json_examples(spec):
            if 'schema_version' in obj and obj['schema_version'] != 3:
                errors.append('service_example_version')
            if obj.get('method') != 'run':
                continue
            p = obj['params']
            required = {'snapshot', 'selection', 'jit', 'jit_tier', 'execution_contract',
                        'policy', 'limits', 'stream_output', 'replace_pending',
                        'allow_macro_exec', 'execution', 'allow_native', 'native_generation'}
            if set(p) != required:
                errors.append('run_example_fields')
            if (p['execution_contract'] == 'throughput') != (p['limits']['fuel'] is None):
                errors.append('fuel_contract')
    except (ValueError, KeyError) as error:
        errors.append(f'example_parse: {error}')
    boundaries = re.findall(r'^\| (B\d\d) \|', spec, re.M)
    if boundaries != [f'B{i:02}' for i in range(1, 11)]:
        errors.append('boundary_inventory')
    for label, clause in {
        'shared_native_storage': 'Both transports use one per-test native context and one slot registry',
        'helper_materialization': 'Before invoking a canonical-state helper',
        'compiler_process': 'The optimizing compiler worker is a supervised subprocess',
        'inlined_call_accounting': 'Fully inlined native bodies contribute zero physical ABI entries',
        'same_native_type': 'NativeResident is the storage contract for ValueKind 14',
    }.items():
        if clause not in spec:
            errors.append(label)
    for label, clause in {
        'binary_bulk_contract': 'Binary bytes, source chunks, checkpoint payloads, compiler jobs/IR/object bundles and bulk output are raw data frames',
        'separate_internal_version': '| LRR internal frame | 4 |',
        'public_version_preserved': '| Public LRR service envelope | 3 |',
        'scalar_invoker_measurement': 'Enable a class for production selection only when its measurements establish a benefit',
        'encoding_priorities': '### 61.15 Required encoding and copy reduction priorities',
    }.items():
        if clause not in spec:
            errors.append(label)
    header = re.search(r'Each internal frame has this exact 32-byte header.*?Small control records', spec, re.S)
    offsets = re.findall(r'^\| (\d+) \|', header[0], re.M) if header else []
    if offsets != ['0', '4', '6', '8', '12', '16', '24']:
        errors.append('binary_header_offsets')
    amendment = ledger.get('revision_transition', {}).get('encoding_amendment', {})
    planned = ['NX029', 'NX030', 'CA023', 'CA024', 'CA025', 'CA026']
    fixture_rows = set(re.findall(r'^\| ((?:NX|CA)\d{3}) \|', spec, re.M))
    if (amendment.get('public_service_schema') != 3
            or amendment.get('internal_framing_version') != 4
            or amendment.get('portable_query_format') != 'lrr-portable-v2'
            or amendment.get('new_pending_fixtures') != planned
            or not set(planned) <= fixture_rows):
        errors.append('encoding_amendment_ledger')
    transfer = ledger.get('revision_transition', {}).get('tier_transfer_amendment', {})
    planned_transfer = [f'CA{i:03}' for i in range(27, 34)]
    if (transfer.get('new_pending_fixtures') != planned_transfer
            or transfer.get('interpreter_first_fixture') != 'CA027'
            or transfer.get('status') != 'requirements_adopted_implementation_pending'
            or not set(planned_transfer) <= fixture_rows):
        errors.append('tier_transfer_amendment_ledger')
    owners = {
        'T022': ['CA027'], 'T024': ['CA027'], 'T070': ['CA028', 'CA032'],
        'T071': ['CA030'], 'T073': ['CA028', 'CA031'], 'T074': ['CA030', 'CA033'],
        'T085': ['CA028', 'CA032'], 'T086': ['CA028', 'CA031', 'CA032'],
        'T088': ['CA029'], 'T089': planned_transfer,
    }
    for task_id, expected in owners.items():
        task = next((t for t in ledger['tasks'] if t['id'] == task_id), {})
        entry = task.get('tier_transfer_amendment_acceptance', {})
        if (entry.get('status') != 'pending' or entry.get('fixture_ids') != expected
                or entry.get('evidence_paths') != []):
            errors.append('tier_transfer_pending_acceptance')
    for label, clause in TIER_REQUIRED.items():
        if clause not in spec:
            errors.append(label)
    if 'prevents further automatic exploration of the same unit/backend/dependency key' in spec:
        errors.append('permanent_optimization_exclusion')
    final = ledger.get('revision_transition', {}).get('final_contract_amendment', {})
    final_fixtures = [f'CA{i:03}' for i in range(34, 38)]
    if (final.get('status') != 'requirements_adopted_implementation_pending'
            or final.get('new_pending_fixtures') != final_fixtures
            or final.get('interpreter_first_fixture') != 'CA037'
            or final.get('scheduling_policy') != 'SchedulingPolicyV1'
            or not set(final_fixtures) <= fixture_rows):
        errors.append('final_contract_amendment_ledger')
    final_owners = {
        'T023': ['CA034'], 'T034': ['CA037'], 'T074': ['CA036'],
        'T085': ['CA034'], 'T086': ['CA034'], 'T088': ['CA035'],
        'T089': final_fixtures, 'M047': ['CA037'],
    }
    for task_id, expected in final_owners.items():
        task = next((t for t in ledger['tasks'] if t['id'] == task_id), {})
        entry = task.get('final_contract_amendment_acceptance', {})
        if (entry.get('status') != 'pending' or entry.get('fixture_ids') != expected
                or entry.get('evidence_paths') != []):
            errors.append('final_contract_pending_acceptance')
    for label, clause in FINAL_REQUIRED.items():
        if clause not in spec:
            errors.append(label)
    if 'Physical reservations count against worker memory, are released on exit' in spec:
        errors.append('premature_backing_release')
    policy = ledger.get('revision_transition', {}).get('runtime_acceptance_policy', {})
    if (policy.get('model') != 'runtime_semantics'
            or policy.get('jit_blocking_contracts') != ['runtime', 'interpreter_correctness']
            or policy.get('nonblocking_contracts') != ['diagnostic', 'compiler_tooling']
            or policy.get('reports') != ['upstream_runtime_acceptance', 'diagnostic_compatibility',
                                        'compiler_tooling_compatibility', 'full_suite_compatibility']
            or any(policy.get(k) is not False for k in
                   ['full_static_type_checker', 'borrow_checker', 'implicit_cargo_check'])
            or policy.get('fixture_ids') != ['CA037']
            or policy.get('evidence_paths') != []):
        errors.append('runtime_acceptance_policy')
    for name, clause in {
        'static_checks_external': 'It does not implement whole-program static type checking, borrow checking or cold-body diagnostic validation',
        'runtime_gate_only': 'Outstanding diagnostic/compiler-tooling contracts do not prevent guest JIT work',
        'mixed_case_accounting': 'a runtime-contract pass may coexist with outstanding diagnostic/tooling assertions',
    }.items():
        if clause not in spec:
            errors.append(name)
    if ('until every applicable configuration is full_pass' in spec
            or 'interpreter fully pass the official rustc test suite before' in spec):
        errors.append('obsolete_all_category_gate')
    return errors


def main():
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--spec', type=Path, default=root / 'lrr_rust2024_macros_traits_implementation_spec.md')
    parser.add_argument('--ledger', type=Path, default=root / 'docs/tasks.json')
    args = parser.parse_args()
    errors = audit(args.spec.read_text(), json.loads(args.ledger.read_text()))
    print(json.dumps({'scope': 'document consistency only', 'passed': not errors, 'errors': errors}, indent=2))
    return bool(errors)


if __name__ == '__main__':
    raise SystemExit(main())
