import { describe, expect, it } from "vitest";
import { describeResult, displayCell, fileName, isQuerySql } from "./useSqlite";

describe("useSqlite", () => {
  it("SQL 查询识别", () => {
    expect(isQuerySql("select * from users")).toBe(true);
    expect(isQuerySql("  SELECT id FROM t")).toBe(true);
    expect(isQuerySql("PRAGMA table_info(t)")).toBe(true);
    expect(isQuerySql("WITH x AS (SELECT 1) SELECT * FROM x")).toBe(true);
    expect(isQuerySql("INSERT INTO t VALUES (1)")).toBe(false);
    expect(isQuerySql("UPDATE t SET a=1")).toBe(false);
    expect(isQuerySql("DELETE FROM t")).toBe(false);
  });

  it("文件名提取（含 Windows 路径）", () => {
    expect(fileName("C:\\data\\app.db")).toBe("app.db");
    expect(fileName("/home/u/test.db")).toBe("test.db");
  });

  it("结果描述", () => {
    expect(describeResult(true, 42)).toBe("42 行");
    expect(describeResult(false, 3)).toBe("影响 3 行");
  });

  it("NULL 单元格", () => {
    expect(displayCell("NULL")).toBe("NULL");
    expect(displayCell("abc")).toBe("abc");
  });
});
