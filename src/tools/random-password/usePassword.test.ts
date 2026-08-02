import { describe, expect, it } from "vitest";
import { estimateStrength, generatePassword } from "./usePassword";

describe("usePassword", () => {
  it("生成指定长度且包含各字符集", () => {
    const pw = generatePassword({
      length: 16,
      upper: true,
      lower: true,
      digits: true,
      symbols: true,
      excludeAmbiguous: false,
    });
    expect(pw).toHaveLength(16);
    expect(/[A-Z]/.test(pw)).toBe(true);
    expect(/[a-z]/.test(pw)).toBe(true);
    expect(/[0-9]/.test(pw)).toBe(true);
    expect(/[^A-Za-z0-9]/.test(pw)).toBe(true);
  });

  it("排除易混字符", () => {
    const pw = generatePassword({
      length: 32,
      upper: true,
      lower: true,
      digits: true,
      symbols: true,
      excludeAmbiguous: true,
    });
    expect(/[0O1lI|`'"\\/ ]/.test(pw)).toBe(false);
  });

  it("空选项返回空串", () => {
    expect(
      generatePassword({
        length: 12,
        upper: false,
        lower: false,
        digits: false,
        symbols: false,
        excludeAmbiguous: false,
      })
    ).toBe("");
  });

  it("强度分级", () => {
    expect(estimateStrength("").score).toBe(0);
    expect(estimateStrength("abc").score).toBe(1);
    expect(estimateStrength("a1B2c3d4").score).toBeGreaterThanOrEqual(1);
    expect(estimateStrength("aB3$kL9#mN2$qW7@xR").score).toBeGreaterThanOrEqual(3);
  });
});
