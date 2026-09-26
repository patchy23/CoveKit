import { formatSql } from './sql'
self.onmessage = (event: MessageEvent<{ id: number; text: string }>) => {
  const { id, text } = event.data
  self.postMessage({ id, result: formatSql(text) })
}
