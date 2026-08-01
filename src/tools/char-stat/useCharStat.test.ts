import { describe, expect, it } from "vitest";
import { statText } from "./useCharStat";

describe("useCharStat", () => {
  it("中英文混合统计", () => {
    const s = statText("你好 world\n第二行");
    expect(s.chars).toBe(12); // 你 好 空格 w o r l d \n 第 二 行 = 12 个码点
    expect(s.lines).toBe(2);
    expect(s.words).toBe(3); // 你好 / world / 第二行
    expect(s.cjk).toBe(5); // 你好第二行
    expect(s.bytes).toBeGreaterThan(s.chars); // 中文占 3 字节
  });

  it("空文本", () => {
    const s = statText("");
    expect(s).toEqual({ chars: 0, charsNoSpace: 0, words: 0, lines: 0, bytes: 0, cjk: 0 });
  });

  it("emoji 按码点计 1", () => {
    expect(statText("a👍b").chars).toBe(3);
  });
});
