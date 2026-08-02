import { describe, expect, it } from "vitest";
import { formatBytes, formatHeaders, isValidUrl, looksLikeJson, parseHeaders } from "./useHttp";

describe("useHttp", () => {
  it("headers 文本解析", () => {
    expect(parseHeaders("Content-Type: application/json\nX-Api-Key: abc\n\n无冒号行")).toEqual([
      ["Content-Type", "application/json"],
      ["X-Api-Key", "abc"],
    ]);
  });

  it("headers 格式化往返", () => {
    expect(
      formatHeaders([
        ["a", "1"],
        ["b", "2"],
      ])
    ).toBe("a: 1\nb: 2");
  });

  it("字节格式化", () => {
    expect(formatBytes(500)).toBe("500 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(3 * 1024 * 1024)).toBe("3.00 MB");
  });

  it("JSON 启发式与 URL 校验", () => {
    expect(looksLikeJson('{"a":1}')).toBe(true);
    expect(looksLikeJson("<html>")).toBe(false);
    expect(isValidUrl("https://example.com")).toBe(true);
    expect(isValidUrl("ws://localhost:8080")).toBe(true);
    expect(isValidUrl("ftp://x")).toBe(false);
    expect(isValidUrl("不是url")).toBe(false);
  });
});
