export type PracticeExitHandler = () => Promise<boolean>;

let activeHandler: PracticeExitHandler | null = null;

export function registerPracticeExitHandler(
  handler: PracticeExitHandler | null,
): () => void {
  activeHandler = handler;
  return () => {
    if (activeHandler === handler) activeHandler = null;
  };
}

export function hasActivePracticeSession(): boolean {
  return activeHandler !== null;
}

export async function terminateActivePracticeSession(): Promise<boolean> {
  const handler = activeHandler;
  if (!handler) return true;
  return handler();
}

export function resetPracticeExitGuardForTests(): void {
  activeHandler = null;
}
