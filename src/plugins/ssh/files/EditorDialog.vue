<script setup lang="ts">
/**
 * EditorDialog · 远程文件编辑弹窗
 *
 * 文件管理页签双击文件打开；编辑底座为 core 的 `UiCodeEditor`（按文件名识别语法、行号、
 * 状态栏、查找替换、Ctrl+S 保存、大文件自动降级），本组件只保留弹窗壳与保存/放弃交互：
 * 有未保存修改时关闭前二次确认，远端冲突时提供强制覆盖。
 */
import { computed, ref, watch } from 'vue'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { UiBadge, UiButton, UiCodeDiff, UiCodeEditor, UiModal } from '@/core/ui'
import { useUiStore } from '@/stores/ui'

const props = defineProps<{
  /** 远程文件完整路径 */
  path: string
  /** 文件内容 */
  content: string
  /** 正在保存，禁用重复提交和关闭 */
  saving?: boolean
  /** 远端文件已被修改（乐观锁冲突）：显示警告并提供强制覆盖 */
  conflict?: boolean
  /** 冲突时远端当前内容（提供时出现「查看差异」入口） */
  remoteContent?: string
}>()

const emit = defineEmits<{
  (e: 'save', content: string, force?: boolean): void
  (e: 'cancel'): void
}>()

const ui = useUiStore()

/** 编辑器实例（读取当前内容） */
const editor = ref<InstanceType<typeof UiCodeEditor> | null>(null)
/** 是否有未保存修改 */
const dirty = ref(false)
/** 放弃修改确认弹窗 */
const discardOpen = ref(false)
/** 差异对比浮层开关 */
const diffOpen = ref(false)
/** 差异对比的本地内容快照（打开时冻结，避免编辑中浮层内容跳动） */
const diffSnapshot = ref('')

/** 文件名（语言自动识别用） */
const filename = computed(() => props.path.split('/').pop() ?? props.path)

/** 当前编辑内容（未挂载时回落到 props 值） */
function currentContent(): string {
  return editor.value?.getValue() ?? props.content
}

/** 保存（force 表示忽略远端修改强制覆盖） */
function save(force = false): void {
  if (props.saving) return
  emit('save', currentContent(), force)
}

/** 取消：有未保存修改时先二次确认 */
function cancel(): void {
  if (props.saving) return
  if (dirty.value) {
    discardOpen.value = true
    return
  }
  emit('cancel')
}

/** 确认放弃修改 */
function confirmDiscard(): void {
  discardOpen.value = false
  emit('cancel')
}

/** 打开差异对比：冻结当前编辑内容作为对比右侧 */
function openDiff(): void {
  diffSnapshot.value = currentContent()
  diffOpen.value = true
}

// 父组件回写内容（保存成功后）→ 重置未保存标记
watch(
  () => props.content,
  () => {
    dirty.value = false
    editor.value?.markSaved()
  }
)
</script>

<template>
  <!-- 工作台型弹窗（UiModal full 档）：Esc/遮罩/焦点管理统一由 UiModal 承载；close 走 cancel() 保留未保存二次确认 -->
  <UiModal :open="true" size="full" width="820px" @close="cancel">
    <template #header>
      <div
        class="flex items-center gap-[10px] border-b border-border px-[16px] py-[10px] pr-[44px] dark:border-border-dark"
      >
        <span class="font-mono text-body font-medium text-primary dark:text-primary-dark">
          {{ path }}
        </span>
        <UiBadge v-if="dirty" tone="warning">已修改</UiBadge>
        <UiBadge v-if="conflict" tone="danger">远端已变化</UiBadge>
        <div class="ml-auto flex items-center gap-[8px]">
          <UiButton variant="ghost" size="sm" :disabled="saving" @click="cancel"> 取消 </UiButton>
          <UiButton
            v-if="conflict"
            variant="ghost"
            size="sm"
            title="对比远端当前内容与当前编辑内容"
            @click="openDiff"
          >
            查看差异
          </UiButton>
          <UiButton
            v-if="conflict"
            variant="danger"
            size="sm"
            :loading="saving"
            title="忽略远端修改，以当前编辑内容覆盖"
            @click="save(true)"
          >
            强制覆盖
          </UiButton>
          <UiButton variant="primary" size="sm" :loading="saving" @click="save(false)">
            {{ conflict ? '重新检查并保存' : saving ? '保存中…' : '保存' }}
          </UiButton>
        </div>
      </div>
    </template>

    <!-- 编辑区：core 编辑器（语法高亮 / 行号 / 状态栏 / 查找替换 / Ctrl+S） -->
    <UiCodeEditor
      ref="editor"
      :model-value="content"
      :filename="filename"
      status-bar
      class="min-h-0 flex-1 !rounded-none !border-0"
      @change="dirty = true"
      @save="save(false)"
      @error="ui.toast($event)"
    />
  </UiModal>

  <!-- 冲突差异对比：左＝远端当前内容，右＝当前编辑内容 -->
  <UiModal :open="diffOpen" size="full" width="1100px" title="冲突差异" @close="diffOpen = false">
    <div class="min-h-0 flex-1 p-[12px]">
      <UiCodeDiff
        :original="remoteContent ?? ''"
        :modified="diffSnapshot"
        :filename="filename"
        mode="split"
        height="100%"
      />
    </div>
  </UiModal>

  <ConfirmDialog
    :open="discardOpen"
    title="放弃未保存的修改"
    message="文件尚未保存，确定放弃修改吗？"
    confirm-label="放弃修改"
    danger
    @close="discardOpen = false"
    @confirm="confirmDiscard"
  />
</template>
