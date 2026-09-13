import assert from "node:assert/strict";
import test from "node:test";
import { cssModuleViolations, isProductionSource, nativeControlViolation } from "../../scripts/ui/presentation-policy.mjs";

test("every generic feature control must use the public facade", () => {
  for (const tag of ["button", "input", "select", "textarea", "details", "summary"]) {
    assert.match(nativeControlViolation("src/screens/Example.tsx", tag, {}), /facade/u);
    assert.equal(nativeControlViolation("src/ui/vendor/shadcn/example.tsx", tag, {}), null);
  }
  assert.equal(nativeControlViolation("src/screens/Example.tsx", "article", {}), null);
  assert.match(nativeControlViolation("src/ui/primitives/Example.tsx", "button", {}), /facade/u);
});

test("Markdown exception is limited to an explicitly disabled task marker", () => {
  const owner = "src/screens/tutor/TutorMarkdown.tsx";
  const allowed = { type: "checkbox", checked: null, disabled: true, "aria-label": "Checklist item" };
  assert.equal(nativeControlViolation(owner, "input", allowed), null);
  for (const attributes of [
    { ...allowed, disabled: false }, { ...allowed, disabled: null },
    { ...allowed, type: "text" }, { ...allowed, "onChange": null },
    { ...allowed, "...spread": null }, { ...allowed, name: "submitted" },
    { ...allowed, "aria-label": "Other control" },
  ]) assert.match(nativeControlViolation(owner, "input", attributes), /facade/u);
  assert.match(nativeControlViolation("src/screens/OtherMarkdown.tsx", "input", allowed), /facade/u);
});

test("production inventory excludes fixtures but not real feature controllers", () => {
  for (const path of ["src/App.tsx", "src/screens/tutor/TutorMarkdown.tsx", "src/controllers/useData.ts"]) {
    assert.equal(isProductionSource(path), true);
  }
  for (const path of ["src/App.test.tsx", "src/ui/Button.stories.tsx", "src/testing/fixture.ts", "src/a.css"]) {
    assert.equal(isProductionSource(path), false);
  }
});

test("retained CSS must exist, be imported and have a specific rationale", () => {
  const path = "src/editor/Content.module.css";
  const manifest = { [path]: "Rich content descendant styling." };
  assert.deepEqual(cssModuleViolations([path], [path], manifest), []);
  assert.match(cssModuleViolations([path], [], manifest).join(), /no source importer/u);
  assert.match(cssModuleViolations([], [], manifest).join(), /owner is missing/u);
  assert.match(cssModuleViolations(["src/Extra.module.css"], [], manifest).join(), /unregistered/u);
  assert.match(cssModuleViolations([path], [path], { [path]: "" }).join(), /rationale/u);
});
