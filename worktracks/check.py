#!/usr/bin/env python3
"""Read-only contribution-plan validation; no compiler, guest or acceptance gate."""
import json
import re
import sys
from datetime import datetime
from pathlib import Path
from urllib.parse import unquote, urlparse

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / 'worktracks'
STATES = {'planned', 'open', 'claimed', 'in_progress', 'blocked', 'in_review', 'merged', 'accepted', 'superseded'}
CLAIM_STATES = {'active', 'blocked', 'in_review', 'released', 'merged'}
REPORTS = ['upstream_runtime_acceptance', 'diagnostic_compatibility', 'compiler_tooling_compatibility', 'full_suite_compatibility']


def check():
    errors = []

    def require(condition, message):
        if not condition:
            errors.append(message)

    def read(name):
        try:
            return json.loads((BASE / name).read_text())
        except (OSError, ValueError) as exc:
            errors.append(f'{name}: {exc}')
            return {}

    def local_path(value, label, must_exist=False):
        if not isinstance(value, str) or not value:
            errors.append(f'{label}: missing local path')
            return None
        path = (ROOT / value).resolve()
        require(not Path(value).is_absolute() and path.is_relative_to(ROOT), f'{label}: path leaves repository')
        if must_exist:
            require(path.is_file(), f'{label}: missing file {value}')
        return path

    def timestamp(value, label):
        try:
            parsed = datetime.fromisoformat(value.replace('Z', '+00:00'))
            require(parsed.tzinfo is not None, f'{label}: timezone required')
        except (ValueError, TypeError, AttributeError):
            errors.append(f'{label}: invalid timestamp')

    def github_link(value, kind, label):
        if value is None:
            return
        if not isinstance(value, str):
            errors.append(f'{label}: URL must be a string or null')
            return
        u = urlparse(value)
        require(u.scheme == 'https' and u.netloc == 'github.com' and
                re.fullmatch(rf'/[^/]+/[^/]+/{kind}/[1-9][0-9]*', u.path) is not None,
                f'{label}: expected actual GitHub {kind} URL')

    registry, claims = read('registry.json'), read('claims.json')
    require(registry.get('schema_version') == 1 and claims.get('schema_version') == 1, 'unsupported registry/claims schema')
    task_rows = json.loads((ROOT / 'docs/tasks.json').read_text())['tasks']
    task_ids = {row['id'] for row in task_rows}
    policy = registry.get('gate_policy', {})
    require(policy.get('reports') == REPORTS, 'four separate compatibility reports required')
    require(policy.get('guest_jit_prerequisites') == ['upstream_runtime_acceptance', 'applicable_interpreter_correctness'], 'wrong guest JIT prerequisites')
    for key in ['unsupported_runtime_is_blocker', 'runtime_only_pass_is_not_full_fixture_pass', 'external_rustc_is_not_interpreter_execution']:
        require(policy.get(key) is True, f'gate policy missing {key}')
    require(policy.get('runtime_gate_status') in {'incomplete', 'passed'}, 'unknown runtime gate state')
    if policy.get('runtime_gate_status') == 'passed':
        require(bool(policy.get('evidence')), 'runtime gate pass needs actual evidence')
    timestamp(registry.get('updated_at'), 'registry.updated_at')
    for name, path in registry.get('source_of_truth', {}).items():
        local_path(path, f'source_of_truth.{name}', True)
    rows = registry.get('tracks', [])
    require(1 <= len(rows) <= 12, 'expected 1–12 worktracks')
    ids, items, graph, plans = set(), {}, {}, []
    for row in rows:
        tid = row.get('id')
        require(isinstance(tid, str) and re.fullmatch(r'WT[0-9]{2}', tid) is not None, f'invalid track ID {tid}')
        require(tid not in ids, f'duplicate track {tid}')
        ids.add(tid)
        require(row.get('status') in STATES, f'{tid}: invalid state')
        plans.append(local_path(row.get('plan'), f'{tid}.plan', True))
        require(bool(row.get('task_ids')), f'{tid}: no ledger cards')
        for task in row.get('task_ids', []):
            require(task in task_ids, f'{tid}: unknown card {task}')
        for path in row.get('suggested_paths', []):
            local_path(path, f'{tid}.suggested_paths')
        graph[tid] = row.get('coordination_dependencies', [])
        for item in row.get('items', []):
            iid = item.get('id')
            require(isinstance(iid, str) and iid.startswith(f'{tid}-') and iid not in items, f'{tid}: duplicate/invalid item {iid}')
            items[iid] = tid
            require(item.get('status') in STATES, f'{iid}: invalid state')
            if item.get('status') in {'merged', 'accepted'}:
                require(bool(item.get('pull_request')) and bool(item.get('merge_commit')), f'{iid}: merged/accepted needs PR and merge commit')
                github_link(item.get('pull_request'), 'pull', iid)
            if item.get('status') == 'accepted':
                require(bool(item.get('evidence')) and bool(item.get('acceptance_scope')), f'{iid}: acceptance evidence/scope required')
    visiting, visited = set(), set()

    def visit(tid):
        if tid in visiting:
            errors.append(f'coordination dependency cycle at {tid}')
            return
        if tid in visited:
            return
        visiting.add(tid)
        for dep in graph.get(tid, []):
            require(dep in ids and dep != tid, f'{tid}: invalid dependency {dep}')
            if dep in ids:
                visit(dep)
        visiting.remove(tid)
        visited.add(tid)

    for tid in sorted(ids):
        visit(tid)
    seen_claims = set()
    active_paths = []
    for claim in claims.get('claims', []):
        cid = claim.get('id')
        require(isinstance(cid, str) and cid not in seen_claims, f'duplicate/invalid claim {cid}')
        seen_claims.add(cid)
        require(claim.get('track') in ids and items.get(claim.get('item')) == claim.get('track'), f'{cid}: unknown track/item')
        require(claim.get('status') in CLAIM_STATES, f'{cid}: invalid status')
        require(claim.get('kind') in {'local_agent', 'contributor'}, f'{cid}: invalid owner kind')
        require(bool(claim.get('owner')) and bool(claim.get('scope')) and bool(claim.get('paths')), f'{cid}: owner/scope/paths required')
        for path in claim.get('paths', []):
            resolved = local_path(path, f'{cid}.paths')
            if resolved is not None and claim.get('status') in {'active', 'blocked', 'in_review'}:
                for other_id, other_path in active_paths:
                    if other_id != cid and (resolved.is_relative_to(other_path) or other_path.is_relative_to(resolved)):
                        errors.append(f'{cid}/{other_id}: overlapping reserved paths; split claims or reserve a shared interface under one owner')
                active_paths.append((cid, resolved))
        timestamp(claim.get('updated_at'), f'{cid}.updated_at')
        timestamp(claim.get('review_after'), f'{cid}.review_after')
        github_link(claim.get('issue'), 'issues', cid)
        github_link(claim.get('pull_request'), 'pull', cid)
        if claim.get('kind') == 'contributor':
            require(bool(claim.get('issue')), f'{cid}: public contributor claim needs issue')
        if claim.get('status') == 'merged':
            require(bool(claim.get('pull_request')), f'{cid}: merged claim needs PR')
    try:
        lines = (BASE / 'history.jsonl').read_text().splitlines()
        require(bool(lines), 'empty history')
        for number, line in enumerate(lines, 1):
            event = json.loads(line)
            require(event.get('schema_version') == 1 and bool(event.get('event')), f'history:{number}: schema/event missing')
            timestamp(event.get('at'), f'history:{number}.at')
            require(bool(event.get('track_ids')) and all(t in ids for t in event['track_ids']), f'history:{number}: unknown tracks')
            github_link(event.get('issue'), 'issues', f'history:{number}')
            github_link(event.get('pull_request'), 'pull', f'history:{number}')
            if event.get('event') in {'merged', 'accepted'}:
                require(bool(event.get('pull_request')) and bool(event.get('merge_commit')), f'history:{number}: missing PR/merge')
            if event.get('event') == 'accepted':
                require(bool(event.get('evidence')) and bool(event.get('acceptance_scope')), f'history:{number}: missing acceptance evidence/scope')
    except (OSError, ValueError, TypeError) as exc:
        errors.append(f'history: {exc}')
    docs = [BASE / 'README.md', ROOT / 'AGENTS.md', ROOT / 'README.md'] + plans
    for path in docs:
        if path is None or not path.is_file():
            continue
        for target in re.findall(r'\]\(([^)\s]+)\)', path.read_text()):
            if urlparse(target).scheme or target.startswith('#'):
                continue
            target_path = (path.parent / unquote(target.split('#', 1)[0])).resolve()
            require(target_path.is_relative_to(ROOT) and target_path.exists(), f'{path.relative_to(ROOT)}: broken local link {target}')
    return errors, len(rows), len(items), len(seen_claims)


if __name__ == '__main__':
    try:
        problems, tracks, items, claims = check()
    except (KeyError, OSError, TypeError, ValueError) as exc:
        print(f'Invalid planning data: {exc}', file=sys.stderr)
        sys.exit(1)
    for problem in problems:
        print(problem, file=sys.stderr)
    if problems:
        sys.exit(1)
    print(f'Plan integrity: {tracks} tracks, {items} items, {claims} claims; no runtime acceptance claimed.')
