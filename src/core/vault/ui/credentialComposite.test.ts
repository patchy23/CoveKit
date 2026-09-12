/**
 * 凭证复合 UI 行为网（AR03 补证据）
 *
 * 用途：AR03 把凭证复合组件从 core/ui 迁到 core/vault/ui 后，需要证明
 * ①选中的引用被删或凭证库不可用时有可见说明，且不误报（未选择时不提示）；
 * ②新建失败不伪装成功（错误行可见、不关闭弹窗、不弹成功 toast、按钮可重试）；
 * ③保存成功后调用方能拿到摘要并重新拉取列表。
 *
 * 手法：mock `@/core/ipc/ipc`（只替身 IPC，不替身待测组件），Pinia 独立测试实例。
 * 弹窗经 reka Teleport 渲染到 document.body，所以表单态断言查 body（不能用组件包裹器查）。
 */
import { mount, type VueWrapper } from '@vue/test-utils'
import { nextTick } from 'vue'
import { describe, expect, it, beforeEach, afterEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import type { Credential, CredentialKind, CredentialSummary } from '@/core/ipc/contracts'
import { useUiStore } from '@/stores/ui'
import CredentialForm from './CredentialForm.vue'
import CredentialPicker from './CredentialPicker.vue'

const env = vi.hoisted(() => ({ vaultList: vi.fn(), vaultSave: vi.fn() }))

vi.mock('@/core/ipc/ipc', () => ({
  ipc: { vaultList: env.vaultList, vaultSave: env.vaultSave },
}))

/** 摘要夹具（vault_list 返回项，无明文秘密） */
function summary(over: Partial<CredentialSummary> = {}): CredentialSummary {
  return {
    id: 'v1',
    name: '生产 SSH',
    kind: 'ssh-key',
    masked: 'ssh-key:root@10.0.0.1',
    note: '',
    createdAt: 0,
    updatedAt: 0,
    ...over,
  }
}

/** 明文凭证夹具（vault_reveal 返回，供编辑态预填） */
function credential(over: Partial<Credential> = {}): Credential {
  return {
    id: 'v1',
    name: '生产 SSH',
    kind: 'ssh-key',
    fields: { type: 'ssh-key', username: 'root', privateKey: 'PEM', passphrase: null },
    note: '跳板机',
    createdAt: 0,
    updatedAt: 0,
    ...over,
  }
}

/**
 * 挂到 document.body：弹窗内容经传送门渲染，只有挂到 body 才查得到。
 * 两个组件各自一个助手，保持 props 的类型检查（联合类型会退化成 object）。
 */
function mountPicker(props: { modelValue: string; kind?: CredentialKind }): VueWrapper {
  return mount(CredentialPicker, { props, attachTo: document.body })
}

/** 挂载凭证表单（弹窗） */
function mountForm(props: { open: boolean; credential?: Credential }): VueWrapper {
  return mount(CredentialForm, { props, attachTo: document.body })
}

/* ── 传送门内的表单：按原生事件驱动，与真实输入一致 ── */

/** 弹窗面板文本（弹窗渲染在 body，不在组件包裹器里） */
function panelText(): string {
  return document.body.querySelector('.ui-modal-panel')?.textContent ?? ''
}

/** 弹窗面板里的输入框（顺序：名称、用户名、密码、备注） */
function panelInputs(): HTMLInputElement[] {
  return Array.from(document.body.querySelectorAll<HTMLInputElement>('.ui-modal-panel input'))
}

/** 弹窗面板里按文字找按钮 */
function panelButton(label: string): HTMLButtonElement | undefined {
  return Array.from(
    document.body.querySelectorAll<HTMLButtonElement>('.ui-modal-panel button')
  ).find((b) => b.textContent?.trim() === label)
}

/** 输入并派发 input 事件（触发组件的受控更新） */
async function typeInto(input: HTMLInputElement, value: string): Promise<void> {
  input.value = value
  input.dispatchEvent(new Event('input'))
  await nextTick()
}

beforeEach(() => {
  setActivePinia(createPinia())
  env.vaultList.mockReset()
  env.vaultSave.mockReset()
})

afterEach(() => {
  vi.restoreAllMocks()
  document.body.innerHTML = ''
})

describe('凭证选择器', () => {
  it('按类型列出凭证，选中项显示名称与掩码摘要', async () => {
    env.vaultList.mockResolvedValue([
      summary(),
      summary({ id: 'v2', name: '生产 MySQL', kind: 'password', masked: 'root / ****' }),
    ])
    const wrapper = mountPicker({ modelValue: 'v1', kind: 'ssh-key' })
    await vi.waitFor(() => expect(wrapper.html()).toContain('生产 SSH'))
    expect(wrapper.html()).toContain('ssh-key:root@10.0.0.1')
    // 当前选中的 ssh 条目有效 → 不给失效提示
    expect(wrapper.text()).not.toContain('所选凭证已删除')
  })

  it('所选凭证不在列表里时提示重新选择', async () => {
    env.vaultList.mockResolvedValue([summary()])
    const wrapper = mountPicker({ modelValue: 'v-not-mine', kind: 'ssh-key' })
    await vi.waitFor(() => expect(wrapper.text()).toContain('所选凭证已删除或不可用，请重新选择。'))
  })

  it('凭证库不可用时说明可改用手工凭据', async () => {
    env.vaultList.mockRejectedValue(new Error('keyring 不可用'))
    const wrapper = mountPicker({ modelValue: 'v1' })
    await vi.waitFor(() => expect(wrapper.text()).toContain('凭证库暂不可用'))
    expect(wrapper.text()).toContain('改用手工凭据')
  })

  it('未选择凭证时不去提示不可用（缺失不报错）', async () => {
    env.vaultList.mockRejectedValue(new Error('keyring 不可用'))
    const wrapper = mountPicker({ modelValue: '' })
    await vi.waitFor(() => expect(env.vaultList).toHaveBeenCalled())
    expect(wrapper.text()).not.toContain('凭证库暂不可用')
  })

  it('内嵌表单保存成功后刷新列表并自动选中新凭证', async () => {
    env.vaultList.mockResolvedValue([summary()])
    const wrapper = mountPicker({ modelValue: '', kind: 'password' })
    await vi.waitFor(() => expect(env.vaultList).toHaveBeenCalledTimes(1))

    const fresh = summary({ id: 'v9', name: '新建的库账号', kind: 'password' })
    wrapper.findComponent(CredentialForm).vm.$emit('saved', fresh)
    await vi.waitFor(() => expect(env.vaultList).toHaveBeenCalledTimes(2))
    const selected = wrapper.emitted('update:modelValue') ?? []
    expect(selected[selected.length - 1]).toEqual(['v9'])
  })
})

describe('凭证表单', () => {
  /** 打开表单（open 由 false → true，与真实交互一致） */
  async function openForm(props: { credential?: Credential } = {}): Promise<VueWrapper> {
    const wrapper = mountForm({ open: false, ...props })
    await wrapper.setProps({ open: true })
    await nextTick()
    return wrapper
  }

  it('必填项为空时给出校验错误且不提交', async () => {
    await openForm()
    panelButton('创建')?.click()
    await nextTick()
    expect(panelText()).toContain('凭证名称不能为空')
    expect(env.vaultSave).not.toHaveBeenCalled()
  })

  it('保存失败时显示后端原因，不关闭弹窗、不提示成功', async () => {
    const toast = vi.spyOn(useUiStore(), 'toast')
    env.vaultSave.mockRejectedValue('主密钥未初始化，请先在设置中完成初始化')

    const wrapper = await openForm()
    const inputs = panelInputs()
    await typeInto(inputs[0], '生产库账号')
    await typeInto(inputs[1], 'root')
    await typeInto(inputs[2], 'secret-value')
    panelButton('创建')?.click()

    await vi.waitFor(() => expect(panelText()).toContain('主密钥未初始化'))
    expect(wrapper.emitted('saved')).toBeUndefined()
    expect(wrapper.emitted('close')).toBeUndefined()
    expect(toast).not.toHaveBeenCalled()
    // 失败后按钮必须恢复可用，否则用户无法重试
    expect(panelButton('创建')?.disabled).toBe(false)
  })

  it('保存成功后提示、回传摘要并关闭弹窗', async () => {
    const toast = vi.spyOn(useUiStore(), 'toast')
    const saved = summary({ id: 'v7', name: '生产库账号', kind: 'password', masked: 'root / ****' })
    env.vaultSave.mockResolvedValue(saved)

    const wrapper = await openForm()
    const inputs = panelInputs()
    await typeInto(inputs[0], '生产库账号')
    await typeInto(inputs[1], 'root')
    await typeInto(inputs[2], 'secret-value')
    panelButton('创建')?.click()

    await vi.waitFor(() => expect(wrapper.emitted('saved')).toBeTruthy())
    expect(wrapper.emitted('saved')?.[0]).toEqual([saved])
    expect(wrapper.emitted('close')).toBeTruthy()
    expect(toast).toHaveBeenCalledWith('凭证已创建')
  })

  it('编辑态预填并锁定类型', async () => {
    await openForm({ credential: credential() })
    expect(panelText()).toContain('编辑凭证 · 生产 SSH')
    const select = document.body.querySelector('.ui-modal-panel [role="combobox"]')
    expect(select?.hasAttribute('disabled')).toBe(true)
    expect(panelInputs()[0].value).toBe('生产 SSH')
  })
})
