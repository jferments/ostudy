#!/usr/bin/env python3
"""Prune GitHub Actions cloud artifacts/caches for a self-hosted repository.

The ostudy runner keeps reusable build caches and diagnostic evidence on the
runner itself. GitHub Actions cloud storage is therefore treated as a bounded,
short-lived transport only.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timedelta, timezone
from typing import Any
from urllib.error import HTTPError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

API_VERSION = "2026-03-10"
PER_PAGE = 100
MAX_PAGES = 100


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--artifact-max-age-hours",
        type=float,
        default=24.0,
        help="Delete workflow artifacts at least this old. Use 0 for a full purge.",
    )
    parser.add_argument(
        "--delete-all-caches",
        action="store_true",
        help="Delete every GitHub-hosted Actions cache for this repository.",
    )
    parser.add_argument(
        "--max-deletes",
        type=int,
        default=4000,
        help="Bound mutation calls so one maintenance run stays below API limits.",
    )
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


class GitHubApi:
    def __init__(self, token: str, repository: str, base_url: str) -> None:
        self.token = token
        self.repository = repository
        self.base_url = base_url.rstrip("/")

    def request(self, method: str, path: str) -> Any:
        request = Request(
            f"{self.base_url}{path}",
            method=method,
            headers={
                "Accept": "application/vnd.github+json",
                "Authorization": f"Bearer {self.token}",
                "X-GitHub-Api-Version": API_VERSION,
                "User-Agent": "ostudy-actions-storage-maintenance",
            },
        )
        try:
            with urlopen(request, timeout=30) as response:
                payload = response.read()
                return json.loads(payload) if payload else None
        except HTTPError as error:
            if method == "DELETE" and error.code in {404, 410}:
                return None
            body = error.read().decode("utf-8", errors="replace")
            raise RuntimeError(
                f"GitHub API {method} {path} failed with HTTP {error.code}: {body[:1000]}"
            ) from error

    def paginated(self, endpoint: str, key: str) -> list[dict[str, Any]]:
        collected: list[dict[str, Any]] = []
        for page in range(1, MAX_PAGES + 1):
            separator = "&" if "?" in endpoint else "?"
            query = urlencode({"per_page": PER_PAGE, "page": page})
            payload = self.request("GET", f"{endpoint}{separator}{query}")
            batch = payload.get(key, []) if isinstance(payload, dict) else []
            collected.extend(batch)
            if len(batch) < PER_PAGE:
                break
        return collected


def parse_github_timestamp(value: str) -> datetime:
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def human_bytes(value: int) -> str:
    units = ["B", "KiB", "MiB", "GiB", "TiB"]
    amount = float(value)
    for unit in units:
        if amount < 1024 or unit == units[-1]:
            return f"{amount:.2f} {unit}"
        amount /= 1024
    raise AssertionError("unreachable")


def write_summary(lines: list[str]) -> None:
    text = "\n".join(lines) + "\n"
    print(text, end="")
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with open(summary_path, "a", encoding="utf-8") as summary:
            summary.write(text)


def main() -> int:
    args = parse_args()
    token = os.environ.get("GITHUB_TOKEN")
    repository = os.environ.get("GITHUB_REPOSITORY")
    base_url = os.environ.get("GITHUB_API_URL", "https://api.github.com")
    if not token or not repository:
        print("GITHUB_TOKEN and GITHUB_REPOSITORY are required", file=sys.stderr)
        return 2
    if args.artifact_max_age_hours < 0 or args.max_deletes < 1:
        print("Retention and deletion bounds must be non-negative/positive", file=sys.stderr)
        return 2

    api = GitHubApi(token, repository, base_url)
    repo_path = f"/repos/{repository}"
    now = datetime.now(timezone.utc)
    cutoff = now - timedelta(hours=args.artifact_max_age_hours)

    artifacts = api.paginated(f"{repo_path}/actions/artifacts", "artifacts")
    artifact_bytes = sum(int(item.get("size_in_bytes", 0)) for item in artifacts)
    artifact_candidates = [
        item
        for item in artifacts
        if parse_github_timestamp(item["created_at"]) <= cutoff
    ]
    artifact_candidates.sort(key=lambda item: item["created_at"])

    caches: list[dict[str, Any]] = []
    if args.delete_all_caches:
        caches = api.paginated(f"{repo_path}/actions/caches", "actions_caches")

    delete_budget = args.max_deletes
    deleted_artifacts = 0
    deleted_artifact_bytes = 0
    deleted_caches = 0
    deleted_cache_bytes = 0

    for artifact in artifact_candidates:
        if delete_budget <= 0:
            break
        size = int(artifact.get("size_in_bytes", 0))
        if not args.dry_run:
            api.request("DELETE", f"{repo_path}/actions/artifacts/{artifact['id']}")
        deleted_artifacts += 1
        deleted_artifact_bytes += size
        delete_budget -= 1

    for cache in caches:
        if delete_budget <= 0:
            break
        size = int(cache.get("size_in_bytes", 0))
        if not args.dry_run:
            api.request("DELETE", f"{repo_path}/actions/caches/{cache['id']}")
        deleted_caches += 1
        deleted_cache_bytes += size
        delete_budget -= 1

    remaining_candidate_count = max(0, len(artifact_candidates) - deleted_artifacts)
    remaining_cache_count = max(0, len(caches) - deleted_caches)
    mode = "DRY RUN" if args.dry_run else "APPLIED"
    lines = [
        "# GitHub Actions storage maintenance",
        "",
        f"- Mode: **{mode}**",
        f"- Repository: `{repository}`",
        f"- Artifacts scanned: **{len(artifacts)}** ({human_bytes(artifact_bytes)})",
        f"- Artifact retention threshold: **{args.artifact_max_age_hours:g} hours**",
        f"- Artifacts deleted: **{deleted_artifacts}** ({human_bytes(deleted_artifact_bytes)})",
        f"- GitHub caches deleted: **{deleted_caches}** ({human_bytes(deleted_cache_bytes)})",
        f"- Delete calls remaining in safety budget: **{delete_budget}**",
    ]
    if remaining_candidate_count or remaining_cache_count:
        lines.append(
            f"- Remaining eligible objects after safety bound: **{remaining_candidate_count + remaining_cache_count}**"
        )
    write_summary(lines)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
