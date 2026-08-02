import { describe, expect, it } from "vitest";
import { formatXml, isValidXml, minifyXml } from "./useXml";

describe("useXml", () => {
  it("格式化缩进", () => {
    const r = formatXml('<a><b x="1"><c>text</c></b><b/></a>');
    expect(r.ok).toBe(true);
    expect(r.output).toBe(
      ["<a>", '  <b x="1">', "    <c>text</c>", "  </b>", "  <b/>", "</a>"].join("\n")
    );
  });

  it("保留声明与注释", () => {
    const r = formatXml('<?xml version="1.0"?><a><!-- 注释 --><b>1</b></a>');
    expect(r.ok).toBe(true);
    expect(r.output).toContain('<?xml version="1.0"?>');
    expect(r.output).toContain("<!-- 注释 -->");
  });

  it("非法 XML 报错", () => {
    const r = formatXml("<a><b></a>");
    expect(r.ok).toBe(false);
    expect(r.error).toBeTruthy();
  });

  it("压缩与校验", () => {
    expect(minifyXml("<a>\n  <b>1</b>\n</a>")).toBe("<a><b>1</b></a>");
    expect(isValidXml("<a><b/></a>")).toBe(true);
    expect(isValidXml("<a><b></a>")).toBe(false);
  });
});
