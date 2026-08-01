import { describe, expect, it } from "vitest";
import { testRegex, validatePattern } from "./useRegex";

describe("useRegex", () => {
  it("基础匹配与计数", () => {
    const r = testRegex("\\d+", "", "a1 b22 c333");
    expect(r.ok).toBe(true);
    expect(r.count).toBe(3);
    expect(r.matches[1]).toEqual({ text: "22", index: 4 });
  });

  it("全局标志自动补全", () => {
    const r = testRegex("a", "", "aaa");
    expect(r.count).toBe(3);
  });

  it("非法正则返回错误", () => {
    const r = testRegex("([", "", "x");
    expect(r.ok).toBe(false);
    expect(r.error).toBeTruthy();
  });

  it("空匹配不死循环", () => {
    const r = testRegex("a*", "", "bbb");
    expect(r.ok).toBe(true);
    expect(r.count).toBeGreaterThanOrEqual(1);
  });

  it("validatePattern", () => {
    expect(validatePattern("\\d+", "")).toBeNull();
    expect(validatePattern("([", "")).toBeTruthy();
  });
});
