/**
 * 哈希计算 · 纯函数（WebCrypto SHA-1/256/384/512 + spark-md5）
 */
import SparkMD5 from "spark-md5";

export interface HashResult {
  md5: string;
  sha1: string;
  sha256: string;
  sha384: string;
  sha512: string;
}

async function subtleHex(algo: string, text: string): Promise<string> {
  const buf = await crypto.subtle.digest(algo, new TextEncoder().encode(text));
  return [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

export function md5Hex(text: string): string {
  return SparkMD5.hash(text);
}

export async function sha256Hex(text: string): Promise<string> {
  return subtleHex("SHA-256", text);
}

/** 计算全部哈希（文本工具主入口） */
export async function computeAllHashes(text: string): Promise<HashResult> {
  const [sha1, sha256, sha384, sha512] = await Promise.all([
    subtleHex("SHA-1", text),
    subtleHex("SHA-256", text),
    subtleHex("SHA-384", text),
    subtleHex("SHA-512", text),
  ]);
  return { md5: md5Hex(text), sha1, sha256, sha384, sha512 };
}

/** 通用：文件/大数据按字节哈希（增量） */
export function md5HexBuffer(data: Uint8Array): string {
  return SparkMD5.ArrayBuffer.hash(data.buffer);
}
