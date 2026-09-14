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
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timedelta, timezone
from typing import Any, Callable
from urllib.error import HTTPError
from urllib.parse import urlencode
from urllib.request import Request, urlopen

API_VERSION = "2026-03-10"
PER_PAGE = 100
MAX_PAGES = 100
DELETE_WORKERS = 8
MAX_REQUEST_ATTEMPTS = 6


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
        for attempt in range(1, MAX_REQUEST_ATTEMPTS + 1):
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

                retry_after = error.headers.get("Retry-After")
                transient = error.code in {429, 500, 502, 503, 504} or (
                    error.code == 403 and retry_after is not None
                )
                if transient and attempt < MAX_REQUEST_ATTEMPTS:
                    delay = float(retry_after) if retry_after else min(2 ** (attempt - 1), 30)
                    time.sleep(delay)
                    continue

                body = error.read().decode("utf-8", errors="replace")
                raise RuntimeError(
                    f"GitHub API {method} {path} failed with HTTP {error.code}: {body[:1000]}"
                ) from error
        raise AssertionError("request retry loop exhausted unexpectedly")

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


def delete_batch(
    items: list[dict[str, Any]],
    *,
    api: GitHubApi,
    path_for: Callable[[dict[str, Any]], str],
    dry_run: bool,
) -> tuple[int, int]:
    if dry_run:
        return len(items), sum(int(item.get("size_in_bytes", 0)) for item in items)

    deleted = 0
    deleted_bytes = 0
    with ThreadPoolExecutor(max_workers=DELETE_WORKERS) as executor:
        futures = {
            executor.submit(api.request, "DELETE", path_for(item)): item
            for item in items
        }
        for future in as_completed(futures):
            item = futures[future]
            future.result()
            deleted += 1
            deleted_bytes += int(item.get("size_in_bytes", 0))
    return deleted, deleted_bytes


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
    cutoff = datetime.now(timezone.utc) - timedelta(hours=args.artifact_max_age_hours)

    artifacts = api.paginated(f"{repo_path}/actions/artifacts", "artifacts")
    artifact_bytes = sum(int(item.get("size_in_bytes", 0)) for item in artifacts)
    artifact_candidates = [
        item
        for item in artifacts
        if parse_github_timestamp(item["created_at"]) <= cutoff
    ]
    # Reclaim the most bytes first during a bounded emergency purge. The daily
    # maintenance pass eventually removes every object that crosses the cutoff.
    artifact_candidates.sort(
        key=lambda item: (int(item.get("size_in_bytes", 0)), item["created_at"]),
        reverse=True,
    )

    caches: list[dict[str, Any]] = []
    if args.delete_all_caches:
        caches = api.paginated(f"{repo_path}/actions/caches", "actions_caches")
        caches.sort(key=lambda item: int(item.get("size_in_bytes", 0)), reverse=True)

    artifact_batch = artifact_candidates[: args.max_deletes]
    remaining_budget = args.max_deletes - len(artifact_batch)
    cache_batch = caches[:remaining_budget]

    deleted_artifacts, deleted_artifact_bytes = delete_batch(
        artifact_batch,
        api=api,
        path_for=lambda item: f"{repo_path}/actions/artifacts/{item['id']}",
        dry_run=args.dry_run,
    )
    deleted_caches, deleted_cache_bytes = delete_batch(
        cache_batch,
        api=api,
        path_for=lambda item: f"{repo_path}/actions/caches/{item['id']}",
        dry_run=args.dry_run,
    )

    delete_budget = args.max_deletes - deleted_artifacts - deleted_caches
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
