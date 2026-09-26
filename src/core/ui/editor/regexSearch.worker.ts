import { createRegexEngine, type RegexRequest } from './regexSearch'

const execute = createRegexEngine()
self.onmessage = (event: MessageEvent<RegexRequest>) => {
  try {
    self.postMessage(execute(event.data))
  } catch (error) {
    self.postMessage({
      id: event.data.id,
      scan: { positions: [], lengths: [], truncated: false, error: String(error) },
      highlights: [],
    })
  }
}
