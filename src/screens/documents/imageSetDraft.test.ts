import assert from "node:assert/strict";
import { test } from "vitest";
import {
  appendImageSetMembers,
  moveImageSetMember,
  removeImageSetMember,
  rotateImageSetDisplayCrop,
  validateImageSetDisplayCrop,
} from "./imageSetDraft";
import type { ImageSetDisplayCrop } from "./imageSetDraft";

const a = Object.freeze({ id: "selection-a", name: "same.png", state: "ready", sha256: "same-bytes" });
const b = Object.freeze({ id: "selection-b", name: "same.png", state: "failed", sha256: "same-bytes" });
const c = Object.freeze({ id: "selection-c", name: "third.png", state: "ready", sha256: "other-bytes" });
const members = Object.freeze([a, b, c]);
const ids = (values: readonly { id: string }[]) => values.map((value) => value.id);
const crop = Object.freeze({ left: 0.125, top: 0.25, right: 0.5, bottom: 0.75 });

function approximatelyEqual(left: ImageSetDisplayCrop, right: ImageSetDisplayCrop) {
  for (const key of ["left", "top", "right", "bottom"] as const) {
    assert.ok(Math.abs(left[key] - right[key]) < 1e-12, `${key}: ${left[key]} != ${right[key]}`);
  }
}

test("moving toward the end uses the final position and preserves member objects", () => {
  const moved = moveImageSetMember(members, a.id, 2);
  assert.deepEqual(ids(moved), [b.id, c.id, a.id]);
  assert.strictEqual(moved[2], a);
  assert.deepEqual(ids(members), [a.id, b.id, c.id]);
});

test("moving toward the start preserves all other relative order", () => {
  assert.deepEqual(ids(moveImageSetMember(members, c.id, 0)), [c.id, a.id, b.id]);
});

test("a no-op move retains the same array", () => {
  assert.strictEqual(moveImageSetMember(members, b.id, 1), members);
});

test("stale selection IDs fail rather than moving another image", () => {
  assert.throws(() => moveImageSetMember(members, "missing", 0), /no longer/);
  assert.throws(() => removeImageSetMember(members, "missing"), /no longer/);
});

test("positions outside the set and fractional or nonfinite positions are refused", () => {
  for (const position of [-1, 3, 0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => moveImageSetMember(members, a.id, position), /position/);
  }
});

test("a failed selection is removable without losing ready siblings", () => {
  const remaining = removeImageSetMember(members, b.id);
  assert.deepEqual(ids(remaining), [a.id, c.id]);
  assert.strictEqual(remaining[0], a);
  assert.strictEqual(remaining[1], c);
  assert.strictEqual(members[1], b);
});

test("the final selection can be removed from an uncommitted preview", () => {
  assert.deepEqual(removeImageSetMember([a], a.id), []);
});

test("different selections of identical bytes or filenames are not conflated", () => {
  const appended = appendImageSetMembers([a], [b], 2);
  assert.deepEqual(ids(appended), [a.id, b.id]);
  assert.strictEqual(appended[1], b);
});

test("append retains selection order and input immutability", () => {
  const before = Object.freeze([a]);
  const added = Object.freeze([b, c]);
  assert.deepEqual(ids(appendImageSetMembers(before, added, 3)), [a.id, b.id, c.id]);
  assert.deepEqual(before, [a]);
  assert.deepEqual(added, [b, c]);
});

test("empty append is a no-op but does not bypass validation", () => {
  assert.strictEqual(appendImageSetMembers(members, [], 3), members);
  assert.throws(() => appendImageSetMembers([a, a], [], 3), /distinct/);
  assert.throws(() => appendImageSetMembers(members, [], 2), /count limit/);
});

test("invalid or repeated selection identities are rejected", () => {
  assert.throws(() => appendImageSetMembers([a], [a], 3), /distinct/);
  assert.throws(() => appendImageSetMembers([], [{ id: "" }], 3), /distinct/);
  assert.throws(() => moveImageSetMember([a, a], a.id, 1), /distinct/);
  assert.throws(() => removeImageSetMember([a, a], a.id), /distinct/);
});

test("the supplied count limit is checked without silently truncating selections", () => {
  assert.throws(() => appendImageSetMembers([a], [b, c], 2), /count limit/);
  for (const limit of [0, -1, 1.5, Infinity, NaN, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => appendImageSetMembers([], [], limit), /limit is unavailable/);
  }
});

test("all three nontrivial display-crop rotations have the expected coordinates", () => {
  assert.deepEqual(rotateImageSetDisplayCrop(crop, 1), { left: 0.25, top: 0.125, right: 0.75, bottom: 0.5 });
  assert.deepEqual(rotateImageSetDisplayCrop(crop, 2), { left: 0.5, top: 0.25, right: 0.875, bottom: 0.75 });
  assert.deepEqual(rotateImageSetDisplayCrop(crop, 3), { left: 0.25, top: 0.5, right: 0.75, bottom: 0.875 });
});

test("rotation supports negative quarters and avoids iterative floating-point drift", () => {
  assert.deepEqual(rotateImageSetDisplayCrop(crop, -1), rotateImageSetDisplayCrop(crop, 3));
  assert.strictEqual(rotateImageSetDisplayCrop(crop, 0), crop);
  assert.strictEqual(rotateImageSetDisplayCrop(crop, 4), crop);
  assert.strictEqual(rotateImageSetDisplayCrop(crop, -4), crop);
});

test("display crop rotations preserve area, bounds, and inverse mapping", () => {
  for (let x = 0; x < 5; x += 1) {
    for (let y = 0; y < 5; y += 1) {
      const rectangle = { left: x / 10, top: y / 10, right: (x + 5) / 10, bottom: (y + 5) / 10 };
      for (let turn = -7; turn <= 7; turn += 1) {
        const rotated = rotateImageSetDisplayCrop(rectangle, turn);
        validateImageSetDisplayCrop(rotated);
        const area = (rotated.right - rotated.left) * (rotated.bottom - rotated.top);
        assert.ok(Math.abs(area - 0.25) < 1e-12);
        approximatelyEqual(rotateImageSetDisplayCrop(rotated, -turn), rectangle);
      }
    }
  }
});

test("empty, inverted, outside-image and nonfinite display crops are rejected", () => {
  const invalid = [
    { ...crop, left: -0.1 }, { ...crop, top: -0.1 },
    { ...crop, right: 1.1 }, { ...crop, bottom: 1.1 },
    { ...crop, left: crop.right }, { ...crop, top: crop.bottom },
    { ...crop, left: 0.9 }, { ...crop, bottom: 0.1 },
    ...["left", "top", "right", "bottom"].flatMap((key) =>
      [NaN, Infinity, -Infinity].map((value) => ({ ...crop, [key]: value }))),
  ];
  for (const value of invalid) {
    assert.throws(() => validateImageSetDisplayCrop(value), /nonempty crop/);
    assert.throws(() => rotateImageSetDisplayCrop(value, 0), /nonempty crop/);
  }
});

test("non-quarter-turn rotations fail instead of silently rounding", () => {
  for (const turn of [0.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => rotateImageSetDisplayCrop(crop, turn), /quarter turns/);
  }
});

test("full-image bounds are retained under every preview rotation", () => {
  const full = Object.freeze({ left: 0, top: 0, right: 1, bottom: 1 });
  for (let turn = 0; turn < 4; turn += 1) {
    assert.deepEqual(rotateImageSetDisplayCrop(full, turn), full);
  }
});

test("an unrepresentably small rotated crop is refused rather than silently collapsed", () => {
  const tiny = { left: 0, top: 0, right: Number.MIN_VALUE, bottom: 1 };
  validateImageSetDisplayCrop(tiny);
  assert.throws(() => rotateImageSetDisplayCrop(tiny, 2), /nonempty crop/);
});
