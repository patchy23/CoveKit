/**
 * Vault 前端引用登记表。
 * 仅用于后端无法读取的浏览器侧配置（当前为 SSH localStorage profile）；
 * 后端持久化插件（DNS 等）由 Rust reference_count 直接扫描。
 */

const STORAGE_KEY = 'vault.references.v1'

export type CredentialReferenceMap = Record<string, string[]>

/** 规范化引用：去空并保留重复项（多个 profile 引用同一凭证应分别计数）。 */
export function normalizeCredentialReferences(ids: string[]): string[] {
  return ids.map((id) => id.trim()).filter(Boolean)
}

/** 统计某凭证被多少个浏览器侧配置引用。 */
export function countCredentialReferences(map: CredentialReferenceMap, id: string): number {
  return Object.values(map).reduce(
    (total, ids) => total + normalizeCredentialReferences(ids).filter((item) => item === id).length,
    0
  )
}

function readReferenceMap(): CredentialReferenceMap {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? (JSON.parse(raw) as CredentialReferenceMap) : {}
  } catch {
    return {}
  }
}

/** 插件以自己的 scope 覆盖登记引用，不接触其它插件的数据。 */
export function syncCredentialReferences(scope: string, ids: string[]): void {
  try {
    const map = readReferenceMap()
    map[scope] = normalizeCredentialReferences(ids)
    localStorage.setItem(STORAGE_KEY, JSON.stringify(map))
  } catch {
    // 浏览器存储不可用不影响工具本身；后端仍会在实际使用时校验引用。
  }
}

/** 凭证管理页读取浏览器侧引用总数。 */
export function clientCredentialReferenceCount(id: string): number {
  return countCredentialReferences(readReferenceMap(), id)
}
