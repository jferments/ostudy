import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { audioFeedback, hapticFeedback } from "../../platform/feedback";
import { ExperienceProvider, useExperience } from "./ExperienceProvider";

vi.mock("../../platform/feedback", () => ({
  audioFeedback: { play: vi.fn().mockResolvedValue(undefined) },
  hapticFeedback: { pulse: vi.fn().mockResolvedValue(undefined) },
}));

function Trigger() {
  const experience = useExperience();
  return (
    <button type="button" onClick={() => experience.emit({ type: "session_completed" })}>
      Complete session
    </button>
  );
}

describe("ExperienceProvider", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    document.documentElement.dataset.motion = "full";
    document.documentElement.dataset.celebrations = "enabled";
  });

  afterEach(() => {
    delete document.documentElement.dataset.motion;
    delete document.documentElement.dataset.celebrations;
  });

  it("interprets completion as a capped visual, haptic, and audio event", () => {
    const { container } = render(
      <ExperienceProvider>
        <Trigger />
      </ExperienceProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Complete session" }));

    const layer = container.querySelector('[data-celebrate="true"]');
    expect(layer).not.toBeNull();
    expect(layer?.childElementCount).toBe(24);
    expect(hapticFeedback.pulse).toHaveBeenCalledWith("strong");
    expect(audioFeedback.play).toHaveBeenCalledWith("session-complete");
  });

  it("keeps semantic feedback but omits particles when motion is reduced", () => {
    document.documentElement.dataset.motion = "reduced";
    const { container } = render(
      <ExperienceProvider>
        <Trigger />
      </ExperienceProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Complete session" }));

    expect(container.querySelector('[data-celebrate="true"]')).toBeNull();
    expect(hapticFeedback.pulse).toHaveBeenCalledWith("strong");
    expect(audioFeedback.play).toHaveBeenCalledWith("session-complete");
  });
});
