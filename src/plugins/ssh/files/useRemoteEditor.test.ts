import { mount, enableAutoUnmount } from '@vue/test-utils'
import { defineComponent, ref } from 'vue'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { useRemoteEditor } from './useRemoteEditor'
import type { ServerConnection } from '../contracts'
const env = vi.hoisted(() => ({ open: vi.fn(), save: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: { sshEditOpen: env.open, sshEditSave: env.save } }))
vi.mock('@/core/lifecycle', () => ({ useToolLifecycle: vi.fn() }))
enableAutoUnmount(afterEach)
beforeEach(() => vi.resetAllMocks())
function setup() {
  const connection = ref<ServerConnection>({ profileId: 'p', sessionId: 's', status: 'connected' })
  let editor!: ReturnType<typeof useRemoteEditor>
  const wrapper = mount(
    defineComponent({
      setup() {
        editor = useRemoteEditor(() => connection.value)
        return () => null
      },
    })
  )
  return { editor, connection, wrapper }
}
const file = (path = '/file', content = 'base', modifiedAt = 1) => ({
  ok: true,
  path,
  content,
  modifiedAt,
})
it('打开多文件、重复打开和收起不会丢失各自草稿', async () => {
  env.open.mockImplementation((_id, path) => Promise.resolve(file(path)))
  const { editor } = setup()
  await editor.openFile({ path: '/a' })
  editor.documents.value[0].content = 'draft'
  await editor.openFile({ path: '/b' })
  editor.visible.value = false
  await editor.openFile({ path: '/a' })
  expect(env.open).toHaveBeenCalledTimes(2)
  expect(editor.current.value?.content).toBe('draft')
  expect(editor.documents.value).toHaveLength(2)
})
it('保存中的新输入继续保持未保存，后端失败不吞草稿', async () => {
  env.open.mockResolvedValue(file())
  const { editor } = setup()
  await editor.openFile({ path: '/file' })
  const doc = editor.current.value!
  doc.content = 'snapshot'
  let finish!: (v: { ok: boolean }) => void
  env.save.mockReturnValue(new Promise((resolve) => (finish = resolve)))
  const pending = editor.save(doc)
  doc.content = 'new typing'
  env.open.mockResolvedValue(file('/file', 'snapshot', 2))
  finish({ ok: true })
  expect(await pending).toBe(true)
  expect(doc.saved).toBe('snapshot')
  expect(doc.content).toBe('new typing')
  env.save.mockRejectedValue(new Error('断线'))
  expect(await editor.save(doc)).toBe(false)
  expect(doc.content).toBe('new typing')
})
it('冲突后禁止普通覆盖，明确覆盖才省略基线', async () => {
  env.open.mockResolvedValue(file())
  const { editor } = setup()
  await editor.openFile({ path: '/file' })
  const doc = editor.current.value!
  doc.content = 'local'
  env.save.mockResolvedValue({ ok: false, conflict: true, remoteContent: 'remote' })
  expect(await editor.save(doc)).toBe(false)
  expect(await editor.save(doc)).toBe(false)
  expect(env.save).toHaveBeenCalledTimes(1)
  env.save.mockResolvedValue({ ok: true })
  env.open.mockResolvedValue(file('/file', 'local', 3))
  expect(await editor.save(doc, true)).toBe(true)
  expect(env.save).toHaveBeenLastCalledWith('s', '/file', 'local', undefined)
})
it('断线保持草稿，卸载后迟到的读取不创建文件', async () => {
  env.open.mockResolvedValue(file())
  const { editor, connection, wrapper } = setup()
  await editor.openFile({ path: '/file' })
  editor.current.value!.content = 'draft'
  connection.value.status = 'disconnected'
  expect(await editor.save(editor.current.value!)).toBe(false)
  expect(editor.dirty.value).toHaveLength(1)
  connection.value.status = 'connected'
  let finish!: (v: ReturnType<typeof file>) => void
  env.open.mockReturnValue(new Promise((resolve) => (finish = resolve)))
  const pending = editor.openFile({ path: '/late' })
  wrapper.unmount()
  finish(file('/late'))
  await pending
  expect(editor.documents.value).toHaveLength(1)
})

it('迟到的读取不抢占最后选择，重新读取期间的新输入不丢失', async () => {
  env.open.mockResolvedValue(file('/a'))
  const { editor } = setup()
  await editor.openFile({ path: '/a' })
  let finish!: (value: ReturnType<typeof file>) => void
  env.open.mockReturnValue(
    new Promise((resolve) => {
      finish = resolve
    })
  )
  const opening = editor.openFile({ path: '/b' })
  editor.activate('/a')
  finish(file('/b'))
  await opening
  expect(editor.active.value).toBe('/a')
  const doc = editor.current.value!
  doc.content = 'old draft'
  env.open.mockReturnValue(
    new Promise((resolve) => {
      finish = resolve
    })
  )
  const reloading = editor.reload(doc)
  doc.content = 'typed while loading'
  finish(file('/a', 'remote'))
  await reloading
  expect(doc.content).toBe('typed while loading')
  expect(doc.saved).toBe('remote')
})
