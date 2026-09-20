/* eslint-disable vue/one-component-per-file -- 用父子夹具验证工作区提供与子页面注入的生命周期 */
import { defineComponent, h, ref } from 'vue'
import { enableAutoUnmount, mount } from '@vue/test-utils'
import { afterEach, expect, it } from 'vitest'
import { provideLogWindows, useLogWindows } from './logWindows'

enableAutoUnmount(afterEach)
it('子页面卸载保留窗口，同一会话目标去重，其他会话独立，失效会话清理', async () => {
  const sessions = ref([
    { sessionId: 'a', title: '主机 A' },
    { sessionId: 'b', title: '主机 B' },
  ])
  const childVisible = ref(true)
  let registry!: ReturnType<typeof provideLogWindows>
  let open!: ReturnType<typeof useLogWindows>
  const Child = defineComponent({
    setup() {
      open = useLogWindows()
      return () => null
    },
  })
  const wrapper = mount(
    defineComponent({
      setup() {
        registry = provideLogWindows(() => sessions.value)
        return () => (childVisible.value ? h(Child) : null)
      },
    })
  )
  const target = {
    connectionId: 'a',
    kind: 'docker' as const,
    targetId: 'container',
    title: 'nginx',
  }
  open(target)
  open(target)
  open({ ...target, connectionId: 'b' })
  expect(registry.windows.value).toHaveLength(2)
  expect(registry.windows.value[0].activation).toBe(1)
  childVisible.value = false
  await wrapper.vm.$nextTick()
  expect(registry.windows.value).toHaveLength(2)
  sessions.value = [{ sessionId: 'b', title: '主机 B' }]
  await wrapper.vm.$nextTick()
  expect(registry.windows.value.map((win) => win.connectionId)).toEqual(['b'])
  open(target)
  expect(registry.windows.value).toHaveLength(1)
  wrapper.unmount()
  expect(registry.windows.value).toHaveLength(0)
})
