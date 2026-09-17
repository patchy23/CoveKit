/** 让原生滚动条消费鼠标按下；xterm 根节点的 preventDefault 会阻止拖动。 */
export function bindTerminalViewport(element: HTMLElement): () => void {
  const viewport = element.querySelector<HTMLElement>('.xterm-viewport')
  const preserveScrollbar = (event: MouseEvent) => {
    // 文本层是 viewport 的兄弟节点，不拦截文本选择与远端鼠标事件。
    if (event.target === viewport && event.button === 0) event.stopPropagation()
  }
  viewport?.addEventListener('mousedown', preserveScrollbar)
  return () => viewport?.removeEventListener('mousedown', preserveScrollbar)
}
