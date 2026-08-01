/**
 * 文本对比 · 纯函数（LCS 逐行 diff，增删改高亮）
 * 注意：O(n×m) DP，超大文本（>2000 行）性能下降，M2 可换 Myers 算法。
 */
export type DiffType = "same" | "add" | "remove";

export interface DiffLine {
  type: DiffType;
  text: string;
}

export function diffLines(a: string, b: string): DiffLine[] {
  // 空文档视为 0 行（"" .split 会产出 1 个空行元素，语义不符）
  const aLines = a ? a.split("\n") : [];
  const bLines = b ? b.split("\n") : [];
  const n = aLines.length;
  const m = bLines.length;

  // LCS 长度表（自底向上）
  const dp: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      dp[i][j] =
        aLines[i] === bLines[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
    }
  }

  // 回溯生成操作序列
  const result: DiffLine[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (aLines[i] === bLines[j]) {
      result.push({ type: "same", text: aLines[i] });
      i++;
      j++;
    } else if (dp[i + 1][j] >= dp[i][j + 1]) {
      result.push({ type: "remove", text: aLines[i] });
      i++;
    } else {
      result.push({ type: "add", text: bLines[j] });
      j++;
    }
  }
  while (i < n) result.push({ type: "remove", text: aLines[i++] });
  while (j < m) result.push({ type: "add", text: bLines[j++] });
  return result;
}

export interface DiffStats {
  adds: number;
  removes: number;
  unchanged: number;
}

export function diffStats(diff: DiffLine[]): DiffStats {
  return {
    adds: diff.filter((l) => l.type === "add").length,
    removes: diff.filter((l) => l.type === "remove").length,
    unchanged: diff.filter((l) => l.type === "same").length,
  };
}
