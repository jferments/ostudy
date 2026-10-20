/** Copies plain text to the system clipboard through the platform-owned adapter. */
export async function copyText(text: string): Promise<void> {
  if (!navigator.clipboard?.writeText) throw new Error("Copying is unavailable here.");
  await navigator.clipboard.writeText(text);
}
