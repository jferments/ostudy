/** Reload the UI only. This never restarts the native process or relaunches with
 * a different XDG root, and it clears module-level caches/old async callbacks.
 */
export function refreshLibraryView(): void {
  window.location.reload();
}
