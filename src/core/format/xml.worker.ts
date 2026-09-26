import { minifyXml, prettyPrintXml } from './xml'
self.onmessage = (
  event: MessageEvent<{
    id: number
    text: string
    indent: number
    mode: 'format' | 'minify'
    loose?: boolean
  }>
) => {
  const { id, text, indent, mode, loose } = event.data
  self.postMessage({
    id,
    result: {
      ok: true,
      output: mode === 'minify' ? minifyXml(text) : prettyPrintXml(text, indent),
      ...(loose ? { loose: true } : {}),
    },
  })
}
