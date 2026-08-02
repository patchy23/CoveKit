import { describe, expect, it } from "vitest";
import { renderQrDataUrl, suggestLevel } from "./useQr";

describe("useQr", () => {
  it("生成 PNG dataURL", async () => {
    const url = await renderQrDataUrl("https://example.com");
    expect(url.startsWith("data:image/png;base64,")).toBe(true);
  });

  it("容错级别建议随长度降级", () => {
    expect(suggestLevel("短文本")).toBe("H");
    expect(suggestLevel("x".repeat(300))).toBe("Q");
    expect(suggestLevel("x".repeat(800))).toBe("L");
  });
});
