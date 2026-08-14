const SETTINGS_SAVED_EVENT = "ostudy:settings-saved";

/**
 * Broadcast persisted settings without making the shared UI layer depend on
 * the IPC domain type. Call sites retain their concrete settings type through
 * this generic event boundary.
 */
export function announceSettingsSaved<T>(settings: T): void {
  window.dispatchEvent(new CustomEvent<T>(SETTINGS_SAVED_EVENT, { detail: settings }));
}

export function listenForSavedSettings<T>(
  listener: (settings: T) => void,
): () => void {
  function handle(event: Event): void {
    listener((event as CustomEvent<T>).detail);
  }
  window.addEventListener(SETTINGS_SAVED_EVENT, handle);
  return () => window.removeEventListener(SETTINGS_SAVED_EVENT, handle);
}
