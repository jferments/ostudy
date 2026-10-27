import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { LessonSummary } from "../../ipc/lessons";
import { listSubjectPodcasts, writePodcast, type Podcast } from "../../ipc/podcasts";
import { getSpeechSettings, type SpeechSettings } from "../../ipc/speech";
import { PodcastsSection } from "./PodcastsSection";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/podcasts", async (original) => ({
  ...(await original<typeof import("../../ipc/podcasts")>()),
  listSubjectPodcasts: vi.fn(),
  writePodcast: vi.fn(),
  deletePodcast: vi.fn(),
}));
vi.mock("../../ipc/speech", () => ({ getSpeechSettings: vi.fn(), getDeviceVoices: vi.fn(async () => ({ supported: false, models: [] })) }));

function lesson(index: number): LessonSummary {
  return {
    id: `lesson-${index}`,
    subjectId: "subject-1",
    title: `Lesson ${index}`,
    revision: 1,
    written: {
      revisionNumber: 1,
      origin: "generated",
      stepCount: 4,
      checkCount: 2,
      estimatedMinutes: 8,
      sources: "current",
      createdAt: 1,
    },
    progress: null,
  } as LessonSummary;
}

const drama: SpeechSettings = {
  profileId: "profile-1",
  modelId: "fish-audio/drama-3",
  voice: "builtin:relaxed-man",
  revision: 1,
  models: [
    {
      id: "hexgrad/kokoro-82m",
      label: "Kokoro",
      voices: ["af_heart", "am_michael"],
      defaultVoice: "af_heart",
      clonesVoices: false, onDevice: false,
    },
    { id: "fish-audio/drama-3", label: "Drama 3", voices: [], defaultVoice: "", clonesVoices: true, onDevice: false },
  ],
  connections: [{ id: "profile-1", name: "OpenRouter" }],
  voices: [
    {
      id: "relaxed-man",
      name: "Relaxed man",
      modelId: "fish-audio/drama-3",
      assetId: null,
      state: "builtIn",
      voice: "builtin:relaxed-man",
    },
    {
      id: "calm-older-woman",
      name: "Calm older woman",
      modelId: "fish-audio/drama-3",
      assetId: null,
      state: "builtIn",
      voice: "builtin:calm-older-woman",
    },
    {
      id: "v1",
      name: "Warm narrator",
      modelId: "fish-audio/drama-3",
      assetId: "asset-1",
      state: "saved",
      voice: "saved:v1",
    },
    {
      id: "v2",
      name: "New voice",
      modelId: "fish-audio/drama-3",
      assetId: "asset-2",
      state: "tried",
      voice: "saved:v2",
    },
  ],
};

const made = { id: "podcast-9", title: "New podcast" } as Podcast;

describe("PodcastsSection", () => {
  beforeEach(() => {
    vi.mocked(listSubjectPodcasts).mockReset();
    vi.mocked(listSubjectPodcasts).mockResolvedValue([]);
    vi.mocked(writePodcast).mockReset();
    vi.mocked(writePodcast).mockResolvedValue(made);
    vi.mocked(getSpeechSettings).mockReset();
    vi.mocked(getSpeechSettings).mockResolvedValue(drama);
  });

  it("lists the subject's podcasts and plays one", async () => {
    vi.mocked(listSubjectPodcasts).mockResolvedValue([
      {
        id: "podcast-1",
        subjectId: "subject-1",
        title: "From filtration to urine",
        targetMinutes: 15,
        minutes: 14,
        lessonTitles: ["Lesson 1", "Lesson 2"],
        createdAt: 1,
      },
    ]);
    const onOpenPodcast = vi.fn();
    render(<PodcastsSection headingId="p" subjectId="subject-1" lessons={[lesson(1)]} onOpenPodcast={onOpenPodcast} />);
    const row = await screen.findByTestId("subject-podcast");
    expect(within(row).getByText("14 min · Lesson 1, Lesson 2")).toBeTruthy();
    fireEvent.click(within(row).getByRole("button", { name: "Play From filtration to urine" }));
    expect(onOpenPodcast).toHaveBeenCalledWith("podcast-1");
    expect(listSubjectPodcasts).toHaveBeenCalledWith("subject-1");
  });

  it("makes a podcast from the picked lessons, length and host voices, then opens it", async () => {
    const onOpenPodcast = vi.fn();
    const lessons = [1, 2, 3, 4].map(lesson);
    render(<PodcastsSection headingId="p" subjectId="subject-1" lessons={lessons} onOpenPodcast={onOpenPodcast} />);
    fireEvent.click(await screen.findByRole("button", { name: /Make a podcast/ }));
    const dialog = await screen.findByRole("dialog", { name: "Make a podcast" });
    // The first three lessons are picked; the hosts take the voices that come with ostudy.
    const boxes = within(dialog).getAllByRole("checkbox");
    expect(boxes.map((box) => box.getAttribute("aria-checked"))).toEqual(["true", "true", "true", "false"]);
    const ben = await within(dialog).findByRole("combobox", { name: "Ben's voice" });
    const ada = within(dialog).getByRole("combobox", { name: "Ada's voice" }) as HTMLSelectElement;
    expect(ada.value).toBe("builtin:calm-older-woman");
    expect((ben as HTMLSelectElement).value).toBe("builtin:relaxed-man");
    // A voice being tried is not offered until it is kept.
    expect(within(ada).queryByText("New voice")).toBeNull();
    expect(within(dialog).getByText(/Drama 3 reads about 13,500 characters/)).toBeTruthy();

    fireEvent.click(boxes[1]!);
    fireEvent.click(boxes[3]!);
    fireEvent.change(ben, { target: { value: "saved:v1" } });
    fireEvent.click(within(dialog).getByRole("button", { name: /Make podcast/ }));
    await vi.waitFor(() => expect(onOpenPodcast).toHaveBeenCalledWith("podcast-9"));
    expect(writePodcast).toHaveBeenCalledWith("subject-1", ["lesson-1", "lesson-3", "lesson-4"], 15, [
      "builtin:calm-older-woman",
      "saved:v1",
    ]);
  });

  it("keeps the dialog open with the reason when writing fails", async () => {
    vi.mocked(writePodcast).mockRejectedValue(new Error("No AI model is set up for documents."));
    render(<PodcastsSection headingId="p" subjectId="subject-1" lessons={[lesson(1)]} onOpenPodcast={vi.fn()} />);
    fireEvent.click(await screen.findByRole("button", { name: /Make a podcast/ }));
    const dialog = await screen.findByRole("dialog", { name: "Make a podcast" });
    await within(dialog).findByRole("combobox", { name: "Ada's voice" });
    fireEvent.click(within(dialog).getByRole("button", { name: /Make podcast/ }));
    expect(
      await within(dialog).findByText("The podcast was not written: No AI model is set up for documents."),
    ).toBeTruthy();
  });

  it("offers nothing without written lessons or podcasts", async () => {
    const { container } = render(
      <PodcastsSection headingId="p" subjectId="subject-1" lessons={[]} onOpenPodcast={vi.fn()} />,
    );
    await vi.waitFor(() => expect(listSubjectPodcasts).toHaveBeenCalled());
    expect(container.textContent).toBe("");
  });
});
