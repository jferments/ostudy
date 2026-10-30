#!/usr/bin/env bash
set -Eeuo pipefail

# Uploads the release files as a draft GitHub release in OSTUDY_RELEASES_REPO
# (default jferments/ostudy). The maintainer reads it there and presses
# Publish; nothing here makes a release public.

usage() {
  cat <<'USAGE'
Usage: ./scripts/publish-github-release.sh TAG DIRECTORY

Create a draft release TAG in OSTUDY_RELEASES_REPO (default
jferments/ostudy) and upload every file DIRECTORY/SHA256SUMS.txt
lists, plus SHA256SUMS.txt itself. It uses OSTUDY_RELEASES_TOKEN if set, or
else the gh login (gh auth token); either needs Contents read and write on
that repository. Refuses a tag that already has a release there.
USAGE
}

[[ $# -eq 2 ]] || { usage >&2; exit 2; }
TAG="$1"
DIR="$2"
REPO="${OSTUDY_RELEASES_REPO:-jferments/ostudy}"
TOKEN="${OSTUDY_RELEASES_TOKEN:-}"
[[ -n "$TOKEN" ]] || TOKEN="$(gh auth token 2>/dev/null || true)"
[[ -n "$TOKEN" ]] || { printf 'ERROR: set OSTUDY_RELEASES_TOKEN or log in with gh auth login.\n' >&2; exit 1; }
[[ "$TAG" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]] || { printf 'ERROR: %s is not a release tag like v0.1.0.\n' "$TAG" >&2; exit 2; }
[[ -f "$DIR/SHA256SUMS.txt" ]] || { printf 'ERROR: %s has no SHA256SUMS.txt.\n' "$DIR" >&2; exit 1; }
(cd "$DIR" && sha256sum --check --strict --quiet SHA256SUMS.txt) || {
  printf 'ERROR: a file in %s does not match SHA256SUMS.txt.\n' "$DIR" >&2
  exit 1
}
mapfile -t FILES < <(awk '{ sub(/^\*/, "", $2); print $2 }' "$DIR/SHA256SUMS.txt")
FILES+=(SHA256SUMS.txt)

api() {
  curl --fail-with-body --silent --show-error --retry 3 \
    -H "Authorization: Bearer $TOKEN" \
    -H "Accept: application/vnd.github+json" \
    -H "X-GitHub-Api-Version: 2022-11-28" "$@"
}

status="$(curl --silent --output /dev/null --write-out '%{http_code}' \
  -H "Authorization: Bearer $TOKEN" -H "Accept: application/vnd.github+json" \
  "https://api.github.com/repos/$REPO/releases/tags/$TAG")"
[[ "$status" == 404 ]] || {
  printf 'ERROR: %s already has a release %s (HTTP %s), or the token cannot read it.\n' "$REPO" "$TAG" "$status" >&2
  exit 1
}

body="$(python3 - "$TAG" "$(git rev-parse HEAD)" <<'PY'
import json, sys
tag, commit = sys.argv[1], sys.argv[2]
print(json.dumps({
    "tag_name": tag,
    "name": f"oStudy {tag[1:]}",
    "draft": True,
    "body": (
        f"oStudy {tag[1:]} for Linux (x86_64) and Android, built from commit {commit}.\n\n"
        "Install steps for each file: https://ostudy.io/install/\n\n"
        "Check a download with `sha256sum --check --ignore-missing SHA256SUMS.txt`. "
        "BUILD-INFO.txt names the Android signing certificate's SHA-256."
    ),
}))
PY
)"
release="$(api -X POST "https://api.github.com/repos/$REPO/releases" -d "$body")"
upload_url="$(python3 -c 'import json, sys; print(json.load(sys.stdin)["upload_url"].split("{")[0])' <<< "$release")"
html_url="$(python3 -c 'import json, sys; print(json.load(sys.stdin)["html_url"])' <<< "$release")"

for file in "${FILES[@]}"; do
  printf 'Uploading %s\n' "$file"
  api -X POST -H "Content-Type: application/octet-stream" \
    --upload-file "$DIR/$file" "$upload_url?name=$file" > /dev/null
done

printf '\nDraft release %s in %s: %s\n' "$TAG" "$REPO" "$html_url"
