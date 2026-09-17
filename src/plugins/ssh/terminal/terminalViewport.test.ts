import { expect, it } from 'vitest'
import { bindTerminalViewport } from './terminalViewport'

it('原生滚动条保留默认拖动，文本层仍由 xterm 处理，卸载可释放', () => {
  const root = document.createElement('div')
  root.innerHTML = '<div class="xterm-viewport"></div><div class="xterm-screen"></div>'
  root.addEventListener('mousedown', (event) => event.preventDefault())
  const dispose = bindTerminalViewport(root)
  const down = () => new MouseEvent('mousedown', { button: 0, bubbles: true, cancelable: true })
  const scrollbar = down()
  root.querySelector('.xterm-viewport')!.dispatchEvent(scrollbar)
  expect(scrollbar.defaultPrevented).toBe(false)
  const selection = down()
  root.querySelector('.xterm-screen')!.dispatchEvent(selection)
  expect(selection.defaultPrevented).toBe(true)
  dispose()
  const unbound = down()
  root.querySelector('.xterm-viewport')!.dispatchEvent(unbound)
  expect(unbound.defaultPrevented).toBe(true)
})
