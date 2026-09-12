/** Match the required offline font fallback after production CSS minification. */
export function hasPackagedFontFallback(source, property) {
  if (property !== "--ui-font-family" && property !== "--card-font-family") {
    throw new Error("Unsupported font property");
  }
  const family = String.raw`(?:"IBM Plex Sans"|'IBM Plex Sans'|IBM\s+Plex\s+Sans)`;
  const declaration = new RegExp(
    String.raw`(?:^|[;{])\s*${property}\s*:\s*${family}\s*,\s*var\(\s*--ostudy-font-family-system\s*\)\s*(?:[;}]|$)`,
    "u",
  );
  return declaration.test(source.replace(/\/\*[\s\S]*?\*\//gu, ""));
}
