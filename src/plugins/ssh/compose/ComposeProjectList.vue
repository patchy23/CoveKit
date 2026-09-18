<script setup lang="ts">
/** 常驻挂载的编排列表页，返回时保留筛选词和原生滚动位置。 */
import { computed, ref, watch } from 'vue'
import {
  UiButton,
  UiEmptyState,
  UiScrollArea,
  UiSearchInput,
  UiTable,
  UiTableExpandableRow,
  UiTableCell,
  UiToolbar,
} from '@/core/ui'
import type { ComposeAction, ComposeProject } from '../contracts'
import { composeStatus, composeContainerCount } from './composeTemplates'
import RuntimeStatus from '../RuntimeStatus.vue'
const props = defineProps<{
  projects: ComposeProject[]
  loading: boolean
  disabled: boolean
  error: string
  connected: boolean
  busyProjectName?: string
}>()
defineEmits<{
  edit: [project: ComposeProject]
  add: []
  refresh: []
  action: [project: ComposeProject, action: ComposeAction]
}>()
const keyword = ref('')
const expandedName = ref('')
const filtered = computed(() =>
  props.projects.filter((p) => p.name.toLowerCase().includes(keyword.value.trim().toLowerCase()))
)
watch(filtered, (projects) => {
  if (!projects.some((project) => project.name === expandedName.value)) expandedName.value = ''
})
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <UiToolbar bordered class="flex-wrap">
      <UiSearchInput
        v-model="keyword"
        size="sm"
        class="!w-[240px] max-w-full"
        placeholder="搜索编排名称"
      />
      <span class="text-caption text-text-muted dark:text-text-muted-dark"
        >{{ filtered.length }} 个编排</span
      >
      <template #trailing>
        <UiButton
          size="sm"
          variant="ghost"
          :loading="loading"
          :disabled="disabled || !connected"
          @click="$emit('refresh')"
          >刷新</UiButton
        >
        <UiButton size="sm" :disabled="disabled || !connected" @click="$emit('add')"
          >添加容器编排</UiButton
        >
      </template>
    </UiToolbar>
    <p
      v-if="!connected"
      role="alert"
      class="px-md py-sm text-body-sm text-warning-strong dark:text-warning-dark"
    >
      SSH 已断开，请恢复连接后操作。
    </p>
    <p
      v-if="error"
      role="alert"
      class="px-md py-sm text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ error }}
    </p>
    <UiScrollArea class="min-h-0 flex-1" axis="both" data-testid="compose-list-scroll">
      <UiTable v-if="filtered.length" :framed="false" density="compact">
        <thead>
          <tr>
            <UiTableCell as="th" resizable>编排名称</UiTableCell
            ><UiTableCell as="th" resizable>状态</UiTableCell
            ><UiTableCell as="th" resizable align="right">容器数量</UiTableCell>
            <UiTableCell as="th" resizable align="right">操作</UiTableCell>
          </tr>
        </thead>
        <tbody>
          <UiTableExpandableRow
            v-for="project in filtered"
            :key="project.name"
            :label="project.name"
            :columns="4"
            :expanded="expandedName === project.name"
            :disabled="disabled || (!connected && expandedName !== project.name)"
            @update:expanded="expandedName = $event ? project.name : ''"
          >
            <template #default="{ toggle, expanded, detailsId }">
              <UiTableCell content="status">
                <RuntimeStatus
                  :status="composeStatus(project.status)"
                  :busy="busyProjectName === project.name"
                />
              </UiTableCell>
              <UiTableCell content="numeric" align="right">{{
                composeContainerCount(project.status) ?? '—'
              }}</UiTableCell>
              <UiTableCell content="action" align="right" class="whitespace-nowrap">
                <div class="flex items-center justify-end gap-[2px]">
                  <UiButton
                    variant="ghost"
                    size="xs"
                    :disabled="disabled || (!connected && !expanded)"
                    :aria-expanded="expanded"
                    :aria-controls="detailsId"
                    @click="toggle"
                    >{{ expanded ? '收起容器' : '查看容器' }}</UiButton
                  >
                  <UiButton
                    variant="ghost"
                    size="xs"
                    :disabled="disabled || !connected"
                    @click="$emit('edit', project)"
                    >编辑</UiButton
                  >
                  <UiButton
                    v-if="composeStatus(project.status) !== '运行中'"
                    variant="ghost"
                    size="xs"
                    :disabled="disabled || !connected || !project.configFiles.length"
                    @click="$emit('action', project, 'up')"
                    >启动</UiButton
                  >
                  <UiButton
                    v-if="project.status.includes('running') || project.status.includes('paused')"
                    variant="ghost"
                    size="xs"
                    :disabled="disabled || !connected"
                    @click="$emit('action', project, 'stop')"
                    >停止</UiButton
                  >
                  <UiButton
                    variant="ghost"
                    size="xs"
                    :disabled="disabled || !connected || composeStatus(project.status) === '未部署'"
                    @click="$emit('action', project, 'restart')"
                    >重启</UiButton
                  >
                  <UiButton
                    variant="ghost"
                    size="xs"
                    :disabled="disabled || !connected || !project.configFiles.length"
                    title="拉取镜像并应用到容器"
                    @click="$emit('action', project, 'update')"
                    >更新镜像</UiButton
                  >
                  <UiButton
                    variant="ghost"
                    size="xs"
                    :disabled="disabled || !connected || !project.configFiles.length"
                    @click="$emit('action', project, 'recreate')"
                    >重建</UiButton
                  >
                  <UiButton
                    variant="ghost"
                    size="xs"
                    class="text-danger-strong dark:text-danger-dark"
                    :disabled="disabled || !connected || composeStatus(project.status) === '未部署'"
                    @click="$emit('action', project, 'down')"
                    >拆除</UiButton
                  >
                </div>
              </UiTableCell>
            </template>
            <template #details><slot name="details" :project="project" /></template>
          </UiTableExpandableRow>
        </tbody>
      </UiTable>
      <UiEmptyState
        v-else
        :title="loading ? '正在查询编排…' : keyword ? '没有匹配的编排' : '暂无容器编排'"
        :description="
          keyword ? '修改搜索条件后重试。' : '添加 Compose 文件，或刷新服务器上的已有编排。'
        "
      />
    </UiScrollArea>
  </div>
</template>
