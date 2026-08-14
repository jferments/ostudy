export type PlatformBackHandler = () => boolean | void;
export type PlatformBackStackListener = (handlerCount: number) => void;

const handlers: PlatformBackHandler[] = [];
const listeners = new Set<PlatformBackStackListener>();

/**
 * Registers one platform Back handler. Handlers are evaluated from newest to
 * oldest so overlays and nested workflows consume Back before application
 * navigation. The native Android callback is enabled only while this stack is
 * non-empty, preserving Android's predictive back-to-home behavior at the root.
 */
export function registerPlatformBackHandler(handler: PlatformBackHandler): () => void {
  handlers.push(handler);
  publishCount();
  return () => {
    const index = handlers.lastIndexOf(handler);
    if (index >= 0) {
      handlers.splice(index, 1);
      publishCount();
    }
  };
}

/**
 * Entry point for Android hardware Back. Returns true when a registered layer
 * consumed the action.
 */
export function dispatchPlatformBack(): boolean {
  for (let index = handlers.length - 1; index >= 0; index -= 1) {
    if (handlers[index]?.() !== false) return true;
  }
  return false;
}

export function subscribePlatformBackStack(listener: PlatformBackStackListener): () => void {
  listeners.add(listener);
  listener(handlers.length);
  return () => listeners.delete(listener);
}

export function platformBackHandlerCountForTests(): number {
  return handlers.length;
}

function publishCount(): void {
  for (const listener of listeners) listener(handlers.length);
}
