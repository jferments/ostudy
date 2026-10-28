import { renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { listAiProviderProfiles } from "../ipc/aiProfiles";
import { AI_FEATURES_WITHOUT_SERVICE_STORAGE_KEY, useAiFeaturesSource } from "./AiFeaturesProvider";

vi.mock("../ipc/aiProfiles", () => ({ listAiProviderProfiles: vi.fn() }));

afterEach(() => window.localStorage.clear());

describe("useAiFeaturesSource", () => {
  it("hides AI features until an enabled service exists", async () => {
    vi.mocked(listAiProviderProfiles).mockResolvedValue([]);
    const { result } = renderHook(() => useAiFeaturesSource("home"));
    await waitFor(() => expect(result.current.known).toBe(true));
    expect(result.current.enabled).toBe(false);
  });

  it("shows them without a service for the native automation profile", async () => {
    window.localStorage.setItem(AI_FEATURES_WITHOUT_SERVICE_STORAGE_KEY, "shown");
    vi.mocked(listAiProviderProfiles).mockResolvedValue([]);
    const { result } = renderHook(() => useAiFeaturesSource("home"));
    await waitFor(() => expect(result.current.known).toBe(true));
    expect(result.current.enabled).toBe(true);
    expect(result.current.hasServices).toBe(false);
  });
});
