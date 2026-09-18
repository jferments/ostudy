import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const configuration = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const directives = new Map<string, string[]>(configuration.app.security.csp.split(";")
  .map((part: string) => part.trim().split(/\s+/u))
  .filter((tokens: string[]) => tokens[0])
  .map(([name, ...sources]: string[]) => [name, sources]));

describe("packaged image-occlusion content policy", () => {
  it("allows locally generated PNG Blob URLs in both SVG and ordinary image elements", () => {
    expect(directives.get("img-src")).toContain("blob:");
  });

  it("does not enable remote images, embedded documents or executable Blob URLs", () => {
    const images = directives.get("img-src") ?? [];
    expect(images).not.toContain("*");
    expect(images).not.toContain("https:");
    expect(images).not.toContain("http:");
    expect(directives.get("object-src")).toEqual(["'none'"]);
    expect(directives.get("frame-src")).toEqual(["'none'"]);
    expect(directives.get("script-src") ?? directives.get("default-src")).toEqual(["'self'"]);
  });
});
