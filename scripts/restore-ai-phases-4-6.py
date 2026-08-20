#!/usr/bin/env python3
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]


def replace_once(path: str, old: str, new: str, label: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if new in text:
        return
    if old not in text:
        raise SystemExit(f"{path}: missing anchor for {label}")
    target.write_text(text.replace(old, new, 1), encoding="utf-8")


def remove_once(path: str, old: str, label: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if old not in text:
        return
    target.write_text(text.replace(old, "", 1), encoding="utf-8")


def regex_once(path: str, pattern: str, replacement: str, label: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.S)
    if count != 1:
        raise SystemExit(f"{path}: expected one match for {label}, found {count}")
    target.write_text(updated, encoding="utf-8")


def remove_lines_containing(path: str, needles: tuple[str, ...]) -> None:
    target = ROOT / path
    lines = target.read_text(encoding="utf-8").splitlines(keepends=True)
    target.write_text(
        "".join(line for line in lines if not any(needle in line for needle in needles)),
        encoding="utf-8",
    )


# Phase 5 moves document reasoning out of device-wide AppSettings. Keep the
# clean Phase 1-3 controller instead of overwriting its navigation work.
remove_once(
    "src/controllers/useAppController.ts",
    '  aiReasoningEffort: "low",\n',
    "legacy default AI reasoning setting",
)
remove_once(
    "src/controllers/useAppController.ts",
    '    aiReasoningEffort: settings.aiReasoningEffort ?? "low",\n',
    "legacy normalized AI reasoning setting",
)

# Remove reasoning from the ordinary frontend settings contract. The value now
# belongs exclusively to the versioned document-generation task policy.
regex_once(
    "src/ipc/settings.ts",
    r'export type AiReasoningEffort =\n(?:  \| .*\n)+\n',
    "",
    "frontend legacy reasoning type",
)
regex_once(
    "src/ipc/settings.ts",
    r'  /\*\* Native settings always return this; optional keeps older fixture/build callers source-compatible\. \*/\n  aiReasoningEffort\?: AiReasoningEffort;\n',
    "",
    "frontend legacy reasoning field",
)
remove_once(
    "src/screens/SettingsScreen.tsx",
    '  aiReasoningEffort: "low",\n',
    "settings default reasoning",
)
replace_once(
    "src/screens/SettingsScreen.tsx",
    '      const saved = await saveSettings({\n        ...draft,\n        aiReasoningEffort: draft.aiReasoningEffort ?? "low",\n      });',
    '      const saved = await saveSettings(draft);',
    "settings persistence reasoning",
)

# Native AppSettings no longer owns document reasoning. The legacy database key
# is migrated into AiTaskPolicy by policy.rs below before it is deleted.
settings_path = ROOT / "src-tauri/src/settings.rs"
settings_text = settings_path.read_text(encoding="utf-8")
settings_text = settings_text.replace("use std::sync::{LazyLock, RwLock};\n\n", "")
settings_text = re.sub(
    r'const AI_REASONING_EFFORTS: &\[&str\] = &\[.*?\];\nconst DEFAULT_AI_REASONING_EFFORT: &str = "low";\n',
    "",
    settings_text,
    count=1,
    flags=re.S,
)
settings_text = re.sub(
    r'static ACTIVE_AI_REASONING_EFFORT: LazyLock<RwLock<String>> =\n    LazyLock::new\(\|\| RwLock::new\(DEFAULT_AI_REASONING_EFFORT\.to_string\(\)\)\);\n\n',
    "",
    settings_text,
    count=1,
)
settings_text = settings_text.replace("    pub ai_reasoning_effort: String,\n", "")
settings_text = settings_text.replace("    publish_ai_reasoning_effort(&input.ai_reasoning_effort);\n", "")
settings_text = settings_text.replace(
    "    let ai_reasoning_effort = load_ai_reasoning_effort(connection)?;\n    publish_ai_reasoning_effort(&ai_reasoning_effort);\n",
    "",
)
settings_text = settings_text.replace("        ai_reasoning_effort,\n", "")
settings_text = re.sub(
    r'pub\(crate\) fn load_ai_reasoning_effort\(.*?\n}\n\nfn legacy_motion_preference',
    "fn legacy_motion_preference",
    settings_text,
    count=1,
    flags=re.S,
)
settings_text = re.sub(
    r'        \(\n            "aiReasoningEffort",\n            serde_json::to_string\(&settings\.ai_reasoning_effort\),\n        \),\n',
    "",
    settings_text,
    count=1,
)
settings_text = re.sub(
    r'    validate_choice\(\n        &settings\.ai_reasoning_effort,\n        AI_REASONING_EFFORTS,\n        "invalid_ai_reasoning_effort",\n        "Select an available document AI reasoning level\.",\n    \)\?;\n',
    "",
    settings_text,
    count=1,
)
settings_path.write_text(settings_text, encoding="utf-8")
remove_lines_containing(
    "src-tauri/src/settings.rs",
    ("ai_reasoning_effort", "current_ai_reasoning_effort"),
)
remove_lines_containing(
    "src-tauri/src/settings/extended_tests.rs",
    ("ai_reasoning_effort", '"aiReasoningEffort"'),
)

# Migrate the old AppSettings reasoning value into the task policy on first
# policy load, then remove the obsolete key so there is one source of truth.
replace_once(
    "src-tauri/src/ai/policy.rs",
    '        None => {\n            let reasoning_effort = load_setting_json::<String>(connection, LEGACY_REASONING_KEY)?\n                .unwrap_or_else(|| "low".to_string());\n            AiTaskPolicyDto {\n                schema_version: POLICY_SCHEMA_VERSION,\n                task_kind: TASK_KIND.to_string(),\n                profile_id: default_profile_id.clone(),\n                reasoning_effort,\n                context_limit_mode: ContextLimitMode::Automatic,\n                manual_context_limit_tokens: None,\n                image_input_policy: ImageInputPolicy::Automatic,\n                coverage_mode: CoverageMode::Balanced,\n                prompt_preset_id: None,\n                revision: 1,\n                updated_at: 0,\n            }\n        }',
    '        None => {\n            let reasoning_effort = load_setting_json::<String>(connection, LEGACY_REASONING_KEY)?\n                .unwrap_or_else(|| "low".to_string());\n            let now = unix_time_millis()?;\n            let migrated = AiTaskPolicyDto {\n                schema_version: POLICY_SCHEMA_VERSION,\n                task_kind: TASK_KIND.to_string(),\n                profile_id: default_profile_id.clone(),\n                reasoning_effort,\n                context_limit_mode: ContextLimitMode::Automatic,\n                manual_context_limit_tokens: None,\n                image_input_policy: ImageInputPolicy::Automatic,\n                coverage_mode: CoverageMode::Balanced,\n                prompt_preset_id: None,\n                revision: 1,\n                updated_at: now,\n            };\n            save_setting_json(connection, TASK_POLICY_KEY, &migrated, now)?;\n            connection\n                .execute("DELETE FROM app_settings WHERE key = ?1", [LEGACY_REASONING_KEY])\n                .map_err(CommandError::database)?;\n            migrated\n        }',
    "legacy reasoning migration",
)

# The recovered historical implementation populated the per-profile routing
# policy everywhere ProfileSnapshot is constructed, but one intermediate commit
# omitted the field from the struct declaration itself.
replace_once(
    "src-tauri/src/ai/profiles.rs",
    "    pub require_zero_data_retention: bool,\n    pub request_timeout_ms: i64,\n",
    "    pub require_zero_data_retention: bool,\n    pub open_router_routing_policy: OpenRouterRoutingPolicyDto,\n    pub request_timeout_ms: i64,\n",
    "ProfileSnapshot OpenRouter routing policy",
)

# The old Settings tests describe the removed create/save/profile-editor flow.
# Keep non-AI settings coverage and replace the obsolete AI file with the new
# transactional setup/prompt contract.
settings_test = ROOT / "src/screens/SettingsScreen.test.tsx"
settings_test_text = settings_test.read_text(encoding="utf-8")
settings_test_text, removed = re.subn(
    r'\n  it\("uses one New profile action and selects provider type inside the connection form", async \(\) => \{.*?\n  \}\);\n',
    "\n",
    settings_test_text,
    count=1,
    flags=re.S,
)
if removed != 1:
    raise SystemExit("SettingsScreen.test.tsx: failed to remove obsolete AI profile-editor test")
settings_test.write_text(settings_test_text, encoding="utf-8")

(ROOT / "src/screens/SettingsScreen.extended.test.tsx").write_text(r'''import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as aiPolicy from "../ipc/aiPolicy";
import * as aiProfiles from "../ipc/aiProfiles";
import * as aiSetup from "../ipc/aiSetup";
import { AiModelSetupSettings } from "./settings/AiModelSetupSettings";

vi.mock("../ipc/aiProfiles", () => ({
  getAiProviderCatalog: vi.fn(),
  listAiProviderProfiles: vi.fn(),
}));
vi.mock("../ipc/aiPolicy", () => ({
  createAiPromptPreset: vi.fn(),
  deleteAiPromptPreset: vi.fn(),
  getAiTaskPolicy: vi.fn(),
  listAiPromptPresets: vi.fn(),
  saveAiTaskPolicy: vi.fn(),
  updateAiPromptPreset: vi.fn(),
}));
vi.mock("../ipc/aiSetup", () => ({
  beginAiProfileSetup: vi.fn(),
  cancelAiProfileSetup: vi.fn(),
  clearAiSetupCredential: vi.fn(),
  commitAiProfileSetup: vi.fn(),
  listAiSetupModels: vi.fn(),
  setAiSetupCredential: vi.fn(),
  testAiSetup: vi.fn(),
  updateAiProfileSetup: vi.fn(),
}));

const setupId = "s".repeat(32);
const profileId = "p".repeat(32);
const promptId = "q".repeat(32);

const taskPolicy: aiPolicy.AiTaskPolicy = {
  schemaVersion: 1,
  taskKind: "flashcard_generation",
  profileId: null,
  reasoningEffort: "low",
  contextLimitMode: "automatic",
  manualContextLimitTokens: null,
  imageInputPolicy: "automatic",
  coverageMode: "balanced",
  promptPresetId: null,
  revision: 1,
  updatedAt: 1,
};

function setupSession(
  providerKind: aiProfiles.AiProviderKind = "llama_cpp",
): aiSetup.AiSetupSession {
  const cloud = providerKind !== "llama_cpp";
  return {
    id: setupId,
    providerKind,
    providerLabel: cloud ? "DeepSeek" : "llama.cpp / Unsloth GGUF",
    baseUrl: cloud ? "https://api.deepseek.com/" : "http://127.0.0.1:8080/",
    baseUrlEditable: !cloud,
    modelId: "",
    authenticationRequired: cloud,
    credentialPresent: false,
    credentialPersistence: "secure",
    allowInsecureHttp: false,
    requestTimeoutMs: 60_000,
    openRouterRoutingPolicy: {
      mode: "automatic",
      providerSlugs: [],
      allowFallbacks: true,
      requireZeroDataRetention: false,
    },
    networkScope: cloud ? "cloud" : "loopback",
    createdAt: 1,
    updatedAt: 1,
  };
}

function renderAi(onDirtyChange = vi.fn(), showToast = vi.fn()) {
  render(<AiModelSetupSettings onDirtyChange={onDirtyChange} showToast={showToast} />);
  return { onDirtyChange, showToast };
}

describe("AI model setup and generation policy", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(aiProfiles.getAiProviderCatalog).mockResolvedValue([
      {
        providerKind: "llama_cpp",
        displayName: "llama.cpp / Unsloth GGUF",
        description: "Local server",
        implementationStatus: "available",
        defaultBaseUrl: "http://127.0.0.1:8080/",
        baseUrlEditable: true,
        authenticationRequired: false,
        modelIdRequired: true,
        modelDiscoverySupported: true,
        chatTemplatePolicy: "server_model",
        defaultTimeoutMs: 60_000,
      },
      {
        providerKind: "deepseek",
        displayName: "DeepSeek",
        description: "Cloud API",
        implementationStatus: "available",
        defaultBaseUrl: "https://api.deepseek.com/",
        baseUrlEditable: false,
        authenticationRequired: true,
        modelIdRequired: true,
        modelDiscoverySupported: true,
        chatTemplatePolicy: "provider_managed",
        defaultTimeoutMs: 60_000,
      },
    ]);
    vi.mocked(aiProfiles.listAiProviderProfiles).mockResolvedValue([]);
    vi.mocked(aiPolicy.getAiTaskPolicy).mockResolvedValue(taskPolicy);
    vi.mocked(aiPolicy.listAiPromptPresets).mockResolvedValue([]);
    vi.mocked(aiPolicy.saveAiTaskPolicy).mockImplementation(async (input) => ({
      ...taskPolicy,
      ...input,
      revision: input.revision + 1,
      updatedAt: 2,
    }));
    vi.mocked(aiSetup.beginAiProfileSetup).mockImplementation(async (providerKind) =>
      setupSession(providerKind),
    );
    vi.mocked(aiSetup.updateAiProfileSetup).mockImplementation(async (input) => ({
      ...setupSession("llama_cpp"),
      ...input,
      id: input.setupId,
    }));
    vi.mocked(aiSetup.setAiSetupCredential).mockImplementation(
      async (_id, _credential, persistence) => ({
        ...setupSession("llama_cpp"),
        credentialPresent: true,
        credentialPersistence: persistence,
      }),
    );
    vi.mocked(aiSetup.clearAiSetupCredential).mockResolvedValue(setupSession("llama_cpp"));
    vi.mocked(aiSetup.listAiSetupModels).mockResolvedValue({
      profileId: setupId,
      profileRevision: 1,
      providerKind: "llama_cpp",
      models: [
        {
          id: "nursing-qwen",
          displayName: "Nursing Qwen",
          ownedBy: "local",
          contextWindowTokens: 131_072,
          reasoning: null,
        },
      ],
      retrievedAt: 1,
    });
    vi.mocked(aiSetup.testAiSetup).mockResolvedValue({
      setupId,
      modelId: "nursing-qwen",
      overallStatus: "verified",
      message: "Connection verified.",
      structuredOutputVerified: true,
      latencyMs: 42,
      testedAt: 1,
    });
    vi.mocked(aiSetup.commitAiProfileSetup).mockResolvedValue({
      id: profileId,
      displayName: "Nursing Qwen",
      providerKind: "llama_cpp",
      providerLabel: "llama.cpp / Unsloth GGUF",
      baseUrl: "http://127.0.0.1:8080/",
      baseUrlEditable: true,
      modelId: "nursing-qwen",
      authMode: "none",
      enabled: true,
      allowInsecureHttp: false,
      requireZeroDataRetention: false,
      requestTimeoutMs: 60_000,
      credentialEpoch: 0,
      networkScope: "local",
      credentialStatus: {
        status: "not_required",
        authenticationRequired: false,
        securePersistenceAvailable: true,
        sessionOnlyAvailable: false,
      },
      configurationStatus: "ready",
      revision: 1,
      createdAt: 1,
      updatedAt: 1,
    });
    vi.mocked(aiSetup.cancelAiProfileSetup).mockResolvedValue(true);
    vi.mocked(aiPolicy.createAiPromptPreset).mockResolvedValue({
      id: promptId,
      name: "NCLEX emphasis",
      outlineInstructions: "Emphasize mechanisms.",
      cardGenerationInstructions: "Prioritize nursing interventions.",
      revision: 1,
      createdAt: 1,
      updatedAt: 1,
      contentHash: "a".repeat(64),
      estimatedTokens: 12,
    });
  });

  it("uses a transactional service-key-model setup without legacy profile jargon", async () => {
    const { onDirtyChange } = renderAi();
    expect(await screen.findByRole("heading", { name: "AI models" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Add AI model" }));

    const service = await screen.findByRole("combobox", { name: "Service" });
    expect((service as HTMLSelectElement).value).toBe("llama_cpp");
    expect(screen.getByLabelText(/API key/u)).toBeTruthy();
    expect(screen.queryByText(/Bearer token/u)).toBeNull();
    expect(screen.queryByText(/Enable profile/u)).toBeNull();
    expect(screen.queryByText(/Profile name/u)).toBeNull();
    await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(true));

    fireEvent.click(screen.getByRole("button", { name: "Load models" }));
    const model = await screen.findByRole("combobox", { name: "Model" });
    await waitFor(() => expect((model as HTMLSelectElement).value).toBe("nursing-qwen"));
    fireEvent.click(screen.getByRole("button", { name: "Test connection" }));
    expect(await screen.findByText(/Connection verified/u)).toBeTruthy();
    expect(screen.getByText(/Structured output verified · 42 ms/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save model" }));
    await waitFor(() => expect(aiSetup.commitAiProfileSetup).toHaveBeenCalledWith(setupId));
  });

  it("cancels an in-progress setup without committing a profile", async () => {
    renderAi();
    await screen.findByRole("heading", { name: "AI models" });
    fireEvent.click(screen.getByRole("button", { name: "Add AI model" }));
    await screen.findByRole("combobox", { name: "Service" });
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(aiSetup.cancelAiProfileSetup).toHaveBeenCalledWith(setupId));
    expect(aiSetup.commitAiProfileSetup).not.toHaveBeenCalled();
  });

  it("creates reusable prompt presets while keeping fixed safety contracts visible", async () => {
    vi.mocked(aiPolicy.listAiPromptPresets)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([
        {
          id: promptId,
          name: "NCLEX emphasis",
          outlineInstructions: "Emphasize mechanisms.",
          cardGenerationInstructions: "Prioritize nursing interventions.",
          revision: 1,
          createdAt: 1,
          updatedAt: 1,
          contentHash: "a".repeat(64),
          estimatedTokens: 12,
        },
      ]);
    renderAi();
    await screen.findByRole("heading", { name: "Advanced document generation" });
    fireEvent.click(screen.getByRole("button", { name: "New preset" }));
    fireEvent.change(screen.getByLabelText("Preset name"), {
      target: { value: "NCLEX emphasis" },
    });
    fireEvent.change(screen.getByLabelText("Outline instructions"), {
      target: { value: "Emphasize mechanisms." },
    });
    fireEvent.change(screen.getByLabelText("Card-generation instructions"), {
      target: { value: "Prioritize nursing interventions." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save prompt preset" }));
    await waitFor(() =>
      expect(aiPolicy.createAiPromptPreset).toHaveBeenCalledWith(
        expect.objectContaining({ name: "NCLEX emphasis" }),
      ),
    );
    expect(screen.getByText(/Source-trust boundaries/u)).toBeTruthy();
  });
});
''', encoding="utf-8")

print("Applied Phase 4-6 migration and test-contract adjustments to the clean UI head.")
