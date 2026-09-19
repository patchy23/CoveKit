/** FRP 凭证引用使用原生环境变量模板；只编码凭证标识，不包含秘密。 */
const PREFIX = '{{ .Envs.COVEKIT_FRP_TOKEN_'
const SUFFIX = ' }}'

export function tokenReference(id: string): string {
  const hex = Array.from(new TextEncoder().encode(id), (byte) =>
    byte.toString(16).padStart(2, '0')
  ).join('')
  return `${PREFIX}${hex}${SUFFIX}`
}

export function tokenCredentialId(value: string): string {
  if (!value.startsWith(PREFIX) || !value.endsWith(SUFFIX)) return ''
  const hex = value.slice(PREFIX.length, -SUFFIX.length)
  if (!/^(?:[0-9a-f]{2})+$/.test(hex)) return ''
  const id = new TextDecoder().decode(
    Uint8Array.from(hex.match(/../g) ?? [], (pair) => parseInt(pair, 16))
  )
  return tokenReference(id) === value ? id : ''
}
