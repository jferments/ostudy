const EXPLICIT_SCHEME = /^[a-z][a-z0-9+.-]*:/iu;
const SUPPORTED_SCHEME = /^(https?:\/\/|mailto:|tel:)/iu;
const UNSAFE_LINK_CHARACTER = /[\s\u0000-\u001f\u007f]/u;

export function normalizeExternalLink(value: string): string {
  const trimmed = value.trim();
  if (trimmed === "" || SUPPORTED_SCHEME.test(trimmed)) {
    return trimmed;
  }

  // Preserve an explicitly supplied unsupported scheme so validation can
  // reject it. Prefixing it with https:// would turn e.g. javascript: into a
  // misleading but technically different URL.
  if (EXPLICIT_SCHEME.test(trimmed)) {
    return trimmed;
  }

  return `https://${trimmed}`;
}

export function isAllowedExternalLink(value: string): boolean {
  if (
    value === "" ||
    value !== value.trim() ||
    value.length > 2_000 ||
    UNSAFE_LINK_CHARACTER.test(value)
  ) {
    return false;
  }

  const lower = value.toLocaleLowerCase();
  if (lower.startsWith("mailto:")) {
    return value.slice("mailto:".length).length > 0;
  }
  if (lower.startsWith("tel:")) {
    return value.slice("tel:".length).length > 0;
  }
  const schemeLength = lower.startsWith("https://")
    ? "https://".length
    : lower.startsWith("http://")
      ? "http://".length
      : 0;
  if (schemeLength === 0) return false;

  const authority = value.slice(schemeLength).split(/[/?#]/u, 1)[0] ?? "";
  if (authority === "") return false;

  try {
    const parsed = new URL(value);
    return (
      (parsed.protocol === "http:" || parsed.protocol === "https:") &&
      parsed.hostname.length > 0
    );
  } catch {
    return false;
  }
}
