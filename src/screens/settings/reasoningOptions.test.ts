import { describe, expect, it } from "vitest";

import type { AiProviderModelReasoning } from "../../ipc/aiProfiles";
import {
  compatibleReasoning,
  reasoningChoices,
  reasoningValidationMessage,
} from "./reasoningOptions";

const optionalHighOnly: AiProviderModelReasoning = {
  supportedEfforts: ["high", "xhigh"],
  defaultEffort: "high",
  defaultEnabled: true,
  supportsMaxTokens: false,
  mandatory: false,
};

const mandatoryHighOnly: AiProviderModelReasoning = {
  ...optionalHighOnly,
  mandatory: true,
};

describe("reasoningOptions", () => {
  it("only presents model-declared efforts plus automatic and optional off", () => {
    expect(reasoningChoices(optionalHighOnly)).toEqual(["automatic", "none", "high", "xhigh"]);
    expect(reasoningChoices(optionalHighOnly)).not.toContain("low");
  });

  it("marks a stale unsupported saved effort instead of silently presenting it as valid", () => {
    const message = reasoningValidationMessage("low", optionalHighOnly, "DeepSeek V4 Flash");
    expect(message).toContain("Low reasoning is not supported by DeepSeek V4 Flash");
    expect(message).toContain("Automatic, Off, High, Extra high");
    expect(compatibleReasoning("low", optionalHighOnly)).toBe("automatic");
  });

  it("keeps explicit off for optional reasoning even when off is not listed as an effort", () => {
    expect(compatibleReasoning("none", optionalHighOnly)).toBe("none");
    expect(reasoningValidationMessage("none", optionalHighOnly, "Optional model")).toBeNull();
  });

  it("does not offer off for mandatory reasoning", () => {
    expect(reasoningChoices(mandatoryHighOnly)).toEqual(["automatic", "high", "xhigh"]);
    expect(compatibleReasoning("none", mandatoryHighOnly)).toBe("automatic");
    expect(reasoningValidationMessage("none", mandatoryHighOnly, "Mandatory model")).toContain(
      "Off reasoning is not supported",
    );
  });

  it("retains the generic effort menu when provider metadata does not declare supported efforts", () => {
    expect(reasoningChoices(null)).toEqual([
      "automatic",
      "none",
      "minimal",
      "low",
      "medium",
      "high",
      "xhigh",
      "max",
    ]);
    expect(reasoningValidationMessage("low", null, "Unknown model")).toBeNull();
  });
});
