<script setup lang="ts">
import { computed, ref } from 'vue'
import {
  UiButton,
  UiSpinner,
  UiIcon,
  UiIconButton,
  UiTabs,
  UiScrollArea,
  UiCodeEditor,
  UiCodeDiff,
  UiModal,
  UiAlert,
  UiToolbar,
  UiSplitPane,
  UiContextMenu,
  type UiContextMenuItem,
} from '@/core/ui'
import RemoteEditorTree from './RemoteEditorTree.vue'
import type { useRemoteEditor } from './useRemoteEditor'
import type { ServerConnection } from '../contracts'
const props = defineProps<{
  editor: ReturnType<typeof useRemoteEditor>
  connection?: ServerConnection
  title: string
}>()
function hide() {
  props.editor.hide()
}
function activate(path: string) {
  props.editor.activate(path)
}
const sidebar = ref(true),
  size = ref(240),
  pending = ref<string[]>([]),
  diff = ref(false)
const menu = ref<{ x: number; y: number; items: UiContextMenuItem[] }>()
const docs = computed(() => props.editor.documents.value)
const tabs = computed(() =>
  docs.value.map((d) => ({
    value: d.path,
    label: d.path.split('/').pop() ?? d.path,
    badge: d.content !== d.saved ? '●' : undefined,
    closable: !d.saving,
  }))
)
const current = computed(() => props.editor.current.value)
function close(paths: string[]) {
  const targets = docs.value.filter((d) => paths.includes(d.path))
  if (props.editor.busy.value) return
  if (targets.some((d) => d.content !== d.saved)) {
    pending.value = paths
    return
  }
  for (const p of paths) props.editor.remove(p)
}
function discard() {
  if (props.editor.busy.value) return
  for (const path of pending.value) props.editor.remove(path)
  pending.value = []
}
async function saveClose() {
  for (const path of pending.value) {
    const doc = docs.value.find((d) => d.path === path)
    if (doc && doc.content !== doc.saved && !(await props.editor.save(doc))) return
  }
  if (docs.value.some((d) => pending.value.includes(d.path) && d.content !== d.saved)) return
  discard()
}
function closeAll() {
  close(docs.value.map((d) => d.path))
  if (!docs.value.length) hide()
}
function context(path: string, event: MouseEvent) {
  menu.value = {
    x: event.clientX,
    y: event.clientY,
    items: [
      { label: '关闭', onClick: () => close([path]) },
      {
        label: '关闭其他',
        onClick: () => close(docs.value.filter((d) => d.path !== path).map((d) => d.path)),
      },
      {
        label: '关闭已保存',
        onClick: () => close(docs.value.filter((d) => d.content === d.saved).map((d) => d.path)),
      },
    ],
  }
}
</script>
<template>
  <section
    v-show="editor.visible.value"
    class="absolute inset-0 z-[100] flex min-h-0 flex-col bg-surface dark:bg-surface-dark"
    aria-label="远程文件编辑工作台"
  >
    <UiToolbar bordered :title="'远程编辑 · ' + title">
      <UiIconButton :label="sidebar ? '收起目录' : '展开目录'" size="sm" @click="sidebar = !sidebar"
        ><UiIcon :name="sidebar ? 'chevrons-left' : 'chevrons-right'" :size="14"
      /></UiIconButton>
      <template #trailing
        ><UiButton size="sm" variant="ghost" @click="hide">收起</UiButton
        ><UiButton size="sm" variant="ghost" @click="closeAll">关闭文件</UiButton></template
      >
    </UiToolbar>
    <UiAlert v-if="connection?.status !== 'connected'" tone="warning" size="sm"
      >SSH 已断开，编辑内容保留；恢复连接后可保存。</UiAlert
    >
    <UiAlert v-if="editor.error.value" tone="danger" size="sm">{{ editor.error.value }}</UiAlert>
    <UiSplitPane
      v-model="size"
      :min="sidebar ? 180 : 0"
      :max="sidebar ? 420 : 0"
      class="min-h-0 flex-1"
    >
      <template #primary
        ><RemoteEditorTree
          v-show="sidebar"
          class="h-full"
          :connection-id="connection?.sessionId"
          :busy="editor.busy.value"
          :connected="connection?.status === 'connected'"
          :initial-path="editor.directory.value"
          @open="editor.openFile"
          @renamed="editor.renamed"
      /></template>
      <template #secondary>
        <div class="flex h-full min-h-0 flex-col">
          <UiScrollArea axis="horizontal" class="shrink-0"
            ><UiTabs
              :model-value="editor.active.value"
              :items="tabs"
              variant="line"
              @update:model-value="activate"
              @close="close([$event])"
              @contextmenu="context"
          /></UiScrollArea>
          <UiToolbar v-if="current" bordered>
            <span class="min-w-0 flex-1 select-text break-all font-mono text-caption">{{
              current.path
            }}</span>
            <template #trailing
              ><UiButton
                size="sm"
                :disabled="connection?.status !== 'connected'"
                :loading="current.saving"
                @click="editor.save(current)"
                >保存</UiButton
              ><UiButton
                size="sm"
                variant="ghost"
                :disabled="connection?.status !== 'connected' || editor.busy.value"
                @click="editor.saveAll()"
                >全部保存</UiButton
              ></template
            >
          </UiToolbar>
          <UiAlert v-if="current?.error" tone="danger" size="sm"
            >{{ current.error
            }}<UiButton v-if="current.conflict" size="xs" variant="ghost" @click="diff = true"
              >查看差异</UiButton
            ></UiAlert
          >
          <UiCodeEditor
            v-for="doc in docs"
            v-show="editor.active.value === doc.path"
            :key="doc.id"
            v-model="doc.content"
            :filename="doc.path"
            height="100%"
            class="min-h-0 flex-1 !rounded-none !border-0"
            status-bar
            @save="editor.save(doc)"
            @error="doc.error = $event"
          />
          <UiSpinner v-if="editor.busy.value && !docs.length" label="正在读取文件" class="p-md" />
          <p
            v-if="!docs.length && !editor.busy.value"
            class="p-md text-body-sm text-secondary dark:text-secondary-dark"
          >
            从左侧打开文件，或收起后从文件列表选择。
          </p>
        </div>
      </template>
    </UiSplitPane>
  </section>
  <UiContextMenu
    v-if="menu"
    :x="menu.x"
    :y="menu.y"
    :items="menu.items"
    @close="menu = undefined"
  />
  <UiModal :open="pending.length > 0" title="文件尚未保存" @close="pending = []">
    <p class="select-text whitespace-pre-wrap text-body-sm">{{ pending.join('\n') }}</p>
    <template #footer
      ><UiButton @click="saveClose">保存并关闭</UiButton
      ><UiButton variant="danger" @click="discard">放弃修改</UiButton
      ><UiButton variant="ghost" @click="pending = []">返回编辑</UiButton></template
    >
  </UiModal>
  <UiModal :open="diff" title="比较远端与本地内容" size="full" @close="diff = false">
    <UiCodeDiff
      v-if="current?.remoteContent !== undefined"
      :original="current.remoteContent"
      :modified="current.content"
      :filename="current.path"
      height="100%"
    />
    <UiAlert v-else tone="warning">未能读取远端内容，无法展示差异。</UiAlert>
    <template #footer
      ><UiButton
        v-if="current"
        variant="danger"
        :loading="current.saving"
        @click="
          editor.save(current, true).then((ok) => {
            if (ok) diff = false
          })
        "
        >以本地内容覆盖远端</UiButton
      ><UiButton
        v-if="current"
        variant="ghost"
        :loading="current.saving"
        @click="
          editor.reload(current).then((ok) => {
            if (ok) diff = false
          })
        "
        >放弃本地修改，重新读取</UiButton
      ><UiButton variant="ghost" @click="diff = false">返回</UiButton></template
    >
  </UiModal>
</template>
