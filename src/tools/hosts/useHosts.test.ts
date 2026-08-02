import { describe, expect, it } from "vitest";
import {
  countErrors,
  countMappings,
  isValidHostname,
  isValidIp,
  parseHostsLines,
} from "./useHosts";

describe("useHosts", () => {
  it("IP 校验", () => {
    expect(isValidIp("127.0.0.1")).toBe(true);
    expect(isValidIp("255.255.255.255")).toBe(true);
    expect(isValidIp("256.1.1.1")).toBe(false);
    expect(isValidIp("1.2.3")).toBe(false);
    expect(isValidIp("1.2.3.4.5")).toBe(false);
    expect(isValidIp("a.b.c.d")).toBe(false);
  });

  it("主机名校验", () => {
    expect(isValidHostname("localhost")).toBe(true);
    expect(isValidHostname("example.com")).toBe(true);
    expect(isValidHostname("my-sub.domain.com")).toBe(true);
    expect(isValidHostname("bad host")).toBe(false);
    expect(isValidHostname("-bad.com")).toBe(false);
  });

  it("行解析与校验", () => {
    const content = [
      "# 注释行",
      "",
      "127.0.0.1 localhost",
      "0.0.0.0 example.com # 屏蔽广告",
      "999.1.1.1 bad-ip",
      "127.0.0.1",
    ].join("\n");
    const lines = parseHostsLines(content);
    expect(lines).toHaveLength(6);
    expect(countErrors(lines)).toBe(2); // bad-ip + 缺主机名
    expect(countMappings(lines)).toBe(2); // localhost + example.com
    expect(lines[3].comment).toBe("# 屏蔽广告");
    expect(lines[5].error).toContain("主机名");
  });

  it("空内容", () => {
    expect(parseHostsLines("")).toHaveLength(1); // 一个空行
    expect(countErrors(parseHostsLines(""))).toBe(0);
  });
});
