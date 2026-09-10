import type { AiProviderModel, AiReasoningEffort } from "../../ipc/aiProfiles";

const ALL_REASONING_EFFORTS: AiReasoningEffort[] = [
  "minimal",
  "low",
  "medium",
  "high",
  "xhigh",
  "max",
];

export function reasoningChoices(
  metadata: AiProviderModel["reasoning"] | null,
): AiReasoningEffort[] {
  const choices: AiReasoningEffort[] = ["automatic"];
  if (!metadata?.mandatory) choices.push("none");
  if (metadata?.supportedEfforts) {
    choices.push(...metadata.supportedEfforts);
  } else {
    choices.push(...ALL_REASONING_EFFORTS);
  }
  return [...new Set(choices)];
}

export function compatibleReasoning(
  current: AiReasoningEffort,
  metadata: AiProviderModel["reasoning"] | null,
): AiReasoningEffort {
  if (current === "automatic") return current;
  if (current === "none") return metadata?.mandatory ? "automatic" : "none";
  if (metadata?.supportedEfforts && !metadata.supportedEfforts.includes(current)) {
    return "automatic";
  }
  return current;
}

export function reasoningValidationMessage(
  current: AiReasoningEffort,
  metadata: AiProviderModel["reasoning"] | null,
  modelLabel: string,
): string | null {
  if (!metadata || current === "automatic") return null;
  if (reasoningChoices(metadata).includes(current)) return null;
  const choices = reasoningChoices(metadata).map(reasoningLabel).join(", ");
  return `${reasoningLabel(current)} reasoning is not supported by ${modelLabel}. Choose one of: ${choices}.`;
}

export function reasoningLabel(value: AiReasoningEffort): string {
  if (value === "automatic") return "Automatic";
  if (value === "none") return "Off";
  return value === "xhigh"
    ? "Extra high"
    : `${value[0]?.toUpperCase() ?? ""}${value.slice(1)}`;
}
