/**
 * frp 客户端展示相关的纯函数（标题 / 摘要 / 下拉选项 / 生效客户端判定）
 * 与组件解耦便于单测：组件只负责渲染与事件转发，判定逻辑全在这里。
 */
import type { FrpClient } from '../contracts'

/** 翻译函数签名（只用到取文案，避免依赖 vue-i18n 具体类型） */
type Translate = (key: string, named?: Record<string, unknown>) => string

/**
 * 客户端展示标题：有版本号用「frpc 版本」，否则退回文件名。
 * 定制客户端常没有版本输出，此时文件名就是用户唯一的辨识依据。
 */
export function clientTitle(client: FrpClient): string {
  return client.version ? `frpc ${client.version}` : client.label
}

/**
 * 下拉选项：默认项前置并标注，文件已消失的项标注为不可用。
 * 用「置顶 + 标注」而不是过滤掉，是因为用户需要看到「我绑定的那个去哪了」。
 */
export function clientOptions(
  clients: FrpClient[],
  defaultId: string | undefined,
  t: Translate
): { value: string; label: string }[] {
  const sorted = [...clients].sort((a, b) => {
    if (a.id === defaultId) return -1
    if (b.id === defaultId) return 1
    return 0
  })
  return sorted.map((client) => {
    const parts = [clientTitle(client)]
    if (client.id === defaultId) parts.push(t('frp.clientIsDefault'))
    if (!client.exists) parts.push(t('frp.clientMissing'))
    return { value: client.id, label: parts.join(' · ') }
  })
}

/**
 * 档案实际生效的客户端：绑定项优先（即使文件已删除也返回它，由调用方提示），
 * 没有绑定则回落到默认项。返回 undefined 表示清单里没有可用客户端。
 */
export function effectiveClient(
  clients: FrpClient[],
  defaultId: string | undefined,
  boundId: string | undefined
): FrpClient | undefined {
  if (boundId !== undefined && boundId !== '') {
    const bound = clients.find((client) => client.id === boundId)
    if (bound) return bound
  }
  if (defaultId === undefined) return undefined
  return clients.find((client) => client.id === defaultId)
}

/** 来源文案的 i18n key（下载 / 外部引用） */
export function sourceLabelKey(client: FrpClient): string {
  return client.source === 'download' ? 'frp.clientSourceDownload' : 'frp.clientSourceExternal'
}

/** 清单是否可直接用于运行：至少有一个文件存在的客户端 */
export function hasUsableClient(clients: FrpClient[]): boolean {
  return clients.some((client) => client.exists)
}
