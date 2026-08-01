/**
 * 字符统计 · 纯函数（字数 / 行数 / 词数 / 字节数 / 中日韩字符）
 */
export interface CharStat {
  /** 总字符数（按 Unicode 码点，emoji 计 1） */
  chars: number;
  /** 去除空白后的字符数 */
  charsNoSpace: number;
  /** 词数（按空白分词） */
  words: number;
  /** 行数 */
  lines: number;
  /** UTF-8 字节数 */
  bytes: number;
  /** 中日韩统一表意文字数 */
  cjk: number;
}

export function statText(text: string): CharStat {
  const chars = [...text].length;
  const noSpace = [...text.replace(/\s/g, "")].length;
  const words = text.trim() ? text.trim().split(/\s+/).length : 0;
  const lines = text ? text.split("\n").length : 0;
  const bytes = new TextEncoder().encode(text).length;
  const cjk = [...text].filter((c) => /[\u4e00-\u9fff\u3040-\u30ff\uac00-\ud7af]/.test(c)).length;
  return { chars, charsNoSpace: noSpace, words, lines, bytes, cjk };
}
