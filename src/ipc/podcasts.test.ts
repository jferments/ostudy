import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { deletePodcast, getPodcast, listSubjectPodcasts, narratePodcastTurn, writePodcast } from "./podcasts";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

describe("podcast IPC", () => {
  const subjectId = "5".repeat(32);
  const podcastId = "a".repeat(32);

  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("lists, reads and deletes a subject's podcasts", async () => {
    await listSubjectPodcasts(subjectId);
    expect(invoke).toHaveBeenCalledWith("list_subject_podcasts", { input: { subjectId } });
    await getPodcast(podcastId);
    expect(invoke).toHaveBeenCalledWith("get_podcast", { input: { podcastId } });
    await deletePodcast(podcastId);
    expect(invoke).toHaveBeenCalledWith("delete_podcast", { input: { podcastId } });
  });

  it("writes a podcast from lessons in order with each host's voice", async () => {
    const lessonIds = ["b".repeat(32), "c".repeat(32)];
    const operationId = "d".repeat(32);
    await writePodcast(subjectId, lessonIds, 15, ["builtin:calm-older-woman", "builtin:relaxed-man"], operationId);
    expect(invoke).toHaveBeenCalledWith("write_podcast", {
      input: {
        subjectId,
        lessonIds,
        targetMinutes: 15,
        voices: ["builtin:calm-older-woman", "builtin:relaxed-man"],
        operationId,
      },
    });
  });

  it("speaks one turn by its key", async () => {
    await narratePodcastTurn(podcastId, "0.3");
    expect(invoke).toHaveBeenCalledWith("narrate_podcast_turn", { input: { podcastId, turnKey: "0.3" } });
  });
});
