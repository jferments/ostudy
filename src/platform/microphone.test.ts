import { describe, expect, it } from "vitest";

import { level, resample, toPcmBase64 } from "./microphone";

describe("microphone recordings", () => {
  it("averages a 48 kHz recording down to the 16 kHz speech rate", () => {
    const samples = Float32Array.from([0.3, 0.3, 0.3, -0.6, -0.6, -0.6, 0.9, 0.0, 0.0]);
    expect([...resample(samples, 48_000, 16_000)].map((value) => Number(value.toFixed(3)))).toEqual([0.3, -0.6, 0.3]);
  });

  it("leaves a recording at or below the speech rate as it is", () => {
    const samples = Float32Array.from([0.1, 0.2]);
    expect(resample(samples, 16_000, 16_000)).toBe(samples);
    expect(resample(samples, 8_000, 16_000)).toBe(samples);
  });

  it("writes samples as little-endian 16-bit PCM, clamping what is out of range", () => {
    const bytes = Uint8Array.from(atob(toPcmBase64(Float32Array.from([0, 1, -1, 2]))), (char) => char.charCodeAt(0));
    const view = new DataView(bytes.buffer);
    expect([0, 1, 2, 3].map((index) => view.getInt16(index * 2, true))).toEqual([0, 32_767, -32_768, 32_767]);
  });

  it("measures silence as zero and loud speech as at most one", () => {
    expect(level(new Float32Array(128))).toBe(0);
    expect(level(Float32Array.from({ length: 128 }, () => 0.9))).toBe(1);
  });
});
