/**
 * Pure, local image-set preview operations. These do not admit source files,
 * authorize extraction, determine evidence equivalence, or mutate native state.
 * Member IDs are opaque selection identities, never filenames or list positions.
 */
export interface ImageSetDraftMember {
  readonly id: string;
}

/** Normalized bounds in the currently displayed image's coordinate space. */
export interface ImageSetDisplayCrop {
  readonly left: number;
  readonly top: number;
  readonly right: number;
  readonly bottom: number;
}

function validateMemberIds(members: readonly ImageSetDraftMember[]): void {
  const ids = new Set<string>();
  for (const member of members) {
    if (!member.id || ids.has(member.id)) {
      throw new Error("Image selections must have distinct, nonempty identities.");
    }
    ids.add(member.id);
  }
}

function memberIndex<T extends ImageSetDraftMember>(
  members: readonly T[],
  id: string,
): number {
  validateMemberIds(members);
  const index = members.findIndex((member) => member.id === id);
  if (index < 0) throw new Error("This image selection is no longer in the preview.");
  return index;
}

/** Destination is the final zero-based position, including moves toward the end. */
export function moveImageSetMember<T extends ImageSetDraftMember>(
  members: readonly T[],
  id: string,
  destination: number,
): readonly T[] {
  const source = memberIndex(members, id);
  if (!Number.isSafeInteger(destination) || destination < 0 || destination >= members.length) {
    throw new Error("Choose a position within the image set.");
  }
  if (source === destination) return members;
  const next = [...members];
  const [member] = next.splice(source, 1);
  if (member === undefined) throw new Error("This image selection is no longer in the preview.");
  next.splice(destination, 0, member);
  return next;
}

/** A failed selection can be removed without changing the other selection objects. */
export function removeImageSetMember<T extends ImageSetDraftMember>(
  members: readonly T[],
  id: string,
): readonly T[] {
  const index = memberIndex(members, id);
  return [...members.slice(0, index), ...members.slice(index + 1)];
}

/**
 * The count limit is supplied by the caller's native capability contract. This
 * preview guard is not a substitute for native byte, pixel, or storage limits.
 * Identical bytes with distinct selection IDs remain distinct occurrences.
 */
export function appendImageSetMembers<T extends ImageSetDraftMember>(
  members: readonly T[],
  additions: readonly T[],
  maximumCount: number,
): readonly T[] {
  if (!Number.isSafeInteger(maximumCount) || maximumCount < 1) {
    throw new Error("The image-selection limit is unavailable.");
  }
  if (members.length > maximumCount || additions.length > maximumCount - members.length) {
    throw new Error("The selected images exceed the image-set count limit.");
  }
  const next = [...members, ...additions];
  validateMemberIds(next);
  return additions.length === 0 ? members : next;
}

export function validateImageSetDisplayCrop(crop: ImageSetDisplayCrop): void {
  const { left, top, right, bottom } = crop;
  if (
    ![left, top, right, bottom].every(Number.isFinite) ||
    left < 0 || top < 0 || right > 1 || bottom > 1 ||
    left >= right || top >= bottom
  ) {
    throw new Error("Select a nonempty crop inside the displayed image.");
  }
}

/**
 * Rotate a local display crop with the preview. Clockwise quarters may be
 * negative. No source pixels, inclusion decision, or evidence locator is edited.
 * Native source orientation and its original-coordinate transform remain separate.
 */
export function rotateImageSetDisplayCrop(
  crop: ImageSetDisplayCrop,
  clockwiseQuarters: number,
): ImageSetDisplayCrop {
  validateImageSetDisplayCrop(crop);
  if (!Number.isSafeInteger(clockwiseQuarters)) {
    throw new Error("Image preview rotation must use whole quarter turns.");
  }
  const turns = ((clockwiseQuarters % 4) + 4) % 4;
  let rotated: ImageSetDisplayCrop;
  switch (turns) {
    case 0:
      return crop;
    case 1:
      rotated = { left: 1 - crop.bottom, top: crop.left, right: 1 - crop.top, bottom: crop.right };
      break;
    case 2:
      rotated = { left: 1 - crop.right, top: 1 - crop.bottom, right: 1 - crop.left, bottom: 1 - crop.top };
      break;
    default:
      rotated = { left: crop.top, top: 1 - crop.right, right: crop.bottom, bottom: 1 - crop.left };
  }
  // A subpixel interval can collapse when complemented in floating-point
  // arithmetic. Refuse that result instead of silently displaying an empty crop.
  validateImageSetDisplayCrop(rotated);
  return rotated;
}
