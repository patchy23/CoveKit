import { describe, expect, it } from "vitest";
import { contrastRatio, hexToRgb, hslToHex, parseColorInput, rgbToHex, rgbToHsl } from "./useColor";

describe("useColor", () => {
  it("hex ↔ rgb 互转", () => {
    expect(hexToRgb("#F0562C")).toEqual({ r: 240, g: 86, b: 44 });
    expect(hexToRgb("#f56")).toEqual({ r: 255, g: 85, b: 102 }); // f→ff 5→55 6→66
    expect(rgbToHex({ r: 194, g: 65, b: 12 })).toBe("#c2410c");
    expect(hexToRgb("无效")).toBeNull();
  });

  it("解析 rgb() 与简写", () => {
    expect(parseColorInput("rgb(12, 34, 56)")).toEqual({
      hex: "#0c2238",
      rgb: { r: 12, g: 34, b: 56 },
    });
    expect(parseColorInput("#fff")?.hex).toBe("#ffffff");
    expect(parseColorInput("oops")).toBeNull();
  });

  it("hsl 转换往返（整数 HSL 精度 ±1）", () => {
    const hex = "#F0562C";
    const rgb = hexToRgb(hex)!;
    const hsl = rgbToHsl(rgb);
    const back = hexToRgb(hslToHex(hsl))!;
    expect(Math.abs(back.r - rgb.r)).toBeLessThanOrEqual(1);
    expect(Math.abs(back.g - rgb.g)).toBeLessThanOrEqual(1);
    expect(Math.abs(back.b - rgb.b)).toBeLessThanOrEqual(1);
  });

  it("WCAG 对比度：黑白最高，同色为 1", () => {
    expect(contrastRatio({ r: 0, g: 0, b: 0 }, { r: 255, g: 255, b: 255 })).toBeGreaterThan(20);
    expect(contrastRatio({ r: 10, g: 10, b: 10 }, { r: 10, g: 10, b: 10 })).toBe(1);
  });
});
