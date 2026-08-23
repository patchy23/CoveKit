<script setup lang="ts">
/**
 * 云解析管理 · 平台切换 + 域名列表
 * 点击域名进入记录管理（RecordPanel）；未配置密钥时给出引导提示。
 */
import { onMounted, ref } from 'vue'
import { ipc } from './ipc'
import { useUiStore } from '@/stores/ui'
import type { CloudDomain, DnsPlatform } from './contracts'
import { platformLabel } from './useDns'
import RecordPanel from './RecordPanel.vue'
import { UiAlert, UiButton, UiTabs } from '@/core/ui'

const ui = useUiStore()

const platform = ref<DnsPlatform>('aliyun')
const domains = ref<CloudDomain[]>([])
const busy = ref(false)
const configured = ref(true)

/** 当前选中的域名（非空 = 记录视图） */
const activeDomain = ref<CloudDomain | null>(null)

/** 切换平台：重新拉取域名列表 */
async function switchPlatform(p: DnsPlatform) {
  if (platform.value === p) return
  platform.value = p
  activeDomain.value = null
  await loadDomains()
}

/** 检查当前平台是否已配置密钥（未配置则静默显示引导条，不请求域名列表） */
async function ensureConfigured(): Promise<boolean> {
  try {
    const cfg = await ipc.dnsConfigGet()
    const c = cfg[platform.value]
    const ok =
      platform.value === 'cloudflare'
        ? Boolean(cfg.cloudflare.credentialRef || cfg.cloudflare.token.trim())
        : Boolean(c.credentialRef || ('id' in c && c.id.trim() && c.key.trim()))
    configured.value = ok
    return ok
  } catch {
    // 读配置失败按未配置处理（引导条兜底，不弹错误）
    configured.value = false
    return false
  }
}

/** 拉取域名列表（未配置密钥时直接显示引导条，不发起请求） */
async function loadDomains() {
  if (!(await ensureConfigured())) {
    domains.value = []
    return
  }
  busy.value = true
  try {
    const r = await ipc.dnsDomains(platform.value)
    domains.value = r.list
    configured.value = true
  } catch (e) {
    // 已配置但请求失败（网络/密钥无效）：明确提示
    ui.toast('加载域名失败：' + (e instanceof Error ? e.message : String(e)))
  } finally {
    busy.value = false
  }
}

/** 从记录视图返回域名列表 */
function backToDomains() {
  activeDomain.value = null
  loadDomains()
}

onMounted(loadDomains)
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[12px]">
    <!-- 平台切换 + 刷新 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <UiTabs
        :model-value="platform"
        size="sm"
        :items="[
          { value: 'aliyun', label: platformLabel('aliyun') },
          { value: 'dnspod', label: platformLabel('dnspod') },
          { value: 'cloudflare', label: platformLabel('cloudflare') },
        ]"
        @update:model-value="switchPlatform($event as DnsPlatform)"
      />
      <UiButton variant="ghost" size="sm" class="shrink-0" :loading="busy" @click="loadDomains">
        {{ busy ? '加载中…' : '刷新' }}
      </UiButton>
    </div>

    <!-- 未配置密钥：引导 -->
    <UiAlert v-if="!configured" tone="warning" size="sm">
      当前平台尚未配置密钥，请先到「密钥设置」页填写
      {{ platformLabel(platform) }} 的 API 密钥。
    </UiAlert>

    <!-- 记录视图（点击域名进入） -->
    <RecordPanel
      v-if="activeDomain"
      :platform="platform"
      :domain="activeDomain"
      class="min-h-0 flex-1"
      @back="backToDomains"
    />

    <!-- 域名列表 -->
    <div v-else class="min-h-0 flex-1 overflow-y-auto pr-[2px]">
      <p
        v-if="!busy && domains.length === 0"
        class="text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        {{ configured ? '暂无域名（或平台侧无解析域名）' : '—' }}
      </p>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(228px,1fr))] gap-[10px]">
        <UiButton
          v-for="d in domains"
          :key="d.domainId"
          variant="ghost"
          class="!h-auto !whitespace-normal flex-col !items-start gap-[6px] rounded-lg border border-border bg-surface !p-[14px] text-left hover:border-tertiary/50 dark:border-border-dark dark:bg-surface-dark dark:hover:border-tertiary-dark/50"
          @click="activeDomain = d"
        >
          <span class="font-mono text-body font-medium text-primary dark:text-primary-dark">
            {{ d.domainName }}
          </span>
          <span
            class="flex items-center gap-[10px] text-body-sm text-text-muted dark:text-text-muted-dark"
          >
            <span>{{
              platform === 'cloudflare' ? '进入查看记录' : `${d.recordTotal} 条记录`
            }}</span>
            <span v-if="d.createTime" class="truncate">{{ d.createTime }}</span>
          </span>
        </UiButton>
      </div>
    </div>
  </div>
</template>
