<script setup lang="ts">
/**
 * 密钥设置 · 阿里云 AccessKey + DNSPod Token
 * 保存到插件数据库（dns.db，明文存储；M3 stronghold 加密升级）。
 */
import { onMounted, ref } from 'vue'
import { ipc } from './ipc'
import { useUiStore } from '@/stores/ui'
import type { ProviderConfig } from './contracts'

const ui = useUiStore()

const aliyun = ref<ProviderConfig>({ id: '', key: '' })
const dnspod = ref<ProviderConfig>({ id: '', key: '' })
const loaded = ref(false)
const saving = ref(false)

/** 加载已保存配置（回显；密钥已保存时显示占位提示不泄密） */
async function load() {
  try {
    const cfg = await ipc.dnsConfigGet()
    aliyun.value = { id: cfg.aliyun.id, key: cfg.aliyun.key }
    dnspod.value = { id: cfg.dnspod.id, key: cfg.dnspod.key }
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
      aliyun: { id: aliyun.value.id.trim(), key: aliyun.value.key.trim() },
      dnspod: { id: dnspod.value.id.trim(), key: dnspod.value.key.trim() },
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
      <section
        class="rounded-lg border border-border bg-surface p-[16px] dark:border-border-dark dark:bg-surface-dark"
      >
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
          <label class="flex flex-col gap-[4px]">
            <span class="field-label">AccessKey ID</span>
            <input
              v-model="aliyun.id"
              class="field-input font-mono"
              placeholder="LTAI5t…"
              spellcheck="false"
            />
          </label>
          <label class="flex flex-col gap-[4px]">
            <span class="field-label">AccessKey Secret</span>
            <input
              v-model="aliyun.key"
              class="field-input font-mono"
              type="password"
              placeholder="••••••••"
              spellcheck="false"
            />
          </label>
        </div>
      </section>

      <!-- 腾讯云 DNSPod -->
      <section
        class="rounded-lg border border-border bg-surface p-[16px] dark:border-border-dark dark:bg-surface-dark"
      >
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
          <label class="flex flex-col gap-[4px]">
            <span class="field-label">SecretId</span>
            <input
              v-model="dnspod.id"
              class="field-input font-mono"
              placeholder="AKID…"
              spellcheck="false"
            />
          </label>
          <label class="flex flex-col gap-[4px]">
            <span class="field-label">SecretKey</span>
            <input
              v-model="dnspod.key"
              class="field-input font-mono"
              type="password"
              placeholder="••••••••"
              spellcheck="false"
            />
          </label>
        </div>
      </section>

      <div class="flex items-center gap-[10px]">
        <button class="btn-primary" :disabled="saving" @click="save">
          {{ saving ? '保存中…' : '保存密钥' }}
        </button>
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
          密钥仅保存在本机应用数据目录（M3 起接入强加密存储）
        </span>
      </div>
    </div>
  </div>
</template>
