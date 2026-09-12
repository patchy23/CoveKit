/**
 * SSH 危险路径安全策略（前端镜像：只决定右键菜单项是否出现）
 * 权威判定在后端 sftp/util.rs（check_delete_allowed/check_chmod_allowed），两侧规则必须一致。
 */

/** 系统目录清单（chmod 拦本体；删除拦本体+子树） */
const SYSTEM_DIRS = [
  '/',
  '/bin',
  '/boot',
  '/dev',
  '/etc',
  '/lib',
  '/lib64',
  '/proc',
  '/run',
  '/sbin',
  '/sys',
  '/usr',
  '/var',
]

/** 虚拟文件系统（chmod 整树禁止） */
const VIRTUAL_DIRS = ['/proc', '/sys', '/dev']

/** 规范化：去尾部斜杠（根除外） */
function normalize(path: string): string {
  return path.length > 1 ? path.replace(/\/+$/, '') : path
}

function isSystemDirItself(path: string): boolean {
  return SYSTEM_DIRS.includes(path)
}

function underSystemDir(path: string): boolean {
  return SYSTEM_DIRS.filter((d) => d !== '/').some((d) => path.startsWith(`${d}/`))
}

/** 删除菜单是否可见（系统目录本体及子树隐藏；根下自定义目录正常） */
export function deleteMenuVisible(path: string): boolean {
  const p = normalize(path)
  return !isSystemDirItself(p) && !underSystemDir(p)
}

/** chmod 菜单是否可见（系统目录本体 + 虚拟文件系统子树隐藏） */
export function chmodMenuVisible(path: string): boolean {
  const p = normalize(path)
  if (isSystemDirItself(p)) return false
  return !VIRTUAL_DIRS.some((d) => p.startsWith(`${d}/`))
}

/** 系统目录内递归 chmod 需要风险确认（前端弹窗据此显示警告勾选项） */
export function chmodNeedsRiskAck(path: string, recursive: boolean): boolean {
  return recursive && underSystemDir(normalize(path))
}
