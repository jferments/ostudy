#!/usr/bin/env python3
"""Prune old oStudy CI evidence from the self-hosted runner safely.

Only top-level run directories named <github-run-id>-<attempt> are eligible.
The active run is always preserved. Age-based retention is applied first, then
oldest completed runs are evicted until the configured byte ceiling is met.
"""

from __future__ import annotations

import argparse
import re
import shutil
import time
from pathlib import Path

RUN_DIR = re.compile(r"^[0-9]+-[0-9]+$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--current", required=True)
    parser.add_argument("--retention-days", type=int, default=7)
    parser.add_argument("--max-bytes", type=int, default=20 * 1024**3)
    return parser.parse_args()


def directory_size(path: Path) -> int:
    total = 0
    for item in path.rglob("*"):
        try:
            if item.is_file() and not item.is_symlink():
                total += item.stat().st_size
        except FileNotFoundError:
            continue
    return total


def eligible_runs(root: Path, current: str) -> list[Path]:
    if not root.exists():
        return []
    return [
        item
        for item in root.iterdir()
        if item.is_dir()
        and not item.is_symlink()
        and RUN_DIR.fullmatch(item.name)
        and item.name != current
    ]


def remove_run(path: Path, root: Path) -> None:
    resolved_root = root.resolve()
    resolved = path.resolve()
    if resolved.parent != resolved_root or not RUN_DIR.fullmatch(resolved.name):
        raise RuntimeError(f"Refusing to prune unexpected path: {resolved}")
    shutil.rmtree(resolved)


def main() -> int:
    args = parse_args()
    if args.retention_days < 1 or args.max_bytes < 1024**3:
        raise SystemExit("retention-days must be >= 1 and max-bytes must be >= 1 GiB")

    root = args.root.expanduser().resolve()
    root.mkdir(parents=True, exist_ok=True)
    cutoff = time.time() - (args.retention_days * 86400)

    removed_age = 0
    for path in sorted(eligible_runs(root, args.current), key=lambda value: value.stat().st_mtime):
        if path.stat().st_mtime < cutoff:
            remove_run(path, root)
            removed_age += 1

    candidates = sorted(
        eligible_runs(root, args.current),
        key=lambda value: value.stat().st_mtime,
    )
    current_path = root / args.current
    current_size = directory_size(current_path) if current_path.exists() else 0
    sizes = {path: directory_size(path) for path in candidates}
    total = current_size + sum(sizes.values())
    removed_size = 0
    for path in candidates:
        if total <= args.max_bytes:
            break
        total -= sizes[path]
        remove_run(path, root)
        removed_size += 1

    print(
        "Runner-local evidence pruning: "
        f"age-removed={removed_age}, size-removed={removed_size}, "
        f"remaining-bytes={total}, max-bytes={args.max_bytes}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
