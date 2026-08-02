import { describe, expect, it } from "vitest";
import { computeAllHashes, md5Hex } from "./useHash";

describe("useHash", () => {
  it("MD5 已知向量", () => {
    expect(md5Hex("")).toBe("d41d8cd98f00b204e9800998ecf8427e");
    expect(md5Hex("abc")).toBe("900150983cd24fb0d6963f7d28e17f72");
  });

  it("SHA-256 已知向量", async () => {
    const { sha256 } = await computeAllHashes("abc");
    expect(sha256).toBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
  });

  it("全部哈希结果格式为 64 位十六进制（sha256/384/512）", async () => {
    const r = await computeAllHashes("hello");
    expect(r.sha1).toMatch(/^[0-9a-f]{40}$/);
    expect(r.sha256).toMatch(/^[0-9a-f]{64}$/);
    expect(r.sha384).toMatch(/^[0-9a-f]{96}$/);
    expect(r.sha512).toMatch(/^[0-9a-f]{128}$/);
  });
});
