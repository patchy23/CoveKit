/** 离线回归只替换 Worker 消息边界，保留真实 CodeMirror 正则计算。 */
import { createRegexEngine, type RegexRequest } from './regexSearch'

export class RegexTestWorker {
  private execute = createRegexEngine()
  private stopped = false
  onmessage?: (event: { data: unknown }) => void
  postMessage(request: RegexRequest) {
    queueMicrotask(() => {
      if (!this.stopped) this.onmessage?.({ data: this.execute(request) })
    })
  }
  terminate() {
    this.stopped = true
  }
}
