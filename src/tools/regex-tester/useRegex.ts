/**
 * 正则测试 · 纯函数（实时匹配 + 常用表达式）
 */
export interface RegexMatch {
  text: string;
  index: number;
}

export interface RegexResult {
  ok: boolean;
  matches: RegexMatch[];
  count: number;
  error?: string;
}

export function testRegex(pattern: string, flags: string, text: string): RegexResult {
  try {
    const re = new RegExp(pattern, flags.includes("g") ? flags : `${flags}g`);
    const matches: RegexMatch[] = [];
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
      matches.push({ text: m[0], index: m.index });
      // 空匹配防死循环
      if (m[0] === "") re.lastIndex++;
    }
    return { ok: true, matches, count: matches.length };
  } catch (err) {
    return {
      ok: false,
      matches: [],
      count: 0,
      error: err instanceof Error ? err.message : String(err),
    };
  }
}

/** 常用正则表达式速查（点击一键插入） */
export const COMMON_PATTERNS: { label: string; pattern: string }[] = [
  { label: "邮箱", pattern: "[\\w.+-]+@[\\w-]+(?:\\.[\\w-]+)+" },
  { label: "URL", pattern: "https?://[\\w.-]+(?:/[\\w./?%&=-]*)?" },
  { label: "IPv4", pattern: "(?:\\d{1,3}\\.){3}\\d{1,3}" },
  { label: "手机号", pattern: "1[3-9]\\d{9}" },
  { label: "中文", pattern: "[\\u4e00-\\u9fff]+" },
  { label: "整数", pattern: "-?\\d+" },
  { label: "十六进制色值", pattern: "#[0-9a-fA-F]{6}\\b" },
];

/** 校验 pattern 是否可编译（返回错误信息，无错返回 null） */
export function validatePattern(pattern: string, flags: string): string | null {
  try {
    new RegExp(pattern, flags);
    return null;
  } catch (err) {
    return err instanceof Error ? err.message : String(err);
  }
}
