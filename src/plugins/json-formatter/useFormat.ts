/**
 * JSON 格式化 · 纯函数（格式化 / 压缩 / 校验 + 错误行号定位）
 */
export interface FormatError {
  message: string;
  line: number;
  col: number;
}

export interface FormatResult {
  ok: boolean;
  output: string;
  error?: FormatError;
}

/** position → 行/列（1 起始） */
export function positionToLineCol(input: string, position: number): { line: number; col: number } {
  const before = input.slice(0, Math.max(0, position));
  const lines = before.split("\n");
  return { line: lines.length, col: lines[lines.length - 1].length + 1 };
}

function toError(err: unknown, input: string): FormatError {
  const msg = err instanceof Error ? err.message : String(err);
  const posMatch = msg.match(/position (\d+)/);
  if (posMatch) {
    const { line, col } = positionToLineCol(input, Number(posMatch[1]));
    return { message: "JSON 语法错误", line, col };
  }
  const lineMatch = msg.match(/line (\d+) column (\d+)/);
  if (lineMatch) {
    return { message: "JSON 语法错误", line: Number(lineMatch[1]), col: Number(lineMatch[2]) };
  }
  return { message: msg || "JSON 语法错误", line: 0, col: 0 };
}

export function formatJson(input: string, indent: number | string = 2): FormatResult {
  try {
    const parsed = JSON.parse(input);
    return { ok: true, output: JSON.stringify(parsed, null, indent) };
  } catch (err) {
    return { ok: false, output: "", error: toError(err, input) };
  }
}

export function minifyJson(input: string): FormatResult {
  try {
    return { ok: true, output: JSON.stringify(JSON.parse(input)) };
  } catch (err) {
    return { ok: false, output: "", error: toError(err, input) };
  }
}

export function isValidJson(input: string): boolean {
  try {
    JSON.parse(input);
    return true;
  } catch {
    return false;
  }
}
