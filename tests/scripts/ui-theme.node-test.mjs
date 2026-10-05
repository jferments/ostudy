import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import test from "node:test";
import { renderTailwindTheme, themeAliases, tokenVariable } from "../../scripts/ui/tailwind-theme.mjs";

const root = new URL("../../src/ui/tokens/", import.meta.url);
const repository = new URL("../../", import.meta.url);
function names(value, prefix = "", result = new Set()) {
  for (const [key, child] of Object.entries(value)) {
    if (key.startsWith("$")) continue;
    const name = prefix ? `${prefix}.${key}` : key;
    if (Object.hasOwn(child, "$value")) result.add(name);
    else names(child, name, result);
  }
  return result;
}
async function tokens(path) {
  return names(JSON.parse(await readFile(new URL(path, root), "utf8")));
}

for (const theme of ["light", "dark", "console", "synthwave"]) {
  test(`every shadcn semantic alias resolves from the ${theme} oStudy theme`, async () => {
    const available = new Set([
      ...await tokens("source/reference.tokens.json"),
      ...await tokens("source/component.tokens.json"),
      ...await tokens(`themes/${theme}.tokens.json`),
    ]);
    const css = renderTailwindTheme(available);
    assert.match(css, /@theme inline/u);
    for (const [alias, token] of Object.entries(themeAliases)) {
      assert.ok(available.has(token), `${alias} -> ${token}`);
      assert.ok(css.includes(`--${alias}: var(${tokenVariable(token)});`));
    }
    assert.ok(!/(?:--muted|--accent|--radius|--sidebar)\s*:/u.test(css));
    assert.ok(!/#[a-f0-9]{3,8}\b/iu.test(css));
    assert.ok(!css.includes(".dark"));
  });
}

test("missing token mappings fail instead of emitting broken CSS", () => {
  assert.throws(() => renderTailwindTheme(new Set()), /unknown oStudy tokens/u);
});

test("muted and accent utility semantics do not overwrite old short aliases", () => {
  assert.equal(themeAliases["color-muted"], "color.surface.soft");
  assert.equal(themeAliases["color-muted-foreground"], "color.text.muted");
  assert.equal(themeAliases["color-accent"], "color.accent.soft");
  assert.equal(themeAliases["color-primary-foreground"], "color.accent.on");
  assert.equal(themeAliases["color-destructive-foreground"], "color.status.dangerOn");
});

test("variable normalization agrees with the existing token generator", () => {
  assert.equal(tokenVariable("shadow.level1"), "--ostudy-shadow-level-1");
  assert.equal(tokenVariable("color.focus.ring"), "--ostudy-color-focus-ring");
});

async function productionSources(directory, pattern) {
  const entries = await readdir(new URL(directory, repository), { recursive: true });
  return entries.filter((path) => pattern.test(path) && !/\.(?:test|spec)\.[jt]sx?$/u.test(path)
    && !path.includes("tokens/generated/"));
}

test("layout breakpoints come from one source for JavaScript, Tailwind and CSS Modules", async () => {
  const queries = await readFile(new URL("generated/viewport.ts", root), "utf8");
  const variants = await readFile(new URL("generated/viewport-variants.css", root), "utf8");
  const compact = /compact: "([^"]+)"/u.exec(queries)?.[1];
  assert.equal(compact, "(width < 760px)");
  assert.equal(/belowWide: "([^"]+)"/u.exec(queries)?.[1], "(width < 1180px)");
  assert.ok(variants.includes(`@custom-variant compact {\n  @media ${compact} {`));
  for (const path of await productionSources("src/", /\.(?:tsx?|css)$/u)) {
    const source = await readFile(new URL(`src/${path}`, repository), "utf8");
    assert.doesNotMatch(source, /max-\[760px\]|max-width:\s*760px|max-width:\s*1180px/u, `${path} repeats a layout breakpoint`);
  }
});

test("stacking uses the documented layer tokens instead of numeric z-index values", async () => {
  for (const path of await productionSources("src/", /\.(?:tsx?|css)$/u)) {
    const source = await readFile(new URL(`src/${path}`, repository), "utf8");
    assert.doesNotMatch(source, /tw:(?:[^\s"'`]*:)?-?z-(?:\d|\[-?\d)|zIndex:\s*-?\d|z-index:\s*-?\d/u, `${path} uses a numeric z-index`);
  }
});
