#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, value) => writeFile(join(root, path), value, "utf8");

function replaceExact(text, oldValue, newValue, label) {
  if (!text.includes(oldValue)) throw new Error(`${label}: expected source was not found`);
  return text.replace(oldValue, newValue);
}

let lib = await read("src-tauri/src/lib.rs");
lib = replaceExact(lib, "mod util;\nmod workload;", "mod ui_checkpoint;\nmod util;\nmod workload;", "register UI checkpoint module");
lib = replaceExact(
  lib,
  "            platform::get_platform_capabilities,\n            external_files::select_and_stage_external_import,",
  "            platform::get_platform_capabilities,\n            platform::set_platform_back_interception,\n            ui_checkpoint::get_ui_checkpoint,\n            ui_checkpoint::save_ui_checkpoint,\n            ui_checkpoint::clear_ui_checkpoint,\n            external_files::select_and_stage_external_import,",
  "register UI checkpoint and Back commands",
);
await write("src-tauri/src/lib.rs", lib);

let platform = await read("src-tauri/src/platform/mod.rs");
platform = replaceExact(
  platform,
  `#[cfg(test)]
pub(crate) fn unavailable_secret_store()`,
  `#[tauri::command]
pub fn set_platform_back_interception(_enabled: bool) -> Result<(), CommandError> {
    // Desktop has no hardware Back dispatcher. The Android adapter replaces
    // this no-op with native OnBackPressedDispatcher registration.
    Ok(())
}

#[cfg(test)]
pub(crate) fn unavailable_secret_store()`,
  "add portable Back interception command",
);
await write("src-tauri/src/platform/mod.rs", platform);

let controller = await read("src/controllers/useAppController.ts");
controller = replaceExact(
  controller,
  'import { getSettings, type AppSettings } from "../ipc/settings";\n',
  'import { getSettings, type AppSettings } from "../ipc/settings";\nimport { getUiCheckpoint, saveUiCheckpoint } from "../ipc/uiCheckpoint";\nimport { registerPlatformBackHandler } from "../platform/backNavigation";\n',
  "import restoration services",
);
controller = replaceExact(
  controller,
  "  useEffect(() => {\n    const token = deckRefreshLane.begin();\n    void Promise.all([getSettings(), listDecks()])\n      .then(([loadedSettings, loadedDecks]) => {",
  "  useEffect(() => {\n    const token = deckRefreshLane.begin();\n    void Promise.all([getSettings(), listDecks(), getUiCheckpoint()])\n      .then(([loadedSettings, loadedDecks, checkpoint]) => {",
  "load UI checkpoint during bootstrap",
);
controller = replaceExact(
  controller,
  "        setDecks(loadedDecks);\n        setFatalError(null);",
  "        setDecks(loadedDecks);\n        if (checkpoint) {\n          setSection(checkpoint.section);\n          if (checkpoint.section === \"analytics\" && checkpoint.analytics) {\n            const requestId = analyticsRequestSequence.current;\n            analyticsRequestSequence.current += 1;\n            setAnalyticsLaunch({\n              requestId,\n              range: checkpoint.analytics.range,\n              deckIds: checkpoint.analytics.deckIds,\n              tagIds: checkpoint.analytics.tagIds,\n            });\n          }\n        }\n        setFatalError(null);",
  "restore top-level destination",
);
const checkpointEffects = `

  useEffect(() => {
    if (isLoading || fatalError) return;
    const timer = window.setTimeout(() => {
      void saveUiCheckpoint({
        section,
        analytics:
          section === "analytics" && analyticsLaunch
            ? {
                range: analyticsLaunch.range,
                deckIds: analyticsLaunch.deckIds,
                tagIds: analyticsLaunch.tagIds,
              }
            : undefined,
      }).catch((error: unknown) => {
        console.error("Unable to persist the device UI checkpoint:", getErrorMessage(error));
      });
    }, 250);
    return () => window.clearTimeout(timer);
  }, [analyticsLaunch, fatalError, isLoading, section]);

  useEffect(
    () =>
      registerPlatformBackHandler(() => {
        if (section === "decks") return false;
        void navigate("decks");
        return true;
      }),
    [section],
  );
`;
controller = replaceExact(
  controller,
  "  async function confirmDiscardCard(): Promise<boolean> {",
  `${checkpointEffects}\n  async function confirmDiscardCard(): Promise<boolean> {`,
  "persist destination and register root Back",
);
await write("src/controllers/useAppController.ts", controller);

let platformIpc = await read("src/ipc/platform.ts");
platformIpc += `

export async function listenToPlatformBack(
  listener: () => void,
): Promise<UnlistenFn> {
  return listen("ostudy-platform-back", () => listener());
}

export async function setPlatformBackInterception(enabled: boolean): Promise<void> {
  return invoke<void>("set_platform_back_interception", { enabled });
}
`;
await write("src/ipc/platform.ts", platformIpc);

let back = await read("src/platform/backNavigation.ts");
back = replaceExact(
  back,
  "const handlers: PlatformBackHandler[] = [];\n",
  "const handlers: PlatformBackHandler[] = [];\nconst listeners = new Set<(active: boolean) => void>();\n\nfunction publishHandlerAvailability(): void {\n  const active = handlers.length > 0;\n  for (const listener of listeners) listener(active);\n}\n",
  "add Back availability listeners",
);
back = replaceExact(back, "  handlers.push(handler);\n", "  handlers.push(handler);\n  publishHandlerAvailability();\n", "publish Back registration");
back = replaceExact(
  back,
  "    if (index >= 0) handlers.splice(index, 1);\n",
  "    if (index >= 0) {\n      handlers.splice(index, 1);\n      publishHandlerAvailability();\n    }\n",
  "publish Back removal",
);
back += `

export function subscribePlatformBackAvailability(
  listener: (active: boolean) => void,
): () => void {
  listeners.add(listener);
  listener(handlers.length > 0);
  return () => listeners.delete(listener);
}
`;
await write("src/platform/backNavigation.ts", back);

let provider = await read("src/platform/PlatformProvider.tsx");
provider = replaceExact(
  provider,
  'import { getErrorMessage } from "../ipc/common";\n',
  'import { getErrorMessage } from "../ipc/common";\nimport { listenToPlatformBack, setPlatformBackInterception } from "../ipc/platform";\nimport { dispatchPlatformBack, subscribePlatformBackAvailability } from "./backNavigation";\n',
  "import native Back bridge",
);
provider = replaceExact(
  provider,
  "    return unsubscribe;\n  }, []);",
  `    let disposed = false;
    let removeNativeBack: (() => void) | undefined;
    void listenToPlatformBack(() => {
      dispatchPlatformBack();
    }).then((remove) => {
      if (disposed) remove();
      else removeNativeBack = remove;
    });
    const removeAvailability = subscribePlatformBackAvailability((active) => {
      void setPlatformBackInterception(active).catch(() => undefined);
    });
    return () => {
      disposed = true;
      unsubscribe();
      removeNativeBack?.();
      removeAvailability();
    };
  }, []);`,
  "bridge native Back events",
);
await write("src/platform/PlatformProvider.tsx", provider);

console.log("Applied shared Android A1-A2 restoration and Back integration.");
