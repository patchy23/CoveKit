/**
 * 随机密码 · 纯函数（crypto.getRandomValues 安全随机）
 */
export interface PasswordOptions {
  length: number;
  upper: boolean;
  lower: boolean;
  digits: boolean;
  symbols: boolean;
  /** 排除易混字符（0O1lI|`'"/\ 等） */
  excludeAmbiguous: boolean;
}

const UPPER = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER = "abcdefghijklmnopqrstuvwxyz";
const DIGITS = "0123456789";
const SYMBOLS = "!@#$%^&*()-_=+[]{};:,.<>?";
const AMBIGUOUS = "0O1lI|`'\"\\/ ";

/** 从候选池中随机取 n 个不重复字符（洗牌取前 n，保证每个字符集至少出现一次） */
function pickChars(pool: string, n: number): string[] {
  const arr = [...pool];
  for (let i = arr.length - 1; i > 0; i--) {
    const j = Math.floor((crypto.getRandomValues(new Uint32Array(1))[0] / 2 ** 32) * (i + 1));
    [arr[i], arr[j]] = [arr[j], arr[i]];
  }
  return arr.slice(0, n);
}

export function generatePassword(opts: PasswordOptions): string {
  let sets: string[] = [];
  if (opts.upper) sets.push(UPPER);
  if (opts.lower) sets.push(LOWER);
  if (opts.digits) sets.push(DIGITS);
  if (opts.symbols) sets.push(SYMBOLS);
  if (sets.length === 0) return "";

  // 排除易混字符：作用于每个字符集与总池（保证首字符也不含易混字符）
  if (opts.excludeAmbiguous) {
    sets = sets.map((s) => [...s].filter((c) => !AMBIGUOUS.includes(c)).join("")).filter(Boolean);
    if (sets.length === 0) return "";
  }

  const pool = sets.join("");
  const len = Math.max(opts.length, sets.length);
  // 先从每个字符集各取一个（保证多样性），剩余随机
  const result: string[] = [];
  for (const s of sets) {
    const chars = pickChars(s, 1);
    result.push(chars[0]);
  }
  const rest = pickChars(pool, len - sets.length);
  result.push(...rest);

  // 洗牌打乱顺序
  for (let i = result.length - 1; i > 0; i--) {
    const j = Math.floor((crypto.getRandomValues(new Uint32Array(1))[0] / 2 ** 32) * (i + 1));
    [result[i], result[j]] = [result[j], result[i]];
  }
  return result.join("");
}

export interface Strength {
  score: 0 | 1 | 2 | 3 | 4;
  label: string;
}

/** 强度估算：按长度与字符集多样性粗略分级 */
export function estimateStrength(pw: string): Strength {
  if (!pw) return { score: 0, label: "空" };
  const variety = new Set([...pw]).size;
  const len = pw.length;
  if (len < 8 || variety < 4) return { score: 1, label: "弱" };
  if (len < 12 || variety < 8) return { score: 2, label: "一般" };
  if (len < 16 || variety < 12) return { score: 3, label: "强" };
  return { score: 4, label: "很强" };
}
