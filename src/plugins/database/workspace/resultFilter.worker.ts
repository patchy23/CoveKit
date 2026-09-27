import { createFilterEngine, type FilterMessage } from './resultFilterEngine'

const engine = createFilterEngine((data, transfer = []) => self.postMessage(data, { transfer }))
self.onmessage = (event: MessageEvent<FilterMessage>) => engine.receive(event.data)
