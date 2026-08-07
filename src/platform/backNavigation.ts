export type PlatformBackHandler = () => boolean | void;

const handlers: PlatformBackHandler[] = [];

/**
 * Registers one platform Back handler. Handlers are evaluated from newest to
 * oldest so overlays and nested workflows can consume Back before navigation.
 */
export function registerPlatformBackHandler(handler: PlatformBackHandler): () => void {
  handlers.push(handler);
  return () => {
    const index = handlers.lastIndexOf(handler);
    if (index >= 0) handlers.splice(index, 1);
  };
}

/**
 * Entry point for Android hardware Back and future desktop shell integrations.
 * Returns true when a registered UI layer consumed the action.
 */
export function dispatchPlatformBack(): boolean {
  for (let index = handlers.length - 1; index >= 0; index -= 1) {
    if (handlers[index]?.() !== false) return true;
  }
  return false;
}

export function platformBackHandlerCountForTests(): number {
  return handlers.length;
}
