import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import { emitTo } from '@tauri-apps/api/event'
import { createEditorBridge } from './bridge'
import type { EditorEnvelope } from './protocol'
export async function editorChannel(
  token: string,
  peer: string,
  handle: (type: string, value: unknown) => unknown | Promise<unknown>,
  onError: (error: unknown) => void
) {
  const event = 'ssh-editor-' + token
  const bridge = createEditorBridge((message) => emitTo(peer, event, message), handle)
  const unlisten = await getCurrentWebviewWindow().listen<EditorEnvelope>(event, ({ payload }) => {
    void bridge.receive(payload).catch(onError)
  })
  return {
    request: bridge.request,
    dispose() {
      unlisten()
      bridge.dispose()
    },
  }
}
