#!/usr/bin/env python3
"""Prune GitHub Actions cloud artifacts/caches for a self-hosted repository.

The oStudy runner keeps reusable build caches, build outputs, and diagnostic
evidence on the runner itself. GitHub Actions cloud storage is therefore not a
persistence boundary. Cleanup is deliberately rate-limit aware: each pass
reclaims as much as its current GitHub REST budget safely permits and leaves
remaining objects for the next scheduled pass.
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
DELETE_CHUNK_SIZE = 64
MAX_REQUEST_ATTEMPTS = 6
RATE_LIMIT_RESERVE = 100


class RateLimitReached(RuntimeError):
    """The current GitHub installation token has exhausted its REST budget."""

    def __init__(self, reset_epoch: int | None, message: str) -> None:
        super().__init__(message)
        self.reset_epoch = reset_epoch


class DeleteResult:
    def __init__(self) -> None:
        self.deleted = 0
        self.deleted_bytes = 0
        self.rate_limited = False


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--artifact-max-age-hours",
        type=float,
        default=0.0,
        help="Delete workflow artifacts at least this old. Zero purges all artifacts.",
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
        help="Upper bound on mutation calls per maintenance pass.",
    )
    parser.add_argument("--dry-run", action="store_true")
    return parser.parse_args()


class GitHubApi:
    def __init__(self, token: str, repository: str, base_url: str) -> None:
        self.token = token
        self.repository = repository
        self.base_url = base_url.rstrip("/")
        self.remaining: int | None = None
        self.reset_epoch: int | None = None

    def _capture_rate_headers(self, headers: Any) -> None:
        remaining = headers.get("X-RateLimit-Remaining")
        reset = headers.get("X-RateLimit-Reset")
        if remaining is not None:
            try:
                self.remaining = int(remaining)
            except ValueError:
                pass
        if reset is not None:
            try:
                self.reset_epoch = int(reset)
            except ValueError:
                pass

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
                    self._capture_rate_headers(response.headers)
                    payload = response.read()
                    return json.loads(payload) if payload else None
            except HTTPError as error:
                self._capture_rate_headers(error.headers)
                body = error.read().decode("utf-8", errors="replace")

                if method == "DELETE" and error.code in {404, 410}:
                    return None

                rate_limited = error.code in {403, 429} and (
                    self.remaining == 0
                    or "rate limit exceeded" in body.lower()
                    or "secondary rate limit" in body.lower()
                )
                if rate_limited:
                    raise RateLimitReached(
                        self.reset_epoch,
                        f"GitHub REST rate limit reached during {method} {path}",
                    ) from error

                retry_after = error.headers.get("Retry-After")
                transient = error.code in {429, 500, 502, 503, 504} or (
                    error.code == 403 and retry_after is not None
                )
                if transient and attempt < MAX_REQUEST_ATTEMPTS:
                    delay = float(retry_after) if retry_after else min(2 ** (attempt - 1), 30)
                    time.sleep(delay)
                    continue

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

    def safe_delete_budget(self, requested: int) -> int:
        if self.remaining is None:
            return requested
        return min(requested, max(0, self.remaining - RATE_LIMIT_RESERVE))


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


def rate_limit_reset_text(reset_epoch: int | None) -> str:
    if reset_epoch is None:
        return "unknown"
    return datetime.fromtimestamp(reset_epoch, tz=timezone.utc).isoformat()


def delete_batch(
    items: list[dict[str, Any]],
    *,
    api: GitHubApi,
    path_for: Callable[[dict[str, Any]], str],
    dry_run: bool,
) -> DeleteResult:
    result = DeleteResult()
    if dry_run:
        result.deleted = len(items)
        result.deleted_bytes = sum(int(item.get("size_in_bytes", 0)) for item in items)
        return result

    for offset in range(0, len(items), DELETE_CHUNK_SIZE):
        chunk = items[offset : offset + DELETE_CHUNK_SIZE]
        with ThreadPoolExecutor(max_workers=DELETE_WORKERS) as executor:
            futures = {
                executor.submit(api.request, "DELETE", path_for(item)): item
                for item in chunk
            }
            for future in as_completed(futures):
                item = futures[future]
                try:
                    future.result()
                except RateLimitReached:
                    result.rate_limited = True
                    continue
                result.deleted += 1
                result.deleted_bytes += int(item.get("size_in_bytes", 0))
        if result.rate_limited:
            break
    return result


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

    try:
        artifacts = api.paginated(f"{repo_path}/actions/artifacts", "artifacts")
        caches: list[dict[str, Any]] = []
        if args.delete_all_caches:
            caches = api.paginated(f"{repo_path}/actions/caches", "actions_caches")
    except RateLimitReached:
        write_summary(
            [
                "# GitHub Actions storage maintenance",
                "",
                "- Status: **deferred at GitHub REST rate-limit checkpoint**",
                f"- Repository: `{repository}`",
                f"- Rate-limit reset: **{rate_limit_reset_text(api.reset_epoch)}**",
                "- No failure: the next scheduled pass will resume cleanup.",
            ]
        )
        return 0

    artifact_bytes = sum(int(item.get("size_in_bytes", 0)) for item in artifacts)
    artifact_candidates = [
        item
        for item in artifacts
        if parse_github_timestamp(item["created_at"]) <= cutoff
    ]
    artifact_candidates.sort(
        key=lambda item: (int(item.get("size_in_bytes", 0)), item["created_at"]),
        reverse=True,
    )
    caches.sort(key=lambda item: int(item.get("size_in_bytes", 0)), reverse=True)

    safe_budget = api.safe_delete_budget(args.max_deletes)
    artifact_batch = artifact_candidates[:safe_budget]
    remaining_budget = max(0, safe_budget - len(artifact_batch))
    cache_batch = caches[:remaining_budget]

    artifact_result = delete_batch(
        artifact_batch,
        api=api,
        path_for=lambda item: f"{repo_path}/actions/artifacts/{item['id']}",
        dry_run=args.dry_run,
    )
    cache_result = DeleteResult()
    if not artifact_result.rate_limited:
        cache_result = delete_batch(
            cache_batch,
            api=api,
            path_for=lambda item: f"{repo_path}/actions/caches/{item['id']}",
            dry_run=args.dry_run,
        )

    rate_limited = artifact_result.rate_limited or cache_result.rate_limited
    deleted_total = artifact_result.deleted + cache_result.deleted
    delete_budget = max(0, safe_budget - deleted_total)
    remaining_candidate_count = max(0, len(artifact_candidates) - artifact_result.deleted)
    remaining_cache_count = max(0, len(caches) - cache_result.deleted)
    mode = "DRY RUN" if args.dry_run else "APPLIED"
    lines = [
        "# GitHub Actions storage maintenance",
        "",
        f"- Mode: **{mode}**",
        f"- Repository: `{repository}`",
        f"- Artifacts scanned: **{len(artifacts)}** ({human_bytes(artifact_bytes)})",
        f"- Artifact retention threshold: **{args.artifact_max_age_hours:g} hours**",
        f"- Safe delete budget for this pass: **{safe_budget}**",
        f"- Artifacts deleted: **{artifact_result.deleted}** ({human_bytes(artifact_result.deleted_bytes)})",
        f"- GitHub caches deleted: **{cache_result.deleted}** ({human_bytes(cache_result.deleted_bytes)})",
        f"- Delete calls remaining in this pass: **{delete_budget}**",
    ]
    if remaining_candidate_count or remaining_cache_count:
        lines.append(
            f"- Remaining eligible objects: **{remaining_candidate_count + remaining_cache_count}**"
        )
    if rate_limited:
        lines.extend(
            [
                "- Status: **rate-limit checkpoint reached; cleanup will resume automatically**",
                f"- Rate-limit reset: **{rate_limit_reset_text(api.reset_epoch)}**",
            ]
        )
    elif remaining_candidate_count or remaining_cache_count:
        lines.append("- Status: **bounded pass complete; remaining objects are deferred to the next run**")
    else:
        lines.append("- Status: **Actions artifact/cache storage is clean**")
    write_summary(lines)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
