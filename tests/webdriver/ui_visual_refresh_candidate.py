#!/usr/bin/env python3
"""Capture an uncommitted visual-refresh candidate without touching historical baselines."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path
from typing import Any

import ui_visual_baseline as baseline


CANDIDATE_ID = "ui-visual-refresh-candidate-v1"
CANDIDATE_SCENARIO_VERSION = "flashcards-library-visual-refresh-v1"
_HISTORICAL_VERIFY = baseline.verify_manifest


def current_source_sha() -> str:
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=baseline.ROOT_DIR,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    sha = completed.stdout.strip()
    if re.fullmatch(r"[0-9a-f]{40}", sha) is None:
        raise RuntimeError(f"Could not resolve an exact Git source SHA: {sha!r}")
    return sha


def configure_candidate(source_sha: str) -> None:
    if re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        raise ValueError("Candidate source SHA must be a full lowercase Git SHA.")
    baseline.BASELINE_ID = CANDIDATE_ID
    baseline.BASELINE_SOURCE_SHA = source_sha
    baseline.SCENARIO_VERSION = CANDIDATE_SCENARIO_VERSION
    baseline.DECK_DESCRIPTION = "Representative visual-refresh candidate deck"


def default_destination(source_sha: str) -> Path:
    return baseline.ROOT_DIR / ".artifacts" / f"ui-visual-refresh-candidate-{source_sha[:12]}"


def ensure_safe_destination(destination: Path) -> Path:
    resolved = destination.resolve()
    historical = baseline.BASELINE_DIR.resolve()
    if resolved == historical:
        raise ValueError(
            "Visual-refresh candidates must not overwrite the committed Stage 0 baseline."
        )
    return resolved


def capture_candidate(application: Path, destination: Path, source_sha: str) -> dict[str, Any]:
    """Run the historical capture engine with exact-head candidate verification."""
    configure_candidate(source_sha)

    # ui_visual_baseline.verify_manifest intentionally binds the historical SHA
    # as a default argument. capture_matrix calls that function internally, so
    # install a narrow exact-head wrapper only for the duration of this capture.
    def verify_candidate(
        baseline_dir: Path = baseline.BASELINE_DIR,
        *,
        expected_source_sha: str = source_sha,
    ) -> dict[str, Any]:
        return _HISTORICAL_VERIFY(
            baseline_dir,
            expected_source_sha=expected_source_sha,
        )

    previous_verify = baseline.verify_manifest
    baseline.verify_manifest = verify_candidate
    try:
        return baseline.capture_matrix(application, destination)
    finally:
        baseline.verify_manifest = previous_verify


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--application", required=True, type=Path)
    parser.add_argument("--output-dir", type=Path)
    args = parser.parse_args()

    source_sha = current_source_sha()
    destination = ensure_safe_destination(
        args.output_dir if args.output_dir is not None else default_destination(source_sha)
    )
    manifest = capture_candidate(args.application, destination, source_sha)
    print(
        f"Captured and verified {len(manifest['captures'])} visual-refresh candidate images "
        f"for {source_sha} in {destination}."
    )
    print("Candidate evidence is review material only; it is not a committed visual baseline.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
