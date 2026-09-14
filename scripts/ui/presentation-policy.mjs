/** Specialized CSS owners retained after the ordinary UI migration. */
export const retainedModules = Object.freeze({
  "src/AppShell.module.css": "Definite-height native route and scroll containment.",
  "src/editor/RichContent.module.css": "ProseMirror, rich content, media/math descendant and print styling.",
  "src/screens/settings/DeviceSyncSettings.module.css": "QR quiet-zone geometry and machine-readable white contrast.",
  "src/components/StudyMarkdown.module.css": "Sanitized Markdown descendants and rich-content overflow.",
  "src/screens/practice/PracticeCard.module.css": "Two-face 3D card geometry, face visibility, scaling and reduced motion.",
  "src/screens/flashcards/deck-library/FolderSection.module.css": "Folder hit-test regions and drag/reorder geometry.",
  "src/screens/flashcards/deck-library/DeckItem.module.css": "Measured deck row, pointer session and insertion geometry.",
  "src/screens/flashcards/deck-library/DeckLibraryView.module.css": "Measured library scrolling and insertion regions.",
  "src/screens/flashcards/deck-library/LibraryDragFeedback.module.css": "Pointer preview transforms and insertion destination feedback.",
  "src/screens/flashcards/deck-library/DeckList.module.css": "Measured list/drop-target containment.",
  "src/ui/artwork/Artwork.module.css": "Responsive illustration slots and decorative asset cropping.",
  "src/ui/motion/Motion.module.css": "Shared transition geometry and reduced-motion behavior.",
  "src/ui/feedback/experience.module.css": "Optional celebration particles and keyframes.",
  "src/ui/primitives/Progress.module.css": "Progress text foreground split across the filled and unfilled track.",
  "src/ui/materials/Materials.module.css": "Optional decorative surface layers and ambient pointer transforms.",
  "src/ui/tutorials/TourProvider.module.css": "Measured spotlight mask and viewport collision geometry.",
});

export function isProductionSource(path) {
  return /\.[cm]?[jt]sx?$/u.test(path) && !/\.(?:test|spec|stories)\.[cm]?[jt]sx?$/u.test(path)
    && !/(?:^|\/)(?:__tests__|test|tests|testing)(?:\/|$)/u.test(path);
}

/** The only raw feature control is an inert marker in sanitized document content. */
export function nativeControlViolation(owner, tag, attributes) {
  if (!["button", "input", "select", "textarea", "details", "summary"].includes(tag)) return null;
  if (owner.startsWith("src/ui/vendor/shadcn/")) return null;
  if (owner === "src/components/StudyMarkdown.tsx" && tag === "input"
      && attributes.type === "checkbox" && attributes.disabled === true
      && attributes["aria-label"] === "Checklist item"
      && Object.keys(attributes).every((name) => ["type", "checked", "disabled", "aria-label"].includes(name))) {
    return null;
  }
  return "Use an oStudy facade instead of a raw generic control.";
}

export function cssModuleViolations(paths, importedPaths, manifest = retainedModules) {
  const existing = new Set(paths);
  const imported = new Set(importedPaths);
  const errors = [];
  for (const path of existing) {
    if (!Object.hasOwn(manifest, path)) errors.push(`${path}: unregistered CSS Module; migrate ordinary styling or record its specialized owner.`);
    else if (!imported.has(path)) errors.push(`${path}: retained CSS Module has no source importer.`);
  }
  for (const [path, reason] of Object.entries(manifest)) {
    if (!existing.has(path)) errors.push(`${path}: retained owner is missing; remove its obsolete manifest entry.`);
    if (typeof reason !== "string" || reason.trim().length < 12) errors.push(`${path}: a specific ownership rationale is required.`);
  }
  return errors;
}
