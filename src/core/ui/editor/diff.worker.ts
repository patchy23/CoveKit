import { computeDiff, type DiffRequest } from './diffComputation'

self.onmessage = (event: MessageEvent<DiffRequest & { id: number }>) => {
  self.postMessage({ id: event.data.id, result: computeDiff(event.data) })
}
