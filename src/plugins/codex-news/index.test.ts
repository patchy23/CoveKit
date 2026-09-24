import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { publishToolVisibility, resetToolVisibilityForTest } from '@/core/lifecycle'
import { UiButton, UiTableExpandableRow } from '@/core/ui'
import NewsPage from './index.vue'
import { feeds, rawEvent } from './testFixtures'
import { parseFeeds, revision } from './news'
const mocks = vi.hoisted(() => ({ fetch: vi.fn(), load: vi.fn(), save: vi.fn(), openUrl: vi.fn() }))
vi.mock('./ipc', () => ({ ipc: mocks }))
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: mocks.openUrl }))
enableAutoUnmount(afterEach)
beforeEach(() => {
  vi.resetAllMocks()
  resetToolVisibilityForTest()
  publishToolVisibility('codex-news', { active: true })
  const snapshot = parseFeeds(feeds())
  mocks.load.mockResolvedValue({
    version: 1,
    snapshot,
    read: { one: revision(snapshot.events[0]!) },
    auto: false,
    checkedAt: '',
  })
  mocks.fetch.mockResolvedValue(
    feeds([rawEvent('new', 'confirmed', '2026-09-18T12:00:00Z'), rawEvent()])
  )
  mocks.save.mockResolvedValue(undefined)
  mocks.openUrl.mockResolvedValue(undefined)
})
it('新消息由用户应用，展开标为已读，原文入口打开来源，无通知开关', async () => {
  const wrapper = mount(NewsPage)
  await flushPromises()
  expect(wrapper.text()).toContain('有 1 条新消息或更新')
  expect(wrapper.text()).not.toContain('消息 new')
  const button = (text: string) =>
    wrapper.findAllComponents(UiButton).find((item) => item.text().includes(text))!
  await button('点击查看').get('button').trigger('click')
  expect(wrapper.text()).toContain('消息 new')
  expect(wrapper.text()).toContain('未读 1')
  const row = wrapper.findAllComponents(UiTableExpandableRow)[0]!
  await row.get('button').trigger('click')
  expect(wrapper.text()).toContain('首次发布')
  expect(wrapper.text()).toContain('未读 0')
  await button('查看原文').get('button').trigger('click')
  expect(mocks.openUrl).toHaveBeenCalledWith('https://x.com/example/status/1')
  expect(wrapper.text()).not.toContain('通知开关')
})
