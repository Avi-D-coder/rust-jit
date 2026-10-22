#!/usr/bin/env python3
"""Sweep imported unchanged rustc UI run-pass programs through the production CLI.

Triage and memory-measurement aid, NOT acceptance: it runs each simple case with
`lrr --edition <ED> run <file>` and records exit status, wall time, user CPU and
peak resident memory (wait4 rusage), plus the first stderr line. Pass here means
exit 0 and a `"status":"completed"` terminal; stdout and directives beyond
`run-pass`/`edition` are not checked. Acceptance stays with
`tools/rustc_checkout_runtime.py`.

The result table is append-only and resumable: rerunning skips cases already
present in <out>/results.tsv, so an interrupted sweep continues where it stopped.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "fixtures/upstream/rustc-checkout"


def simple_cases():
    manifest = json.loads((FIXTURES / "manifest.json").read_text())
    cases = []
    for case in manifest["cases"]:
        path = case["path"]
        if not path.startswith("tests/ui/") or not path.endswith(".rs") or "/auxiliary/" in path:
            continue
        directives = [d.strip() for d in case["directives"]]
        if not any(d.startswith(("//@ run-pass", "//@run-pass")) for d in directives):
            continue
        if not all(d.startswith(("//@ run-pass", "//@run-pass", "//@ edition")) for d in directives):
            continue
        edition = 2015
        for d in directives:
            found = re.match(r"//@ edition:\s*(\d+)", d)
            if found:
                edition = int(found.group(1))
        cases.append((path, edition))
    return cases


def measure(lrr, path, edition, timeout):
    """Run one case; rusage comes from wait4 so peak RSS is the child's own."""
    env = dict(os.environ, LRR_EMBEDDED_FACADE="1")
    argv = [str(lrr), "--edition", str(edition), "run", str(FIXTURES / path)]
    started = time.monotonic()
    proc = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, cwd=ROOT)
    chunks = {"out": [], "err": []}

    def drain(stream, key):
        chunks[key].append(stream.read())

    readers = [threading.Thread(target=drain, args=(proc.stdout, "out")),
               threading.Thread(target=drain, args=(proc.stderr, "err"))]
    for reader in readers:
        reader.start()
    timed_out = False
    deadline = started + timeout
    rusage = None
    status = None
    while True:
        pid, status, rusage = os.wait4(proc.pid, os.WNOHANG)
        if pid != 0:
            break
        if time.monotonic() > deadline:
            proc.kill()
            timed_out = True
            _, status, rusage = os.wait4(proc.pid, 0)
            break
        time.sleep(0.05)
    for reader in readers:
        reader.join()
    wall = time.monotonic() - started
    out = b"".join(chunks["out"])
    err = b"".join(chunks["err"])
    code = os.waitstatus_to_exitcode(status)
    completed = b'"status":"completed"' in out
    if timed_out:
        verdict = "TIMEOUT"
    elif code == 0 and completed:
        verdict = "PASS"
    else:
        verdict = "FAIL"
    first = err.decode("utf-8", "replace").strip().splitlines()[:1]
    first = first[0][:200] if first else ""
    rss_mb = rusage.ru_maxrss / (1024 * 1024)  # macOS reports bytes
    return [verdict, str(code), str(edition), path, f"{wall:.1f}",
            f"{rusage.ru_utime:.1f}", f"{rss_mb:.0f}", first.replace("\t", " ")]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lrr", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=4)
    parser.add_argument("--timeout", type=int, default=300)
    parser.add_argument("--shard", default="0/1", help="i/n: run every n-th case starting at i")
    parser.add_argument("--filter", default="", help="substring of the case path")
    parser.add_argument("--limit", type=int, default=0)
    args = parser.parse_args()
    index, count = (int(x) for x in args.shard.split("/"))
    cases = [c for n, c in enumerate(simple_cases()) if n % count == index and args.filter in c[0]]
    if args.limit:
        cases = cases[: args.limit]
    args.out.mkdir(parents=True, exist_ok=True)
    table = args.out / f"results-{index}of{count}.tsv"
    done = set()
    if table.exists():
        done = {line.split("\t")[3] for line in table.read_text().splitlines() if line.count("\t") >= 7}
    todo = [c for c in cases if c[0] not in done]
    print(f"{len(cases)} cases in shard, {len(done)} done, {len(todo)} to run", flush=True)
    lock = threading.Lock()

    def work(case):
        row = measure(args.lrr, case[0], case[1], args.timeout)
        with lock:
            with table.open("a") as handle:
                handle.write("\t".join(row) + "\n")
        return row

    with ThreadPoolExecutor(max_workers=args.jobs) as pool:
        counts = {}
        for row in pool.map(work, todo):
            counts[row[0]] = counts.get(row[0], 0) + 1
            if sum(counts.values()) % 50 == 0:
                print(counts, flush=True)
    print("finished", counts if todo else "nothing to run", flush=True)


if __name__ == "__main__":
    sys.exit(main())
