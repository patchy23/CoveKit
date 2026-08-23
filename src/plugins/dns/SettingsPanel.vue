<script setup lang="ts">
/**
 * 密钥设置 · 阿里云 AccessKey + DNSPod CAM + Cloudflare API Token
 * 每个平台可选择公共 Vault 凭证，或继续使用原有手工输入。
 */
import { onMounted, ref } from 'vue'
import { ipc } from './ipc'
import { useUiStore } from '@/stores/ui'
import type { CloudflareConfig, ProviderConfig } from './contracts'
import { CredentialPicker, UiButton, UiField, UiInput, UiPanel } from '@/core/ui'

const ui = useUiStore()

const aliyun = ref<ProviderConfig>({ id: '', key: '', credentialRef: '' })
const dnspod = ref<ProviderConfig>({ id: '', key: '', credentialRef: '' })
const cloudflare = ref<CloudflareConfig>({ token: '', credentialRef: '' })
const loaded = ref(false)
const saving = ref(false)

/** 加载已保存配置（回显；密钥已保存时显示占位提示不泄密） */
async function load() {
  try {
    const cfg = await ipc.dnsConfigGet()
    aliyun.value = {
      id: cfg.aliyun.id,
      key: cfg.aliyun.key,
      credentialRef: cfg.aliyun.credentialRef ?? '',
    }
    dnspod.value = {
      id: cfg.dnspod.id,
      key: cfg.dnspod.key,
      credentialRef: cfg.dnspod.credentialRef ?? '',
    }
    cloudflare.value = {
      token: cfg.cloudflare.token,
      credentialRef: cfg.cloudflare.credentialRef ?? '',
    }
  } catch (e) {
    ui.toast('读取配置失败：' + (e instanceof Error ? e.message : String(e)))
  } finally {
    loaded.value = true
  }
}

/** 保存全部配置 */
async function save() {
  saving.value = true
  try {
    await ipc.dnsConfigSet({
      aliyun: {
        id: aliyun.value.id.trim(),
        key: aliyun.value.key.trim(),
        credentialRef: aliyun.value.credentialRef || undefined,
      },
      dnspod: {
        id: dnspod.value.id.trim(),
        key: dnspod.value.key.trim(),
        credentialRef: dnspod.value.credentialRef || undefined,
      },
      cloudflare: {
        token: cloudflare.value.token.trim(),
        credentialRef: cloudflare.value.credentialRef || undefined,
      },
    })
    ui.toast('密钥配置已保存')
  } catch (e) {
    ui.toast('保存失败：' + (e instanceof Error ? e.message : String(e)))
  } finally {
    saving.value = false
  }
}

onMounted(load)
</script>

<template>
  <div class="max-w-[720px]">
    <div v-if="!loaded" class="text-body-sm text-text-muted dark:text-text-muted-dark">
      正在读取配置…
    </div>
    <div v-else class="flex flex-col gap-[16px]">
      <!-- 阿里云 -->
      <UiPanel>
        <h3 class="mb-[4px] text-body font-medium text-primary dark:text-primary-dark">阿里云</h3>
        <p class="mb-[12px] text-body-sm text-text-muted dark:text-text-muted-dark">
          云解析 DNS 的 AccessKey（RAM 子账号最小授权：AliyunDNSFullAccess）。创建入口：
          <a
            class="text-info-strong underline dark:text-info-dark"
            href="https://ram.console.aliyun.com/manage/ak"
            target="_blank"
            rel="noreferrer"
            >阿里云控制台</a
          >
        </p>
        <div class="flex flex-col gap-[10px]">
          <UiField
            label="凭证库（可选）"
            description="选择后优先使用 Vault 凭证；清空选择即可回退下方手工密钥。"
          >
            <CredentialPicker
              :model-value="aliyun.credentialRef ?? ''"
              kind="access-key-pair"
              placeholder="选择阿里云 AccessKey 凭证"
              @update:model-value="aliyun.credentialRef = $event"
            />
          </UiField>
          <UiField label="手工 AccessKey ID">
            <UiInput
              v-model="aliyun.id"
              class="font-mono"
              placeholder="LTAI5t…"
              spellcheck="false"
            />
          </UiField>
          <UiField label="手工 AccessKey Secret">
            <UiInput
              v-model="aliyun.key"
              type="password"
              placeholder="••••••••"
              spellcheck="false"
            />
          </UiField>
        </div>
      </UiPanel>

      <!-- Cloudflare -->
      <UiPanel>
        <h3 class="mb-[4px] text-body font-medium text-primary dark:text-primary-dark">
          Cloudflare
        </h3>
        <p class="mb-[12px] text-body-sm text-text-muted dark:text-text-muted-dark">
          使用 API Token Bearer 鉴权。Token 至少需要 Zone:Read 与 DNS:Edit 权限。创建入口：
          <a
            class="text-info-strong underline dark:text-info-dark"
            href="https://dash.cloudflare.com/profile/api-tokens"
            target="_blank"
            rel="noreferrer"
            >Cloudflare API Tokens</a
          >
        </p>
        <div class="flex flex-col gap-[10px]">
          <UiField
            label="凭证库（可选）"
            description="选择后优先使用 Vault API Token；清空选择即可回退下方手工 Token。"
          >
            <CredentialPicker
              :model-value="cloudflare.credentialRef ?? ''"
              kind="api-token"
              placeholder="选择 Cloudflare API Token 凭证"
              @update:model-value="cloudflare.credentialRef = $event"
            />
          </UiField>
          <UiField label="手工 API Token">
            <UiInput
              v-model="cloudflare.token"
              type="password"
              placeholder="••••••••"
              spellcheck="false"
            />
          </UiField>
        </div>
      </UiPanel>

      <!-- 腾讯云 DNSPod -->
      <UiPanel>
        <h3 class="mb-[4px] text-body font-medium text-primary dark:text-primary-dark">
          腾讯云 DNSPod
        </h3>
        <p class="mb-[12px] text-body-sm text-text-muted dark:text-text-muted-dark">
          腾讯云 DNSPod 云解析（API 3.0）的 CAM 密钥（SecretId + SecretKey）。创建入口：
          <a
            class="text-info-strong underline dark:text-info-dark"
            href="https://console.cloud.tencent.com/cam/capi"
            target="_blank"
            rel="noreferrer"
            >腾讯云访问管理</a
          >
        </p>
        <div class="flex flex-col gap-[10px]">
          <UiField
            label="凭证库（可选）"
            description="选择后优先使用 Vault 凭证；清空选择即可回退下方手工密钥。"
          >
            <CredentialPicker
              :model-value="dnspod.credentialRef ?? ''"
              kind="access-key-pair"
              placeholder="选择腾讯云 CAM 凭证"
              @update:model-value="dnspod.credentialRef = $event"
            />
          </UiField>
          <UiField label="手工 SecretId">
            <UiInput v-model="dnspod.id" class="font-mono" placeholder="AKID…" spellcheck="false" />
          </UiField>
          <UiField label="手工 SecretKey">
            <UiInput
              v-model="dnspod.key"
              type="password"
              placeholder="••••••••"
              spellcheck="false"
            />
          </UiField>
        </div>
      </UiPanel>

      <div class="flex items-center gap-[10px]">
        <UiButton variant="primary" :loading="saving" @click="save">
          {{ saving ? '保存中…' : '保存密钥' }}
        </UiButton>
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
          Vault 凭证加密保存；手工密钥与 Token 继续沿用原 dns.db 存储。
        </span>
      </div>
    </div>
  </div>
</template>
