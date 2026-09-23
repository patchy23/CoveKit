/** CSV/制表符解析与导入预览。密码保留原始空白；错误不带原始单元格。 */
import type { ServerProfile } from '../contracts'
export const fields = [
  'name',
  'host',
  'port',
  'username',
  'password',
  'group',
  'remark',
  'auth_type',
] as const
export type Field = (typeof fields)[number]
export type Mapping = Record<Field, number>
export interface ImportRow {
  row: number
  name: string
  host: string
  port: number
  username: string
  password: string
  group: string
  remark: string
  error: string
  matches: string[]
}
export function parseTable(input: string): string[][] {
  if (new TextEncoder().encode(input).byteLength > 2 * 1024 * 1024)
    throw new Error('内容不可超过 2 MiB')
  const text = input.replace(/^\uFEFF/, '')
  const first = text.split(/\r?\n/, 1)[0] ?? ''
  const sep = first.includes('\t') ? '\t' : ','
  const rows: string[][] = [],
    row: string[] = []
  let value = '',
    quoted = false,
    closed = false
  for (let i = 0; i < text.length; i++) {
    const c = text[i]
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          value += '"'
          i++
        } else {
          quoted = false
          closed = true
        }
      } else value += c
    } else if (c === sep) {
      row.push(value)
      value = ''
      closed = false
    } else if (c === '\r' || c === '\n') {
      if (c === '\r' && text[i + 1] === '\n') i++
      row.push(value)
      if (row.some((v) => v !== '')) rows.push([...row])
      row.length = 0
      value = ''
      closed = false
    } else if (c === '"' && !value && !closed) quoted = true
    else {
      if (closed || c === '"') throw new Error('CSV 引号格式错误')
      value += c
    }
  }
  if (quoted) throw new Error('CSV 引号未闭合')
  row.push(value)
  if (row.some((v) => v !== '')) rows.push(row)
  if (rows.length > 5001) throw new Error('单次最多导入 5000 台服务器')
  if (rows.length < 2) throw new Error('请提供表头和至少一条服务器记录')
  return rows
}
const aliases: Record<Field, string[]> = {
  name: ['name', '名称', '服务器名称'],
  host: ['host', '主机', '地址', 'ip'],
  port: ['port', '端口'],
  username: ['username', 'user', '用户名'],
  password: ['password', '密码'],
  group: ['group', '分组'],
  remark: ['remark', '备注'],
  auth_type: ['auth_type', '认证方式'],
}
export function mapHeaders(headers: string[]): Mapping {
  return Object.fromEntries(
    fields.map((field) => [
      field,
      headers.findIndex((h) => aliases[field].includes(h.trim().toLowerCase())),
    ])
  ) as Mapping
}
export function endpoint(p: { host: string; port: number; username: string }) {
  return JSON.stringify([p.host.toLowerCase(), p.port, p.username])
}
export function previewRows(
  table: string[][],
  map: Mapping,
  profiles: ServerProfile[]
): ImportRow[] {
  const seen = new Set<string>()
  const existing = new Map<string, string[]>()
  for (const p of profiles) {
    const key = endpoint(p)
    existing.set(key, [...(existing.get(key) ?? []), p.id])
  }
  return table.slice(1).map((cells, index) => {
    const get = (key: Field) => cells[map[key]] ?? ''
    const host = get('host').trim(),
      username = get('username').trim(),
      portText = get('port').trim(),
      port = portText ? Number(portText) : 22
    const row: ImportRow = {
      row: index + 2,
      name: get('name').trim() || username + '@' + host,
      host,
      port,
      username,
      password: get('password'),
      group: get('group').trim(),
      remark: get('remark'),
      error: '',
      matches: [],
    }
    const key = endpoint(row)
    if (!host || /\s|\//.test(host)) row.error = '主机必填，且不可包含空白或协议路径'
    else if (!username) row.error = '用户名必填'
    else if (!/^\d+$/.test(portText || '22') || !Number.isInteger(port) || port < 1 || port > 65535)
      row.error = '端口必须为 1–65535'
    else if (get('auth_type') && get('auth_type') !== 'password')
      row.error = '仅支持密码认证，密钥记录不导入'
    else if (seen.has(key)) row.error = '文件中存在重复的主机、端口和用户名'
    seen.add(key)
    row.matches = existing.get(key) ?? []
    return row
  })
}
export function csvText(rows: string[][]) {
  return (
    '\uFEFF' +
    rows
      .map((row) => row.map((cell) => '"' + cell.replaceAll('"', '""') + '"').join(','))
      .join('\r\n') +
    '\r\n'
  )
}
export const serverCsvTemplate = csvText([
  ['name', 'host', 'port', 'username', 'password', 'group', 'remark'],
  ['示例服务器', '192.0.2.10', '22', 'root', '', '测试', '请替换示例，不会自动连接'],
])
