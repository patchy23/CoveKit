import { describe, expect, it } from "vitest";
import {
  countErrors,
  countMappings,
  entriesToText,
  isValidHostname,
  isValidIp,
  parseEntries,
  parseHostsLines,
  validateEntry,
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

  it("条目解析：注释掉的映射 = 未勾选条目", () => {
    const content = ["# 说明注释", "", "127.0.0.1 localhost", "# 0.0.0.0 ads.example.com"].join(
      "\n"
    );
    const entries = parseEntries(content);
    expect(entries).toHaveLength(4);
    expect(entries[0].raw).toBe("# 说明注释"); // 纯注释原样
    expect(entries[1].raw).toBe("");
    expect(entries[2]).toMatchObject({ ip: "127.0.0.1", hosts: ["localhost"], enabled: true });
    // 被注释的映射行 = 未勾选条目（enabled=false）
    expect(entries[3]).toMatchObject({ ip: "0.0.0.0", hosts: ["ads.example.com"], enabled: false });
  });

  it("条目重组：往返一致（未勾选条目恢复 # 前缀）", () => {
    const content = [
      "# 说明注释",
      "",
      "127.0.0.1 localhost # 本机",
      "# 0.0.0.0 ads.example.com",
    ].join("\n");
    expect(entriesToText(parseEntries(content))).toBe(content);
  });

  it("条目编辑：改 IP/禁用/加注释后重组", () => {
    const entries = parseEntries("127.0.0.1 localhost");
    entries[0].ip = "0.0.0.0";
    entries[0].enabled = false;
    entries[0].comment = "# 屏蔽";
    expect(entriesToText(entries)).toBe("# 0.0.0.0 localhost # 屏蔽");
  });

  it("条目即时校验", () => {
    expect(validateEntry("1.2.3.4", ["a.com"]).valid).toBe(true);
    expect(validateEntry("999.1.1.1", ["a.com"]).valid).toBe(false);
    expect(validateEntry("1.2.3.4", []).valid).toBe(false);
    expect(validateEntry("1.2.3.4", ["bad host"]).valid).toBe(false);
  });

  it("被注释的映射行行尾注释正确解析到注释列", () => {
    const entries = parseEntries("# 127.0.0.1 example.com # 广告屏蔽");
    expect(entries[0]).toMatchObject({
      enabled: false,
      ip: "127.0.0.1",
      hosts: ["example.com"],
      comment: "# 广告屏蔽",
    });
  });
});
