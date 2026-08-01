import { describe, expect, it } from "vitest";
import { decodeBase64, encodeBase64, isValidBase64 } from "./useBase64";

describe("useBase64", () => {
  it("ASCII 编解码往返", () => {
    expect(encodeBase64("Hello, 你好")).toBe("SGVsbG8sIOS9oOWlvQ==");
    expect(decodeBase64("SGVsbG8sIOS9oOWlvQ==")).toBe("Hello, 你好");
  });

  it("中文 UTF-8 安全", () => {
    const text = "patchyBox 工具箱";
    expect(decodeBase64(encodeBase64(text))).toBe(text);
  });

  it("isValidBase64 校验", () => {
    expect(isValidBase64("SGVsbG8=")).toBe(true);
    expect(isValidBase64("abc")).toBe(false); // 长度非 4 的倍数
    expect(isValidBase64("ab c=")).toBe(false); // 非法字符
    expect(isValidBase64("")).toBe(false);
  });
});
