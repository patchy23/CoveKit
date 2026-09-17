/** 按文件面板的路径类型转换；远程绝对路径不能落入 Windows 分隔符分支。 */
export function normalizePathInput(input: string, separator: string): string {
  const path = input.trim()
  if (separator === '/') return path.startsWith('/') ? path : `/${path}`
  return path.replace(/\//g, '\\')
}
