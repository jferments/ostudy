#!/usr/bin/env python3
"""Capture the main oStudy screens for before/after UI review, or compare two captures.

``capture`` drives a built application through tauri-driver: it seeds a
representative disposable library, then photographs each main screen in every
theme at phone, tablet, and desktop widths. ``compare`` turns two capture
directories into one self-contained comparison page. Use
``scripts/ui-screenshots.sh`` rather than calling this file directly; the
wrapper provides Xvfb, isolated application data, and the binary.

The engine drives builds from before and after the navigation redesign: it
recognizes which navigation the build has and skips the screens that build
does not have, so one capture tool serves a before/after comparison.
"""

from __future__ import annotations

import argparse
import base64
import datetime
import functools
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
from collections.abc import Callable
from concurrent.futures import Future, ThreadPoolExecutor
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import smoke as webdriver
import ui_platform_smoke as ui
import ui_screenshots_report as report
import ui_screenshots_seed as seed

THEMES = ("light", "dark", "console", "synthwave")
VIEWPORTS: dict[int, tuple[str, int, str]] = {
    390: ("Phone", 844, "compact"),
    1024: ("Tablet", 768, "medium"),
    1440: ("Desktop", 900, "wide"),
}
DIALOG_WIDTHS = (390, 1440)
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"
APP_IDENTIFIER = "io.github.jferments.ostudy.dev"
DEFAULT_DRIVER_PORT = 4460
WEBP_QUALITY = 85
CAPTURE_SETTINGS = {
    # Motion "none" also disables celebrations, so no confetti or transitions.
    "motionPreference": "none",
    "tutorialsEnabled": False,
}
CAPTURE_CSS = """
*, *::before, *::after {
  animation-duration: 0s !important;
  animation-delay: 0s !important;
  transition-duration: 0s !important;
  transition-delay: 0s !important;
  caret-color: transparent !important;
  scroll-behavior: auto !important;
  /* WebKit paints a resized bitmap at low quality until a timer repaints it;
     optimizeQuality always paints it at high quality. */
  image-rendering: optimizeQuality !important;
}
* { scrollbar-width: none !important; }
*::-webkit-scrollbar { display: none !important; width: 0 !important; height: 0 !important; }
"""


def log(message: str) -> None:
    print(f"[ui-screenshots] {message}", flush=True)


def xpath_literal(value: str) -> str:
    if '"' not in value:
        return f'"{value}"'
    if "'" not in value:
        return f"'{value}'"
    parts = value.split('"')
    return "concat(" + ", '\"', ".join(f'"{part}"' for part in parts) + ")"


# --------------------------------------------------------------------------
# Screen catalog
# --------------------------------------------------------------------------


# The two navigations a build can have: the sidebar of section tabs
# (Flashcards, Study Guides, Documents, Tutor, Analytics, Settings), and the
# destinations (Home, Library, Tutor, Progress and Settings; builds before
# 2026-10-20 had Review instead of Settings) with Library's view switcher.
SECTIONS_SHELL = "sections"
DESTINATIONS_SHELL = "destinations"
SHELLS = (SECTIONS_SHELL, DESTINATIONS_SHELL)


@dataclass(frozen=True)
class Screen:
    id: str
    title: str
    group: str
    widths: tuple[int, ...] | None = None  # None: every requested width
    flow: str = "page"  # page | dialog | practice | first-run
    shells: tuple[str, ...] = SHELLS  # the navigations that have this screen


SCREENS: tuple[Screen, ...] = (
    Screen("home", "Home", "Home", shells=(DESTINATIONS_SHELL,)),
    # The landing page of a library with nothing in it.
    Screen("first-run", "First run: empty library", "Home", None, "first-run"),
    # A subject's page, its Library Overview; builds before it existed record a failure.
    Screen("subject", "Subject page", "Library", shells=(DESTINATIONS_SHELL,)),
    Screen("library", "Library: Decks", "Library"),
    Screen("deck", "Deck card list", "Library"),
    Screen("card-editor", "Card editor", "Library"),
    Screen("practice-setup-dialog", "Practice setup dialog", "Library", DIALOG_WIDTHS, "dialog"),
    Screen("practice-question", "Practice: question", "Practice", None, "practice"),
    Screen("practice-answer", "Practice: answer revealed", "Practice", None, "practice"),
    Screen("practice-summary", "Practice: session summary", "Practice", None, "practice"),
    Screen("guides", "Library: Study guides", "Library"),
    Screen("guide-reader", "Study guide reader", "Library"),
    Screen("documents", "Library: Documents", "Library"),
    Screen("document-info", "Document generation info", "Library"),
    Screen("analytics", "Progress overview", "Progress"),
    Screen("analytics-summary", "Progress summary tiles", "Progress"),
    Screen("analytics-details", "Progress topics and card strength", "Progress"),
    Screen("tutor", "Tutor conversation", "Tutor"),
    Screen("settings-appearance", "Settings: Appearance", "Settings"),
    Screen("settings-backup", "Settings: Backup and sync", "Settings"),
    Screen("settings-ai", "Settings: AI", "Settings"),
    Screen("settings-review", "Settings: Review", "Settings"),
    Screen("settings-templates", "Settings: Card templates", "Settings"),
    Screen("settings-diagnostics", "Settings: Diagnostics", "Settings"),
)
SETTINGS_TABS = {
    "settings-appearance": "Appearance",
    "settings-backup": "Backup and sync",
    "settings-ai": "AI",
    "settings-review": "Review",
    "settings-templates": "Card templates",
    "settings-diagnostics": "Diagnostics",
}
SCREEN_BY_ID = {screen.id: screen for screen in SCREENS}


def screen_widths(screen: Screen, widths: tuple[int, ...]) -> tuple[int, ...]:
    if screen.widths is None:
        return widths
    return tuple(width for width in widths if width in screen.widths)


def screens_for_shell(screens: list[Screen], shell: str) -> tuple[list[Screen], list[Screen]]:
    """Split the selected screens into those a build with this navigation has and those it lacks."""
    return (
        [screen for screen in screens if shell in screen.shells],
        [screen for screen in screens if shell not in screen.shells],
    )


# --------------------------------------------------------------------------
# WebDriver session helpers
# --------------------------------------------------------------------------


class CaptureError(RuntimeError):
    pass


@dataclass
class Context:
    session_id: str
    theme: str
    width: int = 0
    layout: str = ""
    shell: str = ""
    warnings: list[str] = field(default_factory=list)

    def deadline(self, seconds: int = 30) -> float:
        return time.monotonic() + seconds


def execute(ctx: Context, script: str, args: list[Any] | None = None) -> Any:
    return ui.execute(ctx.session_id, script, args)


BATCH_SCRIPT = """
const steps = arguments[0];
const done = arguments[arguments.length - 1];
const invoke = window.__TAURI_INTERNALS__.invoke;
const describe = (error) => (typeof error === 'string' ? error : (error && error.message) || JSON.stringify(error));
(async () => {
  const results = [];
  let last = null;
  for (const [index, step] of steps.entries()) {
    const args = step.args;
    if (step.chainRevision) {
      if (!last || typeof last.revision !== 'number') throw new Error(`step ${index} has no revision to chain`);
      args.input.expectedOrganizationRevision = last.revision;
    }
    try {
      last = await invoke(step.command, args);
    } catch (error) {
      throw new Error(`${step.command} (step ${index}) failed: ${describe(error)}`);
    }
    results.push(last === undefined ? null : last);
  }
  return results;
})().then((value) => done({ ok: true, value }), (error) => done({ ok: false, error: describe(error) }));
"""

PRACTICE_SCRIPT = """
const plans = arguments[0];
const done = arguments[arguments.length - 1];
const invoke = window.__TAURI_INTERNALS__.invoke;
const describe = (error) => (typeof error === 'string' ? error : (error && error.message) || JSON.stringify(error));
(async () => {
  const sessions = [];
  for (const plan of plans) {
    const session = await invoke('start_card_practice', { input: {
      cardIds: plan.cardIds, shuffle: false, operationId: plan.operationId } });
    const order = session.firstWindow.cards.map((card) => card.presentation.cardId);
    let last = null;
    for (let index = 0; index < order.length; index += 1) {
      const cardId = order[index];
      await invoke('mark_practice_card_viewed', { input: { sessionId: session.id, cardId } });
      const preview = await invoke('mark_practice_card_revealed', { input: { sessionId: session.id, cardId } });
      last = await invoke('submit_practice_review', { input: {
        sessionId: session.id, cardId, submissionId: plan.submissionIds[index],
        outcome: plan.outcomes[cardId], responseTimeMs: plan.responseTimes[cardId],
        scheduleRevision: preview.scheduleRevision } });
    }
    if (!last || !last.completed) await invoke('complete_practice_session', { sessionId: session.id });
    sessions.push({ sessionId: session.id, order });
  }
  return sessions;
})().then((value) => done({ ok: true, value }), (error) => done({ ok: false, error: describe(error) }));
"""


def ipc_batch(session_id: str, steps: list[dict[str, Any]]) -> list[Any]:
    result = ui.execute_async(session_id, BATCH_SCRIPT, [steps])
    if not isinstance(result, dict) or not result.get("ok"):
        raise CaptureError(f"Seeding failed: {result!r}")
    return list(result["value"])


def ipc_practice(session_id: str, plans: list[dict[str, Any]]) -> list[dict[str, Any]]:
    result = ui.execute_async(session_id, PRACTICE_SCRIPT, [plans])
    if not isinstance(result, dict) or not result.get("ok"):
        raise CaptureError(f"Seeding practice history failed: {result!r}")
    return list(result["value"])


def install_transport_retry() -> None:
    """Resend a WebDriver request that tauri-driver could not deliver.

    tauri-driver keeps a pooled connection to WebKitWebDriver, which closes
    every connection after its response. A request that lands on such a
    connection fails inside tauri-driver before WebKitWebDriver sees it, and
    tauri-driver then drops the client connection without an answer, so the
    request can be sent again.
    """
    if getattr(webdriver.request, "__ostudy_retry__", False):
        return
    send = webdriver.request

    def request(method: str, path: str, payload: dict[str, Any] | None = None) -> Any:
        for attempt in range(4):
            try:
                return send(method, path, payload)
            except (OSError, urllib.error.URLError) as error:
                refused = isinstance(getattr(error, "reason", error), ConnectionRefusedError)
                if attempt == 3 or not (webdriver._is_transient_disconnect(error) or refused):
                    raise
                if path != "/status":
                    log(f"Resending {method} {path.rsplit('/', 1)[-1]} after a dropped driver connection: {error}")
                time.sleep(0.1 * (attempt + 1))
        raise AssertionError("unreachable")

    request.__ostudy_retry__ = True  # type: ignore[attr-defined]
    webdriver.request = request


def install_capture_style(ctx: Context) -> None:
    installed = execute(
        ctx,
        """
        if (document.__ostudyCaptureSheet) return 'present';
        try {
          const sheet = new CSSStyleSheet();
          sheet.replaceSync(arguments[0]);
          document.adoptedStyleSheets = [...document.adoptedStyleSheets, sheet];
          document.__ostudyCaptureSheet = sheet;
          return 'installed';
        } catch (error) {
          return 'error: ' + error;
        }
        """,
        [CAPTURE_CSS],
    )
    if not isinstance(installed, str) or installed.startswith("error"):
        raise CaptureError(f"Could not install the capture stylesheet: {installed!r}")


def viewport_size(ctx: Context) -> tuple[int, int]:
    size = execute(ctx, "return [window.innerWidth, window.innerHeight];")
    return int(size[0]), int(size[1])


def set_viewport(ctx: Context, width: int) -> None:
    """Resize the window until the CSS viewport is exactly the requested size."""
    _label, height, layout = VIEWPORTS[width]
    outer_width, outer_height = width, height
    for _ in range(10):
        webdriver.request(
            "POST", f"/session/{ctx.session_id}/window/rect", {"width": outer_width, "height": outer_height}
        )
        time.sleep(0.05)
        inner_width, inner_height = viewport_size(ctx)
        if (inner_width, inner_height) == (width, height):
            ui.wait_for_layout_mode(ctx.session_id, layout, ctx.deadline(10))
            ctx.width, ctx.layout = width, layout
            return
        outer_width += width - inner_width
        outer_height += height - inner_height
    raise CaptureError(f"Could not reach a {width}x{height} viewport.")


def find(ctx: Context, xpath: str, seconds: int = 30) -> str:
    return webdriver.find_element(ctx.session_id, "xpath", xpath, ctx.deadline(seconds))


def exists(ctx: Context, xpath: str) -> bool:
    result = webdriver.request(
        "POST", f"/session/{ctx.session_id}/elements", {"using": "xpath", "value": xpath}
    )
    return isinstance(result, list) and len(result) > 0


def click(ctx: Context, xpath: str, seconds: int = 30) -> None:
    element = find(ctx, xpath, seconds)
    try:
        webdriver.click(ctx.session_id, element)
    except webdriver.WebDriverProtocolError as error:
        # The Workbench re-renders its tree as it loads; find the target again.
        if error.error != "stale element reference":
            raise
        webdriver.click(ctx.session_id, find(ctx, xpath, seconds))
    except RuntimeError as error:
        # The shared helper centres the target before each hit test. Some
        # targets land on half pixels and move by one pixel on every attempt,
        # so they never look stable; a plain WebDriver click still works.
        if "stable target" not in str(error):
            raise
        click_direct(ctx, element)


def click_direct(ctx: Context, element: str) -> None:
    execute(ctx, "arguments[0].scrollIntoView({ block: 'nearest', inline: 'nearest' }); return true;",
            [{webdriver.ELEMENT_KEY: element}])
    webdriver.request("POST", f"/session/{ctx.session_id}/element/{element}/click", {})


def wait_until(ctx: Context, description: str, predicate: Callable[[], bool], seconds: int = 30) -> None:
    deadline = ctx.deadline(seconds)
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.1)
    raise CaptureError(f"Timed out waiting for {description}.")


def wait_for_text(ctx: Context, text: str, scope: str = "//main", seconds: int = 30) -> None:
    find(ctx, f"{scope}//*[normalize-space(text())={xpath_literal(text)} or normalize-space(.)={xpath_literal(text)}]", seconds)


def settle_dialogs(ctx: Context) -> None:
    """Close a leave-without-saving confirmation if navigation raised one."""
    for label in ("Discard changes", "Discard", "Leave", "Leave without saving", "End session"):
        xpath = f"//*[@role='alertdialog']//button[normalize-space(.)={xpath_literal(label)}]"
        if exists(ctx, xpath):
            click(ctx, xpath, 5)
            return


def close_overlays(ctx: Context) -> None:
    """Dismiss any dialog, sheet, or menu a screen left open."""
    for _ in range(4):
        snapshot = ui.overlay_snapshot(ctx.session_id)
        if not snapshot.get("overlayActive") and not int(snapshot.get("dialogCount", 0)) and not exists(
            ctx, "//*[@role='menu' or @role='listbox' or @role='dialog' or @role='alertdialog']"
        ):
            return
        ui.send_key(ctx.session_id, ui.ESCAPE_KEY)
        time.sleep(0.15)
    ui.wait_for_overlay_inactive(ctx.session_id, ctx.deadline(5), "the next screen")


SHELL_MARKER = "//button[@data-testid='nav-library' or @data-testid='nav-decks']"
LIBRARY_VIEWS = ("overview", "lessons", "decks", "documents", "study-guides")
DESTINATION_OF = {"home": "home", "tutor": "tutor", "analytics": "progress"}


def wait_for_shell(ctx: Context) -> None:
    """Wait for the main navigation and note which navigation this build has."""
    find(ctx, SHELL_MARKER, 60)
    ctx.shell = DESTINATIONS_SHELL if exists(ctx, "//button[@data-testid='nav-library']") else SECTIONS_SHELL


def show_workbench_tree(ctx: Context) -> None:
    """Phones show one Workbench page at a time and Library reopens the last one;
    step back to the tree, whose More section links Documents and Study guides."""
    if ctx.layout != "compact":
        return
    workbench = "//main//*[@data-testid='library-workbench']"
    for _ in range(3):
        if not exists(ctx, workbench) or exists(ctx, f"{workbench}//nav[@aria-label='Library']"):
            return
        close_editor = (
            f"{workbench}//*[@data-testid='workbench-editor']"
            "//button[normalize-space(.)='Cancel' or @aria-label='Close editor' or normalize-space(.)='Close']"
        )
        back = f"{workbench}//button[@aria-label='Library']"
        click(ctx, close_editor if exists(ctx, close_editor) else back)
    find(ctx, f"{workbench}//nav[@aria-label='Library']")


def choose_everything(ctx: Context) -> None:
    """Library keeps the subject chosen last; its screens are photographed on Everything."""
    name = "//main//*[@data-testid='library-subject-name']"
    if not exists(ctx, name) or exists(ctx, f"{name}[normalize-space(.)='Everything']"):
        return
    switcher = "//main//button[@data-testid='library-switch-subject']"
    if exists(ctx, switcher):
        click(ctx, switcher)
        click(ctx, "//*[@role='dialog']//button[@data-testid='library-subject-all']")
    else:
        click(ctx, "//main//aside//button[@data-testid='library-subject-all']")
    find(ctx, f"{name}[normalize-space(.)='Everything']")
    settle_dialogs(ctx)


def go(ctx: Context, section: str) -> None:
    """Navigate to a top-level section through the visible navigation.

    ``section`` is an application section id (decks, documents, study-guides,
    review, analytics, ...), which both navigations can reach.
    """
    if ctx.shell == DESTINATIONS_SHELL:
        if section == "settings":
            click(ctx, "//button[@data-testid='app-settings']")
        elif section in LIBRARY_VIEWS:
            click(ctx, "//button[@data-testid='nav-library']")
            settle_dialogs(ctx)
            # Library opens its last page; every page carries Library's tabs.
            find(ctx, f"{WORKBENCH} | //main//nav[@aria-label='Library pages']")
            show_workbench_tree(ctx)
            choose_everything(ctx)
            show_workbench_tree(ctx)
            if section == "decks" and exists(ctx, WORKBENCH):
                # The Workbench is Library's first page.
                pass
            else:
                # Choosing the open view again returns it to its first page.
                click(ctx, f"//main//nav[@aria-label='Library pages']//button[@data-testid='library-view-{section}']")
        else:
            click(ctx, f"//button[@data-testid='nav-{DESTINATION_OF[section]}']")
    elif section == "settings" and ctx.layout == "compact":
        click(ctx, "//button[@data-testid='app-settings']")
    else:
        click(ctx, f"//button[@data-testid='nav-{section}']")
    settle_dialogs(ctx)


# --------------------------------------------------------------------------
# Capture readiness and the screenshot itself
# --------------------------------------------------------------------------

READY_SCRIPT = """
const visible = (element) => {
  const rect = element.getBoundingClientRect();
  const style = getComputedStyle(element);
  return rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none';
};
const inView = (element) => {
  const rect = element.getBoundingClientRect();
  return rect.width > 0 && rect.height > 0 && rect.bottom > 0 && rect.right > 0
    && rect.top < window.innerHeight && rect.left < window.innerWidth;
};
const busy = [...document.querySelectorAll("[aria-busy='true']")].filter(visible).length;
const loading = [...document.querySelectorAll("[role='status']")]
  .filter((element) => visible(element) && /loading|opening|analyzing|preparing/i.test(element.textContent || '')).length;
return {
  theme: document.documentElement.dataset.theme || '',
  layout: document.documentElement.dataset.layoutMode || '',
  fonts: document.fonts.status,
  busy,
  loading,
  animations: document.getAnimations().filter((animation) => animation.playState === 'running').length,
  images: [...document.images].filter((image) => inView(image) && !image.complete).length,
  toasts: document.querySelectorAll("button[aria-label='Dismiss notification']").length,
};
"""

RESET_SCROLL_SCRIPT = """
window.scrollTo(0, 0);
for (const element of document.querySelectorAll('*')) {
  if (element.scrollTop) element.scrollTop = 0;
  if (element.scrollLeft) element.scrollLeft = 0;
}
// A narrow tab strip scrolls sideways; keep the selected tab in view as the app does.
for (const tab of document.querySelectorAll("[role='tab'][aria-selected='true']")) {
  tab.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'auto' });
}
if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
return true;
"""


# Large images decode asynchronously; wait until the visible ones can paint.
DECODE_IMAGES_SCRIPT = """
const done = arguments[arguments.length - 1];
const images = [...document.images].filter((image) => {
  const rect = image.getBoundingClientRect();
  return (image.currentSrc || image.src) && rect.width > 0 && rect.height > 0 && rect.bottom > 0
    && rect.right > 0 && rect.top < window.innerHeight && rect.left < window.innerWidth;
});
const timeout = new Promise((resolve) => setTimeout(() => resolve('timeout'), 5000));
Promise.race([Promise.all(images.map((image) => image.decode().catch(() => null))), timeout])
  .then((result) => done(result === 'timeout' ? 'timeout' : images.length));
"""


PARK_POINT_SCRIPT = """
const width = window.innerWidth;
const height = window.innerHeight;
const interactive = [
  'a', 'button', 'input', 'select', 'textarea', 'label', 'summary', "[contenteditable='true']",
  "[tabindex]:not([tabindex='-1'])",
  ...['button', 'link', 'menuitem', 'tab', 'checkbox', 'radio', 'switch', 'option', 'row', 'gridcell', 'slider']
    .map((role) => `[role='${role}']`),
].join(', ');
const candidates = [
  [Math.floor(width / 2), 1], [1, Math.floor(height / 2)], [width - 2, Math.floor(height / 2)],
  [1, 1], [Math.floor(width / 2), height - 2],
];
for (const [x, y] of candidates) {
  const element = document.elementFromPoint(x, y);
  if (!element || !element.closest(interactive)) return [x, y];
}
return null;
"""


def park_pointer(ctx: Context) -> None:
    """Move the mouse onto an inert spot so no control keeps a hover style."""
    point = execute(ctx, PARK_POINT_SCRIPT)
    if not isinstance(point, list) or len(point) != 2:
        return
    webdriver.request(
        "POST",
        f"/session/{ctx.session_id}/actions",
        {"actions": [{
            "type": "pointer",
            "id": "capture-mouse",
            "parameters": {"pointerType": "mouse"},
            "actions": [{"type": "pointerMove", "duration": 0, "origin": "viewport", "x": int(point[0]), "y": int(point[1])}],
        }]},
    )
    webdriver.request("DELETE", f"/session/{ctx.session_id}/actions")


def dismiss_toasts(ctx: Context) -> None:
    for _ in range(5):
        result = webdriver.request(
            "POST",
            f"/session/{ctx.session_id}/elements",
            {"using": "css selector", "value": "button[aria-label='Dismiss notification']"},
        )
        if not isinstance(result, list) or not result:
            return
        webdriver.click(ctx.session_id, str(result[0][webdriver.ELEMENT_KEY]))
        time.sleep(0.1)


REVIEW_BADGE_SCRIPT = """
const done = arguments[arguments.length - 1];
// Today's count is on Home; builds before 2026-10-20 put it on Review.
const review = document.querySelector("[data-testid='nav-review']") || document.querySelector("[data-testid='nav-home']");
if (!review) { done(true); return; }
window.__TAURI_INTERNALS__.invoke('get_home_summary').then(
  (summary) => {
    const due = summary.today.scheduledReviews + summary.today.scheduledNew;
    const expected = due > 999 ? '999+' : due > 0 ? String(due) : '';
    const badge = review.querySelector("[data-slot='nav-badge']");
    done((badge?.textContent || '').trim() === expected);
  },
  () => done(false),
);
"""


def wait_for_review_badge(ctx: Context, seconds: int = 20) -> None:
    """Wait until today's badge shows what is due now, as a finished session rereads it."""
    deadline = ctx.deadline(seconds)
    while time.monotonic() < deadline:
        if ui.execute_async(ctx.session_id, REVIEW_BADGE_SCRIPT, []) is True:
            return
        time.sleep(0.1)
    raise CaptureError("Today's badge did not settle on today's count.")


def wait_ready(ctx: Context, seconds: int = 20) -> None:
    deadline = ctx.deadline(seconds)
    last: Any = None
    stable = 0
    while time.monotonic() < deadline:
        last = execute(ctx, READY_SCRIPT)
        if isinstance(last, dict) and last.get("toasts"):
            dismiss_toasts(ctx)
            park_pointer(ctx)
            stable = 0
            continue
        ready = (
            isinstance(last, dict)
            and last.get("theme") == ctx.theme
            and last.get("layout") == ctx.layout
            and last.get("fonts") == "loaded"
            and int(last.get("busy", 1)) == 0
            and int(last.get("loading", 1)) == 0
            and int(last.get("animations", 1)) == 0
            and int(last.get("images", 1)) == 0
        )
        stable = stable + 1 if ready else 0
        if stable >= 2:
            return
        time.sleep(0.1)
    raise CaptureError(f"The screen did not settle: {last!r}")


def take_png(ctx: Context) -> bytes:
    encoded = webdriver.request("GET", f"/session/{ctx.session_id}/screenshot")
    if not isinstance(encoded, str):
        raise CaptureError("The screenshot endpoint returned no image.")
    image = base64.b64decode(encoded, validate=True)
    if not image.startswith(PNG_SIGNATURE):
        raise CaptureError("The screenshot endpoint did not return a PNG.")
    return image


def stable_png(ctx: Context, attempts: int = 12) -> tuple[bytes, bool]:
    """Return a screenshot once two consecutive frames are byte-identical."""
    previous = take_png(ctx)
    for _ in range(attempts):
        time.sleep(0.15)
        current = take_png(ctx)
        if current == previous:
            return current, True
        previous = current
    return previous, False


# --------------------------------------------------------------------------
# Screens
# --------------------------------------------------------------------------


def open_home(ctx: Context) -> None:
    go(ctx, "home")
    find(ctx, "//main//*[@data-testid='home-today']")
    expected = seed.home_tile_count()
    wait_until(
        ctx,
        f"{expected} subject tiles on Home",
        lambda: len(webdriver.request(
            "POST", f"/session/{ctx.session_id}/elements",
            {"using": "css selector", "value": "main [data-testid='subject-tile']"},
        )) == expected,
    )


def open_subject(ctx: Context) -> None:
    open_home(ctx)
    name = seed.SUBJECT_PAGE
    click(
        ctx,
        f"//main//*[@data-testid='subject-tile'][.//*[normalize-space(.)={xpath_literal(name)}]]//button",
    )
    # A subject opens on its Library Overview.
    find(ctx, f"//main//*[@data-testid='library-subject-name'][normalize-space(.)={xpath_literal(name)}]")
    find(ctx, "//main//*[@data-testid='library-overview']//*[@data-testid='library-overview-facts']")


def open_first_run(ctx: Context) -> None:
    """A library with nothing in it opens on Home's welcome, or on the empty deck library before Home existed."""
    if ctx.shell == DESTINATIONS_SHELL:
        go(ctx, "home")
        find(ctx, "//main//*[@data-testid='home-first-run']//h2[normalize-space(.)='Your library is empty']")
    else:
        go(ctx, "decks")
        wait_for_text(ctx, "No decks have been created yet.")


WORKBENCH = "//main//*[@data-testid='library-workbench']"
WORKBENCH_TREE = "//main//*[@data-testid='library-workbench']//nav[@aria-label='Library']"


def open_library(ctx: Context) -> None:
    go(ctx, "decks")
    if exists(ctx, WORKBENCH):
        find(ctx, WORKBENCH_TREE)
        for deck in seed.DECKS:
            find(ctx, f"{WORKBENCH_TREE}//button[.//span[normalize-space(.)={xpath_literal(deck.name)}]]")
        if ctx.layout != "compact":
            find(ctx, "//main//*[@data-testid='library-workbench']//*[@role='list'][@aria-label='Cards']//*[@data-card-id]")
        return
    find(ctx, "//main//*[@role='tablist'][@aria-label='Flashcards workspaces']")
    for deck in seed.DECKS:
        find(ctx, f"//main//strong[normalize-space(.)={xpath_literal(deck.name)}]")


def open_deck(ctx: Context, deck_key: str) -> None:
    open_library(ctx)
    name = seed.deck_name(deck_key)
    expected = sum(1 for note in seed.NOTES if note.deck == deck_key)
    if exists(ctx, WORKBENCH):
        click(ctx, f"{WORKBENCH_TREE}//button[.//span[normalize-space(.)={xpath_literal(name)}]]")
        find(ctx, f"//main//h1[@data-testid='workbench-title'][normalize-space(.)={xpath_literal(name)}]")
        wait_until(
            ctx,
            f"{expected} card rows in {name}",
            lambda: len(webdriver.request(
                "POST", f"/session/{ctx.session_id}/elements",
                {"using": "css selector", "value": "main [data-testid='library-workbench'] [data-card-id]"},
            )) >= min(expected, 6),
        )
        return
    click(ctx, f"//main//*[@role='button' and @aria-label={xpath_literal('Open ' + name)}]")
    webdriver.wait_for_main_heading(ctx.session_id, name, ctx.deadline())
    wait_until(
        ctx,
        f"{expected} card rows in {name}",
        lambda: len(webdriver.request(
            "POST", f"/session/{ctx.session_id}/elements",
            {"using": "css selector", "value": "main article[role='row']"},
        )) >= min(expected, 6),
    )


def open_card_editor(ctx: Context) -> None:
    open_deck(ctx, seed.DETAIL_DECK)
    question = seed.question_text(seed.note_spec(seed.EDITOR_NOTE))
    if exists(ctx, WORKBENCH):
        # An editor left open by an earlier capture covers the list; close it first.
        close_editor = "//main//*[@data-testid='workbench-editor']//button[@aria-label='Close editor']"
        if exists(ctx, close_editor):
            click(ctx, close_editor)
        click(ctx, f"//main//*[@data-card-id]//button[contains(normalize-space(.), {xpath_literal(question[:60])})]")
        find(ctx, "//main//*[@data-testid='workbench-editor']//h1[normalize-space(.)='Edit card']")
        find(ctx, "//main//*[@contenteditable='true'][contains(normalize-space(.), 'Heart rate')]")
        return
    click(ctx, f"//main//article[@role='row']//button[.//strong[normalize-space(.)={xpath_literal(question)}]]")
    find(ctx, "//main//h1[normalize-space(.)='Edit card']")
    find(ctx, "//main//*[@contenteditable='true'][contains(normalize-space(.), 'Heart rate')]")


SCROLL_INTO_VIEW_SCRIPT = """
const element = arguments[0];
const align = () => element.scrollIntoView({ block: 'start', inline: 'nearest', behavior: 'auto' });
align();
// Leave room for an app header that stays on top while the content scrolls.
const headers = [...document.querySelectorAll('header')].filter((node) => {
  const position = getComputedStyle(node).position;
  const rect = node.getBoundingClientRect();
  return (position === 'fixed' || position === 'sticky') && rect.height > 0 && rect.top <= 0
    && !node.contains(element) && !element.contains(node);
});
const clearance = Math.max(0, ...headers.map((node) => node.getBoundingClientRect().bottom)) + 16;
const top = element.getBoundingClientRect().top;
if (top < clearance) {
  element.style.scrollMarginTop = `${Math.round(clearance - top)}px`;
  align();
}
return true;
"""


def scroll_into_view(ctx: Context, xpath: str) -> None:
    """Scroll an element to the top of its scroll area, below any fixed app header."""
    execute(ctx, SCROLL_INTO_VIEW_SCRIPT, [{webdriver.ELEMENT_KEY: find(ctx, xpath, 10)}])


def scroll_card_editor(ctx: Context) -> None:
    """On a phone the filled fields start below the fold; bring them into view."""
    if ctx.layout == "compact":
        scroll_into_view(ctx, "(//main//h3[normalize-space(.)='Question'])[1]/ancestor::section[1]")


def open_guides(ctx: Context) -> None:
    go(ctx, "study-guides")
    for document in seed.DOCUMENTS:
        if document.topics:
            find(ctx, f"//main//li[@data-testid='guide-card']//h3[normalize-space(.)={xpath_literal(document.title)}]")


def open_guide_reader(ctx: Context) -> None:
    open_guides(ctx)
    document = seed.DOCUMENTS[0]
    click(ctx, f"//main//li[@data-testid='guide-card'][.//h3[normalize-space(.)={xpath_literal(document.title)}]]//button[normalize-space(.)='Open']")
    wait_for_text(ctx, document.topics[0].title, seconds=30)


def open_documents(ctx: Context) -> None:
    """Everything's Documents groups the documents under their subjects, then No subject."""
    go(ctx, "documents")
    for document in seed.DOCUMENTS:
        group = document.subject or "No subject"
        find(
            ctx,
            f"//main//section[@data-testid='document-group'][.//h2[normalize-space(text())={xpath_literal(group)}]]"
            f"//li[@data-testid='document-row']//h3[normalize-space(.)={xpath_literal(document.title)}]",
        )


def open_document_info(ctx: Context) -> None:
    open_documents(ctx)
    # Details is an action of a document in no subject.
    document = seed.INFO_DOCUMENT
    click(ctx, f"//main//button[@aria-label={xpath_literal('More actions for ' + document.title)}]")
    # Wide layouts open a dropdown menu; compact layouts open a sheet of buttons.
    click(ctx, "(//*[@role='menuitem'] | //*[@role='dialog']//button)[normalize-space(.)='Details']")
    find(ctx, f"//*[@role='dialog']//*[normalize-space(.)={xpath_literal(document.title)}]")


def open_analytics(ctx: Context) -> None:
    go(ctx, "analytics")
    find(ctx, "//main//section[@aria-label='Learning summary']")
    find(ctx, "//main//section[@aria-labelledby='progress-heading'][not(@aria-busy='true')]//*[@role='img']")


def open_analytics_details(ctx: Context) -> None:
    open_analytics(ctx)
    find(ctx, "//main//h2[normalize-space(.)='Needs attention']")
    find(ctx, "//main//h2[normalize-space(.)='Every answer']")


def scroll_analytics_summary(ctx: Context) -> None:
    scroll_into_view(ctx, "//main//section[@aria-label='Learning summary']")


def scroll_analytics_details(ctx: Context) -> None:
    scroll_into_view(ctx, "(//main//h2[normalize-space(.)='Needs attention'])[1]/ancestor::section[1]")


def open_tutor(ctx: Context) -> None:
    go(ctx, "tutor")
    title = seed.TUTOR_CONVERSATION_TITLE
    conversation = f"//nav[@aria-label='Conversations']//button[normalize-space(.)={xpath_literal(title)}]"
    toggle = "//main//button[@aria-label='Conversations']"
    wait_until(ctx, "the Tutor conversation list", lambda: exists(ctx, conversation) or exists(ctx, toggle))
    if not exists(ctx, conversation):
        click(ctx, toggle)
    click(ctx, conversation)
    wait_until(
        ctx,
        "both tutor responses",
        lambda: len(webdriver.request(
            "POST", f"/session/{ctx.session_id}/elements",
            {"using": "css selector", "value": "main section[aria-label='Tutor response']"},
        )) >= len(seed.TUTOR_TURNS),
    )
    ui.wait_for_overlay_inactive(ctx.session_id, ctx.deadline(), "tutor conversation capture")


def open_settings_tab(ctx: Context, tab: str) -> None:
    go(ctx, "settings")
    # Wider screens list the sections as tabs in a pane; a phone opens on a list of section buttons
    # and then shows the chosen section under its own heading, with Back to Settings beside it.
    back = "//main//button[@aria-label='Back to Settings']"
    find(ctx, f"//main//h1[normalize-space(.)='Settings'] | {back}")
    if exists(ctx, back):
        click(ctx, back)
    webdriver.wait_for_element_text(ctx.session_id, "css selector", "main h1", "Settings", ctx.deadline())
    name = xpath_literal(tab)
    click(ctx, f"//main//*[@role='tab' or self::button][normalize-space(.)={name}]")
    find(ctx, f"//main//*[@role='tab'][normalize-space(.)={name}][@aria-selected='true'] | //main//h1[normalize-space(.)={name}]")
    if tab == "Card templates":
        find(ctx, f"//main//*[normalize-space(.)={xpath_literal(seed.TEMPLATE_NAME)}]")


MASKS: dict[str, str] = {
    # The sync identity is random per library; replace it with a fixed placeholder.
    "settings-backup": """
        for (const label of document.querySelectorAll('main span')) {
          if (label.textContent.trim() !== 'Identity fingerprint') continue;
          const code = label.parentElement && label.parentElement.querySelector('code');
          if (code) code.textContent = code.textContent.replace(/[0-9A-Za-z]/g, '0');
        }
        return true;
    """,
}

OPENERS: dict[str, Callable[[Context], None]] = {
    "home": open_home,
    "subject": open_subject,
    "library": open_library,
    "deck": lambda ctx: open_deck(ctx, seed.DETAIL_DECK),
    "card-editor": open_card_editor,
    "guides": open_guides,
    "guide-reader": open_guide_reader,
    "documents": open_documents,
    "document-info": open_document_info,
    "analytics": open_analytics,
    "analytics-summary": open_analytics,
    "analytics-details": open_analytics_details,
    "tutor": open_tutor,
    **{screen_id: functools.partial(open_settings_tab, tab=tab) for screen_id, tab in SETTINGS_TABS.items()},
}
PRESERVE_SCROLL = {"tutor"}
AFTER_RESET: dict[str, Callable[[Context], None]] = {
    "card-editor": scroll_card_editor,
    "analytics-summary": scroll_analytics_summary,
    "analytics-details": scroll_analytics_details,
}


# --------------------------------------------------------------------------
# Capture run
# --------------------------------------------------------------------------


class Encoder:
    """Convert PNG screenshots to lossy WebP in the background."""

    def __init__(self, quality: int = WEBP_QUALITY, workers: int = 3) -> None:
        self.quality = quality
        self.pool = ThreadPoolExecutor(max_workers=workers)
        cwebp = shutil.which("cwebp")
        magick = shutil.which("magick") or shutil.which("convert")
        if cwebp:
            self.command = lambda source, target: [cwebp, "-quiet", "-q", str(quality), "-m", "4", str(source), "-o", str(target)]
            self.name = "cwebp"
        elif magick:
            self.command = lambda source, target: [
                magick, str(source), "-quality", str(quality), "-define", "webp:method=4", str(target)
            ]
            self.name = Path(magick).name
        else:
            raise CaptureError("Install libwebp (cwebp) or ImageMagick with WebP support to write WebP files.")

    def submit(self, png: bytes, target: Path) -> Future[None]:
        return self.pool.submit(self._encode, png, target)

    def _encode(self, png: bytes, target: Path) -> None:
        with tempfile.NamedTemporaryFile(suffix=".png", dir=target.parent, delete=False) as handle:
            handle.write(png)
            source = Path(handle.name)
        try:
            subprocess.run(self.command(source, target), check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        finally:
            source.unlink(missing_ok=True)
        with target.open("rb") as handle:
            header = handle.read(12)
        if header[:4] != b"RIFF" or header[8:12] != b"WEBP":
            raise CaptureError(f"{self.name} did not write a WebP file: {target}")

    def close(self) -> None:
        self.pool.shutdown(wait=True)


@dataclass
class RunOptions:
    application: Path
    output: Path
    app_data_root: Path
    screens: tuple[str, ...]
    themes: tuple[str, ...]
    widths: tuple[int, ...]
    source: dict[str, Any]
    driver_port: int


class Run:
    def __init__(self, options: RunOptions) -> None:
        self.options = options
        self.output = options.output
        self.encoder = Encoder()
        self.pending: list[tuple[dict[str, Any], Future[None], Path]] = []
        now = datetime.datetime.now(datetime.timezone.utc)
        self.anchor_day_ms = int(datetime.datetime(now.year, now.month, now.day, tzinfo=datetime.timezone.utc).timestamp() * 1000)
        selected = [screen for screen in SCREENS if screen.id in options.screens]
        self.manifest = report.new_manifest(
            source=options.source,
            screens=[{"id": screen.id, "title": screen.title, "group": screen.group} for screen in selected],
            themes=options.themes,
            viewports=[
                {"width": width, "height": VIEWPORTS[width][1], "label": VIEWPORTS[width][0], "layout": VIEWPORTS[width][2]}
                for width in options.widths
            ],
            captured_at=now.replace(microsecond=0).isoformat().replace("+00:00", "Z"),
            capture_date=now.strftime("%Y-%m-%d"),
            environment={
                # The frontend is always the one embedded in the binary.
                "frontend": "embedded",
                "application": str(options.application),
                "applicationSha256": report.sha256_file(options.application),
                "webpEncoder": self.encoder.name,
                "webpQuality": WEBP_QUALITY,
                "timeZone": os.environ.get("TZ", ""),
                "locale": os.environ.get("LC_ALL") or os.environ.get("LANG", ""),
                "seed": seed.SEED_VERSION,
            },
        )
        self.manifest["notes"].append(
            "The app reads the real clock, so seeded dates are relative to the capture day; "
            "captures from different days differ in date text."
        )
        self.selected = selected
        self.driver: subprocess.Popen[str] | None = None
        self.driver_log: Path | None = None

    # -- application data ---------------------------------------------------

    @property
    def app_data_dir(self) -> Path:
        return self.options.app_data_root / APP_IDENTIFIER

    @property
    def snapshot_dir(self) -> Path:
        return self.options.app_data_root.parent / "seeded-library"

    @property
    def empty_snapshot_dir(self) -> Path:
        return self.options.app_data_root.parent / "empty-library"

    def reset_app_data(self) -> None:
        # A WebKit helper of the previous session can still be flushing its
        # storage for a moment after the application itself has exited.
        for attempt in range(20):
            if not self.app_data_dir.exists():
                return
            try:
                shutil.rmtree(self.app_data_dir)
                return
            except OSError:
                if attempt == 19:
                    raise
                time.sleep(0.25)

    def restore_snapshot(self, theme: str, snapshot: Path | None = None) -> None:
        self.reset_app_data()
        shutil.copytree(snapshot or self.snapshot_dir, self.app_data_dir, symlinks=True)
        seed.set_theme(self.app_data_dir, theme)

    def save_snapshot(self, snapshot: Path) -> None:
        if snapshot.exists():
            shutil.rmtree(snapshot)
        shutil.copytree(self.app_data_dir, snapshot, symlinks=True)

    def adopt_shell(self, shell: str) -> None:
        """Keep only the screens this build's navigation has, and say which were skipped."""
        self.manifest["environment"]["navigation"] = shell
        self.selected, missing = screens_for_shell(self.selected, shell)
        if not missing:
            return
        skipped = {screen.id for screen in missing}
        self.manifest["screens"] = [entry for entry in self.manifest["screens"] if entry["id"] not in skipped]
        self.manifest["notes"].append(
            "This build has the sidebar navigation from before Home and Review, so these screens were not "
            f"captured: {', '.join(screen.title for screen in missing)}."
        )

    # -- driver -------------------------------------------------------------

    def start_driver(self) -> None:
        port = self.options.driver_port
        for candidate in (port, port + 1):
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
                if probe.connect_ex(("127.0.0.1", candidate)) == 0:
                    raise CaptureError(f"Port {candidate} is already in use; set OSTUDY_UI_SCREENSHOTS_PORT to a free pair.")
        webdriver.WEBDRIVER_URL = f"http://127.0.0.1:{port}"
        self.driver_log = self.options.app_data_root.parent / "tauri-driver.log"
        with self.driver_log.open("w", encoding="utf-8") as handle:
            self.driver = subprocess.Popen(
                ["tauri-driver", "--port", str(port), "--native-port", str(port + 1)],
                stdout=handle,
                stderr=subprocess.STDOUT,
                text=True,
                start_new_session=True,
            )
        webdriver.wait_for_driver(webdriver.deadline_after(20))

    def stop_driver(self) -> None:
        if self.driver is not None and self.driver.poll() is None:
            try:
                os.killpg(self.driver.pid, signal.SIGTERM)
                self.driver.wait(timeout=10)
            except (ProcessLookupError, subprocess.TimeoutExpired):
                try:
                    os.killpg(self.driver.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass

    def driver_log_tail(self, lines: int = 40) -> str:
        if self.driver_log is None or not self.driver_log.is_file():
            return ""
        text = self.driver_log.read_text(encoding="utf-8", errors="replace").splitlines()
        return "\n".join(text[-lines:])

    def open_session(self, theme: str) -> Context:
        session_id = webdriver.create_session(self.options.application)
        ctx = Context(session_id=session_id, theme=theme)
        wait_for_shell(ctx)
        install_capture_style(ctx)
        return ctx

    def close_session(self, ctx: Context) -> None:
        try:
            webdriver.delete_session(ctx.session_id)
        except Exception as error:  # noqa: BLE001 - shutdown is best effort
            log(f"Could not close the WebDriver session cleanly: {error}")
        self.wait_for_app_exit()

    def application_pids(self) -> list[int]:
        """The captured executable and the WebKit helpers it started (not merely naming them)."""
        target = os.path.realpath(self.options.application)
        marker = ("XDG_DATA_HOME=" + os.environ.get("XDG_DATA_HOME", "")).encode()
        pids: list[int] = []
        for entry in Path("/proc").iterdir():
            if not entry.name.isdigit():
                continue
            try:
                executable = os.readlink(entry / "exe").removesuffix(" (deleted)")
            except OSError:
                continue
            if executable == target:
                pids.append(int(entry.name))
                continue
            name = os.path.basename(executable)
            if not (name.startswith("WebKit") and name.endswith("Process")):
                continue
            try:
                environment = (entry / "environ").read_bytes().split(b"\0")
            except OSError:
                continue
            if marker in environment:
                pids.append(int(entry.name))
        return pids

    def wait_for_app_exit(self, seconds: int = 20) -> None:
        """The next session must not start while this one still owns the library."""
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if not self.application_pids():
                return
            time.sleep(0.1)
        for pid in self.application_pids():
            log(f"Application process {pid} was still running {seconds}s after its session closed; stopping it.")
            try:
                os.kill(pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
        stop = time.monotonic() + 10
        while self.application_pids() and time.monotonic() < stop:
            time.sleep(0.1)

    # -- seeding ------------------------------------------------------------

    def wants_first_run(self) -> bool:
        return any(screen.flow == "first-run" for screen in self.selected)

    def seed_library(self) -> None:
        started = time.monotonic()
        self.reset_app_data()
        ctx = self.open_session("dark")
        self.adopt_shell(ctx.shell)
        if self.wants_first_run():
            # The app has created an empty library; keep a copy for the first-run screen.
            self.close_session(ctx)
            self.save_snapshot(self.empty_snapshot_dir)
            seed.write_settings(self.empty_snapshot_dir, CAPTURE_SETTINGS, self.anchor_day_ms)
            ctx = self.open_session("dark")
        try:
            self.manifest["environment"]["browser"] = execute(
                ctx,
                """
                const options = Intl.DateTimeFormat().resolvedOptions();
                return { language: navigator.language, locale: options.locale, timeZone: options.timeZone,
                  devicePixelRatio: window.devicePixelRatio, userAgent: navigator.userAgent };
                """,
            )
            state = seed.seed_through_ipc(
                lambda steps: ipc_batch(ctx.session_id, steps),
                lambda plans: ipc_practice(ctx.session_id, plans),
            )
        finally:
            self.close_session(ctx)
        notes = seed.finalize_library(self.app_data_dir, state, self.anchor_day_ms, CAPTURE_SETTINGS)
        self.save_snapshot(self.snapshot_dir)
        card_count = sum(len(ids) for ids in state.card_ids.values())
        unfiled = sum(1 for deck in seed.DECKS if deck.subject is None)
        self.manifest["notes"].append(
            f"Seeded {len(seed.SUBJECTS)} subjects and {len(seed.DECKS)} decks ({unfiled} with no subject) "
            f"with {len(seed.NOTES)} notes ({card_count} cards), {len(state.sessions)} practice sessions "
            f"over six weeks, {len(seed.DOCUMENTS)} documents, and one tutor conversation."
        )
        self.manifest["notes"].extend(notes)
        self.manifest["timings"]["seedSeconds"] = round(time.monotonic() - started, 1)
        log(f"Seeded the library in {self.manifest['timings']['seedSeconds']}s")

    # -- capture ------------------------------------------------------------

    def record(self, ctx: Context, screen: Screen, started: float, note: str | None = None) -> None:
        png, stable = stable_png(ctx)
        width, height = ctx.width, VIEWPORTS[ctx.width][1]
        filename = report.capture_filename(screen.id, ctx.theme, width)
        target = self.output / filename
        if not stable:
            note = ((note or "") + " The screen was still changing when it was captured.").strip()
        entry = report.add_capture(
            self.manifest,
            screen=screen.id,
            theme=ctx.theme,
            width=width,
            height=height,
            file=filename,
            pixel_sha256=report.sha256_bytes(png),
            seconds=time.monotonic() - started,
            note=note,
        )
        self.pending.append((entry, self.encoder.submit(png, target), target))

    def record_failure(self, ctx: Context, screen: Screen, width: int, error: BaseException, started: float) -> None:
        message = f"{type(error).__name__}: {error}"
        log(f"FAILED {screen.id} {ctx.theme} {width}: {message}")
        filename = None
        try:
            png = take_png(ctx)
            filename = report.capture_filename(screen.id, ctx.theme, width)
            self.pending.append((
                report.add_capture(
                    self.manifest, screen=screen.id, theme=ctx.theme, width=width, height=VIEWPORTS[width][1],
                    file=filename, status="failed", pixel_sha256=report.sha256_bytes(png),
                    seconds=time.monotonic() - started, note=f"Not reached: {message}",
                ),
                self.encoder.submit(png, self.output / filename),
                self.output / filename,
            ))
        except Exception:  # noqa: BLE001 - the failure itself is what is reported
            report.add_capture(
                self.manifest, screen=screen.id, theme=ctx.theme, width=width, height=VIEWPORTS[width][1],
                file=None, status="failed", seconds=time.monotonic() - started, note=message,
            )
        self.manifest["warnings"].append(f"{screen.id} ({ctx.theme}, {width}px) was not captured: {message}")

    def prepare_capture(self, ctx: Context, screen: Screen) -> None:
        install_capture_style(ctx)
        if screen.id not in PRESERVE_SCROLL:
            execute(ctx, RESET_SCROLL_SCRIPT)
        after = AFTER_RESET.get(screen.id)
        if after is not None:
            after(ctx)
        mask = MASKS.get(screen.id)
        if mask:
            execute(ctx, mask)
        park_pointer(ctx)
        ui.execute_async(ctx.session_id, DECODE_IMAGES_SCRIPT, [])
        wait_ready(ctx)

    def capture_page(self, ctx: Context, screen: Screen, opener: Callable[[Context], None] | None = None) -> None:
        started = time.monotonic()
        try:
            (opener or OPENERS[screen.id])(ctx)
            self.prepare_capture(ctx, screen)
            self.record(ctx, screen, started)
        except Exception as error:  # noqa: BLE001 - one screen must not abort the run
            self.record_failure(ctx, screen, ctx.width, error, started)
        finally:
            self.recover(ctx)

    def recover(self, ctx: Context) -> None:
        try:
            close_overlays(ctx)
        except Exception as error:  # noqa: BLE001 - the next screen reports its own failure
            log(f"Could not close an overlay: {error}")

    def capture_dialog(self, ctx: Context, screen: Screen, widths: tuple[int, ...]) -> None:
        deck = seed.deck_name(seed.PRACTICE_DECK)
        checkbox = f"//*[@role='checkbox' and @aria-label={xpath_literal('Select ' + deck)}]"
        for width in widths:
            started = time.monotonic()
            set_viewport(ctx, width)
            try:
                open_library(ctx)
                click_direct(ctx, find(ctx, checkbox))
                find(ctx, checkbox + "[@aria-checked='true']", 10)
                click(ctx, "//*[@aria-label='Selected deck actions']//button[normalize-space(.)='Practice']")
                find(ctx, "//*[@role='dialog'][.//h2[normalize-space(.)='Practice selected decks']]")
                find(ctx, "//*[@role='dialog']//*[normalize-space(.)='Cards available to study']")
                self.prepare_capture(ctx, screen)
                self.record(ctx, screen, started)
            except Exception as error:  # noqa: BLE001
                self.record_failure(ctx, screen, width, error, started)
            finally:
                if exists(ctx, "//*[@role='dialog']//button[@aria-label='Close dialog']"):
                    click(ctx, "//*[@role='dialog']//button[@aria-label='Close dialog']", 5)
                    ui.wait_for_overlay_inactive(ctx.session_id, ctx.deadline(10), "practice setup close")
                if exists(ctx, checkbox + "[@aria-checked='true']"):
                    click_direct(ctx, find(ctx, checkbox, 5))

    def capture_practice(self, ctx: Context, widths: tuple[int, ...]) -> None:
        wanted = [screen for screen in self.selected if screen.flow == "practice"]
        if not wanted:
            return
        by_id = {screen.id: screen for screen in wanted}
        started = time.monotonic()
        stage = "practice-question"
        try:
            set_viewport(ctx, widths[-1])
            open_deck(ctx, seed.PRACTICE_DECK)
            click(ctx, "//main//button[normalize-space(.)='Practice']")
            find(ctx, "//button[normalize-space(.)='Show answer']")
            first_question = seed.note_spec(seed.PRACTICE_ORDER[0]).front
            wait_for_text(ctx, first_question, seconds=20)
            self._practice_shots(ctx, by_id.get("practice-question"), widths, started)
            stage = "practice-answer"
            started = time.monotonic()
            click(ctx, "//button[normalize-space(.)='Show answer']")
            webdriver.wait_for_enabled_element(ctx.session_id, "css selector", "button[aria-keyshortcuts='2']", ctx.deadline())
            self._practice_shots(ctx, by_id.get("practice-answer"), widths, started)
            stage = "practice-summary"
            started = time.monotonic()
            ratings = list(seed.PRACTICE_RATINGS)
            rated = 0
            while True:
                score = ratings[rated] if rated < len(ratings) else 2
                button = webdriver.wait_for_enabled_element(
                    ctx.session_id, "css selector", f"button[aria-keyshortcuts='{score}']", ctx.deadline())
                webdriver.click(ctx.session_id, button)
                rated += 1
                deadline = ctx.deadline(30)
                while time.monotonic() < deadline:
                    if exists(ctx, "//main//*[@data-testid='practice-complete']"):
                        break
                    if exists(ctx, "//button[normalize-space(.)='Show answer']"):
                        break
                    time.sleep(0.1)
                if exists(ctx, "//main//*[@data-testid='practice-complete']"):
                    break
                if rated > 12:
                    raise CaptureError("Practice did not finish after 12 ratings.")
                click(ctx, "//button[normalize-space(.)='Show answer']")
            wait_for_review_badge(ctx)
            self._practice_shots(ctx, by_id.get("practice-summary"), widths, started)
        except Exception as error:  # noqa: BLE001
            # Every wanted shot from the failed stage on is unreachable.
            reached = SCREENS.index(SCREEN_BY_ID[stage])
            for screen_id, screen in by_id.items():
                if SCREENS.index(screen) < reached:
                    continue
                for width in widths:
                    if not any(
                        entry["screen"] == screen_id and entry["theme"] == ctx.theme and entry["width"] == width
                        for entry in self.manifest["captures"]
                    ):
                        self.record_failure(ctx, screen, width, error, started)

    def _practice_shots(self, ctx: Context, screen: Screen | None, widths: tuple[int, ...], started: float) -> None:
        if screen is None:
            return
        for width in widths:
            set_viewport(ctx, width)
            self.prepare_capture(ctx, screen)
            self.record(ctx, screen, started)
            started = time.monotonic()

    def capture_first_run(self, theme: str) -> None:
        """Photograph the landing page of an empty library in its own session."""
        screen = SCREEN_BY_ID["first-run"]
        self.restore_snapshot(theme, self.empty_snapshot_dir)
        ctx = self.open_session(theme)
        try:
            for width in screen_widths(screen, self.options.widths):
                set_viewport(ctx, width)
                self.capture_page(ctx, screen, open_first_run)
        finally:
            self.close_session(ctx)

    def capture_theme(self, theme: str) -> None:
        started = time.monotonic()
        if any(screen.flow != "first-run" for screen in self.selected):
            self.restore_snapshot(theme)
            ctx = self.open_session(theme)
            try:
                for width in self.options.widths:
                    set_viewport(ctx, width)
                    for screen in self.selected:
                        if screen.flow != "page" or width not in screen_widths(screen, self.options.widths):
                            continue
                        self.capture_page(ctx, screen)
                for screen in self.selected:
                    if screen.flow == "dialog":
                        widths = screen_widths(screen, self.options.widths)
                        if widths:
                            self.capture_dialog(ctx, screen, widths)
                self.capture_practice(ctx, self.options.widths)
            finally:
                self.close_session(ctx)
        if self.wants_first_run():
            self.capture_first_run(theme)
        seconds = round(time.monotonic() - started, 1)
        count = sum(1 for entry in self.manifest["captures"] if entry["theme"] == theme)
        self.manifest["timings"].setdefault("themes", {})[theme] = seconds
        log(f"Captured {count} screens in the {theme} theme in {seconds}s")

    def finish(self) -> None:
        for entry, future, target in self.pending:
            try:
                future.result()
                entry["sha256"] = report.sha256_file(target)
                entry["bytes"] = target.stat().st_size
            except Exception as error:  # noqa: BLE001
                entry["status"] = "failed"
                entry["file"] = None
                entry["note"] = f"WebP conversion failed: {error}"
        self.encoder.close()

    def run(self) -> dict[str, Any]:
        started = time.monotonic()
        self.start_driver()
        try:
            self.seed_library()
            for theme in self.options.themes:
                self.capture_theme(theme)
        except Exception:
            tail = self.driver_log_tail()
            if tail:
                log("Last tauri-driver output:\n" + tail)
            raise
        finally:
            self.stop_driver()
            self.finish()
        self.manifest["timings"]["totalSeconds"] = round(time.monotonic() - started, 1)
        ok = [entry for entry in self.manifest["captures"] if entry["status"] == "ok"]
        if ok:
            self.manifest["timings"]["averageCaptureSeconds"] = round(
                sum(entry["seconds"] or 0 for entry in ok) / len(ok), 2
            )
        report.write_manifest(self.output, self.manifest)
        return self.manifest


# --------------------------------------------------------------------------
# Command line
# --------------------------------------------------------------------------


def parse_list(value: str | None, allowed: tuple[Any, ...], label: str, cast: Callable[[str], Any] = str) -> tuple[Any, ...]:
    if not value:
        return allowed
    chosen: list[Any] = []
    for raw in value.split(","):
        raw = raw.strip()
        if not raw:
            continue
        item = cast(raw)
        if item not in allowed:
            raise SystemExit(f"Unknown {label} {raw!r}; choose from {', '.join(str(a) for a in allowed)}.")
        if item not in chosen:
            chosen.append(item)
    return tuple(sorted(chosen, key=allowed.index))


def width_alias(value: str) -> int:
    aliases = {"phone": 390, "tablet": 1024, "desktop": 1440}
    return aliases.get(value.lower(), int(value) if value.isdigit() else -1)


def screen_alias(value: str) -> list[str]:
    if value == "settings":
        return list(SETTINGS_TABS)
    if value == "practice":
        return ["practice-question", "practice-answer", "practice-summary"]
    return [value]


def parse_screens(value: str | None) -> tuple[str, ...]:
    allowed = tuple(screen.id for screen in SCREENS)
    if not value:
        return allowed
    expanded = ",".join(name for raw in value.split(",") for name in screen_alias(raw.strip()))
    return parse_list(expanded, allowed, "screen")


def prepare_output(path: Path) -> None:
    if path.exists():
        stale = [child for child in path.iterdir() if child.suffix in {".webp", ".png", ".json"}]
        others = [child for child in path.iterdir() if child not in stale]
        if others:
            raise SystemExit(f"{path} contains files this tool did not write; choose an empty directory.")
        for child in stale:
            child.unlink()
    path.mkdir(parents=True, exist_ok=True)


def command_capture(args: argparse.Namespace) -> int:
    application = args.application.resolve()
    if not application.is_file():
        raise SystemExit(f"Application not found: {application}")
    output = args.output.resolve()
    prepare_output(output)
    source = json.loads(args.source_json) if args.source_json else {}
    options = RunOptions(
        application=application,
        output=output,
        app_data_root=Path(os.environ.get("XDG_DATA_HOME", "")).resolve(),
        screens=parse_screens(args.screens),
        themes=parse_list(args.themes, THEMES, "theme"),
        widths=parse_list(args.widths, tuple(VIEWPORTS), "width", width_alias),
        source=source,
        driver_port=int(os.environ.get("OSTUDY_UI_SCREENSHOTS_PORT", DEFAULT_DRIVER_PORT)),
    )
    if not os.environ.get("XDG_DATA_HOME"):
        raise SystemExit("XDG_DATA_HOME must point at disposable application data; run scripts/ui-screenshots.sh.")
    install_transport_retry()
    manifest = Run(options).run()
    counts: dict[str, int] = {}
    for entry in manifest["captures"]:
        counts[entry["status"]] = counts.get(entry["status"], 0) + 1
    log(f"Wrote {len(manifest['captures'])} captures to {output} ({counts}); total {manifest['timings']['totalSeconds']}s")
    for warning in manifest["warnings"]:
        log(f"WARNING {warning}")
    return 0 if counts.get("failed", 0) == 0 else 1


def command_compare(args: argparse.Namespace) -> int:
    selection = report.Selection(
        screens=frozenset(parse_screens(args.screens)) if args.screens else None,
        themes=frozenset(parse_list(args.themes, THEMES, "theme")) if args.themes else None,
        widths=frozenset(parse_list(args.widths, tuple(VIEWPORTS), "width", width_alias)) if args.widths else None,
    )
    output = args.output.resolve()
    if output.exists() and any(child.name not in {"index.html", "images"} for child in output.iterdir()):
        raise SystemExit(f"{output} contains other files; choose an empty directory.")
    summary = report.build_comparison(args.before.resolve(), args.after.resolve(), output, selection=selection)
    counts = summary["counts"]
    print(
        f"Comparison: {counts['changed']} changed, {counts['unchanged']} unchanged, {counts['added']} added, "
        f"{counts['removed']} removed, {counts['failed']} failed; {summary['files']} files, "
        f"{summary['bytes'] / 1048576:.1f} MiB"
    )
    for screen, cells in summary["changedScreens"].items():
        print(f"  {screen}: {', '.join(cells)}")
    if summary.get("warning"):
        print(f"WARNING {summary['warning']}")
    print(f"Open {summary['index']}")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    capture = commands.add_parser("capture", help="photograph every selected screen")
    capture.add_argument("--application", type=Path, required=True)
    capture.add_argument("--output", type=Path, required=True)
    capture.add_argument("--source-json", default="", help="source identity recorded in the manifest")
    for sub in (capture,):
        sub.add_argument("--screens", help="comma-separated screen ids (or 'settings', 'practice')")
        sub.add_argument("--themes", help="comma-separated themes")
        sub.add_argument("--widths", help="comma-separated widths or phone,tablet,desktop")
    compare = commands.add_parser("compare", help="build a before/after comparison page")
    compare.add_argument("before", type=Path)
    compare.add_argument("after", type=Path)
    compare.add_argument("--output", type=Path, required=True)
    compare.add_argument("--screens")
    compare.add_argument("--themes")
    compare.add_argument("--widths")
    commands.add_parser("list", help="print the screen ids")
    args = parser.parse_args(argv)
    if args.command == "list":
        for screen in SCREENS:
            widths = ", ".join(str(width) for width in (screen.widths or tuple(VIEWPORTS)))
            print(f"{screen.id:24} {screen.title} [{widths}]")
        return 0
    if args.command == "compare":
        return command_compare(args)
    return command_capture(args)


if __name__ == "__main__":
    sys.exit(main())
