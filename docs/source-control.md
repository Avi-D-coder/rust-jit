# Source checkpoints

Commit source, tests, documentation, Cargo.lock and required licensed dependency
inputs. Keep Cargo targets, LRR preparation outputs, local validation artifacts,
Python environments and the full rustc checkout local. Selected unchanged rustc
fixtures belong in `fixtures/upstream/rustc-checkout`, with their licenses and
provenance. Repository attributes preserve upstream fixture bytes. Some vendored
projects have their own attributes, which override the root rules. The guard checks
the actual pinned bytes rather than assuming those rules are sufficient.

Before committing, review the staged changes and run the source guard:

```sh
git diff --cached --stat
python3 tools/check_source_checkpoint.py
```

The guard rejects staged build/cache paths and blobs larger than 5 MiB. It also
checks that staged vendor and upstream input bytes match the working files without
Git text conversion. For a changed upstream fixture directory, it also checks all
indexed originals against the indexed import manifest's sizes, SHA256 hashes and
Git blob IDs. A manifest-only change or deleted original cannot evade that check.
The staged path scan treats renames as additions and deletions so moving an
original out of its directory cannot hide the missing file.
This catches accidental formatting even when Git itself preserves the bytes;
the importer separately establishes that the manifest matches the pinned checkout.
Source handoff patches also retain exact bytes, including
upstream CRLF witnesses and the required blank context-line prefixes of unified
diffs. Immutable upstream copies are exempt from Git whitespace normalization and
linting; validate them against their pinned hashes instead of editing expected
output formatting. It checks the staged checkpoint, not previous commit history;
ignores do not remove already tracked files. If a generated output was accidentally
tracked, use `git rm --cached -- path` to remove it from the index while retaining
the local file. Review that change before committing.

An optional local pre-commit hook runs the same guard:

```sh
git config --local core.hooksPath .githooks
```

This replaces any existing `core.hooksPath` for this checkout. If you already use
hooks, add the guard to that workflow instead. Hook installation is opt-in; the
repository does not change your Git configuration automatically.

Restrict formatting to explicit LRR source paths. Never pass copied upstream
fixtures or compiler support sources to a mutating formatter, including through a
recursive directory walk. Check their raw pinned bytes again after formatting
before freezing a source manifest. Hashing the resulting files alone does not
prove that they still match upstream.

For a new checkout that needs to stage pinned vendor inputs, give byte preservation
precedence over nested vendor attributes with this local override:

```sh
attribute_path=$(git rev-parse --git-path info/attributes)
mkdir -p "$(dirname "$attribute_path")"
printf '\nvendor/** -text\n**/fixtures/upstream/rustc-checkout/** -text\n' >> "$attribute_path"
```

This appends rules without replacing existing local attributes. It affects future
Git conversion; it does not repair files already converted during checkout. Restore
those inputs from the pinned source and verify their recorded hashes before staging.

Do not broadly ignore `.stdout`, `.stderr`, `.out`, `.log`, lockfiles or archives:
upstream tests and pinned dependencies may require them. Put generated reports in
`artifacts/`, and custom Cargo target directories under an ignored `target/` or
`artifacts/` directory. The source guard also rejects force-added ignored outputs;
it does not scan file contents for secrets or certify interpreter acceptance.

Push source checkpoints to the implementation branch periodically. Do not fetch,
check out, apply or merge PR code without the user's approval.
