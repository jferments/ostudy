import type { AppSettings } from "../../ipc/settings";

const SETTINGS_SAVED_EVENT = "ostudy:settings-saved";

export function announceSettingsSaved(settings: AppSettings): void {
  window.dispatchEvent(
    new CustomEvent<AppSettings>(SETTINGS_SAVED_EVENT, { detail: settings }),
  );
}

export function listenForSavedSettings(
  listener: (settings: AppSettings) => void,
): () => void {
  function handle(event: Event): void {
    listener((event as CustomEvent<AppSettings>).detail);
  }
  window.addEventListener(SETTINGS_SAVED_EVENT, handle);
  return () => window.removeEventListener(SETTINGS_SAVED_EVENT, handle);
}
