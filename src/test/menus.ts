import { act, fireEvent, screen, waitFor } from "@testing-library/react";

/**
 * Clicks a Base UI menu trigger until its menu is open, waits for focus to
 * move into the menu as it does for a learner, and returns the menu.
 *
 * A trigger first renders with an empty floating context. Base UI gives it the
 * real one in a layout effect, but the trigger re-renders with it only once its
 * store subscription runs, in a passive effect. When a menu mounts outside
 * `act()`, after an awaited load, those passive effects can still be pending
 * when a `findBy`/`waitFor` sees the new DOM, and a click then does nothing. A
 * busy machine widens that window enough to fail full-suite runs. Letting React
 * run the pending effects before each click closes it; a dropped click changes
 * nothing, so trying again is safe. Waiting for focus also means the open
 * menu's own key handling is in place, so a following Escape closes the menu
 * rather than the dialog behind it.
 */
export async function openMenu(trigger: HTMLElement): Promise<HTMLElement> {
  for (let attempt = 0; attempt < 3 && trigger.getAttribute("aria-expanded") !== "true"; attempt += 1) {
    await act(async () => {});
    fireEvent.click(trigger);
  }
  const name = trigger.getAttribute("aria-label") ?? trigger.textContent ?? "";
  if (trigger.getAttribute("aria-expanded") !== "true") throw new Error(`The menu for "${name}" did not open.`);
  const menu = await screen.findByRole("menu", { name });
  await waitFor(() => {
    if (!menu.contains(document.activeElement)) throw new Error(`Focus did not move into the menu for "${name}".`);
  });
  return menu;
}
