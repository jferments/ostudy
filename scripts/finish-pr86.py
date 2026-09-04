#!/usr/bin/env python3
"""Apply the final PR #86 smoke, compatibility, and documentation updates.

This file is temporary implementation tooling. The writer workflow removes it
and itself before committing the final product source.
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def replace_py_function(source: str, name: str, replacement: str) -> str:
    pattern = re.compile(rf"^def {re.escape(name)}\([\s\S]*?(?=^def |\Z)", re.M)
    match = pattern.search(source)
    if not match:
        raise RuntimeError(f"Could not find Python function {name}")
    return source[: match.start()] + replacement.rstrip() + "\n\n\n" + source[match.end() :].lstrip("\n")


def update_mobile_app_bar() -> None:
    path = ROOT / "src/components/MobileAppBar.tsx"
    source = path.read_text()
    if 'data-testid="app-settings"' not in source:
        needle = '          tooltip="Settings"\n          aria-current={settingsActive ? "page" : undefined}'
        if needle not in source:
            raise RuntimeError("MobileAppBar Settings action drifted")
        source = source.replace(
            needle,
            '          tooltip="Settings"\n          data-testid="app-settings"\n          aria-current={settingsActive ? "page" : undefined}',
        )
    path.write_text(source)


def update_study_guides_reselection() -> None:
    helper = ROOT / "src/navigation/studyGuidesNavigation.ts"
    helper.write_text(
        '''import type { AppSection, AppWorkflow } from "../controllers/useAppController";\n\n/**\n * Selecting the already-active Study Guides destination is a deliberate\n * request to return from a nested guide viewer to the Study Guides library.\n */\nexport function shouldResetStudyGuidesRoute(\n  requestedSection: AppSection,\n  currentSection: AppSection,\n  activeWorkflow: AppWorkflow | null,\n): boolean {\n  return (\n    requestedSection === "study-guides" &&\n    currentSection === "study-guides" &&\n    activeWorkflow === null\n  );\n}\n'''
    )
    (ROOT / "src/navigation/studyGuidesNavigation.test.ts").write_text(
        '''import { describe, expect, it } from "vitest";\n\nimport { shouldResetStudyGuidesRoute } from "./studyGuidesNavigation";\n\ndescribe("shouldResetStudyGuidesRoute", () => {\n  it("resets an already-active Study Guides route so a viewer returns to the library", () => {\n    expect(shouldResetStudyGuidesRoute("study-guides", "study-guides", null)).toBe(true);\n  });\n\n  it("does not reset on initial navigation, unrelated destinations, or active workflows", () => {\n    expect(shouldResetStudyGuidesRoute("study-guides", "decks", null)).toBe(false);\n    expect(shouldResetStudyGuidesRoute("documents", "study-guides", null)).toBe(false);\n    expect(shouldResetStudyGuidesRoute("study-guides", "study-guides", "practice")).toBe(false);\n  });\n});\n'''
    )

    path = ROOT / "src/App.tsx"
    source = path.read_text()
    if 'shouldResetStudyGuidesRoute' not in source:
        anchor = 'import { PerformanceProfiler } from "./testing/PerformanceProfiler";'
        if anchor not in source:
            raise RuntimeError("App import boundary drifted")
        source = source.replace(
            anchor,
            'import { shouldResetStudyGuidesRoute } from "./navigation/studyGuidesNavigation";\n' + anchor,
        )
    old = '''    if (\n      section === "study-guides" &&\n      controller.section === "study-guides" &&\n      controller.activeWorkflow === null\n    ) {'''
    if old in source:
        source = source.replace(
            old,
            '''    if (\n      shouldResetStudyGuidesRoute(\n        section,\n        controller.section,\n        controller.activeWorkflow,\n      )\n    ) {''',
        )
    path.write_text(source)


def update_hidden_tabs() -> None:
    path = ROOT / "src/ui/primitives/Tabs.tsx"
    source = path.read_text()
    source = source.replace(
        'import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";',
        'import { useEffect, useId, useRef, useState, type KeyboardEvent, type ReactNode } from "react";',
    )
    marker = '''  const visibleTabs = tabs.filter((tab) => !tab.hiddenFromTabList);\n  const hasVisibleSelection = visibleTabs.some((tab) => tab.value === value);'''
    replacement = '''  const visibleTabs = tabs.filter((tab) => !tab.hiddenFromTabList);\n  const hasVisibleSelection = visibleTabs.some((tab) => tab.value === value);\n  const [mountedHiddenValues, setMountedHiddenValues] = useState<ReadonlySet<T>>(\n    () => new Set(),\n  );\n\n  useEffect(() => {\n    const selectedTab = tabs.find((tab) => tab.value === value);\n    if (!selectedTab?.hiddenFromTabList || mountedHiddenValues.has(value)) return;\n    setMountedHiddenValues((current) => new Set(current).add(value));\n  }, [mountedHiddenValues, tabs, value]);'''
    if "mountedHiddenValues" not in source:
        if marker not in source:
            raise RuntimeError("Tabs selection boundary drifted")
        source = source.replace(marker, replacement)
    source = source.replace(
        '{tab.hiddenFromTabList && !selected ? null : tab.panel}',
        '''{tab.hiddenFromTabList && !selected && !mountedHiddenValues.has(tab.value)\n              ? null\n              : tab.panel}''',
    )
    path.write_text(source)

    (ROOT / "src/ui/primitives/Tabs.hidden.test.tsx").write_text(
        '''import { fireEvent, render, screen } from "@testing-library/react";\nimport { useState } from "react";\nimport { describe, expect, it } from "vitest";\n\nimport { Tabs, type TabDefinition } from "./Tabs";\n\ntype Value = "primary" | "advanced";\n\nfunction AdvancedPanel() {\n  const [draft, setDraft] = useState("");\n  return (\n    <label>\n      Advanced draft\n      <input value={draft} onChange={(event) => setDraft(event.target.value)} />\n    </label>\n  );\n}\n\nconst tabs: ReadonlyArray<TabDefinition<Value>> = [\n  { value: "primary", label: "Primary", panel: <div>Primary panel</div> },\n  {\n    value: "advanced",\n    label: "Advanced tool",\n    hiddenFromTabList: true,\n    panel: <AdvancedPanel />,\n  },\n];\n\nfunction Harness() {\n  const [value, setValue] = useState<Value>("primary");\n  return (\n    <>\n      <button type="button" onClick={() => setValue("advanced")}>\n        Open advanced tool\n      </button>\n      <Tabs value={value} onValueChange={setValue} tabs={tabs} ariaLabel="Sections" />\n    </>\n  );\n}\n\ndescribe("Tabs hidden panels", () => {\n  it("keeps advanced menu destinations out of the tab strip and preserves their draft after first use", () => {\n    render(<Harness />);\n\n    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual(["Primary"]);\n    expect(screen.queryByLabelText("Advanced draft")).toBeNull();\n\n    fireEvent.click(screen.getByRole("button", { name: "Open advanced tool" }));\n    const input = screen.getByLabelText("Advanced draft") as HTMLInputElement;\n    fireEvent.change(input, { target: { value: "custom template" } });\n\n    fireEvent.click(screen.getByRole("tab", { name: "Primary" }));\n    expect(screen.getAllByRole("tab")).toHaveLength(1);\n\n    fireEvent.click(screen.getByRole("button", { name: "Open advanced tool" }));\n    expect((screen.getByLabelText("Advanced draft") as HTMLInputElement).value).toBe(\n      "custom template",\n    );\n  });\n});\n'''
    )


def add_settings_template_test() -> None:
    (ROOT / "src/screens/SettingsScreen.templates.test.tsx").write_text(
        '''import { fireEvent, render, screen } from "@testing-library/react";\nimport { describe, expect, it, vi } from "vitest";\n\nimport type { AppSettings } from "../ipc/settings";\nimport { SettingsScreen } from "./SettingsScreen";\n\nvi.mock("../controllers/aiProfilesController", () => ({\n  useAiProfilesController: () => ({ dirty: false, mutating: false }),\n}));\nvi.mock("./settings/AiLlmSettings", () => ({ AiLlmSettings: () => <div>AI settings</div> }));\nvi.mock("./settings/AppearanceSettings", () => ({ AppearanceSettings: () => <div>Appearance settings</div> }));\nvi.mock("./settings/DefaultAiModelSettings", () => ({ DefaultAiModelSettings: () => <div>Default model</div> }));\nvi.mock("./settings/DiagnosticsSettings", () => ({ DiagnosticsSettings: () => <div>Diagnostics settings</div> }));\nvi.mock("./settings/ReviewSchedulingSettings", () => ({ ReviewSchedulingSettings: () => <div>Review settings</div> }));\nvi.mock("./settings/StorageBackupSettings", () => ({ StorageBackupSettings: () => <div>Storage settings</div> }));\nvi.mock("./TemplatesScreen", () => ({ TemplatesScreen: () => <div aria-label="Card template manager">Template manager</div> }));\n\nconst settings: AppSettings = {\n  theme: "dark",\n  uiFont: "system",\n  cardFont: "system",\n  uiScale: 100,\n  cardBaseSize: 18,\n  motionPreference: "system",\n  materialPreference: "automatic",\n  decorativeArtwork: true,\n  celebrationsEnabled: true,\n  tutorialsEnabled: false,\n  confirmDeletions: true,\n  practiceShuffle: true,\n  desiredRetention: 0.9,\n  dailyNewLimit: 20,\n  dailyReviewLimit: 200,\n};\n\ndescribe("Settings Card templates tool", () => {\n  it("opens template management from Settings without adding another Settings tab", () => {\n    render(\n      <SettingsScreen\n        settings={settings}\n        onPreviewSettings={vi.fn()}\n        onSavedSettings={vi.fn()}\n        onDirtyChange={vi.fn()}\n        showToast={vi.fn()}\n      />,\n    );\n\n    expect(screen.getAllByRole("tab").map((tab) => tab.textContent)).toEqual([\n      "Appearance",\n      "Backup/Sync",\n      "AI/LLM",\n      "Review/Scheduling",\n      "Diagnostics",\n    ]);\n    expect(screen.queryByLabelText("Card template manager")).toBeNull();\n\n    fireEvent.click(screen.getByRole("button", { name: "Card templates" }));\n\n    expect(screen.getByRole("tabpanel", { name: "Card templates" })).toBeTruthy();\n    expect(screen.getByLabelText("Card template manager")).toBeTruthy();\n    expect(screen.getAllByRole("tab")).toHaveLength(5);\n  });\n});\n'''
    )


def update_native_smoke() -> None:
    path = ROOT / "tests/webdriver/ui_platform_smoke.py"
    source = path.read_text()
    source = replace_py_function(
        source,
        "assert_flashcards_tabs",
        '''def assert_flashcards_tabs(session_id: str, deadline: float) -> None:\n    expected = ["Decks", "Search/Filter"]\n    for label in expected:\n        find_workspace_tab(session_id, label, deadline)\n    labels = execute(\n        session_id,\n        """\n        return [...document.querySelectorAll(\n          "[role='tablist'][aria-label='Flashcards workspaces'] [role='tab']"\n        )].map((tab) => tab.getAttribute('aria-label') || (tab.textContent || '').trim());\n        """,\n    )\n    if labels != expected:\n        raise AssertionError(\n            f"Flashcards must expose exactly Decks and Search/Filter: {labels!r}"\n        )''',
    )
    source = replace_py_function(
        source,
        "verify_compact_navigation",
        '''def verify_compact_navigation(session_id: str, deadline: float) -> None:\n    set_window_rect(session_id, 390, 700)\n    wait_for_layout_mode(session_id, "compact", deadline)\n    assert_brand_and_fresh_font_defaults(session_id, "compact layout")\n    assert_navigation_labels(\n        session_id,\n        ["Flashcards", "Study Guides", "Documents", "Analytics"],\n        "compact",\n    )\n    open_study_guides_and_return(\n        session_id,\n        deadline,\n        "compact Study Guides destination",\n    )\n    assert_no_horizontal_overflow(session_id, "compact navigation")\n\n    more_count = execute(\n        session_id,\n        "return document.querySelectorAll(\\"button[data-testid='nav-more']\\").length;",\n    )\n    if int(more_count) != 0:\n        raise AssertionError("Compact navigation still exposes the removed More destination")\n\n    documents = webdriver.find_element(\n        session_id,\n        "css selector",\n        "button[data-testid='nav-documents']",\n        deadline,\n    )\n    webdriver.click(session_id, documents)\n    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Documents", deadline)\n    wait_for_overlay_inactive(session_id, deadline, "compact Documents assertions")\n    assert_no_horizontal_overflow(session_id, "compact Documents screen")\n\n    settings = webdriver.find_element(\n        session_id,\n        "css selector",\n        "button[data-testid='app-settings']",\n        deadline,\n    )\n    webdriver.click(session_id, settings)\n    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Settings", deadline)\n    wait_for_overlay_inactive(session_id, deadline, "compact Settings assertions")\n    assert_no_horizontal_overflow(session_id, "compact Settings screen")\n\n    flashcards = webdriver.find_element(\n        session_id,\n        "css selector",\n        "button[data-testid='nav-decks']",\n        deadline,\n    )\n    webdriver.click(session_id, flashcards)\n    webdriver.find_element(\n        session_id,\n        "css selector",\n        "main [role='tablist'][aria-label='Flashcards workspaces']",\n        deadline,\n    )\n    assert_flashcards_tabs(session_id, deadline)\n    assert_no_horizontal_overflow(session_id, "compact Flashcards workspace")''',
    )
    source = replace_py_function(
        source,
        "verify_dialog_focus",
        '''def verify_dialog_focus(session_id: str, deadline: float) -> None:\n    new_deck = webdriver.find_element(\n        session_id,\n        "xpath",\n        "//main//button[normalize-space(.)='Create a new deck' or normalize-space(.)='New deck']",\n        deadline,\n    )\n    webdriver.click(session_id, new_deck)\n    name_input = webdriver.find_element(\n        session_id,\n        "css selector",\n        "dialog input[data-autofocus='true']",\n        deadline,\n    )\n    wait_for_focus(session_id, name_input, deadline)\n    assert_dialog_viewport_bounds(session_id, "responsive create-deck dialog")\n\n    send_key(session_id, TAB_KEY)\n    inside_dialog = execute(\n        session_id,\n        "return Boolean(document.activeElement && document.activeElement.closest('[role=dialog]'));",\n    )\n    if inside_dialog is not True:\n        raise AssertionError("Tab moved focus outside the active dialog")\n\n    send_key(session_id, ESCAPE_KEY)\n    wait_for_absent(session_id, "css selector", "[role='dialog']", deadline)\n    wait_for_overlay_inactive(session_id, deadline, "create-deck focus restoration")\n    wait_for_active_label(session_id, "Create a new deck", deadline)''',
    )
    source = source.replace(
        'find_workspace_tab(session_id, "Search", deadline)',
        'find_workspace_tab(session_id, "Search/Filter", deadline)',
    )
    source = source.replace('"medium Search workspace"', '"medium Search/Filter workspace"')
    source = source.replace(
        '"//main//button[normalize-space(.)=\'New deck\']"',
        '"//main//button[normalize-space(.)=\'Create a new deck\' or normalize-space(.)=\'New deck\']"',
    )
    source = source.replace("four-workspace Flashcards containment", "two-workspace Flashcards containment")
    path.write_text(source)


def update_android_smoke() -> None:
    path = ROOT / "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt"
    source = path.read_text()
    if "fun a1_freshLibraryUsesQuietNavigation()" not in source:
        anchor = '''    @Test\n    fun a_seedLibraryPracticeAndAnalytics() {'''
        if anchor not in source:
            raise RuntimeError("Android seed smoke boundary drifted")
        fresh = '''    @Test\n    fun a1_freshLibraryUsesQuietNavigation() {\n        withApplication { webView ->\n            waitForBridge(webView)\n            waitUntil(\n                webView,\n                predicate = { it == "true" },\n                expression = {\n                    "document.querySelector('main h1')?.textContent?.trim() === 'Flashcards'"\n                }\n            )\n            val snapshot = evaluateJsonObject(\n                webView,\n                """\n                (() => {\n                  const tabLabels = [...document.querySelectorAll(\n                    "main [role='tablist'][aria-label='Flashcards workspaces'] [role='tab']"\n                  )].map((tab) => tab.getAttribute('aria-label') || (tab.textContent || '').trim());\n                  const navigationLabels = [...document.querySelectorAll(\n                    "nav[aria-label='Main navigation'] button"\n                  )].map((button) => (button.textContent || '').trim());\n                  const mainText = document.querySelector('main')?.textContent || '';\n                  const buttons = [...document.querySelectorAll('main button')];\n                  return {\n                    tabs: tabLabels.join('|'),\n                    navigation: navigationLabels.join('|'),\n                    settingsGear: Boolean(document.querySelector('[data-testid="app-settings"]')),\n                    importAction: Boolean(\n                      document.querySelector('main button[aria-label="Import deck from file"]')\n                    ),\n                    createAction: buttons.some(\n                      (button) => button.textContent?.trim() === 'Create a new deck'\n                    ),\n                    emptyHeading: [...document.querySelectorAll('main h2')].some(\n                      (heading) => heading.textContent?.trim() === 'No decks have been created yet.'\n                    ),\n                    foldersHeading: [...document.querySelectorAll('main h2')].some(\n                      (heading) => heading.textContent?.trim() === 'Folders'\n                    ),\n                    unsortedHeading: [...document.querySelectorAll('main h2')].some(\n                      (heading) => heading.textContent?.trim() === 'Unsorted decks'\n                    ),\n                    staleEmptyCopy:\n                      mainText.includes('No folders yet.') ||\n                      mainText.includes('All active decks are in folders.'),\n                    irrelevantControls: buttons.some((button) => {\n                      const label = (\n                        button.getAttribute('aria-label') ||\n                        button.textContent ||\n                        ''\n                      ).trim();\n                      return label === 'Collapse all' ||\n                        label === 'Expand all' ||\n                        label.startsWith('Select all');\n                    })\n                  };\n                })()\n                """.trimIndent()\n            )\n            assertEquals("Decks|Search/Filter", snapshot.getString("tabs"))\n            assertEquals(\n                "Flashcards|Study Guides|Documents|Analytics",\n                snapshot.getString("navigation")\n            )\n            assertTrue(snapshot.getBoolean("settingsGear"))\n            assertTrue(snapshot.getBoolean("importAction"))\n            assertTrue(snapshot.getBoolean("createAction"))\n            assertTrue(snapshot.getBoolean("emptyHeading"))\n            assertFalse(snapshot.getBoolean("foldersHeading"))\n            assertFalse(snapshot.getBoolean("unsortedHeading"))\n            assertFalse(snapshot.getBoolean("staleEmptyCopy"))\n            assertFalse(snapshot.getBoolean("irrelevantControls"))\n        }\n    }\n\n'''
        source = source.replace(anchor, fresh + anchor)

    pattern = re.compile(
        r'''    @Test\n    fun c_documentsStubAndAndroidBackReturnToFlashcardsDecks\(\) \{[\s\S]*?(?=    @Test\n    fun c_nestedDeckAndEditorBackReturnThroughTheStack\(\))'''
    )
    match = pattern.search(source)
    if not match:
        raise RuntimeError("Android Documents smoke boundary drifted")
    replacement = '''    @Test\n    fun c_documentsStubAndAndroidBackReturnToFlashcardsDecks() {\n        withApplication { webView ->\n            waitForBridge(webView)\n            evaluateSync(\n                webView,\n                "document.querySelector('[data-testid=\\"nav-documents\\"]')?.click(); true"\n            )\n            waitUntil(\n                webView,\n                predicate = { it == "true" },\n                expression = {\n                    "document.querySelector('main h1')?.textContent?.trim() === 'Documents' && " +\n                        "Array.from(document.querySelectorAll('main button')).some(button => button.textContent?.trim() === 'Upload document')"\n                }\n            )\n            waitForAndroidBackReady(webView)\n\n            UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()).pressBack()\n            waitUntil(\n                webView,\n                predicate = { it == "true" },\n                expression = {\n                    "document.querySelector('main h1')?.textContent?.trim() === 'Flashcards' && " +\n                        "Array.from(document.querySelectorAll('main button[role=\\"tab\\"][aria-selected=\\"true\\"]')).some(button => button.textContent?.includes('Decks'))"\n                }\n            )\n        }\n    }\n\n'''
    source = source[: match.start()] + replacement + source[match.end() :]
    path.write_text(source)

    runner = ROOT / "scripts/android-emulator-smoke.sh"
    runner_source = runner.read_text()
    if "a1_freshLibraryUsesQuietNavigation" not in runner_source:
        output: list[str] = []
        changed = False
        for line in runner_source.splitlines():
            output.append(line)
            scheduled = re.search(r"\b(record_case|run_case)\s+a0_appLaunchesAndTestBridgeIsReady\b", line)
            if scheduled:
                command = scheduled.group(1)
                indent = line[: len(line) - len(line.lstrip())]
                output.append(f"{indent}{command} a1_freshLibraryUsesQuietNavigation")
                changed = True
        if not changed:
            raise RuntimeError("Android smoke schedule boundary drifted")
        runner_source = "\n".join(output) + "\n"
    runner.write_text(runner_source)


def update_route_contract() -> None:
    path = ROOT / "scripts/route-smoke-contract.json"
    contract = json.loads(path.read_text())
    contract["authoritativeSources"]["mobileAppBar"] = "src/components/MobileAppBar.tsx"
    contract["compactNavigation"] = {
        "primary": ["decks", "study-guides", "documents", "analytics"],
        "more": [],
        "appBar": ["settings"],
    }
    primary = {entry["section"]: entry for entry in contract["primaryDestinations"]}
    for evidence in primary["settings"]["smoke"]["evidence"]:
        if evidence["runner"] == "native-webdriver":
            evidence["entryFragments"] = [
                "button[data-testid='app-settings']",
                "webdriver.click(session_id, settings)",
            ]
            evidence["assertionFragments"] = ["main h1", "compact Settings assertions"]

    android_path = "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt"
    deck_evidence = primary["decks"]["smoke"]["evidence"]
    if not any(
        item.get("runner") == "android-emulator"
        and item.get("instrumentationCase", {}).get("method")
        == "a1_freshLibraryUsesQuietNavigation"
        for item in deck_evidence
    ):
        deck_evidence.append(
            {
                "runner": "android-emulator",
                "path": android_path,
                "instrumentationCase": {
                    "class": "AndroidSmokeTest",
                    "method": "a1_freshLibraryUsesQuietNavigation",
                },
                "entryFragments": [
                    "fun a1_freshLibraryUsesQuietNavigation()",
                    "document.querySelector('main h1')?.textContent?.trim() === 'Flashcards'",
                ],
                "assertionFragments": [
                    "No decks have been created yet.",
                    "Import deck from file",
                    "Flashcards|Study Guides|Documents|Analytics",
                ],
            }
        )
    for evidence in primary["documents"]["smoke"]["evidence"]:
        if evidence["runner"] == "android-emulator":
            evidence["entryFragments"] = [
                "fun c_documentsStubAndAndroidBackReturnToFlashcardsDecks()",
                "nav-documents",
            ]
            evidence["assertionFragments"] = ["=== 'Documents'", "Upload document"]

    workspaces = {entry["id"]: entry for entry in contract["flashcardsWorkspaces"]}
    if set(workspaces) != {"decks", "search"}:
        raise RuntimeError("Flashcards workspace inventory drifted")
    workspaces["search"]["label"] = "Search/Filter"
    for evidence in workspaces["search"]["smoke"]["evidence"]:
        if evidence["runner"] == "native-webdriver":
            evidence["entryFragments"] = [
                'search = find_workspace_tab(session_id, "Search/Filter", deadline)',
                "webdriver.click(session_id, search)",
            ]
            evidence["assertionFragments"] = [
                "main section[aria-label='Card filters']",
                "medium Search/Filter workspace",
            ]
    path.write_text(json.dumps(contract, indent=2) + "\n")


def update_route_verifier() -> None:
    path = ROOT / "scripts/verify-route-smoke-contract.mjs"
    source = path.read_text()
    source = source.replace(
        '    "primaryNavigation",\n    "compactNavigation",\n    "flashcardsRoutes",',
        '    "primaryNavigation",\n    "compactNavigation",\n    "mobileAppBar",\n    "flashcardsRoutes",',
    )
    compact_pattern = re.compile(
        r'''if \(sources\.compactNavigation !== null\) \{[\s\S]*?\n\}\nif \(sources\.flashcardsRoutes !== null\) \{'''
    )
    match = compact_pattern.search(source)
    if not match:
        raise RuntimeError("Route verifier compact comparison drifted")
    compact_block = '''if (sources.compactNavigation !== null) {\n  const visibleDestinations = parseNavigationItems(\n    sources.compactNavigation,\n    "ITEMS",\n    "section",\n  ).map((entry) => entry.id);\n  const compactPrimary = parseStringArray(\n    sources.compactNavigation,\n    "COMPACT_PRIMARY_SECTIONS",\n  );\n  const compactAppBar =\n    sources.mobileAppBar === null ? [] : parseAppBarDestinations(sources.mobileAppBar);\n  const reserved = new Set([...compactPrimary, ...compactAppBar]);\n  compareOrderedValues(\n    "compact primary destinations",\n    contract.compactNavigation.primary,\n    compactPrimary,\n  );\n  compareOrderedValues(\n    "compact More destinations",\n    contract.compactNavigation.more,\n    visibleDestinations.filter((section) => !reserved.has(section)),\n  );\n  compareOrderedValues(\n    "compact app-bar destinations",\n    contract.compactNavigation.appBar,\n    compactAppBar,\n  );\n}\nif (sources.flashcardsRoutes !== null) {'''
    source = source[: match.start()] + compact_block + source[match.end() :]

    validation_pattern = re.compile(
        r'''function validateCompactNavigation\(compactNavigation, primaryDestinations\) \{[\s\S]*?(?=\nfunction validateApplicationSections\()'''
    )
    match = validation_pattern.search(source)
    if not match:
        raise RuntimeError("Route verifier compact validation drifted")
    validation = '''function validateCompactNavigation(compactNavigation, primaryDestinations) {\n  const groups = ["primary", "more", "appBar"];\n  for (const group of groups) {\n    if (\n      !Array.isArray(compactNavigation[group]) ||\n      compactNavigation[group].some((section) => !isIdentifier(section))\n    ) {\n      fail(`compactNavigation.${group} must be an array of destination identifiers.`);\n      return;\n    }\n    reportDuplicates(`compact ${group} destination`, compactNavigation[group]);\n  }\n\n  const visible = primaryDestinations.map((entry) => entry.section);\n  const compactPrimary = compactNavigation.primary;\n  const compactMore = compactNavigation.more;\n  const compactAppBar = compactNavigation.appBar;\n  const allCompact = [...compactPrimary, ...compactMore, ...compactAppBar];\n  reportDuplicates("compact destination", allCompact);\n  compareSets("compact navigation coverage", visible, allCompact);\n\n  for (const [label, group] of [\n    ["primary", compactPrimary],\n    ["More", compactMore],\n    ["app-bar", compactAppBar],\n  ]) {\n    const groupSet = new Set(group);\n    const expectedOrder = visible.filter((section) => groupSet.has(section));\n    compareOrderedValues(`compact ${label} destination order`, expectedOrder, group);\n  }\n}\n'''
    source = source[: match.start()] + validation + source[match.end() :]

    anchor = "function parseNativeSmokeEntrypoints(source) {"
    if "function parseAppBarDestinations" not in source:
        if anchor not in source:
            raise RuntimeError("Route verifier parser insertion point drifted")
        parser = '''function parseAppBarDestinations(source) {\n  const pattern =\n    /<IconButton[\\s\\S]*?data-testid="app-([^"]+)"[\\s\\S]*?onClick=\\{\\(\\) => void onNavigate\\("([^"]+)"\\)\\}[\\s\\S]*?\\/>/gu;\n  const destinations = [];\n  for (const match of source.matchAll(pattern)) {\n    const testId = match[1];\n    const destination = match[2];\n    if (testId !== destination) {\n      fail(\n        `Compact app-bar test ID app-${testId} does not match destination ${destination}.`,\n      );\n    }\n    destinations.push(destination);\n  }\n  if (destinations.length === 0) {\n    fail("Could not parse compact app-bar destinations from MobileAppBar.");\n  }\n  return destinations;\n}\n\n'''
        source = source.replace(anchor, parser + anchor)
    source = source.replace(
        '''      `${contract.compactNavigation.primary.length} compact primary / ` +\n      `${contract.compactNavigation.more.length} More destinations.`,''',
        '''      `${contract.compactNavigation.primary.length} compact primary / ` +\n      `${contract.compactNavigation.more.length} More / ` +\n      `${contract.compactNavigation.appBar.length} app-bar destinations.`,''',
    )
    path.write_text(source)


def update_contract_tests() -> None:
    path = ROOT / "tests/scripts/test_route_smoke_contract.py"
    source = path.read_text()
    for old in (
        "4 compact primary / 1 More destinations.",
        "3 compact primary / 2 More destinations.",
    ):
        source = source.replace(old, "4 compact primary / 0 More / 1 app-bar destinations.")
    source = source.replace(
        "compact primary / 1 More destinations",
        "compact primary / 0 More / 1 app-bar destinations",
    )
    path.write_text(source)

    (ROOT / "tests/scripts/test_mobile_navigation_contract.py").write_text(
        '''import json\nimport unittest\nfrom pathlib import Path\n\nROOT = Path(__file__).resolve().parents[2]\n\n\nclass MobileNavigationContractTests(unittest.TestCase):\n    def test_compact_learning_destinations_and_settings_app_bar_are_disjoint(self) -> None:\n        contract = json.loads((ROOT / "scripts/route-smoke-contract.json").read_text())\n        compact = contract["compactNavigation"]\n        self.assertEqual(\n            compact["primary"],\n            ["decks", "study-guides", "documents", "analytics"],\n        )\n        self.assertEqual(compact["more"], [])\n        self.assertEqual(compact["appBar"], ["settings"])\n        all_destinations = compact["primary"] + compact["more"] + compact["appBar"]\n        self.assertEqual(len(all_destinations), len(set(all_destinations)))\n\n        navigation = (ROOT / "src/components/NavigationSidebar.tsx").read_text()\n        app_bar = (ROOT / "src/components/MobileAppBar.tsx").read_text()\n        self.assertIn('"documents",\\n  "analytics"', navigation)\n        self.assertNotIn("nav-more", navigation)\n        self.assertIn('data-testid="app-settings"', app_bar)\n        self.assertIn('onNavigate("settings")', app_bar)\n\n\nif __name__ == "__main__":\n    unittest.main()\n'''
    )


def update_docs_and_remove_obsolete_files() -> None:
    spec = ROOT / "docs/development/mobile-flashcards-navigation-cleanup.md"
    source = spec.read_text()
    if "## Presentation ownership" not in source:
        source += '''\n## Presentation ownership\n\n- The Flashcards workspace owns only Decks and Search/Filter.\n- Portable deck-file orchestration is presentation-independent in `DeckTransferController`; the Decks page and focused dialogs supply the UI entry points.\n- Card-template management remains backed by the existing template controller and IPC contracts, but Settings owns its navigation entry.\n- Compact primary navigation owns four direct learning destinations. Settings is a distinct upper-right app-bar action rather than a fifth bottom item or a More-menu destination.\n- Re-selecting the active Study Guides destination is an explicit reset-to-library request, so nested viewer state does not trap the user.\n'''
    spec.write_text(source)

    architecture = ROOT / "docs/architecture/ui.md"
    if architecture.exists():
        source = architecture.read_text()
        if "Contextual Flashcards tools" not in source:
            source += '''\n\n## Contextual Flashcards tools\n\nFlashcards exposes two primary modes: **Decks** and **Search/Filter**. Advanced or object-scoped actions do not consume permanent workspace tabs. Card-template management is entered from Settings, while import and export are launched from the Decks library, individual deck actions, or the selected-deck toolbar. Portable file operations are coordinated by a presentation-independent controller so desktop and Android surfaces share the same behavior without coupling backend commands to a particular layout.\n\nCompact navigation contains **Flashcards**, **Study Guides**, **Documents**, and **Analytics**. Settings is an app-bar gear action. Re-selecting Study Guides while a guide viewer is open resets the route to the Study Guides library.\n'''
            architecture.write_text(source)

    for relative in (
        "docs/development/mobile-flashcards-navigation-cleanup-progress.md",
        "src/screens/flashcards/TransferWorkspace.tsx",
        "src/components/AndroidDeckSharePanel.tsx",
    ):
        path = ROOT / relative
        if path.exists():
            path.unlink()


def main() -> None:
    update_mobile_app_bar()
    update_study_guides_reselection()
    update_hidden_tabs()
    add_settings_template_test()
    update_native_smoke()
    update_android_smoke()
    update_route_contract()
    update_route_verifier()
    update_contract_tests()
    update_docs_and_remove_obsolete_files()


if __name__ == "__main__":
    main()
