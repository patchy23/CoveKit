import { describe, expect, it } from "vitest";

// M0 冒烟测试：验证 Vitest 链路可用；M1 起按 tools/<id>/useXxx.ts 补纯函数单测
describe("sanity", () => {
  it("框架可运行", () => {
    expect(1 + 1).toBe(2);
  });
});
