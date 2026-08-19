#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import re
import textwrap

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    target = ROOT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(content)


def replace_once(text: str, old: str, new: str, path: str) -> str:
    if new in text and old not in text:
        return text
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old[:160]!r}")
    return text.replace(old, new, 1)


def patch_product_fixes() -> None:
    # A new unsaved setup draft is the expected place to choose another service;
    # switching providers there must not ask the user to discard a draft that has
    # never been committed.
    path = "src/screens/settings/AiLlmSettings.tsx"
    text = read(path)
    marker = "  async function beginCreate(providerKind: AiProviderKind): Promise<void> {"
    start = text.find(marker)
    if start < 0:
        raise RuntimeError(f"{path}: beginCreate not found")
    end = text.find("\n  }", start)
    if end < 0:
        raise RuntimeError(f"{path}: beginCreate boundary not found")
    block = text[start:end]
    block = block.replace(
        "    if (controller.dirty) {",
        '    if (controller.dirty && !controller.selectedProfileId?.startsWith("new:")) {',
        1,
    )
    text = text[:start] + block + text[end:]
    write(path, text)


def patch_practice_tests() -> None:
    path = "src/screens/PracticeScreen.test.tsx"
    text = read(path)
    text = text.replace(
        '    expect(screen.getByRole("checkbox", { name: /Dog facts/u })).toBeChecked();',
        '    expect((screen.getByRole("checkbox", { name: /Dog facts/u }) as HTMLInputElement).checked).toBe(true);',
    )
    text = text.replace(
        '    expect(screen.getByRole("checkbox", { name: /Cat facts/u })).toBeChecked();',
        '    expect((screen.getByRole("checkbox", { name: /Cat facts/u }) as HTMLInputElement).checked).toBe(true);',
    )
    old = '''    renderPractice({ decks: [noDueDeck] });
    expect(await screen.findByRole("checkbox", { name: /Reviewed dog facts/u, checked: true })).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: "Study this selection anyway" }));'''
    new = '''    renderPractice({ decks: [noDueDeck] });
    expect(await screen.findByRole("checkbox", { name: /Reviewed dog facts/u, checked: true })).toBeTruthy();
    await waitFor(() =>
      expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledWith([noDueDeck.id], []),
    );
    expect(await screen.findByText("0 cards in the scheduled session")).toBeTruthy();
    fireEvent.click(await screen.findByRole("button", { name: "Study this selection anyway" }));'''
    if old in text:
        text = text.replace(old, new, 1)
    elif new not in text:
        raise RuntimeError(f"{path}: extra-practice regression target not found")
    write(path, text)


def patch_settings_smoke_test() -> None:
    path = "src/screens/SettingsScreen.test.tsx"
    text = read(path)
    pattern = re.compile(
        r'''  it\("uses one New profile action and selects provider type inside the connection form", async \(\) => \{.*?\n  \}\);''',
        re.S,
    )
    replacement = '''  it("uses one simple Add AI model action and chooses service inside setup", async () => {
    renderSettings();
    fireEvent.click(screen.getByRole("tab", { name: "AI/LLM" }));
    expect(await screen.findByRole("heading", { name: "AI models" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add AI model" })).toBeTruthy();
    expect(screen.queryByText("Bearer token")).toBeNull();
    expect(screen.queryByText("Enable profile")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Add AI model" }));
    const service = await screen.findByRole("combobox", { name: "1. Service" });
    expect((service as HTMLSelectElement).value).toBe("llama_cpp");
    expect(screen.getByRole("textbox", { name: "2. Server address" })).toBeTruthy();
    expect(screen.getByLabelText("API key (optional)")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Connect and retrieve models" })).toBeTruthy();

    fireEvent.change(service, { target: { value: "deepseek" } });
    expect((screen.getByRole("combobox", { name: "1. Service" }) as HTMLSelectElement).value).toBe("deepseek");
    expect(screen.getByLabelText("2. API key")).toBeTruthy();
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });'''
    text, count = pattern.subn(replacement, text, count=1)
    if count != 1 and replacement not in text:
        raise RuntimeError(f"{path}: legacy AI smoke test not found")
    write(path, text)


def write_settings_extended_tests() -> None:
    write(
        "src/screens/SettingsScreen.extended.test.tsx",
        textwrap.dedent(
            r'''import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiIpc from "../ipc/aiProfiles";
import * as settingsIpc from "../ipc/settings";
import { makeSettings } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { SettingsScreen } from "./SettingsScreen";

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32), assetDraftId: "7".repeat(32), revision: 1,
    status: "saved", errorMessage: null, recovered: false, hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined), discard: vi.fn().mockResolvedValue(undefined),
    prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),
    acceptCanonicalCommit: vi.fn(), commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn(), publishExport: vi.fn() }));
vi.mock("../ipc/backup", () => ({ prepareLibraryBackup: vi.fn(), prepareLibraryRestore: vi.fn() }));
vi.mock("../ipc/settings", () => ({ getAppInfo: vi.fn(), saveSettings: vi.fn() }));
vi.mock("../ipc/aiProfiles", () => ({
  getAiProviderCatalog: vi.fn(), listAiProviderProfiles: vi.fn(), getAiProviderProfile: vi.fn(),
  createAiProviderProfile: vi.fn(), updateAiProviderProfile: vi.fn(), deleteAiProviderProfile: vi.fn(),
  setAiProviderCredential: vi.fn(), clearAiProviderCredential: vi.fn(), getAiTaskDefaults: vi.fn(),
  setFlashcardGenerationProfile: vi.fn(), listAiProviderModels: vi.fn(), testAiProviderProfile: vi.fn(),
  getAiProviderProbeResult: vi.fn(),
}));
vi.mock("../ipc/common", async (importOriginal) => {
  const original = await importOriginal<typeof import("../ipc/common")>();
  return { ...original, getErrorMessage: (error: unknown) => error instanceof Error ? error.message : "Unexpected error" };
});

const llamaId = "a".repeat(32);
const deepseekId = "b".repeat(32);

function catalog(): aiIpc.AiProviderCatalogEntry[] {
  return [
    {
      providerKind: "llama_cpp", displayName: "llama.cpp / Unsloth GGUF",
      description: "Connect to an existing llama.cpp server.", implementationStatus: "available",
      defaultBaseUrl: "http://127.0.0.1:8080/", baseUrlEditable: true,
      authenticationRequired: false, modelIdRequired: true, modelDiscoverySupported: true,
      chatTemplatePolicy: "server_model", defaultTimeoutMs: 60_000,
    },
    {
      providerKind: "deepseek", displayName: "DeepSeek",
      description: "Connect to DeepSeek's official cloud API.", implementationStatus: "available",
      defaultBaseUrl: "https://api.deepseek.com/", baseUrlEditable: false,
      authenticationRequired: true, modelIdRequired: true, modelDiscoverySupported: true,
      chatTemplatePolicy: "provider_managed", defaultTimeoutMs: 60_000,
    },
    {
      providerKind: "openrouter", displayName: "OpenRouter",
      description: "Use an OpenRouter API key.", implementationStatus: "available",
      defaultBaseUrl: "https://openrouter.ai/api/v1/", baseUrlEditable: false,
      authenticationRequired: true, modelIdRequired: true, modelDiscoverySupported: true,
      chatTemplatePolicy: "provider_managed", defaultTimeoutMs: 60_000,
    },
  ];
}

function profileDetail(
  providerKind: "llama_cpp" | "deepseek" = "llama_cpp",
  overrides: Partial<aiIpc.AiProviderProfileDetail> = {},
): aiIpc.AiProviderProfileDetail {
  const cloud = providerKind === "deepseek";
  return {
    id: cloud ? deepseekId : llamaId,
    displayName: cloud ? "DeepSeek course model" : "Local nursing model",
    providerKind,
    providerLabel: cloud ? "DeepSeek" : "llama.cpp / Unsloth GGUF",
    baseUrl: cloud ? "https://api.deepseek.com/" : "http://127.0.0.1:8080/",
    baseUrlEditable: !cloud,
    modelId: cloud ? "deepseek-v4-flash" : "nursing-qwen",
    authMode: cloud ? "bearer" : "none", enabled: true, allowInsecureHttp: false,
    requestTimeoutMs: 60_000, credentialEpoch: cloud ? 1 : 0,
    networkScope: cloud ? "cloud" : "local",
    credentialStatus: {
      status: cloud ? "saved_securely" : "not_required",
      authenticationRequired: cloud, securePersistenceAvailable: true, sessionOnlyAvailable: true,
    },
    configurationStatus: "ready", revision: 1, createdAt: 1, updatedAt: 1,
    ...overrides,
  };
}

function summary(detail: aiIpc.AiProviderProfileDetail): aiIpc.AiProviderProfileSummary {
  return {
    id: detail.id, displayName: detail.displayName, providerKind: detail.providerKind,
    providerLabel: detail.providerLabel, modelId: detail.modelId, enabled: detail.enabled,
    networkScope: detail.networkScope, credentialStatus: detail.credentialStatus,
    configurationStatus: detail.configurationStatus, revision: detail.revision, updatedAt: detail.updatedAt,
  };
}

function renderSettings(onDirtyChange = vi.fn(), showToast = vi.fn()) {
  render(
    <OverlayProvider>
      <SettingsScreen
        settings={makeSettings()} onPreviewSettings={vi.fn()} onSavedSettings={vi.fn()}
        onDirtyChange={onDirtyChange} showToast={showToast}
      />
    </OverlayProvider>,
  );
  return { onDirtyChange, showToast };
}

async function openAiTab(): Promise<void> {
  fireEvent.click(screen.getByRole("tab", { name: "AI/LLM" }));
  await screen.findByRole("heading", { name: "AI models" });
}

function modelList(profileId: string, revision: number, providerKind: "llama_cpp" | "deepseek") {
  return {
    profileId, profileRevision: revision, providerKind,
    models: providerKind === "deepseek"
      ? [
          { id: "deepseek-v4-flash", displayName: "deepseek-v4-flash", ownedBy: "deepseek" },
          { id: "deepseek-v4-pro", displayName: "deepseek-v4-pro", ownedBy: "deepseek" },
        ]
      : [{ id: "nursing-qwen", displayName: "nursing-qwen", ownedBy: "local" }],
    retrievedAt: 2,
  } as aiIpc.AiProviderModels;
}

describe("SettingsScreen simplified AI setup", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(settingsIpc.getAppInfo).mockResolvedValue({
      appVersion: "0.1.0", dataDirectory: "/tmp/ostudy/data",
      databasePath: "/tmp/ostudy/data/library.sqlite3", imageDirectory: "/tmp/ostudy/data/assets/images",
      databaseSchemaVersion: 16,
    });
    vi.mocked(settingsIpc.saveSettings).mockImplementation(async (value) => value);
    vi.mocked(aiIpc.getAiProviderCatalog).mockResolvedValue(catalog());
    vi.mocked(aiIpc.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null });
    vi.mocked(aiIpc.getAiProviderProbeResult).mockResolvedValue(null);
    vi.mocked(aiIpc.setFlashcardGenerationProfile).mockImplementation(async (profileId) => ({
      flashcardGenerationProfileId: profileId,
    }));
    vi.mocked(aiIpc.listAiProviderModels).mockImplementation(async (profileId, revision) =>
      modelList(profileId, revision, profileId === deepseekId ? "deepseek" : "llama_cpp"),
    );
  });

  it("shows only service, connection/key, and model in the normal setup path", async () => {
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    renderSettings();
    await openAiTab();
    fireEvent.click(screen.getByRole("button", { name: "Add AI model" }));

    expect(screen.getByRole("combobox", { name: "1. Service" })).toBeTruthy();
    expect(screen.getByRole("textbox", { name: "2. Server address" })).toBeTruthy();
    expect(screen.getByLabelText("API key (optional)")).toBeTruthy();
    expect(screen.queryByText("Bearer token")).toBeNull();
    expect(screen.queryByText("Enable profile")).toBeNull();
    expect(screen.queryByText("Inactivity timeout")).toBeNull();
    expect(screen.getByLabelText("Show advanced settings")).toBeTruthy();
  });

  it("connects a local server and automatically adopts its sole discovered model", async () => {
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    const created = profileDetail("llama_cpp", { modelId: "", configurationStatus: "incomplete" });
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(created);
    renderSettings();
    await openAiTab();
    fireEvent.click(screen.getByRole("button", { name: "Add AI model" }));
    fireEvent.click(screen.getByRole("button", { name: "Connect and retrieve models" }));

    await waitFor(() => {
      expect(aiIpc.createAiProviderProfile).toHaveBeenCalledWith(
        expect.objectContaining({ providerKind: "llama_cpp", authMode: "none", enabled: true }),
      );
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(llamaId, 1);
    });
    const model = await screen.findByRole("combobox", { name: "3. Model" });
    expect((model as HTMLSelectElement).value).toBe("nursing-qwen");
    expect(screen.getByRole("button", { name: "Save model" })).toBeTruthy();
  });

  it("saves a cloud API key natively before exposing its model choices", async () => {
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
    const created = profileDetail("deepseek", {
      modelId: "", credentialEpoch: 0, configurationStatus: "incomplete",
      credentialStatus: {
        status: "missing", authenticationRequired: true,
        securePersistenceAvailable: true, sessionOnlyAvailable: true,
      },
    });
    const credentialSaved = profileDetail("deepseek", { modelId: "", revision: 2 });
    vi.mocked(aiIpc.createAiProviderProfile).mockResolvedValue(created);
    vi.mocked(aiIpc.setAiProviderCredential).mockResolvedValue(credentialSaved);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(credentialSaved);
    renderSettings();
    await openAiTab();
    fireEvent.click(screen.getByRole("button", { name: "Add AI model" }));
    fireEvent.change(screen.getByRole("combobox", { name: "1. Service" }), {
      target: { value: "deepseek" },
    });
    const key = screen.getByLabelText("2. API key");
    fireEvent.change(key, { target: { value: "sentinel-secret" } });
    fireEvent.click(screen.getByRole("button", { name: "Connect and retrieve models" }));

    await waitFor(() => {
      expect(aiIpc.setAiProviderCredential).toHaveBeenCalledWith(
        deepseekId, 1, "sentinel-secret", "secure",
      );
      expect(aiIpc.listAiProviderModels).toHaveBeenCalledWith(deepseekId, 2);
    });
    expect((screen.getByLabelText("2. API key") as HTMLInputElement).value).toBe("");
    expect(screen.queryByText("sentinel-secret")).toBeNull();
    expect(await screen.findByRole("option", { name: "deepseek-v4-pro" })).toBeTruthy();
  });

  it("never reads a persisted credential back into the API-key field", async () => {
    const detail = profileDetail("deepseek");
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    renderSettings();
    await openAiTab();

    const key = await screen.findByLabelText("2. API key");
    expect((key as HTMLInputElement).value).toBe("");
    expect(key.getAttribute("placeholder")).toMatch(/Saved/u);
  });

  it("keeps advanced connection and generation policy hidden until requested", async () => {
    const detail = profileDetail("llama_cpp");
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    renderSettings();
    await openAiTab();
    await screen.findByRole("combobox", { name: "3. Model" });

    expect(screen.queryByText("Inactivity timeout")).toBeNull();
    fireEvent.click(screen.getByLabelText("Show advanced settings"));
    expect(screen.getByText("Inactivity timeout")).toBeTruthy();
    expect(screen.getByText("API-key storage")).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Document-generation policy" })).toBeTruthy();
  });

  it("renders connection verification as a compact summary with expandable phases", async () => {
    const detail = profileDetail("llama_cpp");
    vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([summary(detail)]);
    vi.mocked(aiIpc.getAiProviderProfile).mockResolvedValue(detail);
    vi.mocked(aiIpc.testAiProviderProfile).mockResolvedValue({
      profileId: llamaId, profileRevision: 1, credentialEpoch: 0,
      testedModelId: "nursing-qwen", adapterRevision: 3, resultSchemaVersion: 1,
      overallStatus: "verified",
      phases: [
        { phase: "model_discovery", label: "Model discovery", status: "passed", message: "Available.", latencyMs: 18, error: null },
        { phase: "structured_output", label: "Structured output", status: "passed", message: "Schema verified.", latencyMs: 42, error: null },
      ],
      capabilities: [], testedAt: 3, persisted: true, stale: false,
    });
    renderSettings();
    await openAiTab();
    await screen.findByRole("combobox", { name: "3. Model" });
    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));

    expect(await screen.findByText("Connected · structured output verified")).toBeTruthy();
    expect(screen.getByText(/42 ms/u)).toBeTruthy();
    expect(screen.getByText("Details")).toBeTruthy();
    expect(screen.queryByLabelText("Connection diagnostic log")).toBeNull();
  });
});
'''
        ),
    )


def write_policy_test() -> None:
    write(
        "src/ui/uiAiOverhaulPolicy.test.ts",
        textwrap.dedent(
            '''import fs from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

function source(relativePath: string): string {
  return fs.readFileSync(path.resolve(process.cwd(), relativePath), "utf8");
}

describe("responsive AI-assisted workflow policy", () => {
  it("keeps card creation deck-local and removes density switching", () => {
    expect(source("src/navigation/flashcardsRoutes.ts")).not.toContain('"create"');
    expect(source("src/ipc/settings.ts")).not.toContain("deckViewDensity");
    expect(source("src/screens/DecksScreen.tsx")).toContain("Bulk add");
  });

  it("keeps AI setup simple while retaining an advanced disclosure", () => {
    const ai = source("src/screens/settings/AiLlmSettings.tsx");
    expect(ai).toContain("Connect and retrieve models");
    expect(ai).toContain("Show advanced settings");
    expect(ai).not.toContain("Bearer token");
    expect(ai).not.toContain("Enable profile");
  });

  it("keeps tutorials manual and generation semantically batched", () => {
    expect(source("src/ui/tutorials/TourProvider.tsx")).not.toContain("findContextualModule");
    const native = source("src-tauri/src/ai/document_intelligence.rs");
    expect(native).toContain("DocumentLearningTarget");
    expect(native).toContain("document_generation_target_gap_pass_started");
    expect(native).toContain("prompt_policy_hash");
  });
});
'''
        ),
    )


def write_installation_doc() -> None:
    write(
        "docs/installation/building-from-source.md",
        textwrap.dedent(
            '''# Building and installing ostudy from source

## Shared preparation

```bash
git fetch origin --prune
git switch agent/ui-ai-semantic-generation-overhaul
git pull --ff-only
corepack enable
pnpm install --frozen-lockfile
```

## Linux desktop

```bash
./scripts/install-native-deps.sh
./scripts/check-fast.sh
./scripts/check.sh
./run-app.sh
```

Build release bundles with `pnpm tauri build`. Tauri writes distribution-specific packages under `src-tauri/target/release/bundle/` and prints their exact paths.

## Android

With the Android SDK, JDK, Rust Android targets, emulator/device, and USB debugging configured:

```bash
./scripts/android-test.sh
./scripts/android-emulator-smoke.sh --artifacts /tmp/ostudy-android-smoke
pnpm tauri android build --apk
```

Install the APK printed by the build with `adb install -r <apk-path>`. Retrieve the debug app's native JSONL diagnostics with `pnpm android:diagnostics`; use `adb logcat` for Android Activity, camera, package, or native-crash diagnostics.

## Manual acceptance

Verify root-reset navigation, deck-local card creation, compact phone Q/A previews, tablet portrait/landscape layout, simple-material/no-motion e-ink behavior, truthful practice workload state, manual tutorials, simplified and advanced AI setup, connection testing, prompt-policy persistence, and sparse/dense document generation.
'''
        ),
    )
    index = ROOT / "docs/README.md"
    if index.exists():
        text = index.read_text()
        link = "[Building and installing from source](installation/building-from-source.md)"
        if link not in text:
            index.write_text(text.rstrip() + f"\n\n- {link}\n")


def cleanup_retired_sources() -> None:
    create_workspace = ROOT / "src/screens/flashcards/CreateCardsWorkspace.tsx"
    if create_workspace.exists():
        create_workspace.unlink()
    for file in (ROOT / "src").rglob("*"):
        if file.suffix not in {".ts", ".tsx"}:
            continue
        original = file.read_text()
        updated = re.sub(r'^\s*deckViewDensity:.*\n', "", original, flags=re.M)
        if updated != original:
            file.write_text(updated)


def fail_on_remaining_test_failures(report_path: pathlib.Path) -> None:
    report = json.loads(report_path.read_text())
    failures: list[str] = []
    for result in report.get("testResults", []):
        file_name = result.get("name", "unknown")
        try:
            display = str(pathlib.Path(file_name).relative_to(ROOT))
        except (ValueError, TypeError):
            display = file_name
        for assertion in result.get("assertionResults", []):
            if assertion.get("status") == "failed":
                failures.append(f"- {display}: {assertion.get('title') or assertion.get('fullName') or 'unknown test'}")
    if failures:
        raise RuntimeError(
            "PR #55 still has real frontend regressions; refusing to delete or suppress them:\n"
            + "\n".join(failures)
        )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vitest-json", type=pathlib.Path)
    args = parser.parse_args()
    cleanup_retired_sources()
    patch_product_fixes()
    patch_practice_tests()
    patch_settings_smoke_test()
    write_settings_extended_tests()
    write_policy_test()
    write_installation_doc()
    if args.vitest_json and args.vitest_json.exists():
        fail_on_remaining_test_failures(args.vitest_json)


if __name__ == "__main__":
    main()
