/**
 * 快捷键比较行为网（真机走查回归）
 *
 * 用途：正式版设置页曾误报「当前快捷键未生效（可能已被其他程序占用）」——
 * 框架写入 `shift+control+Space`、设置页读 `Ctrl+Shift+Space`，字面比较必然不等。
 * 这里锁定「写法不同但同一组合判为相等、真正不同或缺失才判为不生效」。
 */
import { describe, expect, it } from 'vitest'
import { sameShortcut } from './shortcut'

describe('快捷键规范化比较', () => {
  it('框架规范形式与设置页显示形式视为同一组合', () => {
    expect(sameShortcut('Ctrl+Shift+Space', 'shift+control+Space')).toBe(true)
  })

  it('忽略大小写与修饰键顺序', () => {
    expect(sameShortcut('ctrl+alt+sPace', 'Alt+Ctrl+Space')).toBe(true)
  })

  it('跨平台别名归一', () => {
    expect(sameShortcut('Cmd+Shift+O', 'Super+shift+O')).toBe(true)
    expect(sameShortcut('CmdOrCtrl+Shift+O', 'Control+Shift+O')).toBe(true)
  })

  it('主键不同判为不相等', () => {
    expect(sameShortcut('Ctrl+Shift+Space', 'shift+control+O')).toBe(false)
  })

  it('缺失生效值时判为未生效，含两侧都空', () => {
    expect(sameShortcut('Ctrl+Shift+Space', '')).toBe(false)
    expect(sameShortcut('', 'shift+control+Space')).toBe(false)
    expect(sameShortcut('', '')).toBe(false)
  })
})
