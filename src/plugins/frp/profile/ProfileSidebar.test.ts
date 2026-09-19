/** 名称弹窗独立保留操作目标，右键菜单关闭不能使提交失效。 */
import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import { i18n } from '@/i18n'
import { UiContextMenu, UiListRow } from '@/core/ui'
import type { FrpProfileSummary } from '../contracts'
import ProfileSidebar from './ProfileSidebar.vue'
import ProfileNameDialog from './ProfileNameDialog.vue'

const item: FrpProfileSummary = {
  fileName: 'test.toml',
  displayName: 'test',
  remark: '',
  serverAddr: '',
  serverPort: 7000,
  proxyCount: 1,
  enabledProxyCount: 1,
  proxyTypes: ['TCP'],
  mtime: 0,
  state: 'stopped',
}

describe('FRP 右键名称操作', () => {
  it.each(['rename', 'duplicate'] as const)('菜单关闭后 %s 仍提交原档案', async (mode) => {
    const wrapper = mount(ProfileSidebar, {
      props: { items: [item], active: item.fileName, loading: false, error: '', clientLabel: '' },
      global: {
        plugins: [i18n],
        stubs: {
          UiContextMenu: true,
          ProfileNameDialog: true,
          ProfileRemarkDialog: true,
          UiConfirmDialog: true,
        },
      },
    })
    await wrapper.findComponent(UiListRow).trigger('contextmenu')
    const menu = wrapper.findComponent(UiContextMenu)
    const action = menu
      .props('items')
      .find(
        (entry) =>
          entry.label === i18n.global.t(mode === 'rename' ? 'frp.menuRename' : 'frp.menuDuplicate')
      )!
    menu.vm.$emit('close')
    action.onClick?.()
    await wrapper.vm.$nextTick()
    expect(wrapper.findComponent(UiContextMenu).exists()).toBe(false)
    const dialog = wrapper.findComponent(ProfileNameDialog)
    expect(dialog.props('open')).toBe(true)
    dialog.vm.$emit('submit', 'changed.toml', 'tcp')
    expect(wrapper.emitted(mode)).toEqual([['test.toml', 'changed.toml']])
    wrapper.unmount()
  })
})
