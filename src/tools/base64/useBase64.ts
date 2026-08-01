/**
 * Base64 编解码 · 纯函数（UTF-8 安全，完整支持中文）
 */
export function encodeBase64(text: string): string {
  const bytes = new TextEncoder().encode(text);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

export function decodeBase64(input: string): string {
  const bin = atob(input.trim());
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

export function isValidBase64(input: string): boolean {
  const s = input.trim();
  if (!s || s.length % 4 !== 0) return false;
  return /^[A-Za-z0-9+/]*={0,2}$/.test(s);
}
