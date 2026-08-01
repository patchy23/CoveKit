import { describe, expect, it } from "vitest";
import { formatJson, isValidJson, minifyJson, positionToLineCol } from "./useFormat";

describe("useFormat", () => {
  it("格式化 JSON：缩进 2 空格", () => {
    const r = formatJson('{"a":1,"b":[1,2]}');
    expect(r.ok).toBe(true);
    expect(r.output).toBe('{\n  "a": 1,\n  "b": [\n    1,\n    2\n  ]\n}');
  });

  it("格式化 JSON：自定义缩进", () => {
    const r = formatJson('{"a":1}', 4);
    expect(r.ok).toBe(true);
    expect(r.output).toBe('{\n    "a": 1\n}');
  });

  it("压缩 JSON", () => {
    const r = minifyJson('{\n  "a": 1\n}');
    expect(r.ok).toBe(true);
    expect(r.output).toBe('{"a":1}');
  });

  it("非法 JSON：返回错误与行号", () => {
    const r = formatJson('{\n  "a": 1,\n}');
    expect(r.ok).toBe(false);
    expect(r.error?.line).toBeGreaterThan(0);
    expect(r.error?.message).toBeTruthy();
  });

  it("isValidJson 边界", () => {
    expect(isValidJson('{"a":1}')).toBe(true);
    expect(isValidJson("{a:1}")).toBe(false);
    expect(isValidJson("")).toBe(false);
  });

  it("positionToLineCol 定位", () => {
    expect(positionToLineCol("ab\ncd", 4)).toEqual({ line: 2, col: 2 });
    expect(positionToLineCol("abc", 0)).toEqual({ line: 1, col: 1 });
  });
});
