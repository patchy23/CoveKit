import { onScopeDispose, shallowRef } from 'vue'
import { onSpaceDataChanged } from '@/core/ipc/spaceEvents'
import type { CredentialOverride, ServerProfile } from '../contracts'

/** 手工认证仅驻留当前工具实例内存，不写配置、日志或浏览器存储。 */
export function useConnectionCredentials() {
  const cache = new Map<string, { binding: string; value: CredentialOverride }>()
  const requestProfile = shallowRef<ServerProfile | null>(null)
  let pending: ((value: CredentialOverride | undefined) => void) | undefined
  let disposed = false
  const binding = (p: ServerProfile) => JSON.stringify([p.host, p.port, p.username, p.authMethod])

  function get(profile: ServerProfile): CredentialOverride | undefined {
    const entry = cache.get(profile.id)
    return !profile.credentialRef && entry?.binding === binding(profile)
      ? { ...entry.value }
      : undefined
  }

  function remember(profile: ServerProfile, value: CredentialOverride) {
    if (disposed) return
    if (profile.credentialRef) cache.delete(profile.id)
    else if (profile.authMethod === 'password' ? value.password : value.privateKey) {
      cache.set(profile.id, { binding: binding(profile), value: { ...value } })
    } else if (!get(profile)) cache.delete(profile.id)
  }

  function respond(value?: CredentialOverride) {
    if (value && requestProfile.value) remember(requestProfile.value, value)
    const resolve = pending
    pending = undefined
    requestProfile.value = null
    resolve?.(value)
  }

  function request(profile: ServerProfile): Promise<CredentialOverride | undefined> {
    if (disposed) return Promise.resolve(undefined)
    const value = get(profile)
    if (value) return Promise.resolve(value)
    respond()
    requestProfile.value = { ...profile }
    return new Promise((resolve) => {
      pending = resolve
    })
  }

  function clear() {
    cache.clear()
    respond()
  }
  const unlisten = onSpaceDataChanged(clear)
  onScopeDispose(() => {
    disposed = true
    unlisten()
    clear()
  })
  function forget(id: string) {
    cache.delete(id)
    if (requestProfile.value?.id === id) respond()
  }
  return { requestProfile, request, respond, get, remember, forget }
}
