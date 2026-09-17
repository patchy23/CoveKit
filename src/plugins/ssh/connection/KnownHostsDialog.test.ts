import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import KnownHostsDialog from './KnownHostsDialog.vue'

const { list } = vi.hoisted(() => ({ list: vi.fn() }))
vi.mock('../ipc', () => ({ ipc: { sshKnownHostList: list } }))
enableAutoUnmount(afterEach)
beforeEach(() => list.mockReset())

const global = {
  stubs: {
    UiModal: { template: '<div><slot /><slot name="footer" /></div>' },
  },
}
const entry = {
  host: 'example.test',
  port: 22,
  algorithm: 'ssh-ed25519',
  fingerprint: 'SHA256:test-fingerprint',
}

it('关闭状态挂载不读取，首次打开及再次打开自动显示最新指纹', async () => {
  list.mockResolvedValue([entry])
  const wrapper = mount(KnownHostsDialog, { props: { open: false }, global })
  expect(list).not.toHaveBeenCalled()
  await wrapper.setProps({ open: true })
  await flushPromises()
  expect(list).toHaveBeenCalledTimes(1)
  expect(wrapper.text()).toContain(entry.host)
  expect(wrapper.text()).toContain(entry.fingerprint)

  await wrapper.setProps({ open: false })
  expect(list).toHaveBeenCalledTimes(1)
  list.mockResolvedValue([{ ...entry, host: 'new.example.test' }])
  await wrapper.setProps({ open: true })
  await flushPromises()
  expect(list).toHaveBeenCalledTimes(2)
  expect(wrapper.text()).toContain('new.example.test')
})

it('初始即打开时也读取，加载失败后重新打开可恢复', async () => {
  list.mockRejectedValueOnce(new Error('读取失败')).mockResolvedValueOnce([entry])
  const wrapper = mount(KnownHostsDialog, { props: { open: true }, global })
  await flushPromises()
  expect(list).toHaveBeenCalledTimes(1)
  expect(wrapper.text()).toContain('已知主机列表加载失败')
  await wrapper.setProps({ open: false })
  await wrapper.setProps({ open: true })
  await flushPromises()
  expect(wrapper.text()).not.toContain('已知主机列表加载失败')
  expect(wrapper.text()).toContain(entry.fingerprint)
})
