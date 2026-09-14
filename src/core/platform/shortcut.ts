/**
 * 平台能力 · 快捷键字符串比较
 *
 * 设置页显示与保存的是人类可读形式（`Ctrl+Shift+Space`），而框架 `globalHotkeyActive`
 * 写入的是 Shortcut 规范形式（`shift+control+Space`）：两者只是写法不同，
 * 直接字符串相等会把已生效的快捷键误判成「未生效」，因此统一规范化后再比较。
 */

/** 修饰键别名 → 规范名（小写）；`cmd`/`super` 等跨平台写法归一到同一语义位 */
const MODIFIER_ALIASES: Record<string, string> = {
  ctrl: 'control',
  control: 'control',
  cmdorctrl: 'control',
  commandorcontrol: 'control',
  shift: 'shift',
  alt: 'alt',
  option: 'alt',
  cmd: 'meta',
  command: 'meta',
  super: 'meta',
  meta: 'meta',
  win: 'meta',
}

/** 规范化单个按键项：去空白、转小写、替换别名 */
function normalizeToken(token: string): string {
  const trimmed = token.trim().toLowerCase()
  return MODIFIER_ALIASES[trimmed] ?? trimmed
}

/** 规范化整个组合：按 `+` 拆分并排序，使比较与修饰键书写顺序无关 */
function normalize(shortcut: string): string {
  if (!shortcut) return ''
  return shortcut
    .split('+')
    .map(normalizeToken)
    .filter((token) => token.length > 0)
    .sort()
    .join('+')
}

/**
 * 判断两个快捷键字符串是否指向同一组合。
 *
 * 忽略大小写、修饰键书写形式与顺序；任一侧为空视为不相等（空串 = 当前没有生效的快捷键）。
 */
export function sameShortcut(a: string, b: string): boolean {
  const left = normalize(a)
  return left.length > 0 && left === normalize(b)
}
