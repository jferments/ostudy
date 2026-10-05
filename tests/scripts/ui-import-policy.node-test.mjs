import assert from "node:assert/strict";
import test from "node:test";
import { importViolation } from "../../scripts/ui/import-policy.mjs";

test("Base UI is confined to copied implementation source", () => {
  assert.equal(importViolation("src/ui/vendor/shadcn/button.tsx", "@base-ui/react/button"), null);
  for (const owner of ["src/screens/tutor/TutorWorkspace.tsx", "src/ui/primitives/Button.tsx", "src/ipc/tutor.ts"]) {
    assert.match(importViolation(owner, "@base-ui/react/button"), /Base UI belongs/u);
    assert.match(importViolation(owner, "@base-ui/react"), /Base UI belongs/u);
  }
});
test("relative paths, aliases and dot segments cannot bypass the vendor boundary", () => {
  for (const specifier of ["../../ui/vendor/shadcn/button", "@/ui/vendor/shadcn/button", "@/ui/primitives/../vendor/shadcn/button"]) {
    assert.match(importViolation("src/screens/tutor/TutorWorkspace.tsx", specifier), /Only oStudy/u);
  }
});
test("approved adapters may consume vendor implementations without leaking to features", () => {
  assert.equal(importViolation("src/ui/primitives/Button.tsx", "../vendor/shadcn/button"), null);
  assert.equal(importViolation("src/ui/overlays/Dialog.tsx", "../vendor/shadcn/dialog"), null);
  assert.equal(importViolation("src/ui/layout/Navigation.tsx", "../vendor/shadcn/sidebar"), null);
  assert.equal(importViolation("src/screens/Tutor.tsx", "../ui/primitives/Button"), null);
  assert.match(importViolation("src/ui/primitivesExtra/Leaky.tsx", "../vendor/shadcn/button"), /Only oStudy/u);
});
test("TanStack Virtual stays behind the VirtualList pattern", () => {
  assert.equal(importViolation("src/ui/patterns/VirtualList.tsx", "@tanstack/react-virtual"), null);
  for (const owner of ["src/screens/library/LibraryWorkbench.tsx", "src/ui/primitives/ListRow.tsx"]) {
    assert.match(importViolation(owner, "@tanstack/react-virtual"), /VirtualList/u);
    assert.match(importViolation(owner, "@tanstack/virtual-core"), /VirtualList/u);
  }
});
