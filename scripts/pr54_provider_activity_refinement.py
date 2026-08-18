#!/usr/bin/env python3
"""One-shot refinement for aggregate provider/model activity reporting."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if new in value:
        return
    count = value.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {count}")
    p.write_text(value.replace(old, new, 1))


replace_once(
    "src/screens/DocumentsScreen.tsx",
    '''function formatProviderActivity(
  progress: DocumentGenerationProgress | null,
  now: number,
): string {
  if (!progress) return "Preparing";
  const age = progress.lastProviderActivityAt === null
    ? null
    : Math.max(0, Math.floor((now - progress.lastProviderActivityAt) / 1_000));
  const suffix = age === null ? "" : age < 2 ? " · now" : ` · ${age}s ago`;
  switch (progress.providerActivity) {
    case "streaming":
      return `Receiving output${suffix}`;
    case "responsive":
      return `Responsive, waiting${suffix}`;
    case "connecting":
      return "Connecting";
    default:
      return "Preparing";
  }
}''',
    '''function formatProviderActivity(
  progress: DocumentGenerationProgress | null,
  now: number,
): string {
  if (!progress) return "Preparing";
  const providerAge = activityAgeSeconds(progress.lastProviderActivityAt, now);
  const modelAge = activityAgeSeconds(progress.lastModelActivityAt, now);
  switch (progress.providerActivity) {
    case "streaming":
      if (modelAge === null) return `Responsive, waiting${activityAgeSuffix(providerAge)}`;
      if (modelAge < 2) return "Receiving output · now";
      if (providerAge !== null && providerAge < modelAge) {
        return `Responsive · output ${formatActivityAge(modelAge)}`;
      }
      return `Receiving output · ${formatActivityAge(modelAge)}`;
    case "responsive":
      return `Responsive, waiting${activityAgeSuffix(providerAge)}`;
    case "connecting":
      return "Connecting";
    default:
      return "Preparing";
  }
}

function activityAgeSeconds(timestamp: number | null, now: number): number | null {
  return timestamp === null ? null : Math.max(0, Math.floor((now - timestamp) / 1_000));
}

function activityAgeSuffix(ageSeconds: number | null): string {
  return ageSeconds === null ? "" : ` · ${formatActivityAge(ageSeconds)}`;
}

function formatActivityAge(ageSeconds: number): string {
  if (ageSeconds < 2) return "now";
  if (ageSeconds < 60) return `${ageSeconds}s ago`;
  const minutes = Math.floor(ageSeconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m ago`;
}''',
    "provider activity/model progress distinction",
)

anchor = '''  it("shows stage, window progress, elapsed runtime, requests, retries, reasoning and cache usage", async () => {'''
regression = '''  it("distinguishes a responsive heartbeat from recent model output", async () => {
    documentIpcMocks.getDocumentGenerationProgress.mockResolvedValue({
      ...progress,
      providerActivity: "streaming",
      lastProviderActivityAt: Date.now() - 1_000,
      lastModelActivityAt: Date.now() - 125_000,
    });

    render(
      <DocumentsScreen
        refreshDecks={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    expect(await screen.findByText("Responsive · output 2m ago")).toBeTruthy();
  });

'''
replace_once(
    "src/screens/DocumentsProgress.test.tsx",
    anchor,
    regression + anchor,
    "provider heartbeat versus model output regression",
)

print("PR #54 provider activity refinement applied.")
