<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * ChmodDialog · 远程权限修改弹窗（勾选矩阵 ↔ 八进制输入双向联动 + 递归 + 风险确认 + 二次确认）
 * 仅单选使用；安全策略见 sshPolicy（菜单可见性）与后端 check_chmod_allowed（最终防线）。
 */
import { computed, ref, watch } from 'vue'
import type { RemoteFile } from '../contracts'
import { UiButton, UiCheckbox, UiInput, UiModal } from '@/core/ui'
import ConfirmDialog from '@/core/ui/ConfirmDialog.vue'
import { chmodNeedsRiskAck } from '../connection/sshPolicy'
import {
  formatModeRwx,
  matrixToMode,
  modeFromPermissions,
  modeToMatrix,
  parseOctal,
} from './sshChmod'

const props = defineProps<{
  open: boolean
  file: RemoteFile | null
}>()

const emit = defineEmits<{
  (e: 'close'): void
  /** mode 为低 12 位（含特殊位）；recursive/acknowledgeRisk 透传后端 */
  (e: 'confirm', mode: number, recursive: boolean, acknowledgeRisk: boolean): void
}>()

/* ── 状态：矩阵 ↔ 八进制双向同步 ── */
const matrix = ref<boolean[]>([])
const octal = ref('')
const octalError = ref('')
const recursive = ref(false)
const riskAck = ref(false)
const confirming = ref(false)

/** 打开时用目标文件当前权限初始化 */
watch(
  () => props.open,
  (open) => {
    if (!open || !props.file) return
    const mode = modeFromPermissions(props.file.permissions)
    matrix.value = modeToMatrix(mode)
    octal.value = mode.toString(8)
    octalError.value = ''
    recursive.value = false
    riskAck.value = false
    confirming.value = false
  }
)

/** 当前 mode = 矩阵低 9 位 | 原文件特殊位（setuid/setgid/sticky，矩阵不展示但保留） */
const currentMode = computed(() => {
  const special = props.file ? modeFromPermissions(props.file.permissions) & 0o7000 : 0
  return special | matrixToMode(matrix.value)
})

/** 矩阵勾选 → 同步八进制框 */
function onToggle() {
  octal.value = (matrixToMode(matrix.value) | (currentMode.value & 0o7000)).toString(8)
  octalError.value = ''
}

/** 八进制手输 → 合法则同步矩阵，非法红边提示（矩阵不动） */
function onOctalInput() {
  const parsed = parseOctal(octal.value)
  if (parsed === null) {
    octalError.value = '格式：3 位或 4 位八进制（每位 0-7），如 755'
    return
  }
  octalError.value = ''
  matrix.value = modeToMatrix(parsed)
}

/** 递归风险确认（系统目录内递归需勾选） */
const needRiskAck = computed(() =>
  props.file ? chmodNeedsRiskAck(props.file.path, recursive.value) : false
)

const canSubmit = computed(
  () => !octalError.value && octal.value.trim() !== '' && (!needRiskAck.value || riskAck.value)
)

/** 二次确认通过：上抛参数并关闭（多语句内联处理器会被 vite 拒绝，必须抽函数） */
function onConfirmed() {
  emit('confirm', currentMode.value, recursive.value, riskAck.value)
  confirming.value = false
  emit('close')
}

const GROUPS = ['所有者', '所属组', '其他'] as const
const PERMS = ['读', '写', '执行'] as const
</script>

<template>
  <UiModal
    :open="open"
    :title="`修改权限：${file?.name ?? ''}`"
    width="380px"
    @close="emit('close')"
  >
    <div class="flex flex-col gap-[12px]">
      <UiTooltip :content="file?.path">
        <div class="truncate font-mono text-caption text-text-muted">
          {{ file?.path }}
        </div>
      </UiTooltip>
      <div class="text-caption text-text-muted">当前：{{ file?.permissions }}</div>

      <!-- 八进制输入（与矩阵双向同步） -->
      <div class="flex items-center gap-[8px]">
        <UiInput
          v-model="octal"
          class="w-[90px] font-mono"
          placeholder="755"
          :class="octalError ? '!border-danger-strong' : ''"
          @input="onOctalInput"
        />
        <span v-if="octalError" class="text-caption text-danger-strong">{{ octalError }}</span>
        <span v-else class="text-caption text-text-muted">八进制（矩阵自动同步）</span>
      </div>

      <!-- 勾选矩阵（div 网格；原生 table 违反组件契约） -->
      <div class="grid grid-cols-[auto_repeat(3,1fr)] items-center gap-y-[4px] text-body-sm">
        <span></span>
        <span v-for="g in GROUPS" :key="g" class="text-center text-caption text-text-muted">{{
          g
        }}</span>
        <template v-for="(perm, row) in PERMS" :key="perm">
          <span class="text-secondary dark:text-secondary-dark">{{ perm }}</span>
          <span v-for="(_, col) in GROUPS" :key="col" class="text-center">
            <UiCheckbox
              :model-value="matrix[col * 3 + row]"
              @update:model-value="
                (v: boolean) => {
                  matrix[col * 3 + row] = v
                  onToggle()
                }
              "
            />
          </span>
        </template>
      </div>

      <div class="text-caption text-text-muted">
        预览：<span class="font-mono">{{ formatModeRwx(currentMode) }}</span>
      </div>

      <!-- 目录递归 -->
      <label v-if="file?.isDir" class="flex items-center gap-[8px] text-body-sm">
        <UiCheckbox v-model="recursive" />
        递归应用到子项
      </label>

      <!-- 系统目录内递归风险确认 -->
      <label
        v-if="needRiskAck"
        class="flex items-center gap-[8px] text-body-sm text-danger-strong dark:text-danger-dark"
      >
        <UiCheckbox v-model="riskAck" />
        目标位于系统目录内，我知道递归修改的风险
      </label>
    </div>

    <template #footer>
      <UiButton variant="ghost" @click="emit('close')">取消</UiButton>
      <UiButton :disabled="!canSubmit" @click="confirming = true">确定</UiButton>
    </template>

    <!-- 二次确认 -->
    <ConfirmDialog
      :open="confirming"
      title="确认修改权限"
      :message="`将把 ${file?.path} 的权限从 ${file?.permissions} 修改为 ${formatModeRwx(currentMode)}（${currentMode.toString(8)}）${file?.isDir && recursive ? '，并递归应用到子项' : ''}。`"
      confirm-label="确认修改"
      @close="confirming = false"
      @confirm="onConfirmed"
    />
  </UiModal>
</template>
