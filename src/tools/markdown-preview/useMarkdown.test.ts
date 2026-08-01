import { describe, expect, it } from "vitest";
import { renderMarkdown, sanitizeHtml } from "./useMarkdown";

describe("useMarkdown", () => {
  it("标题与加粗渲染", () => {
    const html = renderMarkdown("# 标题\n\n**粗体**");
    expect(html).toContain("<h1");
    expect(html).toContain("<strong>粗体</strong>");
  });

  it("GFM 表格与任务列表", () => {
    const html = renderMarkdown("- [x] 完成\n- [ ] 待办");
    expect(html).toContain('type="checkbox"');
  });

  it("剥离脚本标签", () => {
    const html = renderMarkdown("<script>alert(1)</script>text");
    expect(html).not.toContain("<script>");
    expect(html).toContain("text");
  });

  it("sanitizeHtml 剥离事件属性", () => {
    const out = sanitizeHtml('<p onclick="x()">hi</p>');
    expect(out).not.toContain("onclick");
  });

  it("代码块渲染", () => {
    const html = renderMarkdown("```js\nconst a = 1;\n```");
    expect(html).toContain("<code");
  });
});
