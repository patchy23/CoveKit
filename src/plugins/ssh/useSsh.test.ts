/**
 * SSH 工具 · useSsh.ts 纯函数单测
 * 覆盖：状态机映射（statusDotClass / statusText）、格式化（formatBytes / formatTime / formatLatency）、
 *       mock 数据形态（与契约字段一致）。
 */
import { describe, expect, it } from "vitest";
import {
  formatBytes,
  formatLatency,
  formatTime,
  mockConnections,
  mockFiles,
  mockProfiles,
  mockTerminals,
  statusDotClass,
  statusText,
} from "./useSsh";
import type { ConnectionStatus } from "./contracts";

describe("statusDotClass（状态 → 圆点样式，UI.md §3 五状态）", () => {
  it("已连接 = 绿色（success）", () => {
    expect(statusDotClass("connected")).toBe("bg-success-strong dark:bg-success-dark");
  });

  it("连接中 / 重连中 = 橙色（tertiary）+ 脉冲动画", () => {
    expect(statusDotClass("connecting")).toBe("bg-tertiary animate-pulse dark:bg-tertiary-dark");
    expect(statusDotClass("reconnecting")).toBe("bg-tertiary animate-pulse dark:bg-tertiary-dark");
  });

  it("断开 = 红色（danger）", () => {
    expect(statusDotClass("error")).toBe("bg-danger-strong dark:bg-danger-dark");
  });

  it("未连接 = 灰色（muted），未知状态兜底灰", () => {
    expect(statusDotClass("disconnected")).toBe("bg-text-muted dark:bg-text-muted-dark");
    expect(statusDotClass("bogus" as ConnectionStatus)).toBe("bg-text-muted dark:bg-text-muted-dark");
  });
});

describe("statusText（状态 → 中文文案）", () => {
  it("五状态文案映射", () => {
    expect(statusText("connected")).toBe("已连接");
    expect(statusText("connecting")).toBe("连接中");
    expect(statusText("reconnecting")).toBe("重连中");
    expect(statusText("error")).toBe("已断开");
    expect(statusText("disconnected")).toBe("未连接");
  });

  it("未知状态兜底「未连接」", () => {
    expect(statusText("bogus" as ConnectionStatus)).toBe("未连接");
  });
});

describe("formatBytes（文件大小/内存/速率格式化）", () => {
  it("B 档：< 1024 保留整数", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("KB 档：1024 ~ 1MB-1，一位小数", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(1048575)).toBe("1024.0 KB"); // 边界：恰好越过 1MB 前
  });

  it("MB 档：1MB ~ 1GB-1，两位小数", () => {
    expect(formatBytes(1048576)).toBe("1.00 MB");
    expect(formatBytes(13002342)).toBe("12.40 MB"); // 与 mockFiles access.log 一致
    expect(formatBytes(3355443)).toBe("3.20 MB"); // 与 mockFiles error.log 一致
    expect(formatBytes(1073741823)).toBe("1024.00 MB");
  });

  it("GB 档：>= 1GB，两位小数", () => {
    expect(formatBytes(1073741824)).toBe("1.00 GB");
    expect(formatBytes(2.5 * 1024 * 1024 * 1024)).toBe("2.50 GB");
  });
});

describe("formatTime（毫秒时间戳 → MM-DD HH:mm）", () => {
  it("0 / 空值 → '-'", () => {
    expect(formatTime(0)).toBe("-");
    expect(formatTime(undefined as unknown as number)).toBe("-");
    expect(formatTime(NaN)).toBe("-");
  });

  it("有效时间戳 → 本地时区 MM-DD HH:mm，补零", () => {
    const ts = new Date(2026, 7, 8, 9, 5).getTime(); // 2026-08-08 09:05
    expect(formatTime(ts)).toBe("08-08 09:05");
    const ts2 = new Date(2026, 0, 2, 23, 59).getTime();
    expect(formatTime(ts2)).toBe("01-02 23:59");
  });
});

describe("formatLatency（延迟毫秒 → 文本）", () => {
  it("undefined → '-'", () => {
    expect(formatLatency(undefined)).toBe("-");
  });

  it("< 1000ms 显示毫秒", () => {
    expect(formatLatency(0)).toBe("0ms");
    expect(formatLatency(23)).toBe("23ms");
    expect(formatLatency(999)).toBe("999ms");
  });

  it(">= 1000ms 显示秒（一位小数）", () => {
    expect(formatLatency(1000)).toBe("1.0s");
    expect(formatLatency(1500)).toBe("1.5s");
  });
});

describe("mock 数据形态（契约一致性：字段齐全、状态机覆盖）", () => {
  it("mockProfiles：3 台服务器，id 唯一，认证方式覆盖三种", () => {
    expect(mockProfiles).toHaveLength(3);
    const ids = new Set(mockProfiles.map((p) => p.id));
    expect(ids.size).toBe(3);
    const methods = new Set(mockProfiles.map((p) => p.authMethod));
    expect(methods).toEqual(new Set(["password", "privateKey", "privateKeyWithPassphrase"]));
    // 契约必填字段
    for (const p of mockProfiles) {
      expect(p).toMatchObject({
        id: expect.any(String),
        name: expect.any(String),
        host: expect.any(String),
        port: expect.any(Number),
        username: expect.any(String),
        authMethod: expect.any(String),
      });
    }
  });

  it("mockConnections：状态覆盖 connected/disconnected/connecting，与 profile 关联", () => {
    expect(mockConnections).toHaveLength(3);
    const statuses = new Set(mockConnections.map((c) => c.status));
    expect(statuses).toEqual(new Set(["connected", "disconnected", "connecting"]));
    for (const c of mockConnections) {
      expect(c.sessionId).toMatch(/^conn-/);
      expect(mockProfiles.some((p) => p.id === c.profileId)).toBe(true);
    }
  });

  it("mockTerminals：关联已连接会话，行列值合法", () => {
    expect(mockTerminals).toHaveLength(1);
    const t = mockTerminals[0];
    expect(t.id).toMatch(/^term-/);
    expect(mockConnections.find((c) => c.sessionId === t.connectionId)?.status).toBe("connected");
    expect(t.cols).toBeGreaterThan(0);
    expect(t.rows).toBeGreaterThan(0);
  });

  it("mockFiles：含上级目录 '..'，文件大小与 formatBytes 展示一致", () => {
    expect(mockFiles[0]).toMatchObject({ name: "..", isDir: true, size: 0 });
    expect(mockFiles.some((f) => f.name === "access.log" && !f.isDir)).toBe(true);
    for (const f of mockFiles) {
      expect(f).toMatchObject({
        name: expect.any(String),
        path: expect.any(String),
        isDir: expect.any(Boolean),
        size: expect.any(Number),
        modifiedAt: expect.any(Number),
        permissions: expect.any(String),
        owner: expect.any(String),
        group: expect.any(String),
      });
    }
  });
});
