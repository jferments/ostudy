#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re
import textwrap

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


def replace_it(path: str, title: str, replacement: str) -> None:
    text = read(path)
    pattern = re.compile(
        r'  it\("' + re.escape(title) + r'",(?: async)? \(\) => \{.*?\n  \}\);',
        re.S,
    )
    updated, count = pattern.subn(textwrap.dedent(replacement).rstrip(), text, count=1)
    if count != 1:
        raise RuntimeError(f"{path}: could not replace {title!r}")
    write(path, updated)


# Avoid Rust -D warnings and make test literals independent of local imports.
setup = ROOT / "src-tauri/src/ai/setup.rs"
if setup.exists():
    source = setup.read_text().replace("use std::collections::HashMap;\n\n", "", 1)
    setup.write_text(source)
for file in (ROOT / "src-tauri").rglob("*.rs"):
    source = file.read_text()
    updated = source.replace(
        "learning_target_exclusions: HashSet::new(),",
        "learning_target_exclusions: std::collections::HashSet::new(),",
    )
    if updated != source:
        file.write_text(updated)

# Every explicit Vitest module mock must provide the three new narrow setup IPC
# functions imported by the controller. This is test infrastructure only; the
# product never falls back to the old durable-profile setup path.
for file in (ROOT / "src").rglob("*.test.ts*"):
    source = file.read_text()
    anchor = 'vi.mock("../ipc/aiProfiles", () => ({\n'
    if anchor in source and "beginAiProviderSetup: vi.fn()" not in source:
        source = source.replace(
            anchor,
            anchor
            + "  beginAiProviderSetup: vi.fn(),\n"
            + "  cancelAiProviderSetup: vi.fn(),\n"
            + "  commitAiProviderSetup: vi.fn(),\n",
            1,
        )
    file.write_text(source)

# Controller object literals created by focused component tests need the new
# ephemeral-session state field after the final test-generation pass.
for path in [
    "src/screens/settings/AiLlmSettings.test.tsx",
    "src/screens/settings/DefaultAiModelSettings.test.tsx",
]:
    target = ROOT / path
    if not target.exists():
        continue
    source = target.read_text()
    source = source.replace(
        "probeResult: null, dirty:",
        "probeResult: null, setupId: null, dirty:",
    )
    target.write_text(source)

# The simplified Settings integration tests now assert the real ephemeral setup
# boundary: connection/model discovery does not create an incomplete profile or
# persist an API key. The profile is committed only after Save model.
extended = ROOT / "src/screens/SettingsScreen.extended.test.tsx"
if extended.exists():
    path = str(extended.relative_to(ROOT))
    replace_it(
        path,
        "connects a local server and automatically adopts its sole discovered model",
        '''
          it("discovers a local model without writing an incomplete durable profile", async () => {
            vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
            vi.mocked(aiIpc.beginAiProviderSetup).mockResolvedValue({
              setupId: "c".repeat(32),
              models: [{ id: "nursing-qwen", displayName: "nursing-qwen", ownedBy: "local" }],
            });
            renderSettings();
            await openAiTab();
            fireEvent.click(screen.getByRole("button", { name: "Add AI model" }));
            fireEvent.click(screen.getByRole("button", { name: "Connect and retrieve models" }));

            await waitFor(() => {
              expect(aiIpc.beginAiProviderSetup).toHaveBeenCalledWith(
                expect.objectContaining({
                  providerKind: "llama_cpp",
                  baseUrl: "http://127.0.0.1:8080/",
                  credential: "",
                }),
              );
            });
            expect(aiIpc.createAiProviderProfile).not.toHaveBeenCalled();
            const model = await screen.findByRole("combobox", { name: "3. Model" });
            expect((model as HTMLSelectElement).value).toBe("nursing-qwen");
            expect(screen.getByRole("button", { name: "Save model" })).toBeTruthy();
          });
        ''',
    )
    replace_it(
        path,
        "saves a cloud API key natively before exposing its model choices",
        '''
          it("holds a cloud API key only inside the native setup session during discovery", async () => {
            vi.mocked(aiIpc.listAiProviderProfiles).mockResolvedValue([]);
            vi.mocked(aiIpc.beginAiProviderSetup).mockResolvedValue({
              setupId: "c".repeat(32),
              models: [
                { id: "deepseek-v4-flash", displayName: "deepseek-v4-flash", ownedBy: "deepseek" },
                { id: "deepseek-v4-pro", displayName: "deepseek-v4-pro", ownedBy: "deepseek" },
              ],
            });
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
              expect(aiIpc.beginAiProviderSetup).toHaveBeenCalledWith(
                expect.objectContaining({
                  providerKind: "deepseek",
                  credential: "sentinel-secret",
                  persistence: "secure",
                }),
              );
            });
            expect(aiIpc.createAiProviderProfile).not.toHaveBeenCalled();
            expect(aiIpc.setAiProviderCredential).not.toHaveBeenCalled();
            expect((screen.getByLabelText("2. API key") as HTMLInputElement).value).toBe("");
            expect(screen.queryByText("sentinel-secret")).toBeNull();
            expect(await screen.findByRole("option", { name: "deepseek-v4-pro" })).toBeTruthy();
          });
        ''',
    )

# The hook test imports the new functions even when it exercises only an already
# persisted profile; its module mock still must expose them.
controller_test = ROOT / "src/controllers/aiProfilesController.test.tsx"
if controller_test.exists():
    source = controller_test.read_text()
    anchor = 'vi.mock("../ipc/aiProfiles", () => ({\n'
    if anchor in source and "beginAiProviderSetup: vi.fn()" not in source:
        source = source.replace(
            anchor,
            anchor
            + "  beginAiProviderSetup: vi.fn(),\n"
            + "  cancelAiProviderSetup: vi.fn(),\n"
            + "  commitAiProviderSetup: vi.fn(),\n",
            1,
        )
    controller_test.write_text(source)

# Strengthen the static verifier so future refactors cannot silently move the
# policy back into generic settings or drop the durable coverage/setup boundary.
verifier = ROOT / "scripts/verify-ui-ai-overhaul.mjs"
if verifier.exists():
    source = verifier.read_text()
    marker = "console.log("
    checks = r'''
const nativeSetup = fs.readFileSync("src-tauri/src/ai/setup.rs", "utf8");
const nativeSettings = fs.readFileSync("src-tauri/src/settings.rs", "utf8");
const migration2 = fs.readFileSync("src-tauri/migrations/0002_ai_task_policy_learning_coverage.sql", "utf8");
const documentAi = fs.readFileSync("src-tauri/src/ai/document_intelligence.rs", "utf8");
assert(nativeSetup.includes("begin_ai_provider_setup") && nativeSetup.includes("commit_ai_provider_setup"), "AI setup must remain ephemeral until model commit");
assert(migration2.includes("CREATE TABLE ai_task_policies") && migration2.includes("CREATE TABLE ai_prompt_presets"), "AI task policy and prompt presets require dedicated native tables");
assert(migration2.includes("CREATE TABLE document_learning_target_coverage"), "semantic target coverage must be durable");
assert(nativeSettings.includes("save_ai_policy_in_transaction") && !nativeSettings.includes('(\"aiPromptPresets\", serde_json::to_string'), "AI task policy must not be persisted as generic app_settings rows");
assert(documentAi.includes("load_covered_learning_target_ids") && documentAi.includes("persist_learning_target_coverage"), "Generate More must use durable semantic coverage");

'''
    if "AI setup must remain ephemeral until model commit" not in source:
        index = source.find(marker)
        if index < 0:
            source += "\n" + checks
        else:
            source = source[:index] + checks + source[index:]
    verifier.write_text(source)
