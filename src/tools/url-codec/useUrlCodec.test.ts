import { describe, expect, it } from "vitest";
import { decodeUrl, encodeUrl, hasEncodedFragment } from "./useUrlCodec";

describe("useUrlCodec", () => {
  it("编码：中文与特殊字符", () => {
    expect(encodeUrl("你好 world")).toEqual({ ok: true, output: "%E4%BD%A0%E5%A5%BD%20world" });
  });

  it("解码往返", () => {
    const src = "https://example.com/搜索?q=patchy box";
    const encoded = encodeUrl(src);
    expect(decodeUrl(encoded.output).output).toBe(src);
  });

  it("非法编码输入报错", () => {
    const r = decodeUrl("%E4%BD%A0%ZZ");
    expect(r.ok).toBe(false);
    expect(r.error).toBeTruthy();
  });

  it("hasEncodedFragment 探测", () => {
    expect(hasEncodedFragment("%E4%BD%A0")).toBe(true);
    expect(hasEncodedFragment("plain text")).toBe(false);
  });
});
