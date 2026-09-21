/** Compose ps 输出适配：兼容旧版 JSON 数组与新版逐行 JSON，归属由 Compose 查询保证。 */
import type { ComposeOutput, DockerContainer } from '../contracts'

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value))
    throw new Error('Compose 容器列表格式异常：应为对象')
  return value as Record<string, unknown>
}

function text(row: Record<string, unknown>, key: string, required = false): string {
  const value = row[key]
  if ((value === undefined || value === null) && !required) return ''
  if (typeof value !== 'string' || (required && !value.trim()))
    throw new Error(`Compose 容器列表字段异常：${key}`)
  return value
}

function ports(row: Record<string, unknown>): string {
  if (row.Ports !== undefined) return text(row, 'Ports')
  if (row.Publishers == null) return ''
  if (!Array.isArray(row.Publishers)) throw new Error('Compose 容器端口格式异常')
  return row.Publishers.map((value) => {
    const publisher = record(value)
    const target = publisher.TargetPort
    const published = publisher.PublishedPort
    if (
      !Number.isInteger(target) ||
      Number(target) < 1 ||
      Number(target) > 65535 ||
      !Number.isInteger(published) ||
      Number(published) < 0 ||
      Number(published) > 65535
    )
      throw new Error('Compose 容器端口数值异常')
    const protocol = text(publisher, 'Protocol', true)
    const url = text(publisher, 'URL')
    const host = url.includes(':') && !url.startsWith('[') ? `[${url}]` : url
    return published
      ? `${host ? `${host}:` : ''}${published}->${target}/${protocol}`
      : `${target}/${protocol}`
  }).join(', ')
}

/** 非零退出码、损坏 JSON 和不完整记录均报错，不能伪装成空列表。 */
export function parseComposeContainers(result: ComposeOutput): DockerContainer[] {
  if (result.exitCode !== 0)
    throw new Error(`Compose 容器查询失败（${result.exitCode}）：${result.stderr || result.stdout}`)
  const raw = result.stdout.trim()
  if (!raw) return []
  let rows: unknown[]
  try {
    rows = raw.startsWith('[')
      ? JSON.parse(raw)
      : raw
          .split(/\r?\n/)
          .filter((line) => line.trim())
          .map((line) => JSON.parse(line))
  } catch {
    throw new Error('Compose 容器列表不是有效 JSON')
  }
  return rows
    .map((value): DockerContainer => {
      const row = record(value)
      const state = text(row, 'State', true)
      // RunningFor 是创建至今的时长，不能当作最近一次启动后的运行时间。
      const status = text(row, 'Status')
      const uptime = /^Up\s+(.+?)(?:\s+\(.*)?$/i.exec(status)?.[1] ?? '—'
      return {
        id: text(row, 'ID', true),
        name: text(row, 'Name', true),
        image: text(row, 'Image') || '—',
        status: state,
        uptime: state === 'running' || state === 'paused' ? uptime : '—',
        ports: ports(row),
        createdAt: 0,
      }
    })
    .sort((a, b) => a.name.localeCompare(b.name))
}
