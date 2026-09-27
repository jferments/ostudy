#!/usr/bin/env bash
set -Eeuo pipefail

# Keep normal qualification immutable. When manifests legitimately outgrow the
# committed lockfile, resolve a candidate only in a disposable exact-source copy
# and print reviewable bytes/hash; this helper never rewrites maintained source.
if pnpm install --frozen-lockfile; then
  exit 0
else
  frozen_status=$?
fi

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

path = pathlib.Path(sys.argv[1])
data = path.read_bytes()
git_header = f"blob {len(data)}\0".encode()
print("LOCKFILE_CANDIDATE_GIT_BLOB=" + hashlib.sha1(git_header + data).hexdigest())
print("LOCKFILE_CANDIDATE_SHA256=" + hashlib.sha256(data).hexdigest())
print("LOCKFILE_CANDIDATE_TEXT_BEGIN")
sys.stdout.write(data.decode("utf-8"))
if not data.endswith(b"\n"):
    print()
print("LOCKFILE_CANDIDATE_TEXT_END")
PY
)

exit "$frozen_status"
