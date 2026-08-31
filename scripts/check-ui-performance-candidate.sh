#!/usr/bin/env bash
set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

usage() {
  echo "Usage: ./scripts/check-ui-performance-candidate.sh /absolute/new/output-directory" >&2
}

if (( $# != 1 )); then
  usage
  exit 2
fi

OUTPUT_DIR="$1"
if [[ "$OUTPUT_DIR" != /* ]]; then
  echo "ERROR: UI performance candidate output must be an explicit absolute path." >&2
  exit 2
fi
if [[ -e "$OUTPUT_DIR" || -L "$OUTPUT_DIR" ]]; then
  echo "ERROR: UI performance candidate output must not already exist: $OUTPUT_DIR" >&2
  exit 1
fi

echo "[ui-performance-check] Capturing deterministic candidate evidence at $OUTPUT_DIR"
bash "$SCRIPT_DIR/ui-performance-baselines.sh" capture-candidate "$OUTPUT_DIR"

MANIFEST="$OUTPUT_DIR/manifest.json"
MEASUREMENT="$OUTPUT_DIR/measurements/deck-drag-v1.json"
if [[ ! -f "$MANIFEST" || ! -f "$MEASUREMENT" ]]; then
  echo "ERROR: Candidate capture did not install the canonical evidence files." >&2
  exit 1
fi

echo "[ui-performance-check] Comparing deterministic candidate evidence with the immutable reference"
comparison_status=0
bash "$SCRIPT_DIR/ui-performance-baselines.sh" compare-candidate "$OUTPUT_DIR" \
  || comparison_status=$?
if (( comparison_status != 0 )); then
  echo "ERROR: UI performance candidate comparison failed; evidence retained at $OUTPUT_DIR" >&2
  exit "$comparison_status"
fi

echo "OK   Deterministic UI performance and trusted X11 mouse qualification passed; evidence retained at $OUTPUT_DIR"
