import { afterEach, expect, it } from 'vitest'
import { enableAutoUnmount, shallowMount } from '@vue/test-utils'
import { UiInput, UiTextarea } from '@/core/ui'
import ConnectionCredentialsDialog from './ConnectionCredentialsDialog.vue'

enableAutoUnmount(afterEach)
const profile = {
  id: 'manual',
  name: '测试',
  host: 'example.test',
  port: 22,
  username: 'root',
  authMethod: 'password' as const,
}
const global = { renderStubDefaultSlot: true }

it('密码必填且保留首尾空格，不要求凭证引用', async () => {
  const wrapper = shallowMount(ConnectionCredentialsDialog, { props: { profile }, global })
  await wrapper.get('form').trigger('submit')
  expect(wrapper.emitted('confirm')).toBeUndefined()
  wrapper.getComponent(UiInput).vm.$emit('update:modelValue', ' fixture ')
  await wrapper.get('form').trigger('submit')
  expect(wrapper.emitted('confirm')?.[0]).toEqual([{ password: ' fixture ' }])
})

it('私钥口令必填且原样传递', async () => {
  const wrapper = shallowMount(ConnectionCredentialsDialog, {
    props: { profile: { ...profile, authMethod: 'privateKeyWithPassphrase' } },
    global,
  })
  wrapper.getComponent(UiTextarea).vm.$emit('update:modelValue', ' fixture-key ')
  await wrapper.get('form').trigger('submit')
  expect(wrapper.emitted('confirm')).toBeUndefined()
  wrapper.getComponent(UiInput).vm.$emit('update:modelValue', ' passphrase ')
  await wrapper.get('form').trigger('submit')
  expect(wrapper.emitted('confirm')?.[0]).toEqual([
    { privateKey: 'fixture-key', passphrase: ' passphrase ' },
  ])
})
