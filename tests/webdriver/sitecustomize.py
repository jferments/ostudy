"""Bounded retries for transient WebDriver transport disconnects.

The unified self-hosted runner occasionally observes a single connection reset
between WebKitWebDriver and tauri-driver after a native app restart. Retrying a
read-only hit-test request is safe; retrying element clicks or session creation
is not. This module is imported automatically by Python's site initialization
because the smoke-test script directory is first on ``sys.path``.
"""

from __future__ import annotations

import http.client
import json
import time
import urllib.error
import urllib.request
from typing import Any

_ORIGINAL_URLOPEN = urllib.request.urlopen
_RETRY_DELAYS_SECONDS = (0.05, 0.15, 0.35)


def _request_is_retryable(target: Any, data: bytes | None) -> bool:
    if isinstance(target, urllib.request.Request):
        request = target
        method = request.get_method().upper()
        url = request.full_url
        body = request.data if request.data is not None else data
    else:
        method = "POST" if data is not None else "GET"
        url = str(target)
        body = data

    if method in {"GET", "HEAD"}:
        return True
    if method != "POST" or not url.rstrip("/").endswith("/execute/sync") or not body:
        return False

    try:
        payload = json.loads(body.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError):
        return False
    script = payload.get("script")
    if not isinstance(script, str):
        return False

    read_only_markers = (
        "elementFromPoint",
        "getBoundingClientRect",
        "document.readyState",
        "window.getComputedStyle",
    )
    return any(marker in script for marker in read_only_markers)


def _is_transient_disconnect(error: BaseException) -> bool:
    if isinstance(error, (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError)):
        return True
    if isinstance(error, urllib.error.URLError):
        return isinstance(
            error.reason,
            (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError),
        )
    return False


def _retrying_urlopen(
    target: Any,
    data: bytes | None = None,
    timeout: float | object = urllib.request._GLOBAL_DEFAULT_TIMEOUT,
    *args: Any,
    **kwargs: Any,
):
    if not _request_is_retryable(target, data):
        return _ORIGINAL_URLOPEN(target, data=data, timeout=timeout, *args, **kwargs)

    for delay in (*_RETRY_DELAYS_SECONDS, None):
        try:
            return _ORIGINAL_URLOPEN(target, data=data, timeout=timeout, *args, **kwargs)
        except BaseException as error:
            if delay is None or not _is_transient_disconnect(error):
                raise
            time.sleep(delay)

    raise AssertionError("unreachable WebDriver retry state")


urllib.request.urlopen = _retrying_urlopen
