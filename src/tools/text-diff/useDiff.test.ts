import { describe, expect, it } from "vitest";
import { diffLines, diffStats } from "./useDiff";

describe("useDiff", () => {
  it("完全相同", () => {
    const d = diffLines("a\nb", "a\nb");
    expect(d.every((l) => l.type === "same")).toBe(true);
    expect(diffStats(d)).toEqual({ adds: 0, removes: 0, unchanged: 2 });
  });

  it("新增行", () => {
    const d = diffLines("a", "a\nb");
    expect(d[d.length - 1]).toEqual({ type: "add", text: "b" });
  });

  it("删除行", () => {
    const d = diffLines("a\nb", "a");
    expect(d[d.length - 1]).toEqual({ type: "remove", text: "b" });
  });

  it("修改 = 删除 + 新增", () => {
    const d = diffLines("line1\nold\nline3", "line1\nnew\nline3");
    const types = d.map((l) => l.type);
    expect(types).toContain("remove");
    expect(types).toContain("add");
    expect(diffStats(d)).toEqual({ adds: 1, removes: 1, unchanged: 2 });
  });

  it("空文本对比", () => {
    expect(diffLines("", "")).toEqual([]);
    expect(diffLines("", "a")[0]).toEqual({ type: "add", text: "a" });
  });
});
