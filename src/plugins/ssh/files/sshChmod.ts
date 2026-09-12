/**
 * chmod 弹窗的权限换算（纯函数，可单测）
 * 八进制 mode（含类型位的高 16 位忽略）↔ 勾选矩阵（所有者/组/其他 × r/w/x）双向转换。
 */

/** 勾选矩阵：['user','group','other'][r,w,x] */
export type ChmodMatrix = boolean[]

/** mode（取低 12 位）→ 9 格矩阵（user rwx / group rwx / other rwx） */
export function modeToMatrix(mode: number): ChmodMatrix {
  const m = mode & 0o7777
  const out: boolean[] = []
  for (const shift of [6, 3, 0]) {
    const bits = (m >> shift) & 0o7
    out.push((bits & 0o4) !== 0, (bits & 0o2) !== 0, (bits & 0o1) !== 0)
  }
  return out
}

/** 9 格矩阵 → 低 9 位权限（不含类型/特殊位）；位序：索引 i 对应位 8-i（user r=0o400 … other x=0o001） */
export function matrixToMode(matrix: ChmodMatrix): number {
  let mode = 0
  matrix.slice(0, 9).forEach((on, i) => {
    if (on) mode |= 1 << (8 - i)
  })
  return mode
}

/** mode → rwx 字符串（9 位，不含类型位；展示用） */
export function formatModeRwx(mode: number): string {
  const matrix = modeToMatrix(mode)
  return matrix
    .map((on, i) => {
      const kind = i % 3
      return on ? 'rwx'[kind] : '-'
    })
    .join('')
}

/** 解析八进制输入：3 位（755）或 4 位（4755），每位 0-7；合法返回 mode，非法返回 null */
export function parseOctal(input: string): number | null {
  const trimmed = input.trim()
  if (!/^[0-7]{3}$/.test(trimmed) && !/^[0-7]{4}$/.test(trimmed)) return null
  return parseInt(trimmed, 8)
}

/** 从 rwx 权限字符串（drwxr-xr-x / -rw-r--r--）提取低 9 位 mode；解析失败回退 0 */
export function modeFromPermissions(permissions: string): number {
  const rwx = permissions.length === 10 ? permissions.slice(1) : permissions
  if (!/^[rwx-]{9}$/.test(rwx)) return 0
  const matrix = [...rwx].map((ch, i) => ch === 'rwx'[i % 3])
  return matrixToMode(matrix)
}
