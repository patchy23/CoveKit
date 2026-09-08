import { describe, expect, it } from 'vitest'
import {
  formatModeRwx,
  matrixToMode,
  modeFromPermissions,
  modeToMatrix,
  parseOctal,
} from './sshChmod'

describe('sshChmod 八进制 ↔ 矩阵换算', () => {
  it('755 → 矩阵', () => {
    expect(modeToMatrix(0o755)).toEqual([true, true, true, true, false, true, true, false, true])
  })

  it('矩阵 → 644', () => {
    expect(matrixToMode([true, true, false, true, false, false, true, false, false])).toBe(0o644)
  })

  it('往返一致', () => {
    for (const mode of [0o755, 0o644, 0o600, 0o777, 0o000, 0o751]) {
      expect(matrixToMode(modeToMatrix(mode))).toBe(mode)
    }
  })

  it('忽略类型位高 16 位', () => {
    expect(matrixToMode(modeToMatrix(0o040755))).toBe(0o755)
    expect(matrixToMode(modeToMatrix(0o100644))).toBe(0o644)
  })
})

describe('sshChmod 展示与解析', () => {
  it('formatModeRwx', () => {
    expect(formatModeRwx(0o755)).toBe('rwxr-xr-x')
    expect(formatModeRwx(0o600)).toBe('rw-------')
  })

  it('parseOctal 合法/非法', () => {
    expect(parseOctal('755')).toBe(0o755)
    expect(parseOctal('4755')).toBe(0o4755)
    expect(parseOctal('888')).toBeNull()
    expect(parseOctal('75')).toBeNull()
    expect(parseOctal('abc')).toBeNull()
  })

  it('modeFromPermissions 从 rwx 字符串提取', () => {
    expect(modeFromPermissions('drwxr-xr-x')).toBe(0o755)
    expect(modeFromPermissions('-rw-r--r--')).toBe(0o644)
    expect(modeFromPermissions('-')).toBe(0)
  })
})
