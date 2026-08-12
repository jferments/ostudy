#!/usr/bin/env node

import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFile(join(root, path), "utf8");
const write = (path, content) => writeFile(join(root, path), content, "utf8");
function replaceExact(text, before, after, label) {
  if (!text.includes(before)) throw new Error(`${label}: expected source not found`);
  return text.replace(before, after);
}

// Register authoritative practice restoration.
let lib = await read("src-tauri/src/lib.rs");
if (!lib.includes("mod practice_resume;")) {
  lib = replaceExact(lib, "mod practice;", "mod practice;\nmod practice_resume;", "register practice resume module");
}
if (!lib.includes("practice_resume::resume_practice_session,")) {
  lib = replaceExact(
    lib,
    "            practice_selection::get_practice_card_window,",
    "            practice_selection::get_practice_card_window,\n            practice_resume::resume_practice_session,",
    "register practice resume command",
  );
}
await write("src-tauri/src/lib.rs", lib);

// Restore top-level navigation and scopes from the device-local checkpoint.
let controller = await read("src/controllers/useAppController.ts");
controller = replaceExact(
  controller,
  `import { useCallback, useEffect, useRef, useState } from "react";\n`,
  `import { useCallback, useEffect, useRef, useState } from "react";\n\nimport { registerPlatformBackHandler } from "../platform/backNavigation";\n`,
  "import platform Back registry",
);
controller = replaceExact(
  controller,
  `import { getSettings, type AppSettings } from "../ipc/settings";\n`,
  `import { getSettings, type AppSettings } from "../ipc/settings";\nimport { listenToLifecycle } from "../ipc/platform";\nimport {\n  getUiCheckpoint,\n  saveUiCheckpoint,\n  type PracticeCheckpoint,\n} from "../ipc/uiCheckpoint";\n`,
  "import UI checkpoint APIs",
);
controller = replaceExact(
  controller,
  `  const [analyticsLaunch, setAnalyticsLaunch] = useState<AnalyticsLaunchRequest | null>(null);\n`,
  `  const [analyticsLaunch, setAnalyticsLaunch] = useState<AnalyticsLaunchRequest | null>(null);\n  const [practiceRestore, setPracticeRestore] = useState<PracticeCheckpoint | null>(null);\n`,
  "add practice restoration state",
);
controller = replaceExact(
  controller,
  `    void Promise.all([getSettings(), listDecks()])\n      .then(([loadedSettings, loadedDecks]) => {\n        if (!deckRefreshLane.isCurrent(token)) return;\n        setSavedSettings(loadedSettings);\n        setActiveSettings(loadedSettings);\n        setDecks(loadedDecks);\n        setFatalError(null);\n      })`,
  `    void Promise.all([getSettings(), listDecks(), getUiCheckpoint()])\n      .then(([loadedSettings, loadedDecks, checkpoint]) => {\n        if (!deckRefreshLane.isCurrent(token)) return;\n        setSavedSettings(loadedSettings);\n        setActiveSettings(loadedSettings);\n        setDecks(loadedDecks);\n        if (checkpoint) {\n          setSection(checkpoint.section);\n          if (checkpoint.deckId && loadedDecks.some((deck) => deck.id === checkpoint.deckId)) {\n            setDeckOpenRequest({\n              requestId: deckOpenRequestSequence.current++,\n              deckId: checkpoint.deckId,\n            });\n          }\n          if (checkpoint.practice) setPracticeRestore(checkpoint.practice);\n          if (checkpoint.analytics) {\n            setAnalyticsLaunch({\n              requestId: analyticsRequestSequence.current++,\n              range: checkpoint.analytics.range,\n              deckIds: checkpoint.analytics.deckIds,\n              tagIds: checkpoint.analytics.tagIds,\n            });\n          }\n        }\n        setFatalError(null);\n      })`,
  "restore application checkpoint at bootstrap",
);
const checkpointEffects = `
  useEffect(() => {
    if (isLoading || section === "practice") return;
    const analytics =
      section === "analytics" && analyticsLaunch
        ? {
            range: analyticsLaunch.range,
            deckIds: analyticsLaunch.deckIds,
            tagIds: analyticsLaunch.tagIds,
          }
        : undefined;
    void saveUiCheckpoint({
      section,
      deckId: deckOpenRequest?.deckId,
      analytics,
    }).catch((error: unknown) => {
      console.error("Unable to save the device navigation checkpoint:", getErrorMessage(error));
    });
  }, [analyticsLaunch, deckOpenRequest?.deckId, isLoading, section]);

  useEffect(() => {
    if (isLoading) return;
    let dispose: (() => void) | undefined;
    void listenToLifecycle((snapshot) => {
      if (
        section !== "practice" &&
        (snapshot.phase === "background" ||
          snapshot.phase === "suspending" ||
          snapshot.phase === "termination_requested")
      ) {
        void saveUiCheckpoint({ section, deckId: deckOpenRequest?.deckId });
      }
    }).then((unlisten) => {
      dispose = unlisten;
    });
    return () => dispose?.();
  }, [deckOpenRequest?.deckId, isLoading, section]);

  useEffect(() => {
    if (section === "decks") return;
    return registerPlatformBackHandler(() => {
      void navigate("decks");
      return true;
    });
  }, [section]);
`;
controller = replaceExact(
  controller,
  `  async function confirmDiscardCard(): Promise<boolean> {`,
  `${checkpointEffects}\n  async function confirmDiscardCard(): Promise<boolean> {`,
  "add navigation checkpoint and root Back effects",
);
controller = controller.replaceAll(
  `    if (next === "practice") nextPracticeRequest([], [], [], false, "scheduled", null);`,
  `    if (next === "practice") {\n      setPracticeRestore(null);\n      nextPracticeRequest([], [], [], false, "scheduled", null);\n    }`,
);
controller = controller.replaceAll(
  `    nextPracticeRequest(`,
  `    setPracticeRestore(null);\n    nextPracticeRequest(`,
);
controller = replaceExact(
  controller,
  `    consumeAnalyticsLaunch,\n`,
  `    consumeAnalyticsLaunch,\n    practiceRestore,\n    consumePracticeRestore: () => setPracticeRestore(null),\n`,
  "return practice restoration state",
);
await write("src/controllers/useAppController.ts", controller);

// Wire checkpoint state into the practice screen.
let app = await read("src/App.tsx");
app = replaceExact(
  app,
  `                launchRequest={controller.practiceLaunch}\n                onLaunchConsumed={controller.consumePracticeLaunch}\n`,
  `                launchRequest={controller.practiceLaunch}\n                onLaunchConsumed={controller.consumePracticeLaunch}\n                restoreCheckpoint={controller.practiceRestore}\n                onRestoreConsumed={controller.consumePracticeRestore}\n`,
  "wire practice restoration props",
);
await write("src/App.tsx", app);

let practice = await read("src/screens/PracticeScreen.tsx");
practice = replaceExact(
  practice,
  `import {\n  type AppSettings,\n} from "../ipc/settings";\n`,
  `import {\n  type AppSettings,\n} from "../ipc/settings";\nimport { saveUiCheckpoint, type PracticeCheckpoint } from "../ipc/uiCheckpoint";\n`,
  "import practice checkpoint APIs",
);
practice = replaceExact(
  practice,
  `  launchRequest: PracticeLaunchRequest | null;\n  onLaunchConsumed: () => void;\n`,
  `  launchRequest: PracticeLaunchRequest | null;\n  onLaunchConsumed: () => void;\n  restoreCheckpoint: PracticeCheckpoint | null;\n  onRestoreConsumed: () => void;\n`,
  "add practice restoration props",
);
practice = replaceExact(
  practice,
  `  launchRequest,\n  onLaunchConsumed,\n`,
  `  launchRequest,\n  onLaunchConsumed,\n  restoreCheckpoint,\n  onRestoreConsumed,\n`,
  "destructure practice restoration props",
);
practice = replaceExact(
  practice,
  `  const [sessionCards, setSessionCards] = useState<PracticeCard[]>([]);\n  const [position, setPosition] = useState(0);\n`,
  `  const [sessionCards, setSessionCards] = useState<PracticeCard[]>([]);\n  const [sessionWindowStart, setSessionWindowStart] = useState(0);\n  const [position, setPosition] = useState(0);\n`,
  "add practice window origin",
);
practice = replaceExact(
  practice,
  `    setSession(started);\n    setSessionCards(started.firstWindow.cards);\n    setPosition(0);`,
  `    setSession(started);\n    setSessionCards(started.firstWindow.cards);\n    setSessionWindowStart(started.firstWindow.startPosition);\n    setPosition(0);`,
  "initialize practice window origin",
);
practice = replaceExact(
  practice,
  `    if (!session || loadingWindowRef.current || sessionCards.length >= session.cardCount) {\n      return sessionCards.length;\n    }`,
  `    const loadedThrough = sessionWindowStart + sessionCards.length;\n    if (!session || loadingWindowRef.current || loadedThrough >= session.cardCount) {\n      return loadedThrough;\n    }`,
  "calculate global practice window length",
);
practice = replaceExact(
  practice,
  `        startPosition: sessionCards.length,`,
  `        startPosition: loadedThrough,`,
  "load next global practice window",
);
practice = replaceExact(
  practice,
  `      if (!window) return sessionCards.length;`,
  `      if (!window) return loadedThrough;`,
  "return global loaded count",
);
practice = replaceExact(
  practice,
  `      return sessionCards.length + window.cards.length;`,
  `      return loadedThrough + window.cards.length;`,
  "return extended global count",
);
practice = practice.replaceAll(
  `  }, [controller, session, sessionCards.length]);`,
  `  }, [controller, session, sessionCards.length, sessionWindowStart]);`,
);
const restorationEffect = `
  useEffect(() => {
    if (!restoreCheckpoint) return;
    let active = true;
    onRestoreConsumed();
    void controller
      .resume(restoreCheckpoint.sessionId)
      .then(async (restored) => {
        if (!active) return;
        setSession(restored.session);
        setSessionCards(restored.session.firstWindow.cards);
        setSessionWindowStart(restored.resumePosition);
        setPosition(0);
        setRatedCount(restored.ratedCount);
        setCompleted(restored.completed);
        setRevealed(restored.revealed);
        setPreviewSet(null);
        pendingStartRef.current = null;
        pendingReviewRef.current = null;
        const first = restored.session.firstWindow.cards[0];
        if (restored.revealed && first && !restored.completed) {
          setPreviewSet(
            await controller.reveal(restored.session.id, first.presentation.cardId),
          );
        }
      })
      .catch((error: unknown) => {
        if (!active) return;
        setErrorMessage(
          `The interrupted practice session could not be restored: ${getErrorMessage(error)}`,
        );
        setSession(null);
        setSessionCards([]);
        setSessionWindowStart(0);
      });
    return () => {
      active = false;
    };
  }, [controller, onRestoreConsumed, restoreCheckpoint]);

  useEffect(() => {
    if (!session || completed) return;
    const timer = window.setTimeout(() => {
      void saveUiCheckpoint({
        section: "practice",
        practice: {
          sessionId: session.id,
          position: sessionWindowStart + position,
          revealed,
        },
      }).catch((error: unknown) => {
        console.error("Unable to save the practice restoration checkpoint:", getErrorMessage(error));
      });
    }, 150);
    return () => window.clearTimeout(timer);
  }, [completed, position, revealed, session, sessionWindowStart]);
`;
practice = replaceExact(
  practice,
  `  useEffect(() => {\n    if (!session || !current || completed) return;`,
  `${restorationEffect}\n  useEffect(() => {\n    if (!session || !current || completed) return;`,
  "add practice restore and checkpoint effects",
);
practice = replaceExact(
  practice,
  `      startPosition: position,`,
  `      startPosition: sessionWindowStart + position,`,
  "refresh practice from global position",
);
practice = replaceExact(
  practice,
  `    setPosition(refreshed.startPosition);\n    setSessionCards((cards) => [\n      ...cards.slice(0, refreshed.startPosition),\n      ...refreshed.window.cards,\n    ]);`,
  `    setSessionWindowStart(refreshed.startPosition);\n    setPosition(0);\n    setSessionCards(refreshed.window.cards);`,
  "reset refreshed practice window",
);
practice = replaceExact(
  practice,
  `        await advanceOrComplete(position + 1 >= session.cardCount);`,
  `        await advanceOrComplete(sessionWindowStart + position + 1 >= session.cardCount);`,
  "compare schedule action with global position",
);
practice = replaceExact(
  practice,
  `    if (!session || backendCompleted || position + 1 >= session.cardCount) {`,
  `    if (!session || backendCompleted || sessionWindowStart + position + 1 >= session.cardCount) {`,
  "complete against global practice position",
);
practice = replaceExact(
  practice,
  `    setSessionCards([]);\n    setPosition(0);`,
  `    setSessionCards([]);\n    setSessionWindowStart(0);\n    setPosition(0);`,
  "reset practice window origin",
);
practice = replaceExact(
  practice,
  `        position={position}\n`,
  `        position={sessionWindowStart + position}\n`,
  "render global practice position",
);
await write("src/screens/PracticeScreen.tsx", practice);

console.log("Applied Android A2 navigation and practice restoration integration.");
