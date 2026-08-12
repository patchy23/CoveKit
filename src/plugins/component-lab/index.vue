<script setup lang="ts">
import { ref } from 'vue'
import AppIcon from '@/features/ui/AppIcon.vue'
import {
  UiAlert,
  UiBadge,
  UiButton,
  UiEmptyState,
  UiField,
  UiInput,
  UiModal,
  UiPanel,
  UiSelect,
  UiTabs,
  UiTextarea,
  UiToolbar,
} from '@/core/ui'

const pillTab = ref('overview')
const lineTab = ref('request')
const input = ref('example.com')
const invalidInput = ref('invalid value')
const textarea = ref('公共组件只负责稳定的视觉、状态与交互约定。\n业务含义仍由各工具自行维护。')
const select = ref('postgres')
const modalOpen = ref(false)
const loading = ref(false)

const tabs = [
  { value: 'overview', label: '概览' },
  { value: 'records', label: '记录', badge: 12 },
  { value: 'settings', label: '设置' },
]
const lineTabs = [
  { value: 'request', label: '请求' },
  { value: 'response', label: '响应' },
  { value: 'console', label: '控制台', badge: 3 },
]
const options = [
  { value: 'sqlite', label: 'SQLite' },
  { value: 'mysql', label: 'MySQL' },
  { value: 'postgres', label: 'PostgreSQL' },
  { value: 'disabled', label: '不可用选项', disabled: true },
]

function previewLoading() {
  loading.value = true
  window.setTimeout(() => (loading.value = false), 1000)
}
</script>

<template>
  <div class="mx-auto flex max-w-[1100px] flex-col gap-md pb-xl">
    <header>
      <div class="flex items-center gap-sm">
        <h1 class="text-h1 font-bold text-primary dark:text-primary-dark">公共组件实验室</h1>
        <UiBadge tone="accent">v1</UiBadge>
      </div>
      <p class="mt-xs text-body-sm text-secondary dark:text-secondary-dark">
        用于统一验收亮色/深色、交互状态、尺寸和常见业务组合；这里不承载真实业务。
      </p>
    </header>

    <UiPanel title="按钮" description="主操作每个视区尽量只保留一个；危险操作必须显式使用 danger。">
      <UiToolbar>
        <UiButton variant="primary">主要操作</UiButton>
        <UiButton>次要操作</UiButton>
        <UiButton variant="ghost">幽灵按钮</UiButton>
        <UiButton variant="danger">危险操作</UiButton>
        <UiButton size="sm">紧凑按钮</UiButton>
        <UiButton variant="primary" :loading="loading" @click="previewLoading">
          {{ loading ? '处理中…' : '加载状态' }}
        </UiButton>
        <UiButton disabled>禁用状态</UiButton>
      </UiToolbar>
    </UiPanel>

    <div class="grid gap-md lg:grid-cols-2">
      <UiPanel title="表单控件" description="字段组件统一标签、说明和错误信息的垂直节奏。">
        <div class="flex flex-col gap-md">
          <UiField label="域名" description="支持原生 placeholder、disabled 与键盘事件。" required>
            <UiInput v-model.trim="input" placeholder="example.com" />
          </UiField>
          <UiField label="数据库类型">
            <UiSelect v-model="select" :options="options" />
          </UiField>
          <UiField label="校验错误" error="当前输入不符合格式要求">
            <UiInput v-model="invalidInput" invalid />
          </UiField>
          <UiField label="多行内容" description="文本域默认允许垂直缩放。">
            <UiTextarea v-model="textarea" rows="3" />
          </UiField>
        </div>
      </UiPanel>

      <UiPanel title="徽标与状态" description="语义颜色只用于状态和小面积信息提示。">
        <div class="flex flex-wrap gap-sm">
          <UiBadge>默认</UiBadge>
          <UiBadge tone="accent">强调</UiBadge>
          <UiBadge tone="success">成功</UiBadge>
          <UiBadge tone="warning">警告</UiBadge>
          <UiBadge tone="danger">失败</UiBadge>
          <UiBadge tone="info">信息</UiBadge>
          <UiBadge tone="purple">扩展</UiBadge>
        </div>
        <div class="mt-lg flex flex-col gap-sm">
          <div
            class="flex items-center justify-between border-b border-border pb-sm text-body dark:border-border-dark"
          >
            <span>服务连接</span><UiBadge tone="success">已连接 · 24ms</UiBadge>
          </div>
          <div
            class="flex items-center justify-between border-b border-border pb-sm text-body dark:border-border-dark"
          >
            <span>配置校验</span><UiBadge tone="warning">需要检查</UiBadge>
          </div>
          <div class="flex items-center justify-between text-body">
            <span>最近任务</span><UiBadge tone="danger">执行失败</UiBadge>
          </div>
        </div>
      </UiPanel>
    </div>

    <UiPanel title="页签" description="pill 用于同级视图切换，line 用于工作区内部功能切换。">
      <div class="flex flex-col gap-lg">
        <div><UiTabs v-model="pillTab" :items="tabs" /></div>
        <div class="overflow-hidden rounded-lg border border-border dark:border-border-dark">
          <UiTabs v-model="lineTab" :items="lineTabs" variant="line" />
          <div class="p-md text-body-sm text-secondary dark:text-secondary-dark">
            当前内容：{{ lineTabs.find((item) => item.value === lineTab)?.label }}
          </div>
        </div>
      </div>
    </UiPanel>

    <UiPanel title="反馈提示" description="提示使用语义 tone，不在业务页面散落颜色组合。">
      <div class="grid gap-sm lg:grid-cols-2">
        <UiAlert tone="info">这是一条普通说明信息。</UiAlert>
        <UiAlert tone="success">配置已经保存成功。</UiAlert>
        <UiAlert tone="warning">当前操作可能需要管理员权限。</UiAlert>
        <UiAlert tone="danger">连接失败，请检查主机地址。</UiAlert>
      </div>
    </UiPanel>

    <div class="grid gap-md lg:grid-cols-2">
      <UiPanel title="空状态" padding="none">
        <UiEmptyState title="还没有查询结果" description="输入查询条件并执行后，结果会显示在这里。">
          <template #icon><AppIcon name="search" :size="22" /></template>
          <UiButton variant="primary" size="sm">开始查询</UiButton>
        </UiEmptyState>
      </UiPanel>
      <UiPanel title="弹窗" description="统一 Esc、遮罩点击、宽度、标题与操作区。">
        <UiButton variant="primary" @click="modalOpen = true">打开示例弹窗</UiButton>
      </UiPanel>
    </div>

    <UiModal
      :open="modalOpen"
      title="保存连接配置"
      description="该弹窗由 UiModal 提供结构，表单内容由业务组件组合。"
      width="min(440px, 92vw)"
      @close="modalOpen = false"
    >
      <UiField label="配置名称" required><UiInput value="本地开发数据库" /></UiField>
      <template #footer>
        <UiButton variant="ghost" @click="modalOpen = false">取消</UiButton>
        <UiButton variant="primary" @click="modalOpen = false">保存</UiButton>
      </template>
    </UiModal>
  </div>
</template>
