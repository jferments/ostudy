#!/usr/bin/env bash
set -Eeuo pipefail

# Resolve an outdated JavaScript lockfile only in a disposable exact-source copy.
# The candidate is review evidence; this script never rewrites maintained source.
candidate="$(mktemp -d -t ostudy-pnpm-lock-candidate.XXXXXXXX)"
cleanup() { rm -rf -- "$candidate"; }
trap cleanup EXIT

git archive --format=tar HEAD | tar -x -C "$candidate"
(
  cd "$candidate"
  pnpm install --ignore-scripts --lockfile-only --no-frozen-lockfile
  python3 - pnpm-lock.yaml <<'PY'
import hashlib
import pathlib
import sys

data = pathlib.Path(sys.argv[1]).read_bytes()
git_header = f"blob {len(data)}\0".encode()
print("LOCKFILE_CANDIDATE_GIT_BLOB=" + hashlib.sha1(git_header + data).hexdigest())
print("LOCKFILE_CANDIDATE_SHA256=" + hashlib.sha256(data).hexdigest())
PY
)
printf 'LOCKFILE_CANDIDATE_DIFF_BEGIN\n'
diff -u --label a/pnpm-lock.yaml --label b/pnpm-lock.yaml \
  pnpm-lock.yaml "$candidate/pnpm-lock.yaml" || true
printf 'LOCKFILE_CANDIDATE_DIFF_END\n'
