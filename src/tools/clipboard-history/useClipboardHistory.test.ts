import { describe, expect, it } from "vitest";
import type { ClipboardRecord } from "@/core/ipc/contracts";
import { filterRecords, formatTime, previewText } from "./useClipboardHistory";

const rec = (
  id: string,
  content: string,
  pinned = false,
  createdAt = Date.now()
): ClipboardRecord => ({
  id,
  kind: "text",
  content,
  preview: content.slice(0, 10),
  pinned,
  createdAt,
});

describe("useClipboardHistory", () => {
  it("搜索过滤", () => {
    const rs = [rec("1", "hello world"), rec("2", "你好世界")];
    expect(filterRecords(rs, "hello")).toHaveLength(1);
    expect(filterRecords(rs, "世界")).toHaveLength(1);
    expect(filterRecords(rs, "")).toHaveLength(2);
  });

  it("置顶优先排序", () => {
    const rs = [rec("1", "a", false, 100), rec("2", "b", true, 200), rec("3", "c", false, 300)];
    const sorted = filterRecords(rs, "");
    expect(sorted[0].id).toBe("2");
  });

  it("预览截断", () => {
    expect(previewText("多行\n第二行")).toBe("多行");
    expect(previewText("x".repeat(200))).toHaveLength(121);
    expect(previewText("")).toBe("(空白内容)");
  });

  it("相对时间", () => {
    expect(formatTime(Date.now() - 5000)).toBe("刚刚");
    expect(formatTime(Date.now() - 5 * 60_000)).toBe("5 分钟前");
    expect(formatTime(Date.now() - 3 * 3600_000)).toBe("3 小时前");
  });
});
