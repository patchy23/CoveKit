/**
 * 时间戳转换 · 纯函数（秒/毫秒自动识别，双向互转）
 */
export interface TsResult {
  local: string;
  utc: string;
  ms: number;
  sec: number;
}

/** 时间戳 → Date 毫秒；10 位按秒补足为毫秒，无法解析返回 null */
export function parseTimestamp(input: string): number | null {
  const s = input.trim();
  if (!s) return null;
  const v = Number(s);
  if (!Number.isFinite(v)) return null;
  return v < 1e12 ? v * 1000 : v;
}

export function timestampToResult(input: string): TsResult | null {
  const ms = parseTimestamp(input);
  if (ms === null) return null;
  const d = new Date(ms);
  return {
    local: d.toLocaleString("zh-CN", { hour12: false }),
    utc: d.toUTCString(),
    ms,
    sec: Math.floor(ms / 1000),
  };
}

/** 日期字符串 → 时间戳毫秒；无法解析返回 null */
export function dateToTimestamp(input: string): number | null {
  const t = Date.parse(input.trim());
  return Number.isNaN(t) ? null : t;
}

export function nowSeconds(): number {
  return Math.floor(Date.now() / 1000);
}

export function nowMillis(): number {
  return Date.now();
}
