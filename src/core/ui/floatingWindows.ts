import { computed, onUnmounted, shallowRef } from 'vue'

// 仅存活窗口参与排序，不随激活次数无限增长。宿主的 isolate 隔离模态层级。
const windows = shallowRef<symbol[]>([])
export function useFloatingWindowOrder() {
  const id = Symbol('floating-window')
  function activate() {
    if (windows.value.at(-1) === id) return
    windows.value = [...windows.value.filter((item) => item !== id), id]
  }
  activate()
  onUnmounted(() => {
    windows.value = windows.value.filter((item) => item !== id)
  })
  return {
    activate,
    active: computed(() => windows.value.at(-1) === id),
    index: windows.value.length - 1,
    zIndex: computed(() => 100 + windows.value.indexOf(id)),
  }
}
